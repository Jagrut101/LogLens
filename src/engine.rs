use crate::{capture::for_each_capture_packet,resources,index::FrameIndex,model::{BusType,CapturePacket,Direction,NetworkMeta,NormalizedFrame},protocols::{detect_container_review,normalize_tecmp,parse_ethernet_packet,parse_socketcan},tecmp::parse_ethernet_tecmp};
use anyhow::{Context, Result};
use calamine::{open_workbook_auto, Reader};
use std::{collections::{BTreeMap, BTreeSet}, fs, io::{BufRead, BufReader}, path::Path};

pub struct Dataset { pub source_time:bool, pub capture_memory_estimate:Option<usize>, pub frames:Vec<NormalizedFrame>, pub index:FrameIndex, pub trace_dt_s:Vec<f64>, pub timeline_s:Vec<f64>, pub warnings:Vec<String> }

pub fn load(path:&Path,max_packets:usize)->Result<Dataset>{ load_selected(path,max_packets,None) }

pub fn load_selected(path:&Path,max_packets:usize,selected:Option<&BTreeSet<u32>>)->Result<Dataset>{
    load_selected_with_timeline(path,max_packets,selected,true)
}
pub fn load_selected_with_timeline(path:&Path,max_packets:usize,selected:Option<&BTreeSet<u32>>,source_time:bool)->Result<Dataset>{
    let ext=path.extension().and_then(|x|x.to_str()).unwrap_or("").to_ascii_lowercase();
    let limits=resources::limits();
    let is_capture=matches!(ext.as_str(),"pcap"|"pcapng");
    let effective_max=if is_capture{max_packets}else{limits.effective_packet_limit(max_packets)};
    if matches!(ext.as_str(),"xlsx"|"xls") { resources::validate_xlsx_size(path)?; }
    // Probe one extra decoded record so exact EOF is not mistaken for truncation.
    let probe_max=effective_max.saturating_add(1);
    let (mut frames,catalog,memory_limited,capture_memory_estimate)=match ext.as_str(){
        "pcap"|"pcapng"=>{let (frames,catalog,limited,bytes)=load_capture(path,probe_max,selected,limits.capture_memory_budget())?;(frames,catalog,limited,Some(bytes))},
        "csv"=>(load_csv(path,probe_max)?,BTreeMap::new(),false,None),
        "xlsx"|"xls"=>(load_xlsx(path,probe_max)?,BTreeMap::new(),false,None),
        "txt"|"log"=>(load_text(path,probe_max)?,BTreeMap::new(),false,None),
        _=>anyhow::bail!("Unsupported log format: .{}",ext),
    };
    let truncated=memory_limited || frames.len()>effective_max;
    frames.truncate(effective_max);
    let projected=if source_time{align_source_clock_domains(&mut frames)}else{0};
    if !source_time {
        for f in &mut frames {if let Some(ns)=f.tecmp.as_ref().and_then(|m|m.capture_timestamp_ns){f.timestamp_ns=ns;}}
    }
    frames.sort_by_key(|f|f.timestamp_ns);
    if let Some(t0)=frames.first().map(|f|f.timestamp_ns){for f in &mut frames{f.relative_time_s=f.timestamp_ns.saturating_sub(t0) as f64/1e9}}
    let mut warnings=Vec::new();
    if projected>0{warnings.push(format!("Source timeline: {projected} records with a missing/different clock were placed using capture time and a source-clock reference. Original timestamps are retained in Details."));}
    if frames.iter().any(|f|f.tecmp.is_some()) {
        warnings.push(format!("TECMP timeline: {}. Both source and capture times are available in Details.",if source_time{"source time"}else{"Wireshark capture time"}));
        if source_time && frames.iter().any(|f|f.tecmp.as_ref().is_some_and(|m|m.source_timestamp_ns==Some(0))){
            warnings.push("Zero TECMP source timestamps are retained in Details; graph time uses an aligned capture reference when available. Select capture time to compare with Wireshark's packet list.".into());
        }
    }
    if !source_time {
        let block_ns=frames.iter().filter(|f|f.bus_type==BusType::Analog)
            .filter_map(|f|f.analog.as_ref().map(|meta|(meta.sample_count.min(f.payload.len()/2) as u64).saturating_mul(meta.sample_period_ns)))
            .max().unwrap_or(0);
        if block_ns>0 {
            warnings.push(format!("Analog capture-time timing: packets contain up to {:.3} ms of samples. Capture time is packet arrival time, so analog changes can appear delayed against CM markers. Use TECMP source time for acquisition timing.",block_ns as f64/1e6));
        }
    }
    if frames.is_empty(){warnings.push("No supported frames were decoded from this file.".into())}
    if effective_max < max_packets {
        warnings.push(format!("Resource protection active ({}): load limit reduced from {} to {} packets/rows.",limits.memory_label(),max_packets,effective_max));
    }
    if truncated {
        warnings.push(if memory_limited {
            format!("Safe load limit reached at {} decoded records: the estimated capture memory budget of {:.0} MiB was reached. Select fewer channels or close other applications and retry.",frames.len(),limits.capture_memory_budget() as f64/1048576.0)
        }else{format!("Safe load limit reached at approximately {} decoded records. Select fewer channels, a smaller log, or close other applications before increasing the workload.",effective_max)});
    }
    // Preserve TECMP entry data (including CRC trailers and malformed bytes) for
    // inspection/export. Resource-safe record limits still bound the load.

    // Compute Trace delta once. The UI must never rebuild previous-frame timing by
    // scanning the complete capture on every repaint.
    let mut last_by_key=std::collections::HashMap::<(u8,u32,String),f64>::new();
    let mut trace_dt_s=Vec::with_capacity(frames.len());
    for f in &frames {
        let bus=match f.bus_type { BusType::Can=>1,BusType::CanFd=>2,BusType::Lin=>3,BusType::Ethernet=>4,BusType::Analog=>5,BusType::Tecmp=>6,BusType::Unknown=>0 };
        let key=(bus,f.channel_id,f.frame_key.clone());
        let prev=last_by_key.insert(key,f.relative_time_s);
        trace_dt_s.push(prev.map(|p|(f.relative_time_s-p).max(0.0)).unwrap_or(0.0));
    }
    let index=FrameIndex::build_with_catalog(&frames,&catalog); let timeline_s=frames.iter().map(|f|f.relative_time_s).collect(); Ok(Dataset{source_time,capture_memory_estimate,frames,index,trace_dt_s,timeline_s,warnings})
}

// Early device status may use uptime while bus data already uses UTC. A shared
// graph cannot subtract uptime from UTC. Place incompatible records using the
// device's capture-to-source reference, preserving their original clock metadata.
fn align_source_clock_domains(frames:&mut [NormalizedFrame])->usize {
    const EPOCH_2000:u64=946_684_800_000_000_000;
    let reference=frames.iter().filter_map(|f|f.tecmp.as_ref())
        .filter(|m|m.message_type==3 && m.timestamp_flags&2==0)
        .find(|m|m.source_timestamp_ns.is_some_and(|t|t>=EPOCH_2000) && m.capture_timestamp_ns.is_some());
    let reference=reference.or_else(||frames.iter().filter_map(|f|f.tecmp.as_ref())
        .find(|m|m.source_timestamp_ns.is_some_and(|t|t>0) && m.capture_timestamp_ns.is_some()));
    let Some(reference)=reference else{return 0;};
    let epoch=reference.source_timestamp_ns.unwrap()>=EPOCH_2000;
    let global_offset=reference.source_timestamp_ns.unwrap() as i128-reference.capture_timestamp_ns.unwrap() as i128;
    let mut offsets=std::collections::HashMap::new();
    for preferred in [true,false]{
        for m in frames.iter().filter_map(|f|f.tecmp.as_ref()){
            if preferred && (m.message_type!=3 || m.timestamp_flags&2!=0){continue;}
            if let (Some(source),Some(capture))=(m.source_timestamp_ns,m.capture_timestamp_ns){
                if source>0 && (source>=EPOCH_2000)==epoch{offsets.entry(m.cm_id).or_insert(source as i128-capture as i128);}
            }
        }
    }
    let mut projected=0;
    for f in frames {
        if let Some(shared)=f.tecmp.as_mut(){
            let m=shared.as_ref();
            let source=m.source_timestamp_ns.unwrap_or(f.timestamp_ns);
            if source==0 || (source>=EPOCH_2000)!=epoch {
                if let Some(capture)=m.capture_timestamp_ns {
                    let offset=offsets.get(&m.cm_id).copied().unwrap_or(global_offset);
                    f.timestamp_ns=(capture as i128+offset).clamp(0,u64::MAX as i128) as u64;
                    std::sync::Arc::make_mut(shared).timeline_projected=true;projected+=1;
                }
            }
        }else{
            f.timestamp_ns=(f.timestamp_ns as i128+global_offset).clamp(0,u64::MAX as i128) as u64;
            projected+=1;
        }
    }
    projected
}

// Metadata is shared without changing raw bytes, protocol keys, or timestamps.
// The bounded pool prevents high-cardinality traffic from creating an unbounded
// interning table. Unpooled contexts are conservatively charged per frame.
const NETWORK_POOL_LIMIT:usize=4096;
fn network_allocation_cost(network:&NetworkMeta)->usize {
    let strings=[&network.eth_src,&network.eth_dst,&network.ip_src,&network.ip_dst,&network.transport];
    std::mem::size_of::<NetworkMeta>()+16
        + strings.into_iter().filter_map(|s|s.as_ref()).map(|s|s.capacity()).sum::<usize>()
        + network.vlan_ids.capacity()*std::mem::size_of::<u16>()
}
fn frame_owned_cost(f:&NormalizedFrame)->usize {
    std::mem::size_of::<NormalizedFrame>().saturating_mul(5)/4
        + 256 + f.frame_key.capacity() + f.frame_key.len().saturating_mul(2)
        + f.payload.capacity() + f.raw_payload.capacity()
        + f.analog.as_ref().map(|m|m.unit.capacity()).unwrap_or(0)
}
fn tecmp_allocation_cost(f:&NormalizedFrame)->usize {
    f.tecmp.as_ref().map(|m|std::mem::size_of::<crate::model::TecmpMeta>()+16+m.message_type_name.capacity()).unwrap_or(0)
}
pub(crate) fn capture_frame_memory_cost(f:&NormalizedFrame)->usize {
    let network=network_allocation_cost(&f.network);
    frame_owned_cost(f)+network*2+64+tecmp_allocation_cost(f)
}
struct CaptureStorage {
    bytes:usize,budget:usize,
    networks:std::collections::HashMap<NetworkMeta,std::sync::Arc<NetworkMeta>>,
    last_tecmp:Option<usize>,
}
impl CaptureStorage {
    fn new(budget:usize)->Self { Self{bytes:0,budget,networks:Default::default(),last_tecmp:None} }
    fn retain(&mut self,frames:&mut Vec<NormalizedFrame>,mut f:NormalizedFrame)->Result<bool> {
        let network=self.networks.get(f.network.as_ref()).cloned();
        let pool_new=network.is_none() && self.networks.len()<NETWORK_POOL_LIMIT;
        let network_cost=if network.is_some(){0}else{
            let allocation=network_allocation_cost(&f.network);
            allocation+if pool_new {allocation+64}else{0}
        };
        // A parent is immediately followed by its extracted PDUs, which share
        // its immutable TECMP metadata. Charge that allocation once per group.
        let tecmp=f.tecmp.as_ref().map(|m|std::sync::Arc::as_ptr(m)as usize);
        let meta_cost=if tecmp==self.last_tecmp{0}else{tecmp_allocation_cost(&f)};
        let next=self.bytes.saturating_add(frame_owned_cost(&f)).saturating_add(network_cost).saturating_add(meta_cost);
        if next>self.budget{return Ok(false);}
        frames.try_reserve(1).map_err(|_|anyhow::anyhow!("Not enough memory to retain decoded capture data"))?;
        if let Some(shared)=network{f.network=shared;}
        else if pool_new{self.networks.insert((*f.network).clone(),std::sync::Arc::clone(&f.network));}
        frames.push(f);self.bytes=next;self.last_tecmp=tecmp;Ok(true)
    }
}

fn load_capture(path:&Path,max_packets:usize,selected:Option<&BTreeSet<u32>>,memory_budget:usize)->Result<(Vec<NormalizedFrame>,BTreeMap<u32,BTreeSet<String>>,bool,usize)>{
    let mut frames=Vec::new();
    frames.try_reserve(max_packets.min(65_536)).map_err(|_|anyhow::anyhow!("Not enough memory to start normalized frame decoding"))?;
    let mut seq=0u64;
    let mut storage=CaptureStorage::new(memory_budget);let mut memory_limited=false;
    let mut catalog:BTreeMap<u32,BTreeSet<String>>=BTreeMap::new();

    let mut consume=|p:CapturePacket|->Result<bool>{
        let recs=if p.linktype==1 { parse_ethernet_tecmp(&p.data) } else { vec![] };
        if recs.is_empty() {
            if p.linktype==227 {
                if selected.map(|set| set.contains(&0)).unwrap_or(true) {
                    if let Some(f)=parse_socketcan(&p.data,p.timestamp_ns,seq){
                        catalog.entry(f.channel_id).or_default().insert(f.bus_type.to_string());
                        if frames.len()<max_packets && !memory_limited {
                            if storage.retain(&mut frames,f)?{seq+=1;}else{memory_limited=true;}
                        }
                    }
                }
            } else if p.linktype==1 {
                if selected.map(|set| !set.contains(&p.interface_id)).unwrap_or(false) { return Ok(true); }
                catalog.entry(p.interface_id).or_default().insert("ETH".into());
                if frames.len()<max_packets && !memory_limited {if let Some(f)=parse_ethernet_packet(&p.data,p.timestamp_ns,seq,p.interface_id){
                    if storage.retain(&mut frames,f)?{seq+=1;}else{memory_limited=true;}
                }}
            }
        } else {
            for r in recs {
                // Control messages provide capture-wide timestamp markers. Keep
                // them alongside a nonempty channel selection even when they
                // arrive on a dedicated control interface.
                if selected.is_some_and(|set| !set.contains(&r.channel_id)
                    && (set.is_empty() || r.meta.message_type!=0x00)) { continue; }
                let bus=match discovery_kind_for_tecmp(&r) {
                    "CAN"=>"CAN","CAN-FD"=>"CAN-FD","LIN"=>"LIN","ETH"=>"ETH","Analog"=>"ANALOG","Unknown"=>"UNKNOWN",_=>"TECMP",
                };
                catalog.entry(r.channel_id).or_default().insert(bus.into());
                if frames.len()>=max_packets || memory_limited {continue;}
                let mut f=normalize_tecmp(&r,seq);
                f.timestamp_ns=canonical_capture_timestamp_ns(p.timestamp_ns,r.timestamp_ns);
                if let Some(shared)=f.tecmp.as_mut(){
                    let meta=std::sync::Arc::make_mut(shared);
                    meta.source_timestamp_ns=Some(r.timestamp_ns);
                    meta.capture_timestamp_ns=Some(p.timestamp_ns);
                }
                let children=detect_container_review(&f);
                if storage.retain(&mut frames,f)?{seq+=1;}else{memory_limited=true;continue;}
                for mut child in children {
                    if frames.len()>=max_packets {break;}
                    child.sequence=seq;
                    if storage.retain(&mut frames,child)?{seq+=1;}else{memory_limited=true;break;}
                }
            }
        }
        Ok(true)
    };

    // Always stream: keeping a second, whole-file raw packet array in memory
    // defeats the decoded-data budget even for a modest compressed capture.
    for_each_capture_packet(path,usize::MAX,&mut consume)?;
    drop(consume);
    Ok((frames,catalog,memory_limited,storage.bytes))
}

fn discovery_kind_for_tecmp(r:&crate::tecmp::TecmpRecord)->&'static str {
    // Message Type takes precedence over Data Type for TECMP system traffic.
    match r.meta.message_type {
        0x00 => "Control Messages",
        0x01 => "TECMP Status Device",
        0x02 => "TECMP Status Bus/IVN",
        0x04 => "TECMP Status Configuration",
        _ => match r.meta.data_type {
            crate::tecmp::DT_CAN=>"CAN", crate::tecmp::DT_CANFD=>"CAN-FD", crate::tecmp::DT_LIN=>"LIN",
            crate::tecmp::DT_ETH=>"ETH", crate::tecmp::DT_ANALOG|crate::tecmp::DT_ANALOG_SLA=>"Analog",
            crate::tecmp::DT_ANALOG_ALT=>"Analog (Alt, not decoded)", crate::tecmp::DT_FLEXRAY=>"FlexRay", crate::tecmp::DT_GPIO=>"GPIO",
            crate::tecmp::DT_UART_ASCII|crate::tecmp::DT_UART_SLA=>"UART", _=>"Unknown"
        }
    }
}

pub fn discover_capture_channels(path:&Path,_max_packets:usize)->Result<Vec<(u32,String)>> {
    let mut found:BTreeMap<u32,BTreeSet<String>>=BTreeMap::new();
    let mut consume=|p:CapturePacket|->Result<bool>{
        let recs=if p.linktype==1 { parse_ethernet_tecmp(&p.data) } else { vec![] };
        if recs.is_empty() {
            if p.linktype==227 { found.entry(0).or_default().insert("CAN/CAN-FD".into()); }
            else if p.linktype==1 { found.entry(p.interface_id).or_default().insert("ETH".into()); }
        } else {
            for r in recs {
                found.entry(r.channel_id).or_default().insert(discovery_kind_for_tecmp(&r).into());
            }
        }
        Ok(true)
    };
    // Discovery keeps channel IDs/kinds only. Stream to EOF so a decoded-record
    // limit cannot hide channels that first appear later in the capture.
    for_each_capture_packet(path,usize::MAX,&mut consume)?;
    drop(consume);
    Ok(found.into_iter().map(|(id,kinds)|(id,kinds.into_iter().collect::<Vec<_>>().join(" / "))).collect())
}

fn canonical_capture_timestamp_ns(outer_ns:u64, inner_ns:u64)->u64 {
    if inner_ns != 0 { inner_ns } else { outer_ns }
}

fn load_csv(path:&Path,max:usize)->Result<Vec<NormalizedFrame>>{
    let mut rdr=csv::ReaderBuilder::new().flexible(true).from_path(path)?; let headers=rdr.headers()?.iter().map(norm).collect::<Vec<_>>(); let mut out=Vec::new();
    for (seq,rec) in rdr.records().enumerate(){if out.len()>=max{break;}let r=rec?; if let Some(f)=row_to_frame(seq as u64,&headers,&r.iter().map(|x|x.to_string()).collect::<Vec<_>>()){out.push(f)}} Ok(out)
}

fn load_xlsx(path:&Path,max:usize)->Result<Vec<NormalizedFrame>>{
    let mut wb=open_workbook_auto(path).with_context(||format!("open {}",path.display()))?;let sheet=wb.sheet_names().first().cloned().context("Workbook has no sheets")?;let range=wb.worksheet_range(&sheet)?;let mut rows=range.rows();let headers=rows.next().unwrap_or(&[]).iter().map(|v|norm(&v.to_string())).collect::<Vec<_>>();let mut out=Vec::new();
    for (seq,row) in rows.enumerate(){if out.len()>=max{break;}let vals=row.iter().map(|v|v.to_string()).collect::<Vec<_>>();if let Some(f)=row_to_frame(seq as u64,&headers,&vals){out.push(f)}}Ok(out)
}

fn load_text(path:&Path,max:usize)->Result<Vec<NormalizedFrame>>{
    let file=fs::File::open(path).with_context(||format!("open {}",path.display()))?;
    let reader=BufReader::new(file); let mut out=Vec::new();
    for (seq,line) in reader.lines().enumerate(){if out.len()>=max{break;}let line=line?;let l=line.trim();if l.is_empty(){continue}
        // candump: (0.123456) can0 123#0102 or generic key=value lines
        if let Some(hash)=l.find('#'){let left=&l[..hash];let payload=&l[hash+1..];let id=left.split_whitespace().last().and_then(parse_id);let time=left.trim_start_matches('(').split(')').next().and_then(|x|x.parse::<f64>().ok()).unwrap_or(seq as f64/1000.0);if let Some(id)=id{out.push(simple_frame(seq as u64,time,0,BusType::Can,id,hex_bytes(payload)));continue}}
        let lower=l.to_ascii_lowercase();if lower.contains("id="){let id=extract_kv(l,"id").and_then(|x|parse_id(&x)).unwrap_or(0);let t=extract_kv(l,"time").and_then(|x|x.parse().ok()).unwrap_or(seq as f64/1000.0);let data=extract_kv(l,"data").map(|x|hex_bytes(&x)).unwrap_or_default();out.push(simple_frame(seq as u64,t,0,BusType::Can,id,data));}
    }Ok(out)
}

fn row_to_frame(seq:u64,h:&[String],r:&[String])->Option<NormalizedFrame>{
    let get=|names:&[&str]|->String{for n in names{let nn=norm(n);if let Some(i)=h.iter().position(|x|x==&nn){return r.get(i).cloned().unwrap_or_default()}}String::new()};
    let time=get(&["timestamp","timestamps","time","time_s","seconds"]).parse::<f64>().unwrap_or(seq as f64/1000.0);let ch=crate::model::parse_channel_id(&get(&["channel","channelid","channel_id"])).or_else(||crate::model::parse_channel_id(&get(&["bus"]))).unwrap_or(0);let id=parse_id(&get(&["canid","can_id","frameid","frame_id","id"]))?;let data_s=get(&["payload","data","bytes","datahex","data_hex"]);let mut data=hex_bytes(&data_s);
    if data.is_empty(){
        // byte0..byteN columns hold decimal values unless 0x-prefixed (e.g. 201 or 0xC9).
        for i in 0..64{let key=format!("byte{i}");if let Some(p)=h.iter().position(|x|x==&key){match r.get(p).and_then(|x|parse_byte_cell(x,false)){Some(v)=>data.push(v),None=>break}}else{break}}
    }
    if data.is_empty(){
        // SavvyCan style D1..Dn columns hold hexadecimal byte values.
        for i in 1..=64{let key=format!("d{i}");if let Some(p)=h.iter().position(|x|x==&key){match r.get(p).and_then(|x|parse_byte_cell(x,true)){Some(v)=>data.push(v),None=>break}}else{break}}
    }
    // Honour an explicit length column when present.
    if let Ok(len)=get(&["len","dlc","length"]).trim().parse::<usize>(){ if len<data.len(){data.truncate(len)} }
    let bus_s=get(&["bus","bustype","bus_type","protocol"]).to_ascii_lowercase();let bus=if bus_s.contains("lin"){BusType::Lin}else if bus_s.contains("fd"){BusType::CanFd}else if bus_s.contains("eth"){BusType::Ethernet}else{BusType::Can};Some(simple_frame(seq,time,ch,bus,id,data))
}

fn simple_frame(seq:u64,time:f64,ch:u32,bus:BusType,id:u32,payload:Vec<u8>)->NormalizedFrame{NormalizedFrame{sequence:seq,timestamp_ns:(time.max(0.0)*1e9)as u64,relative_time_s:time,channel_id:ch,bus_type:bus,frame_key:match bus{BusType::Lin=>format!("LIN:0x{id:02X}"),BusType::Ethernet=>format!("ETH:0x{id:04X}"),_=>format!("0x{id:X}")},identifier:Some(id),direction:Direction::Unknown,raw_payload:payload.clone(),payload,tecmp:None,analog:None,network:crate::model::empty_network()}}
fn norm(s:&str)->String{s.trim().to_ascii_lowercase().replace([' ','-'],"_").replace('_',"")}
fn parse_id(s:&str)->Option<u32>{let s=s.trim().trim_start_matches("0x").trim_start_matches("0X");u32::from_str_radix(s,16).ok().or_else(||s.parse().ok())}
/// Parse a hex byte string. Tokens separated by whitespace/`,;:-` may carry a `0x` prefix
/// ("0xAA 0xBB", "AA-BB"); a single unseparated token is read as consecutive byte pairs
/// ("AABB"). Parsing stops at the first invalid or incomplete byte instead of silently
/// dropping it, which would shift every following byte.
fn hex_bytes(s:&str)->Vec<u8>{
    let tokens:Vec<&str>=s.split(|c:char|c.is_whitespace()||matches!(c,','|';'|':'|'-')).filter(|t|!t.is_empty()).collect();
    let mut out=Vec::new();
    for t in tokens {
        let t=t.strip_prefix("0x").or_else(||t.strip_prefix("0X")).unwrap_or(t);
        if t.len()<=2 && !t.is_empty() {
            match u8::from_str_radix(t,16){Ok(v)=>out.push(v),Err(_)=>return out}
        } else {
            for i in 0..t.len()/2 {
                match t.get(i*2..i*2+2).and_then(|x|u8::from_str_radix(x,16).ok()){Some(v)=>out.push(v),None=>return out}
            }
            if t.len()%2!=0 {return out}
        }
    }
    out
}
/// Parse one byte cell. `0x` prefix is always hex; otherwise `hex_default` selects the radix.
fn parse_byte_cell(s:&str,hex_default:bool)->Option<u8>{
    let t=s.trim();
    if let Some(h)=t.strip_prefix("0x").or_else(||t.strip_prefix("0X")){return u8::from_str_radix(h,16).ok()}
    if hex_default { u8::from_str_radix(t,16).ok() }
    else if t.chars().all(|c|c.is_ascii_digit()) { t.parse::<u8>().ok() }
    else { u8::from_str_radix(t,16).ok() }
}
fn extract_kv(line:&str,key:&str)->Option<String>{for part in line.split_whitespace(){let mut sp=part.splitn(2,'=');if sp.next()?.eq_ignore_ascii_case(key){return sp.next().map(|x|x.trim_matches(',').to_string())}}None}


#[cfg(test)]
mod discovery_tests {
    use super::*;
    use crate::tecmp::TecmpRecord;

    fn rec(message_type:u8,data_type:u16)->TecmpRecord {
        TecmpRecord{
            timestamp_ns:0, channel_id:0x000F_FF03, payload:vec![0,0,0,2],
            meta:crate::model::TecmpMeta{cm_id:1,counter:0,version:1,message_type,
                message_type_name:crate::tecmp::message_type_name(message_type).into(),
                data_type,data_flags:0,timestamp_flags:0,device_flags:0,source_timestamp_ns:None,capture_timestamp_ns:None,timeline_projected:false}
        }
    }

    #[test]
    fn control_message_precedes_data_type_in_start_classification(){
        assert_eq!(discovery_kind_for_tecmp(&rec(0x00,crate::tecmp::DT_LIN)),"Control Messages");
    }

    #[test]
    fn logging_stream_still_uses_data_type(){
        assert_eq!(discovery_kind_for_tecmp(&rec(0x03,crate::tecmp::DT_LIN)),"LIN");
    }
}

#[cfg(test)]
mod loader_parsing_tests {
    use super::*;

    #[test]
    fn shared_capture_metadata_preserves_values_and_isolates_edits() {
        use std::sync::Arc;
        let mut first=simple_frame(1,0.0,7,BusType::Can,0x123,vec![1,2,3]);
        Arc::make_mut(&mut first.network).ip_src=Some("10.0.0.1".into());
        first.tecmp=Some(Arc::new(crate::model::TecmpMeta{cm_id:1,counter:2,version:3,message_type:3,message_type_name:"Logging Stream".into(),data_type:2,data_flags:0,timestamp_flags:0,device_flags:0,source_timestamp_ns:Some(123),capture_timestamp_ns:Some(456),timeline_projected:false}));
        let mut second=first.clone();second.sequence=2;second.payload=vec![4,5,6];
        // Equal contexts decoded independently must also be interned.
        second.network=Arc::new((*first.network).clone());
        let expected=[serde_json::to_value(&first).unwrap(),serde_json::to_value(&second).unwrap()];
        let unshared_cost=capture_frame_memory_cost(&first)+capture_frame_memory_cost(&second);
        let mut frames=Vec::new();let mut storage=CaptureStorage::new(unshared_cost);
        assert!(storage.retain(&mut frames,first).unwrap());assert!(storage.retain(&mut frames,second).unwrap());
        assert!(storage.bytes<unshared_cost);
        assert!(Arc::ptr_eq(&frames[0].network,&frames[1].network));
        assert!(Arc::ptr_eq(frames[0].tecmp.as_ref().unwrap(),frames[1].tecmp.as_ref().unwrap()));
        for (frame,json) in frames.iter().zip(expected){
            assert_eq!(serde_json::to_value(frame).unwrap(),json);
            let decoded:NormalizedFrame=serde_json::from_value(json.clone()).unwrap();assert_eq!(serde_json::to_value(decoded).unwrap(),json);
        }
        Arc::make_mut(&mut frames[0].network).ip_src=Some("10.0.0.2".into());
        Arc::make_mut(frames[0].tecmp.as_mut().unwrap()).source_timestamp_ns=Some(999);
        assert_eq!(frames[1].network.ip_src.as_deref(),Some("10.0.0.1"));
        assert_eq!(frames[1].tecmp.as_ref().unwrap().source_timestamp_ns,Some(123));
        assert_eq!(frames[0].raw_payload,vec![1,2,3]);assert_eq!(frames[1].payload,vec![4,5,6]);
    }

    #[test]
    fn capture_network_pool_is_bounded_for_unique_streams() {
        use std::sync::Arc;
        let mut frames=Vec::new();let mut storage=CaptureStorage::new(usize::MAX);
        for n in 0..NETWORK_POOL_LIMIT+20 {
            let mut frame=simple_frame(n as u64,0.0,1,BusType::Ethernet,0x800,vec![1]);
            Arc::make_mut(&mut frame.network).ip_src=Some(format!("stream-{n}"));
            assert!(storage.retain(&mut frames,frame).unwrap());
        }
        assert_eq!(storage.networks.len(),NETWORK_POOL_LIMIT);
        assert_eq!(frames.len(),NETWORK_POOL_LIMIT+20);
        assert_eq!(frames.last().unwrap().network.ip_src.as_deref(),Some(format!("stream-{}",NETWORK_POOL_LIMIT+19).as_str()));
        let repeated=frames[0].clone();assert!(storage.retain(&mut frames,repeated).unwrap());
        assert!(Arc::ptr_eq(&frames[0].network,&frames.last().unwrap().network));
        assert_eq!(storage.networks.len(),NETWORK_POOL_LIMIT);
    }


    #[test]
    fn capture_byte_budget_retains_a_prefix_and_complete_late_channel_catalog() {
        let input=tempfile::Builder::new().suffix(".pcap").tempfile().unwrap();
        let mut raw=vec![0u8;12];raw.extend(0x99feu16.to_be_bytes());raw.extend([0,1,0,1,3,3,0,0x20,0,0,0,0]);
        for channel in 1u32..=3 {
            raw.extend(channel.to_be_bytes());raw.extend((1_760_000_000_000_000_000u64+channel as u64).to_be_bytes());
            raw.extend(2u16.to_be_bytes());raw.extend(0x5900u16.to_be_bytes());raw.extend(100i16.to_be_bytes());
        }
        let mut bytes=vec![0x4d,0x3c,0xb2,0xa1];bytes.extend(2u16.to_le_bytes());bytes.extend(4u16.to_le_bytes());bytes.extend([0u8;8]);
        bytes.extend(65535u32.to_le_bytes());bytes.extend(1u32.to_le_bytes());bytes.extend(1_760_000_000u32.to_le_bytes());bytes.extend(0u32.to_le_bytes());
        bytes.extend((raw.len()as u32).to_le_bytes());bytes.extend((raw.len()as u32).to_le_bytes());bytes.extend(raw);std::fs::write(input.path(),bytes).unwrap();
        let (full,_,limited,_)=load_capture(input.path(),usize::MAX,None,usize::MAX).unwrap();assert_eq!(full.len(),3);assert!(!limited);
        let budget=capture_frame_memory_cost(&full[0]);
        let (prefix,catalog,limited,_)=load_capture(input.path(),usize::MAX,None,budget).unwrap();
        assert!(limited);assert_eq!(prefix.len(),1);assert_eq!(prefix[0].channel_id,1);
        assert_eq!(catalog.keys().copied().collect::<Vec<_>>(),vec![1,2,3]);
        let total=full.iter().map(capture_frame_memory_cost).sum();
        let (exact,_,limited,_)=load_capture(input.path(),usize::MAX,None,total).unwrap();assert!(!limited);assert_eq!(exact.len(),3);
        let (_,_,limited,_)=load_capture(input.path(),2,None,total).unwrap();assert!(!limited,"explicit record limits are distinguished from memory exhaustion");
    }


    #[test]
    fn exact_eof_is_complete_but_an_extra_decoded_row_is_partial() {
        let input=tempfile::Builder::new().suffix(".csv").tempfile().unwrap();
        std::fs::write(input.path(),"time,channel,id,payload\n0,1,123,00\n0.5,1,invalid,00\n1,1,123,01\n").unwrap();
        let full=load(input.path(),2).unwrap();assert_eq!(full.frames.len(),2);
        assert!(!full.warnings.iter().any(|w|w.starts_with("Safe load limit reached")));
        let partial=load(input.path(),1).unwrap();assert_eq!(partial.frames.len(),1);
        assert!(partial.warnings.iter().any(|w|w.starts_with("Safe load limit reached")));
    }
    #[test]
    fn hex_bytes_supports_separators_prefixes_and_pairs() {
        assert_eq!(hex_bytes("01 02 AB"),vec![1,2,0xAB]);
        assert_eq!(hex_bytes("0xAA 0xBB"),vec![0xAA,0xBB]);
        assert_eq!(hex_bytes("AABBCC"),vec![0xAA,0xBB,0xCC]);
        assert_eq!(hex_bytes("AA-BB:CC"),vec![0xAA,0xBB,0xCC]);
    }

    #[test]
    fn hex_bytes_stops_at_invalid_byte_instead_of_shifting() {
        assert_eq!(hex_bytes("AA ZZ BB"),vec![0xAA]);
        assert_eq!(hex_bytes("AABBC"),vec![0xAA,0xBB]);
    }

    #[test]
    fn byte_columns_are_decimal_and_savvycan_columns_are_hex() {
        assert_eq!(parse_byte_cell("201",false),Some(201));
        assert_eq!(parse_byte_cell("0xC9",false),Some(0xC9));
        assert_eq!(parse_byte_cell("16",true),Some(0x16));
        assert_eq!(parse_byte_cell("256",false),None);
    }

    #[test]
    fn csv_row_with_decimal_byte_columns_and_timestamp_s() {
        let h=["timestamps","channelid","canid","dlc","byte0","byte1"].iter().map(|x|x.to_string()).collect::<Vec<_>>();
        let r=["0.5","1","0x0040","2","16","201"].iter().map(|x|x.to_string()).collect::<Vec<_>>();
        let f=row_to_frame(0,&h,&r).unwrap();
        assert_eq!(f.payload,vec![16,201]);
        assert!((f.relative_time_s-0.5).abs()<1e-9);
        assert_eq!(f.channel_id,1);
    }

    #[test]
    fn savvycan_row_uses_d_columns_and_len() {
        let h=["timestamp","id","extended","bus","len","d1","d2","d3"].iter().map(|x|x.to_string()).collect::<Vec<_>>();
        let r=["0.01","100","false","0","2","01","10","FF"].iter().map(|x|x.to_string()).collect::<Vec<_>>();
        let f=row_to_frame(0,&h,&r).unwrap();
        assert_eq!(f.payload,vec![0x01,0x10]);
        assert_eq!(f.identifier,Some(0x100));
    }
}
