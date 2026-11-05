#[cfg(test)] use crate::model::NetworkMeta;
use std::sync::Arc;
use crate::{model::{AnalogMeta,BusType,Direction,NormalizedFrame}, tecmp::{TecmpRecord,DT_ANALOG,DT_ANALOG_SLA,DT_ANALOG_ALT,DT_CAN,DT_CANFD,DT_ETH,DT_LIN,DT_FLEXRAY,DT_GPIO,DT_UART_ASCII,DT_UART_SLA}};
fn be16(x:&[u8])->u16{u16::from_be_bytes([x[0],x[1]])} fn be32(x:&[u8])->u32{u32::from_be_bytes([x[0],x[1],x[2],x[3]])}
fn direction(flags:u16)->Direction { if flags&(1<<14)!=0{Direction::Tx}else{Direction::Rx} }
fn mac(x:&[u8])->String{x.iter().map(|b|format!("{b:02X}")).collect::<Vec<_>>().join(":")}

/// Report only known error/decode conditions, using the protocol-specific flag
/// meanings in TECMP v1.7 and Wireshark. Overflow reports lost earlier data, not
/// corruption of this record. Replay flags, analog sample-time bits, RTR, ESI,
/// BRS and normal wake/sleep events must not be treated as receive errors.
pub fn frame_error_reason(f:&NormalizedFrame)->Option<&'static str> {
    if let Some(meta)=&f.tecmp {
        if matches!(meta.message_type,0x00|0x01|0x02|0x04){return None;}
        if meta.message_type==0x03 {
            let mask=match meta.data_type {
                DT_CAN=>0x20F8, DT_CANFD=>0x21E8, DT_LIN=>0x2007,
                DT_ETH=>0x2000, DT_FLEXRAY=>0x3000,
                DT_UART_ASCII=>0x0001,
                _=>0,
            };
            if meta.data_flags&mask!=0{return Some("Protocol error flags");}
        }
    }
    let key=f.frame_key.as_str();
    if key.starts_with("CAN:ERROR:") || key.starts_with("CANFD:ERROR:") || key.starts_with("LIN:ERROR:") {
        return Some("Error frame");
    }
    if key.starts_with("CAN:DECODE_ISSUE:") || key.starts_with("CANFD:DECODE_ISSUE:") || key.starts_with("LIN:DECODE_ISSUE:") {
        return Some("Malformed / truncated frame");
    }
    None
}

pub fn frame_visible(f:&NormalizedFrame,hide_errors:bool)->bool {
    !hide_errors || frame_error_reason(f).is_none()
}

pub fn normalize_tecmp(r:&TecmpRecord,seq:u64)->NormalizedFrame {
    let mut f=NormalizedFrame{sequence:seq,timestamp_ns:r.timestamp_ns,relative_time_s:0.0,channel_id:r.channel_id,bus_type:BusType::Unknown,frame_key:format!("TECMP:0x{:04X}",r.meta.data_type),identifier:None,direction:if r.meta.message_type==0x0A{Direction::Tx}else{direction(r.meta.data_flags)},payload:r.payload.clone(),raw_payload:r.payload.clone(),tecmp:Some(Arc::new(r.meta.clone())),analog:None,network:crate::model::empty_network()};

    // TECMP message type is the first-level semantic discriminator. Control and
    // status records are protocol-management/system traffic, not CAN/LIN/ETH
    // payload records even if their Data Type field resembles a bus data type.
    match r.meta.message_type {
        0x00 => {
            f.bus_type=BusType::Tecmp;
            let control_id=if r.payload.len()>=4 {
                u16::from_be_bytes([r.payload[2],r.payload[3]])
            } else { r.meta.cm_id };
            f.identifier=Some(control_id as u32);
            f.frame_key=format!("TECMP:CONTROL:0x{control_id:04X}");
            return f;
        },
        0x01 => { f.bus_type=BusType::Tecmp; f.frame_key=format!("TECMP:STATUS_DEVICE:CH{}",r.channel_id); return f; },
        0x02 => { f.bus_type=BusType::Tecmp; f.frame_key=format!("TECMP:STATUS_BUS_IVN:CH{}",r.channel_id); return f; },
        0x04 => { f.bus_type=BusType::Tecmp; f.frame_key=format!("TECMP:STATUS_CONFIGURATION:CH{}",r.channel_id); return f; },
        _ => {}
    }

    match r.meta.data_type {
        DT_CAN => { f.bus_type=BusType::Can; f.frame_key=format!("CAN:RAW:CH{}",f.channel_id); parse_can(&mut f,false); },
        DT_CANFD => { f.bus_type=BusType::CanFd; f.frame_key=format!("CANFD:RAW:CH{}",f.channel_id); parse_can(&mut f,true); },
        DT_LIN => { f.bus_type=BusType::Lin; f.frame_key=format!("LIN:DECODE_PENDING:CH{}",f.channel_id); parse_lin(&mut f); },
        DT_ETH => { f.bus_type=BusType::Ethernet; f.frame_key=format!("ETH:RAW:CH{}",f.channel_id); parse_eth(&mut f); },
        DT_ANALOG|DT_ANALOG_SLA=>{
            f.bus_type=BusType::Analog;
            f.frame_key=format!("ANALOG:{}",f.channel_id);
            parse_analog(&mut f);
        },
        DT_ANALOG_ALT=>{f.bus_type=BusType::Tecmp;f.frame_key=format!("TECMP:ANALOG_ALT:CH{}",r.channel_id)},
        DT_FLEXRAY|DT_GPIO|DT_UART_ASCII|DT_UART_SLA=>{f.bus_type=BusType::Tecmp;f.frame_key=format!("TECMP:0x{:04X}:CH{}",r.meta.data_type,r.channel_id)},
        _=>{f.bus_type=BusType::Unknown}
    } f
}

fn analog_sample_period_ns(flags:u16)->u64 {
    match (flags>>11)&0x0F {
        0x1=>2_500_000_000, 0x2=>1_000_000_000, 0x3=>500_000_000,
        0x4=>250_000_000, 0x5=>100_000_000, 0x6=>50_000_000,
        0x7=>25_000_000, 0x8=>10_000_000, 0x9=>5_000_000,
        0xA=>2_500_000, 0xB=>1_000_000,
        // 0xC..=0xF are valid sample times in the Wireshark/Technica sample-time table.
        // Omitting them made every high-rate (<=0.5 ms) analog packet decode to period 0
        // and silently disappear from the waveform.
        0xC=>500_000, 0xD=>250_000, 0xE=>100_000, 0xF=>50_000,
        _=>0
    }
}
fn analog_factor(flags:u16)->f64 {
    match (flags>>7)&0x03 {0=>0.1,1=>0.01,2=>0.001,_=>0.0001}
}
fn analog_unit(flags:u16)->&'static str {
    match (flags>>2)&0x07 {0=>"V",1=>"A",2=>"W",3=>"Ah",4=>"°C",_=>""}
}
fn parse_analog(f:&mut NormalizedFrame){
    // TECMP User Manual 1.7 §2.4.3.6:
    // data is an array of INT16 samples; timestamp belongs to the first sample.
    // TECMP fields/data use network byte order, so each INT16 is big-endian.
    let flags=f.tecmp.as_ref().map(|m|m.data_flags).unwrap_or(0);
    let count=f.raw_payload.len()/2;
    f.payload=f.raw_payload.clone(); // retain the original packet bytes for Trace/inspection.
    f.analog=Some(AnalogMeta{
        sample_count:count,
        sample_width_bytes:2,
        sample_period_ns:analog_sample_period_ns(flags),
        factor:analog_factor(flags),
        unit:analog_unit(flags).to_string(),
    });
}

/// Frame key for a CAN identifier. Extended (29-bit) IDs get an `EXT:` prefix so that
/// standard 0x123 and extended 0x123 are never merged by indexing or analysis.
fn can_frame_key(id:u32,extended:bool)->String {
    if extended { format!("EXT:0x{id:X}") } else { format!("0x{id:X}") }
}

fn parse_can(f:&mut NormalizedFrame,fd:bool){
    // TECMP CAN/CAN-FD entry data: [BE32 ID, bit31 = extended-ID flag][length][data].
    if f.raw_payload.len()<5 {
        f.frame_key=format!("{}:DECODE_ISSUE:TRUNCATED:CH{}",if fd{"CANFD"}else{"CAN"},f.channel_id);
        f.payload.clear();
        return;
    }
    let raw=be32(&f.raw_payload[..4]);
    let extended=raw&0x8000_0000!=0;
    let id=raw&if extended{0x1FFF_FFFF}else{0x7FF};
    let n=f.raw_payload[4] as usize;
    let valid_len=if fd{matches!(n,0..=8|12|16|20|24|32|48|64)}else{n<=8};
    let flags=f.tecmp.as_ref().map(|m|m.data_flags).unwrap_or(0);
    let rx=f.tecmp.as_ref().is_some_and(|m|m.message_type==0x03);
    let errors=if fd{flags&0x21E8}else{flags&0x20F8};
    if rx && errors!=0 {
        f.identifier=None;
        f.payload.clear();
        f.frame_key=format!("{}:ERROR:FLAGS_0x{errors:04X}:CH{}",if fd{"CANFD"}else{"CAN"},f.channel_id);
        return;
    }
    if rx && !fd && flags&2!=0 {
        f.identifier=Some(id);
        f.payload.clear();
        f.frame_key=format!("CAN:RTR:{}",can_frame_key(id,extended));
        return;
    }
    if !valid_len || f.raw_payload.len()<5+n {
        // Keep the record visible for diagnosis, but isolate it from the normal ID stream
        // and from automatic payload analysis (the 5-byte header must not become payload).
        f.identifier=Some(id);
        f.payload.clear();
        f.frame_key=format!("{}:DECODE_ISSUE:BAD_LENGTH_{}:{}",if fd{"CANFD"}else{"CAN"},n,can_frame_key(id,extended));
        return;
    }
    f.identifier=Some(id);
    f.payload=f.raw_payload[5..5+n].to_vec();
    let trailer=&f.raw_payload[5+n..];
    Arc::make_mut(&mut f.network).can_crc=if fd && trailer.len()>=3 {
        Some(((trailer[0] as u32)<<16)|((trailer[1] as u32)<<8)|trailer[2] as u32)
    } else if !fd && trailer.len()>=2 {Some(be16(trailer) as u32)}else{None};
    f.bus_type=if fd{BusType::CanFd}else{BusType::Can};
    f.frame_key=can_frame_key(id,extended);
}
const LIN_RX_COLLISION:u16=1<<0;
const LIN_RX_PARITY:u16=1<<1;
const LIN_RX_NO_SLAVE:u16=1<<2;
const LIN_RX_WUP:u16=1<<8;
const LIN_RX_SHORT_WUP:u16=1<<9;
const LIN_RX_SLEEP:u16=1<<10;
const LIN_RX_CHECKSUM:u16=1<<13;
const LIN_RX_TX:u16=1<<14;
const LIN_RX_OVERFLOW:u16=1<<15;

/// Human-readable TECMP LIN flags. Logging Stream (0x03) and Replay Data (0x0A)
/// intentionally use different flag semantics in TECMP User Manual v1.7.
pub fn lin_flags_text(message_type:u8,flags:u16)->String {
    let mut out=Vec::<&'static str>::new();
    if message_type==0x0A {
        if flags&(1<<1)!=0{out.push("Use Parity");}
        if flags&(1<<8)!=0{out.push("WUP");}
        if flags&(1<<9)!=0{out.push("Short WUP");}
        if flags&(1<<13)!=0{out.push("Use Checksum");}
    } else {
        if flags&LIN_RX_COLLISION!=0{out.push("Collision error");}
        if flags&LIN_RX_PARITY!=0{out.push("Parity error");}
        if flags&LIN_RX_NO_SLAVE!=0{out.push("No Slave Response");}
        if flags&LIN_RX_WUP!=0{out.push("WUP");}
        if flags&LIN_RX_SHORT_WUP!=0{out.push("Short WUP");}
        if flags&LIN_RX_SLEEP!=0{out.push("SLEEP");}
        if flags&LIN_RX_CHECKSUM!=0{out.push("Checksum error");}
        if flags&LIN_RX_TX!=0{out.push("Tx");}
        if flags&LIN_RX_OVERFLOW!=0{out.push("Overflow");}
    }
    if out.is_empty(){"none".into()}else{out.join(", ")}
}

fn lin_rx_zero_data_condition(flags:u16)->Option<String> {
    let mut events=Vec::<&'static str>::new();
    let mut errors=Vec::<&'static str>::new();
    if flags&LIN_RX_WUP!=0{events.push("WAKE_UP");}
    if flags&LIN_RX_SHORT_WUP!=0{events.push("SHORT_WAKE_UP");}
    if flags&LIN_RX_SLEEP!=0{events.push("SLEEP");}
    if flags&LIN_RX_NO_SLAVE!=0{events.push("NO_SLAVE_RESPONSE");}
    if flags&LIN_RX_COLLISION!=0{errors.push("COLLISION");}

    if !events.is_empty() && errors.is_empty(){return Some(format!("LIN:EVENT:{}",events.join("+")));}
    if events.is_empty() && !errors.is_empty(){return Some(format!("LIN:ERROR:{}",errors.join("+")));}
    if !events.is_empty() || !errors.is_empty(){
        let mut all=events; all.extend(errors);
        return Some(format!("LIN:CONDITION:{}",all.join("+")));
    }
    None
}

fn lin_rx_data_error_key(flags:u16,id:Option<u32>)->Option<String> {
    let mut errors=Vec::<&'static str>::new();
    if flags&LIN_RX_PARITY!=0{errors.push("PARITY");}
    if flags&LIN_RX_CHECKSUM!=0{errors.push("CHECKSUM");}
    if errors.is_empty(){return None;}
    Some(match id {
        Some(id)=>format!("LIN:ERROR:{}:0x{id:02X}",errors.join("+")),
        None=>format!("LIN:ERROR:{}",errors.join("+")),
    })
}

fn parse_lin(f:&mut NormalizedFrame){
    let (message_type,flags)=f.tecmp.as_ref().map(|m|(m.message_type,m.data_flags)).unwrap_or((0x03,0));
    f.bus_type=BusType::Lin;

    // TECMP v1.7 §2.4.3: for special reception conditions where no valid payload
    // can be captured (including LIN WUP), all Data-section fields are 0x00.
    // Decode the Data Flags first so ID=0x00/Length=0 is not misreported as a
    // normal LIN 0x00 frame or left as an ambiguous "Raw LIN" record.
    if message_type==0x03 {
        if let Some(key)=lin_rx_zero_data_condition(flags){
            f.identifier=None;
            f.payload.clear();
            Arc::make_mut(&mut f.network).lin_checksum=None;
            f.frame_key=key;
            return;
        }
    }

    // Replay Data has different LIN flags. WUP/Short-WUP are transmission events,
    // not normal LIN payload frames.
    if message_type==0x0A {
        f.direction=Direction::Tx;
        let wup=flags&(1<<8)!=0;
        let short=flags&(1<<9)!=0;
        if wup || short {
            f.identifier=None;
            f.payload.clear();
            Arc::make_mut(&mut f.network).lin_checksum=None;
            f.frame_key=match (wup,short) {
                (true,true)=>"LIN:EVENT:TX_WAKE_UP+TX_SHORT_WAKE_UP".into(),
                (true,false)=>"LIN:EVENT:TX_WAKE_UP".into(),
                (false,true)=>"LIN:EVENT:TX_SHORT_WAKE_UP".into(),
                _=>unreachable!(),
            };
            return;
        }
    }

    if f.raw_payload.len()<2 {
        f.identifier=None;
        f.payload.clear();
        Arc::make_mut(&mut f.network).lin_checksum=None;
        f.frame_key=if message_type==0x03 {
            lin_rx_data_error_key(flags,None).unwrap_or_else(||"LIN:DECODE_ISSUE:TRUNCATED".into())
        } else {"LIN:DECODE_ISSUE:TRUNCATED".into()};
        return;
    }

    let id=(f.raw_payload[0]&0x3F) as u32;
    let n=f.raw_payload[1] as usize;

    // TECMP LIN Tx permits length=0 specifically to remove the slave-response
    // configuration entry for the supplied LIN ID. It is not an Rx frame.
    if message_type==0x0A && n==0 {
        f.identifier=Some(id);
        f.payload.clear();
        Arc::make_mut(&mut f.network).lin_checksum=f.raw_payload.get(2).copied();
        f.frame_key=format!("LIN:TX:REMOVE_SLAVE_RESPONSE:0x{id:02X}");
        return;
    }

    // Accept LIN payload lengths 1..=8. TECMP v1.7 documents common lengths
    // 1/2/4/8, while real Technica captures can carry valid LIN responses with
    // intermediate lengths (3/5/6/7). Length 0 remains reserved for special
    // conditions/replay operations handled above.
    if n==0 || n>8 {
        f.payload.clear();
        Arc::make_mut(&mut f.network).lin_checksum=None;
        if message_type==0x03 {
            if let Some(key)=lin_rx_data_error_key(flags,None) {
                // With an invalid/zeroed Data section the ID byte is not trustworthy.
                f.identifier=None;
                f.frame_key=key;
                return;
            }
        }
        f.identifier=Some(id);
        f.frame_key=format!("LIN:DECODE_ISSUE:INVALID_LENGTH_{n}:0x{id:02X}");
        return;
    }
    if f.raw_payload.len()<3+n {
        f.identifier=Some(id);
        f.payload.clear();
        Arc::make_mut(&mut f.network).lin_checksum=None;
        f.frame_key=format!("LIN:DECODE_ISSUE:TRUNCATED:0x{id:02X}");
        return;
    }

    f.identifier=Some(id);
    f.payload=f.raw_payload[2..2+n].to_vec();
    Arc::make_mut(&mut f.network).lin_checksum=Some(f.raw_payload[2+n]);

    // Parity/checksum-error frames remain visible for diagnosis but are isolated
    // from the normal LIN:0xNN stream so corrupted/error traffic is not mixed into
    // ordinary signal analysis. ID 0x00 with a valid length remains a normal frame.
    if message_type==0x03 {
        if let Some(key)=lin_rx_data_error_key(flags,Some(id)) {
            f.frame_key=key;
            return;
        }
    }
    f.frame_key=format!("LIN:0x{id:02X}");
}

fn parse_eth(f:&mut NormalizedFrame){
    let p=&f.raw_payload;
    if p.len()<14{return}
    f.bus_type=BusType::Ethernet;
    Arc::make_mut(&mut f.network).eth_dst=Some(mac(&p[0..6]));
    Arc::make_mut(&mut f.network).eth_src=Some(mac(&p[6..12]));
    let mut o=12;
    let mut et=be16(&p[o..o+2]);o+=2;
    while [0x8100,0x88A8,0x9100].contains(&et){
        if p.len()<o+4{return}
        Arc::make_mut(&mut f.network).vlan_ids.push(be16(&p[o..o+2])&0x0FFF);
        et=be16(&p[o+2..o+4]);o+=4
    }
    Arc::make_mut(&mut f.network).ethertype=Some(et);
    f.identifier=Some(et as u32);
    f.payload=p[o..].to_vec();
    f.frame_key=format!("ETH:0x{et:04X}");

    if et==0x0800&&p.len()>=o+20&&(p[o]>>4)==4{
        let ihl=((p[o]&0x0F) as usize)*4;
        let ip_end=(o+be16(&p[o+2..o+4]) as usize).min(p.len());
        if ihl>=20 && ip_end>=o+ihl {
            let src=format!("{}.{}.{}.{}",p[o+12],p[o+13],p[o+14],p[o+15]);
            let dst=format!("{}.{}.{}.{}",p[o+16],p[o+17],p[o+18],p[o+19]);
            Arc::make_mut(&mut f.network).ip_src=Some(src.clone());
            Arc::make_mut(&mut f.network).ip_dst=Some(dst.clone());
            let proto=p[o+9];
            let l4=o+ihl;
            if proto==17&&ip_end>=l4+8{
                let sp=be16(&p[l4..l4+2]); let dp=be16(&p[l4+2..l4+4]);
                Arc::make_mut(&mut f.network).transport=Some("UDP".into());Arc::make_mut(&mut f.network).src_port=Some(sp);Arc::make_mut(&mut f.network).dst_port=Some(dp);
                // Respect the UDP length. TECMP Ethernet records can retain a trailing
                // Ethernet FCS; those bytes are not part of the UDP application payload.
                let udp_len=be16(&p[l4+4..l4+6]) as usize;
                if udp_len>=8 && ip_end>=l4+udp_len {
                    f.payload=p[l4+8..l4+udp_len].to_vec();
                } else {
                    f.payload=p[l4+8..ip_end].to_vec();
                }
                f.frame_key=format!("ETH:UDP:{src}:{sp}>{dst}:{dp}");
                detect_someip(f);
            }else if proto==6&&ip_end>=l4+20{
                let sp=be16(&p[l4..l4+2]); let dp=be16(&p[l4+2..l4+4]);
                Arc::make_mut(&mut f.network).transport=Some("TCP".into());Arc::make_mut(&mut f.network).src_port=Some(sp);Arc::make_mut(&mut f.network).dst_port=Some(dp);
                let h=((p[l4+12]>>4) as usize)*4;
                if h>=20 && ip_end>=l4+h { f.payload=p[l4+h..ip_end].to_vec(); }
                f.frame_key=format!("ETH:TCP:{src}:{sp}>{dst}:{dp}");
                detect_someip(f);
            } else {
                f.frame_key=format!("ETH:IPv4:{src}>{dst}:P{proto}");
            }
        }
    } else if et==0x86DD {
        f.frame_key="ETH:IPv6".into();
    } else if et==0x0806 {
        f.frame_key="ETH:ARP".into();
    }
}

// SOME/IP base header is 16 bytes. This is intentionally conservative to avoid
// labeling arbitrary UDP/TCP payload as SOME/IP.
fn detect_someip(f:&mut NormalizedFrame) {
    let p=&f.payload;
    if p.len()<16 { return; }
    let length=be32(&p[4..8]) as usize;
    let proto_ver=p[12];
    let msg_type=p[14];
    let known_type=matches!(msg_type,0x00|0x01|0x02|0x20|0x21|0x22|0x80|0x81|0xA0|0xA1);
    if proto_ver!=1 || !known_type || length<8 || length+8>p.len() { return; }
    let message_id=be32(&p[0..4]);
    f.identifier=Some(message_id);
    f.frame_key=format!("SOMEIP:0x{message_id:08X}");
}


/// Automatically detect a repeated long-header container in an Ethernet application payload.
///
/// Detection is intentionally conservative: [BE32 id][BE32 payload_len][payload] must repeat
/// at least twice and consume the complete application payload. This avoids turning arbitrary
/// Ethernet data into child PDUs. Detected children are deliberately labelled REVIEW so the
/// user can validate them before treating the IDs as known vehicle PDUs.
pub fn detect_container_review(parent:&NormalizedFrame)->Vec<NormalizedFrame> {
    if parent.bus_type!=BusType::Ethernet || parent.network.transport.as_deref()!=Some("UDP") { return vec![]; }
    let p=&parent.payload;
    if p.len()<18 { return vec![]; } // at least two 8-byte headers + data
    let mut o=0usize;
    let mut records:Vec<(u32,usize,usize)>=Vec::new();
    while o+8<=p.len() {
        let id=be32(&p[o..o+4]);
        let len=be32(&p[o+4..o+8]) as usize;
        // Zero IDs/lengths and absurd lengths are poor evidence for this container form.
        if id==0 || len==0 || len>65535 || o+8+len>p.len() { return vec![]; }
        records.push((id,o+8,len));
        o+=8+len;
        if records.len()>4096 { return vec![]; }
    }
    // Tolerate up to 4 trailing bytes (a retained Ethernet FCS / padding); anything more
    // means the payload is not fully described by the container records.
    if p.len()-o>4 || records.len()<2 { return vec![]; }

    // Repeated IDs in a packet are allowed, but a wildly large identifier is unlikely to be
    // the vehicle PDU/container format we are trying to discover. Keep 29-bit compatibility.
    if records.iter().any(|(id,_,_)| *id>0x1FFF_FFFF) { return vec![]; }

    records.into_iter().enumerate().map(|(i,(id,start,len))| {
        let mut network=(*parent.network).clone();
        network.transport=Some("CONTAINER-REVIEW".into());
        NormalizedFrame{
            sequence:parent.sequence.saturating_mul(4096).saturating_add(i as u64+1),
            timestamp_ns:parent.timestamp_ns,
            relative_time_s:parent.relative_time_s,
            channel_id:parent.channel_id,
            bus_type:BusType::Ethernet,
            // Include the Ethernet/UDP parent identity in the internal key. This prevents
            // identical PDU IDs from different Ethernet streams being merged by indexing,
            // classification, waveform caching or signal discovery.
            frame_key:{
                let src=parent.network.ip_src.as_deref().unwrap_or("?");
                let dst=parent.network.ip_dst.as_deref().unwrap_or("?");
                let sp=parent.network.src_port.map(|v|v.to_string()).unwrap_or_else(||"?".into());
                let dp=parent.network.dst_port.map(|v|v.to_string()).unwrap_or_else(||"?".into());
                format!("REVIEW:PDU:0x{id:X}@UDP:{src}:{sp}->{dst}:{dp}")
            },
            identifier:Some(id),
            direction:parent.direction,
            payload:p[start..start+len].to_vec(),
            raw_payload:Vec::new(),
            tecmp:parent.tecmp.clone(),
            analog:None,
            network:Arc::new(network),
        }
    }).collect()
}

pub fn parse_ethernet_packet(raw:&[u8],ts:u64,seq:u64,channel_id:u32)->Option<NormalizedFrame>{
    if raw.len()<14{return None}
    let mut f=NormalizedFrame{sequence:seq,timestamp_ns:ts,relative_time_s:0.0,channel_id,bus_type:BusType::Ethernet,frame_key:"ETH".into(),identifier:None,direction:Direction::Unknown,payload:raw.to_vec(),raw_payload:raw.to_vec(),tecmp:None,analog:None,network:crate::model::empty_network()};
    parse_eth(&mut f);
    Some(f)
}

/// Parse a LINKTYPE_CAN_SOCKETCAN (227) packet.
/// The pcap link-type specification stores the CAN ID/flags word in network (big-endian)
/// order. Some tools write host (little-endian) order, so when the big-endian word is
/// implausible but the little-endian word is a clean standard/extended ID, use that.
pub fn parse_socketcan(raw:&[u8],ts:u64,seq:u64)->Option<NormalizedFrame>{
    if raw.len()<8{return None}
    let word:[u8;4]=raw[0..4].try_into().ok()?;
    let be=u32::from_be_bytes(word);
    let le=u32::from_le_bytes(word);
    let plausible=|w:u32|{
        // A standard-ID frame must not have bits set in the 29-bit ID area above bit 10.
        let eff=w&0x8000_0000!=0;
        eff || (w&0x1FFF_F800)==0
    };
    let idraw=if plausible(be){be}else if plausible(le){le}else{be};
    let n=raw[4] as usize;
    let fd_flags=raw.get(5).copied().unwrap_or(0);
    if n>64||raw.len()<8+n{return None}
    let eff=idraw&0x8000_0000!=0;
    let rtr=idraw&0x4000_0000!=0;
    let err=idraw&0x2000_0000!=0;
    let id=idraw&if eff{0x1FFF_FFFF}else if err{0x1FFF_FFFF}else{0x7FF};
    let is_fd=n>8 || raw.len()>=72 || fd_flags&0x04!=0;
    let key=if err {
        format!("CAN:ERROR:0x{id:X}")
    } else if rtr {
        format!("CAN:RTR:{}",can_frame_key(id,eff))
    } else {
        can_frame_key(id,eff)
    };
    Some(NormalizedFrame{
        sequence:seq,timestamp_ns:ts,relative_time_s:0.0,channel_id:0,
        bus_type:if is_fd{BusType::CanFd}else{BusType::Can},
        frame_key:key,identifier:Some(id),direction:Direction::Unknown,
        payload:if rtr{Vec::new()}else{raw[8..8+n].to_vec()},raw_payload:raw.to_vec(),
        tecmp:None,analog:None,network:crate::model::empty_network()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn analog_record(flags:u16,data_type:u16)->TecmpRecord {
        TecmpRecord{
            timestamp_ns:1,channel_id:5,payload:vec![0x00,0x0A,0xFF,0xF6],
            meta:crate::model::TecmpMeta{cm_id:1,counter:1,version:1,message_type:0x03,
                message_type_name:crate::tecmp::message_type_name(0x03).into(),
                data_type,data_flags:flags,timestamp_flags:0,device_flags:0,source_timestamp_ns:None,capture_timestamp_ns:None,timeline_projected:false},
        }
    }

    #[test]
    fn analog_sample_time_table_covers_all_defined_codes() {
        let expect=[(1u16,2_500_000_000u64),(2,1_000_000_000),(3,500_000_000),(4,250_000_000),(5,100_000_000),(6,50_000_000),
            (7,25_000_000),(8,10_000_000),(9,5_000_000),(0xA,2_500_000),(0xB,1_000_000),(0xC,500_000),(0xD,250_000),(0xE,100_000),(0xF,50_000)];
        for (code,ns) in expect {
            let f=normalize_tecmp(&analog_record(code<<11,DT_ANALOG),1);
            assert_eq!(f.analog.as_ref().unwrap().sample_period_ns,ns,"code {code:#x}");
        }
        let f=normalize_tecmp(&analog_record(0,DT_ANALOG),1);
        assert_eq!(f.analog.unwrap().sample_period_ns,0); // reserved
    }

    #[test]
    fn analog_factor_unit_and_samples() {
        let flags=(0xCu16<<11)|(2<<7)|(1<<2); // 0.5 ms, factor 0.001, unit A
        let f=normalize_tecmp(&analog_record(flags,DT_ANALOG),1);
        assert_eq!(f.bus_type,BusType::Analog);
        let a=f.analog.unwrap();
        assert_eq!(a.sample_count,2); assert_eq!(a.unit,"A"); assert!((a.factor-0.001).abs()<1e-12);
    }

    #[test]
    fn analog_alt_is_explicit_tecmp_record_not_unknown_or_misdecoded_analog() {
        let f=normalize_tecmp(&analog_record(0,DT_ANALOG_ALT),1);
        assert_eq!(f.bus_type,BusType::Tecmp);
        assert_eq!(f.frame_key,"TECMP:ANALOG_ALT:CH5");
        assert!(f.analog.is_none());
    }

    #[test]
    fn raw_ethernet_ipv4_udp_is_preserved() {
        let mut p = vec![0u8; 14 + 20 + 8 + 4];
        p[0..6].copy_from_slice(&[1,2,3,4,5,6]);
        p[6..12].copy_from_slice(&[6,5,4,3,2,1]);
        p[12..14].copy_from_slice(&0x0800u16.to_be_bytes());
        let ip=14; p[ip]=0x45; p[ip+9]=17;p[ip+2..ip+4].copy_from_slice(&32u16.to_be_bytes());
        p[ip+12..ip+16].copy_from_slice(&[10,0,0,1]);
        p[ip+16..ip+20].copy_from_slice(&[10,0,0,2]);
        let udp=34;p[udp+4..udp+6].copy_from_slice(&12u16.to_be_bytes());
        p[udp..udp+2].copy_from_slice(&30490u16.to_be_bytes());
        p[udp+2..udp+4].copy_from_slice(&30500u16.to_be_bytes());
        p[42..46].copy_from_slice(&[0xAA,0xBB,0xCC,0xDD]);
        let f=parse_ethernet_packet(&p,123,1,7).unwrap();
        assert_eq!(f.bus_type,BusType::Ethernet);
        assert_eq!(f.channel_id,7);
        assert_eq!(f.network.transport.as_deref(),Some("UDP"));
        assert_eq!(f.payload,vec![0xAA,0xBB,0xCC,0xDD]);
        assert!(f.frame_key.starts_with("ETH:UDP:"));
    }

    #[test]
    fn tcp_application_payload_excludes_ethernet_crc_and_padding(){
        for data in [Vec::new(),vec![0xAA,0xBB,0xCC]] {
            let mut p=vec![0u8;14+20+20];p[12..14].copy_from_slice(&0x0800u16.to_be_bytes());
            p[14]=0x45;p[23]=6;p[16..18].copy_from_slice(&((40+data.len()) as u16).to_be_bytes());
            p[34..36].copy_from_slice(&1339u16.to_be_bytes());p[36..38].copy_from_slice(&42220u16.to_be_bytes());p[46]=0x50;
            p.extend(&data);p.extend([0xDE,0xAD,0xBE,0xEF]);p.extend([0,0,0,0]);
            let f=parse_ethernet_packet(&p,1,0,1).unwrap();
            assert_eq!(f.network.transport.as_deref(),Some("TCP"));assert_eq!(f.payload,data);
            assert_eq!(f.raw_payload,p);
        }
    }

    fn lin_record(payload:Vec<u8>,flags:u16,message_type:u8)->TecmpRecord {
        TecmpRecord{
            timestamp_ns:1,
            channel_id:2,
            payload,
            meta:crate::model::TecmpMeta{
                cm_id:1,counter:1,version:1,message_type,
                message_type_name:crate::tecmp::message_type_name(message_type).into(),
                data_type:DT_LIN,data_flags:flags,timestamp_flags:0,device_flags:0,source_timestamp_ns:None,capture_timestamp_ns:None,timeline_projected:false,
            },
        }
    }

    #[test]
    fn lin_id_zero_with_valid_length_is_a_normal_frame() {
        let f=normalize_tecmp(&lin_record(vec![0x00,0x02,0x11,0x22,0xA5],0,0x03),1);
        assert_eq!(f.identifier,Some(0));
        assert_eq!(f.frame_key,"LIN:0x00");
        assert_eq!(f.payload,vec![0x11,0x22]);
        assert_eq!(f.network.lin_checksum,Some(0xA5));
    }

    #[test]
    fn lin_wakeup_zero_data_is_event_not_raw_or_id_zero() {
        let f=normalize_tecmp(&lin_record(vec![0x00,0x00,0x00],1<<8,0x03),1);
        assert_eq!(f.identifier,None);
        assert_eq!(f.frame_key,"LIN:EVENT:WAKE_UP");
        assert!(f.payload.is_empty());
    }

    #[test]
    fn lin_no_slave_zero_data_is_event() {
        let f=normalize_tecmp(&lin_record(vec![0x00,0x00,0x00],1<<2,0x03),1);
        assert_eq!(f.identifier,None);
        assert_eq!(f.frame_key,"LIN:EVENT:NO_SLAVE_RESPONSE");
        assert!(f.payload.is_empty());
    }

    #[test]
    fn lin_parity_error_with_zeroed_data_is_error_not_id_zero() {
        let f=normalize_tecmp(&lin_record(vec![0x00,0x00,0x00],1<<1,0x03),1);
        assert_eq!(f.identifier,None);
        assert_eq!(f.frame_key,"LIN:ERROR:PARITY");
        assert!(f.payload.is_empty());
    }

    #[test]
    fn lin_checksum_error_isolated_from_normal_id_stream() {
        let f=normalize_tecmp(&lin_record(vec![0x00,0x01,0x55,0xAA],1<<13,0x03),1);
        assert_eq!(f.identifier,Some(0));
        assert_eq!(f.frame_key,"LIN:ERROR:CHECKSUM:0x00");
        assert_eq!(f.payload,vec![0x55]);
    }

    #[test]
    fn lin_zero_length_without_condition_is_explicit_decode_issue() {
        let f=normalize_tecmp(&lin_record(vec![0x00,0x00,0x00],0,0x03),1);
        assert_eq!(f.identifier,Some(0));
        assert_eq!(f.frame_key,"LIN:DECODE_ISSUE:INVALID_LENGTH_0:0x00");
        assert!(f.payload.is_empty());
    }

    #[test]
    fn lin_replay_zero_length_is_remove_slave_response_operation() {
        let f=normalize_tecmp(&lin_record(vec![0x00,0x00,0x00],0,0x0A),1);
        assert_eq!(f.direction,Direction::Tx);
        assert_eq!(f.identifier,Some(0));
        assert_eq!(f.frame_key,"LIN:TX:REMOVE_SLAVE_RESPONSE:0x00");
        assert!(f.payload.is_empty());
    }

    #[test]
    fn lin_intermediate_length_five_is_a_normal_frame() {
        let f=normalize_tecmp(&lin_record(vec![0x00,0x05,0x06,0x02,0x0A,0x01,0x0F,0x5D],0,0x03),1);
        assert_eq!(f.identifier,Some(0));
        assert_eq!(f.frame_key,"LIN:0x00");
        assert_eq!(f.payload,vec![0x06,0x02,0x0A,0x01,0x0F]);
        assert_eq!(f.network.lin_checksum,Some(0x5D));
    }

    #[test]
    fn tecmp_control_message_is_system_data_not_unknown_bus_data() {
        let r=TecmpRecord{
            timestamp_ns:1, channel_id:0x000F_FF03, payload:vec![0x00,0x00,0x00,0x02],
            meta:crate::model::TecmpMeta{
                cm_id:1,counter:1,version:1,message_type:0x00,
                message_type_name:crate::tecmp::message_type_name(0x00).into(),
                data_type:0,data_flags:0,timestamp_flags:0,device_flags:0,source_timestamp_ns:None,capture_timestamp_ns:None,timeline_projected:false,
            },
        };
        let f=normalize_tecmp(&r,1);
        assert_eq!(f.bus_type,BusType::Tecmp);
        assert_eq!(f.identifier,Some(0x0002));
        assert_eq!(f.frame_key,"TECMP:CONTROL:0x0002");
    }

    #[test]
    fn detects_sample_long_header_container_as_review_pdus() {
        let h="000001500000000f1d641100100000000734e02600124000000152000000043e040000000001800000000a7414b3009400ff000000000004c700000040d1a49250903d40a925dc3f6c35c835a0000000000000110000000600030040010000000000088990c03b2435b004000000dc0000000000060080a10000000018d9793d27";
        let payload=(0..h.len()/2).map(|i|u8::from_str_radix(&h[i*2..i*2+2],16).unwrap()).collect::<Vec<_>>();
        let mut n=NetworkMeta::default(); n.transport=Some("UDP".into()); n.src_port=Some(20000); n.dst_port=Some(20000);
        let f=NormalizedFrame{sequence:1,timestamp_ns:1,relative_time_s:0.0,channel_id:7,bus_type:BusType::Ethernet,frame_key:"ETH:UDP:test".into(),identifier:None,direction:Direction::Rx,payload,raw_payload:vec![],tecmp:Some(Arc::new(analog_record(0,DT_ETH).meta)),analog:None,network:Arc::new(n)};
        let c=detect_container_review(&f);
        assert_eq!(c.len(),4);
        assert!(c.iter().all(|child|Arc::ptr_eq(child.tecmp.as_ref().unwrap(),f.tecmp.as_ref().unwrap())));
        assert_eq!(c.iter().map(|x|x.identifier.unwrap()).collect::<Vec<_>>(),vec![0x150,0x152,0x180,0x4C7]);
        assert_eq!(c.iter().map(|x|x.payload.len()).collect::<Vec<_>>(),vec![15,4,10,64]);
        assert!(c.iter().all(|x|x.frame_key.starts_with("REVIEW:PDU:")));
        assert!(c.iter().all(|x|x.frame_key.contains("@UDP:")));
    }


    fn can_record(raw:Vec<u8>,fd:bool)->TecmpRecord {
        TecmpRecord{
            timestamp_ns:1,channel_id:3,payload:raw,
            meta:crate::model::TecmpMeta{cm_id:1,counter:1,version:1,message_type:0x03,
                message_type_name:crate::tecmp::message_type_name(0x03).into(),
                data_type:if fd{crate::tecmp::DT_CANFD}else{crate::tecmp::DT_CAN},data_flags:0,timestamp_flags:0,device_flags:0,source_timestamp_ns:None,capture_timestamp_ns:None,timeline_projected:false},
        }
    }

    #[test]
    fn tecmp_can_extended_and_standard_ids_have_distinct_keys() {
        let std=normalize_tecmp(&can_record(vec![0,0,1,0x23,2,0xAA,0xBB],false),1);
        let ext=normalize_tecmp(&can_record(vec![0x80,0,1,0x23,2,0xAA,0xBB],false),2);
        assert_eq!(std.identifier,Some(0x123));
        assert_eq!(ext.identifier,Some(0x123));
        assert_eq!(std.frame_key,"0x123");
        assert_eq!(ext.frame_key,"EXT:0x123");
        assert_eq!(ext.payload,vec![0xAA,0xBB]);
    }

    #[test]
    fn tecmp_can_bad_length_is_isolated_and_has_no_header_payload() {
        let f=normalize_tecmp(&can_record(vec![0,0,1,0x23,9,1,2,3,4,5,6,7,8,9],false),1);
        assert!(f.frame_key.starts_with("CAN:DECODE_ISSUE:BAD_LENGTH_9"));
        assert!(f.payload.is_empty());
        let t=normalize_tecmp(&can_record(vec![0,0,1,0x23,4,1,2],false),2);
        assert!(t.frame_key.starts_with("CAN:DECODE_ISSUE:BAD_LENGTH_4"));
        let short=normalize_tecmp(&can_record(vec![0,0,1],false),3);
        assert!(short.frame_key.contains("TRUNCATED"));
        assert!(short.payload.is_empty());
    }

    fn socketcan_pkt(word:[u8;4],n:u8,data:&[u8])->Vec<u8> {
        let mut v=vec![0u8;16]; v[0..4].copy_from_slice(&word); v[4]=n; v[8..8+data.len()].copy_from_slice(data); v
    }

    #[test]
    fn socketcan_big_endian_standard_id_is_decoded() {
        let f=parse_socketcan(&socketcan_pkt([0,0,1,0x23],2,&[1,2]),10,1).unwrap();
        assert_eq!(f.identifier,Some(0x123)); assert_eq!(f.frame_key,"0x123"); assert_eq!(f.bus_type,BusType::Can);
    }

    #[test]
    fn socketcan_little_endian_writers_are_still_decoded() {
        let std=parse_socketcan(&socketcan_pkt([0x23,0x01,0,0],2,&[1,2]),10,1).unwrap();
        assert_eq!(std.identifier,Some(0x123));
        let ext=parse_socketcan(&socketcan_pkt([0x23,0x01,0,0x80],1,&[7]),10,2).unwrap();
        assert_eq!(ext.identifier,Some(0x123)); assert_eq!(ext.frame_key,"EXT:0x123");
    }

    #[test]
    fn socketcan_error_and_rtr_frames_are_not_normal_data_frames() {
        let err=parse_socketcan(&socketcan_pkt([0x20,0,0,0x01],8,&[0;8]),10,1).unwrap();
        assert!(err.frame_key.starts_with("CAN:ERROR:"));
        let rtr=parse_socketcan(&socketcan_pkt([0x40,0,0x01,0x23],0,&[]),10,2).unwrap();
        assert_eq!(rtr.frame_key,"CAN:RTR:0x123");
        assert!(rtr.payload.is_empty());
    }

}
