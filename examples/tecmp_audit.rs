#![allow(dead_code)]
#[path="../src/model.rs"] mod model;
#[path="../src/tecmp.rs"] mod tecmp;
#[path="../src/protocols.rs"] mod protocols;
#[path="../src/capture.rs"] mod capture;
#[path="../src/resources.rs"] mod resources;
#[path="../src/native_engine.rs"] mod native_engine;
#[path="../src/diagnostics.rs"] mod diagnostics;
#[path="../src/index.rs"] mod index;
#[path="../src/engine.rs"] mod engine;

use std::{collections::BTreeMap,path::Path};
fn record(dt:u16,mt:u8,flags:u16,payload:Vec<u8>)->tecmp::TecmpRecord {
 tecmp::TecmpRecord{timestamp_ns:1_760_000_000_123_456_789,channel_id:1,payload,meta:model::TecmpMeta{cm_id:1,counter:1,version:3,message_type:mt,message_type_name:tecmp::message_type_name(mt).into(),data_type:dt,data_flags:flags,timestamp_flags:0,device_flags:0,source_timestamp_ns:None,capture_timestamp_ns:None,timeline_projected:false}}
}
fn ethernet(r:&tecmp::TecmpRecord)->Vec<u8>{
 let mut v=vec![0;12];v.extend(0x99feu16.to_be_bytes());v.extend([0,1,0,1,3,r.meta.message_type]);v.extend(r.meta.data_type.to_be_bytes());v.extend([0,0,0,7]);v.extend(r.channel_id.to_be_bytes());v.extend(r.timestamp_ns.to_be_bytes());v.extend((r.payload.len() as u16).to_be_bytes());v.extend(r.meta.data_flags.to_be_bytes());v.extend(&r.payload);v
}
fn probe(name:&str,pass:bool,detail:impl std::fmt::Display){println!("{} {name}: {detail}",if pass{"PASS"}else{"FAIL"});}
fn main()->anyhow::Result<()> {
 let args:Vec<String>=std::env::args().collect();
 if args.get(1).is_some_and(|a|a=="--protocols") {
  fn hash(bytes:&[u8])->u64{bytes.iter().fold(0xcbf29ce484222325u64,|h,b|(h^*b as u64).wrapping_mul(0x100000001b3))}
  let mut packet=0;
  capture::for_each_capture_packet(Path::new(&args[2]),usize::MAX,|p|{
   packet+=1;
   if p.linktype==1 {for (entry,r) in tecmp::parse_ethernet_tecmp(&p.data).into_iter().enumerate(){
    let f=protocols::normalize_tecmp(&r,packet);
    let payload=if matches!(r.meta.data_type,2|3|4){f.payload.iter().map(|b|format!("{b:02x}")).collect::<String>()}else{String::new()};
    println!("{packet}\t{entry}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",r.meta.message_type,r.meta.data_type,r.channel_id,r.timestamp_ns,r.meta.data_flags,f.bus_type,f.frame_key,f.identifier.map(|v|v.to_string()).unwrap_or_default(),f.payload.len(),hash(&f.payload),f.network.can_crc.map(|v|v.to_string()).unwrap_or_default(),f.network.lin_checksum.map(|v|v.to_string()).unwrap_or_default(),r.payload.len(),hash(&r.payload),f.network.eth_src.as_deref().unwrap_or_default(),f.network.eth_dst.as_deref().unwrap_or_default(),f.network.src_port.map(|v|v.to_string()).unwrap_or_default(),f.network.dst_port.map(|v|v.to_string()).unwrap_or_default(),f.network.transport.as_deref().unwrap_or_default(),payload);
   }}Ok(true)
  })?;
  return Ok(());
 }
 if args.get(1).is_some_and(|a|a=="--analog") {
  let mut packet=0;
  capture::for_each_capture_packet(Path::new(&args[2]),usize::MAX,|p|{
   packet+=1;
   if p.linktype==1 {for r in tecmp::parse_ethernet_tecmp(&p.data){
    let f=protocols::normalize_tecmp(&r,packet);
    if let Some(m)=f.analog {
     let samples=f.payload.chunks_exact(2).map(|b|(i16::from_be_bytes([b[0],b[1]]) as f64*m.factor).to_string()).collect::<Vec<_>>().join(",");
     println!("{packet}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{samples}",r.channel_id,r.timestamp_ns,r.meta.data_flags,m.sample_period_ns,m.factor,m.unit,f.payload.len(),m.sample_count,p.timestamp_ns);
    }
   }}Ok(true)
  })?;
  return Ok(());
 }
 if args.get(1).is_some_and(|a|a=="--memory-scan") {
  let mut count=0usize;let mut memory=0usize;
  capture::for_each_capture_packet(Path::new(&args[2]),usize::MAX,|packet|{
   let records=if packet.linktype==1{tecmp::parse_ethernet_tecmp(&packet.data)}else{vec![]};
   let mut measure=|frame:model::NormalizedFrame|{count+=1;memory=memory.saturating_add(engine::capture_frame_memory_cost(&frame));};
   if records.is_empty(){
    if packet.linktype==1{if let Some(frame)=protocols::parse_ethernet_packet(&packet.data,packet.timestamp_ns,0,packet.interface_id){measure(frame);}}
    else if packet.linktype==227{if let Some(frame)=protocols::parse_socketcan(&packet.data,packet.timestamp_ns,0){measure(frame);}}
   }else{for record in records {
    let frame=protocols::normalize_tecmp(&record,0);let children=protocols::detect_container_review(&frame);measure(frame);for child in children{measure(child);}
   }}Ok(true)
  })?;
  println!("normalized_records={count} estimated_memory_bytes={memory} capture_budget_bytes={}",resources::limits().capture_memory_budget());
  return Ok(());
 }
 if args.get(1).is_some_and(|a|a=="--timeline") {
  let d=engine::load_selected_with_timeline(Path::new(&args[2]),usize::MAX,None,true)?;
  let mut ranges=std::collections::BTreeMap::<String,(usize,f64,f64)>::new();
  for f in &d.frames {let key=format!("{} channel {}",f.bus_type,f.channel_id);let r=ranges.entry(key).or_insert((0,f.relative_time_s,f.relative_time_s));r.0+=1;r.1=r.1.min(f.relative_time_s);r.2=r.2.max(f.relative_time_s);}
  println!("records={} duration={} first={} last={} estimated_bytes={:?} warnings={:?}\n{ranges:#?}",d.frames.len(),d.frames.last().unwrap().relative_time_s,d.frames.first().unwrap().timestamp_ns,d.frames.last().unwrap().timestamp_ns,d.capture_memory_estimate,d.warnings);
  return Ok(());
 }
 if args.get(1).is_some_and(|a|a=="--analog-load") {
  let channels=std::collections::BTreeSet::from([32785,32786,36881,36882,36883,36884]);
  let d=engine::load_selected_with_timeline(Path::new(&args[2]),usize::MAX,Some(&channels),false)?;
  println!("records={} duration={} warnings={:?}",d.frames.len(),d.frames.last().map(|f|f.relative_time_s).unwrap_or(0.0),d.warnings);
  return Ok(());
 }
 if args.get(1).is_some_and(|a|a=="--load") {
  let d=engine::load_selected_with_timeline(Path::new(&args[2]),usize::MAX,None,false)?;
  println!("duration={} warnings={:?}",d.frames.last().map(|f|f.relative_time_s).unwrap_or(0.0),d.warnings);
  let errors=d.frames.iter().filter(|f|f.frame_key.contains(":ERROR:")).count();
  let raw=d.frames.iter().filter(|f|!f.raw_payload.is_empty()).count();
  let clocks=d.frames.iter().filter(|f|f.tecmp.as_ref().is_some_and(|m|m.capture_timestamp_ns.is_some() && m.source_timestamp_ns.is_some())).count();
  println!("LOADED records={} error_records={} retained_raw={} both_clocks={} native_unique_keys={}",d.frames.len(),errors,raw,clocks,native_engine::analyze_frames(&d.frames)?.unique_keys);
  return Ok(());
 }
 if args.get(1).is_some_and(|a|a=="--offset") {
  let p=Path::new(&args[2]);
  let rust=capture::read_capture(p,10)?;let native=native_engine::read_capture(p,10)?;
  probe("PCAPNG if_tsoffset Rust",rust[0].timestamp_ns==11_000_000_000,format!("timestamp_ns={} expected=11000000000",rust[0].timestamp_ns));
  probe("PCAPNG if_tsoffset native",native[0].timestamp_ns==11_000_000_000,format!("timestamp_ns={} expected=11000000000",native[0].timestamp_ns));
  return Ok(());
 }
 if args.get(1).is_some_and(|a|a=="--dump") {
  let mut packet=0;
  capture::for_each_capture_packet(Path::new(&args[2]),5000,|p|{
   packet+=1;
   if p.linktype==1 {for r in tecmp::parse_ethernet_tecmp(&p.data){println!("{}\t{}\t{}\t{}\t{}\t{}",packet,r.meta.message_type,r.meta.data_type,r.channel_id,r.timestamp_ns,r.meta.data_flags);}}
   Ok(true)
  })?;
  return Ok(());
 }

 let good=record(2,3,0,vec![0,0,1,0x23,1,0xaa,0,0]);
 let mut raw=ethernet(&good);let marked=good.timestamp_ns|(3u64<<62);raw[30..38].copy_from_slice(&marked.to_be_bytes());
 let parsed=tecmp::parse_ethernet_tecmp(&raw);
 probe("62-bit timestamp and flags",parsed[0].timestamp_ns==good.timestamp_ns && parsed[0].meta.timestamp_flags==3,format!("timestamp={} flags={}",parsed[0].timestamp_ns,parsed[0].meta.timestamp_flags));
 let mut vlan=raw[..12].to_vec();vlan.extend([0x81,0,0,1]);vlan.extend(&raw[12..]);
 probe("VLAN TECMP header",tecmp::parse_ethernet_tecmp(&vlan).len()==1,"one record");
 let f=protocols::normalize_tecmp(&good,1);
 probe("Valid CAN payload",f.payload==vec![0xaa] && f.identifier==Some(0x123),f.frame_key);

 for (name,dt,mt,flags,p) in [
  ("CAN error isolated",2,3,8,vec![0,0,0,0,0,0,0]),
  ("CAN RTR isolated",2,3,2,vec![0,0,1,0x23,0,0,0]),
  ("CANFD illegal length 9 rejected",3,3,0,vec![0,0,1,0x23,9,1,2,3,4,5,6,7,8,9,0,0,0]),
  ("CAN replay direction Tx",2,10,0,vec![0,0,1,0x23,1,0xaa]),
  ("Ethernet replay direction Tx",0x80,10,0,vec![0;14]),
 ] {
  let f=protocols::normalize_tecmp(&record(dt,mt,flags,p),1);
  let pass=if mt==10{f.direction==model::Direction::Tx}else{f.frame_key.contains("ERROR")||f.frame_key.contains("RTR")||f.frame_key.contains("DECODE_ISSUE")};
  probe(name,pass,format!("key={} direction={}",f.frame_key,f.direction));
 }
 let r=record(2,3,0,vec![0,0,1,0x23,1,0xaa,0,0]);
 let outer=r.timestamp_ns+1_000_000;
 let raw=ethernet(&r);
 let temp=tempfile::Builder::new().suffix(".pcap").tempfile()?;
 let mut p=Vec::new();p.extend([0x4d,0x3c,0xb2,0xa1]);p.extend(2u16.to_le_bytes());p.extend(4u16.to_le_bytes());p.extend([0;8]);p.extend(65535u32.to_le_bytes());p.extend(1u32.to_le_bytes());p.extend(((outer/1_000_000_000)as u32).to_le_bytes());p.extend(((outer%1_000_000_000)as u32).to_le_bytes());p.extend((raw.len()as u32).to_le_bytes());p.extend((raw.len()as u32).to_le_bytes());p.extend(&raw);std::fs::write(temp.path(),p)?;
 let ds=engine::load(temp.path(),10)?;
 probe("Loader preserves TECMP source timestamp",ds.frames[0].timestamp_ns==r.timestamp_ns,format!("source={} loaded={} outer={}",r.timestamp_ns,ds.frames[0].timestamp_ns,outer));
 let std=protocols::normalize_tecmp(&record(2,3,0,vec![0,0,1,0x23,1,0xaa]),1);
 let ext=protocols::normalize_tecmp(&record(2,3,4,vec![0x80,0,1,0x23,1,0xbb]),2);
 let stats=native_engine::analyze_frames(&[std,ext])?;
 probe("Native standard/extended CAN keys distinct",stats.unique_keys==2,format!("unique_keys={}",stats.unique_keys));
 for path in std::env::args().skip(1){
  let mut counts=BTreeMap::<String,usize>::new();let mut types=BTreeMap::<String,usize>::new();let mut records=0;let mut diff=0;let mut min_diff=i128::MAX;let mut max_diff=i128::MIN;let mut issues=0;let mut unknown=0;
  let packets=capture::for_each_capture_packet(Path::new(&path),usize::MAX,|p|{
   if p.linktype==1 {for r in tecmp::parse_ethernet_tecmp(&p.data){records+=1;*types.entry(format!("mt={:02x} dt={:04x}",r.meta.message_type,r.meta.data_type)).or_default()+=1;
    let f=protocols::normalize_tecmp(&r,records as u64);*counts.entry(f.bus_type.to_string()).or_default()+=1;if f.frame_key.contains("DECODE_ISSUE"){issues+=1;}if f.bus_type==model::BusType::Unknown{unknown+=1;}
    if p.timestamp_ns>=946_684_800_000_000_000 && r.timestamp_ns!=p.timestamp_ns{diff+=1;let d=p.timestamp_ns as i128-r.timestamp_ns as i128;min_diff=min_diff.min(d);max_diff=max_diff.max(d);}
   }}Ok(true)
  })?;
  println!("CAPTURE {path}\npackets={packets} records={records} bus_counts={counts:?}\ntypes={types:?}\ndecode_issues={issues} unknown={unknown} outer_inner_differences={diff} delta_ns_range={min_diff}..{max_diff}");
 }
 Ok(())
}
