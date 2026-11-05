use serde::{Deserialize, Serialize};
use std::{fmt, sync::{Arc,OnceLock}};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BusType { Can, CanFd, Lin, Ethernet, Analog, Tecmp, Unknown }
impl fmt::Display for BusType { fn fmt(&self, f:&mut fmt::Formatter<'_>)->fmt::Result { write!(f,"{}", match self { Self::Can=>"CAN",Self::CanFd=>"CAN-FD",Self::Lin=>"LIN",Self::Ethernet=>"ETH",Self::Analog=>"ANALOG",Self::Tecmp=>"TECMP",Self::Unknown=>"UNKNOWN" }) } }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction { Rx, Tx, Unknown }
impl fmt::Display for Direction { fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result { write!(f,"{}",match self{Self::Rx=>"RX",Self::Tx=>"TX",Self::Unknown=>"-"}) } }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TecmpMeta {
    pub cm_id:u16, pub counter:u16, pub version:u8, pub message_type:u8,
    pub message_type_name:String, pub data_type:u16, pub data_flags:u16,
    pub timestamp_flags:u8,
    #[serde(default)] pub device_flags:u16,
    #[serde(default)] pub source_timestamp_ns:Option<u64>,
    #[serde(default)] pub capture_timestamp_ns:Option<u64>,
    #[serde(default)] pub timeline_projected:bool,
}

impl TecmpMeta {
    pub fn device_flags_text(&self)->String {
        let mut flags=Vec::new();
        for (mask,name) in [(1,"end of segment"),(2,"start of segment"),(4,"spy"),(8,"multi-interface"),(0x8000,"device overflow")]{
            if self.device_flags&mask!=0{flags.push(name);}
        }
        if flags.is_empty(){"none".into()}else{flags.join(", ")}
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnalogMeta {
    pub sample_count:usize,
    pub sample_width_bytes:usize,
    pub sample_period_ns:u64,
    pub factor:f64,
    pub unit:String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
pub struct NetworkMeta {
    pub eth_src:Option<String>, pub eth_dst:Option<String>, pub ethertype:Option<u16>,
    pub vlan_ids:Vec<u16>, pub ip_src:Option<String>, pub ip_dst:Option<String>,
    pub transport:Option<String>, pub src_port:Option<u16>, pub dst_port:Option<u16>,
    pub lin_checksum:Option<u8>,
    #[serde(default)] pub can_crc:Option<u32>,
}

// Most vehicle-bus frames have no network fields. Keep one immutable empty
// context rather than storing or allocating it for every record.
pub fn empty_network()->Arc<NetworkMeta> {
    static EMPTY:OnceLock<Arc<NetworkMeta>>=OnceLock::new();
    Arc::clone(EMPTY.get_or_init(||Arc::new(NetworkMeta::default())))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NormalizedFrame {
    pub sequence:u64,
    pub timestamp_ns:u64,
    pub relative_time_s:f64,
    pub channel_id:u32,
    pub bus_type:BusType,
    pub frame_key:String,
    pub identifier:Option<u32>,
    pub direction:Direction,
    pub payload:Vec<u8>,
    pub raw_payload:Vec<u8>,
    pub tecmp:Option<Arc<TecmpMeta>>,
    pub analog:Option<AnalogMeta>,
    pub network:Arc<NetworkMeta>,
}
impl NormalizedFrame {
    pub fn payload_hex(&self)->String { self.payload.iter().map(|b|format!("{b:02X}")).collect::<Vec<_>>().join(" ") }
    pub fn id_text(&self)->String { self.identifier.map(|v|format!("0x{v:X}")).unwrap_or_else(||"-".into()) }
}

#[derive(Clone, Debug)]
pub struct CapturePacket { pub timestamp_ns:u64, pub linktype:u32, pub interface_id:u32, pub data:Vec<u8> }

// Channel IDs have one presentation format. Numeric IDs and persisted lookup keys
// remain unchanged so saved selections and aliases continue to resolve.
pub fn channel_id_hex(id:u32)->String { format!("0x{id:X}") }

pub fn parse_channel_id(s:&str)->Option<u32> {
    let t=s.trim();
    if t.is_empty(){return None;}
    if let Some(h)=t.strip_prefix("0x").or_else(||t.strip_prefix("0X")){return u32::from_str_radix(h,16).ok();}
    if t.chars().any(|c|c.is_ascii_alphabetic()) && t.chars().all(|c|c.is_ascii_hexdigit()) {return u32::from_str_radix(t,16).ok();}
    if let Ok(v)=t.parse::<u32>(){return Some(v);}
    t.parse::<f64>().ok().filter(|v|v.is_finite() && *v>=0.0 && *v<=u32::MAX as f64 && v.fract()==0.0).map(|v|v as u32)
}

pub fn channel_text_hex(raw:&str)->String {
    parse_channel_id(raw).map(channel_id_hex).unwrap_or_else(||raw.to_string())
}

pub fn channel_display(id:u32,name:Option<&str>,show_raw:bool)->String {
    match name.filter(|n|!n.trim().is_empty()) {
        Some(n) if show_raw=>format!("{} ({})",n,channel_id_hex(id)),
        Some(n)=>n.to_string(),
        None=>channel_id_hex(id),
    }
}

// Only protocol-generated channel tokens are converted; frame IDs, IPs, ports
// and stable internal frame keys are not reinterpreted.
pub fn frame_key_display(key:&str)->String {
    if key.contains(" / ") {
        return key.split(" / ").map(|part| {
            if let Some(id)=part.strip_prefix("CH").and_then(parse_channel_id) {
                format!("CH{}",channel_id_hex(id))
            }else{frame_key_display(part)}
        }).collect::<Vec<_>>().join(" / ");
    }
    if let Some(raw)=key.strip_prefix("ANALOG:") {
        if let Some(id)=parse_channel_id(raw){return format!("ANALOG:{}",channel_id_hex(id));}
    }
    if let Some((prefix,raw))=key.rsplit_once(":CH") {
        if let Some(id)=parse_channel_id(raw){return format!("{prefix}:CH{}",channel_id_hex(id));}
    }
    key.to_string()
}

#[cfg(test)]
mod channel_presentation_tests {
    use super::*;
    #[test]
    fn channel_ids_are_unambiguous_hex_including_embedded_tokens(){
        for id in [0,1,32785,0x000FFF03,u32::MAX] {
            let text=channel_id_hex(id);assert!(text.starts_with("0x"));assert!(!text[2..].starts_with('0') || id==0);assert_eq!(parse_channel_id(&text),Some(id));
        }
        assert_eq!(channel_display(32785,Some("Voltage"),true),"Voltage (0x8011)");
        assert_eq!(channel_display(32785,None,false),"0x8011");
        assert_eq!(channel_text_hex("32785"),"0x8011");
        assert_eq!(channel_text_hex("*"),"*");
        assert_eq!(frame_key_display("ANALOG:32785"),"ANALOG:0x8011");
        assert_eq!(frame_key_display("TECMP:STATUS_DEVICE:CH32785"),"TECMP:STATUS_DEVICE:CH0x8011");
        assert_eq!(frame_key_display("CAN:0x123"),"CAN:0x123");
        assert_eq!(frame_key_display("ETH:UDP:10.0.0.1:123>10.0.0.2:456"),"ETH:UDP:10.0.0.1:123>10.0.0.2:456");
    }
}
