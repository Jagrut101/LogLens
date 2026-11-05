use crate::model::TecmpMeta;

pub const TECMP_ETHERTYPES:[u16;2]=[0x2090,0x99FE];
pub const DT_CAN:u16=0x0002; pub const DT_CANFD:u16=0x0003; pub const DT_LIN:u16=0x0004; pub const DT_FLEXRAY:u16=0x0008; pub const DT_GPIO:u16=0x000A; pub const DT_UART_ASCII:u16=0x0010; pub const DT_UART_SLA:u16=0x0012; pub const DT_ANALOG:u16=0x0020; pub const DT_ANALOG_SLA:u16=0x0021; pub const DT_ANALOG_ALT:u16=0x0028; pub const DT_ETH:u16=0x0080;
#[derive(Clone,Debug)] pub struct TecmpRecord { pub timestamp_ns:u64, pub channel_id:u32, pub payload:Vec<u8>, pub meta:TecmpMeta }
fn be16(x:&[u8])->u16{u16::from_be_bytes([x[0],x[1]])} fn be32(x:&[u8])->u32{u32::from_be_bytes([x[0],x[1],x[2],x[3]])} fn be64(x:&[u8])->u64{u64::from_be_bytes([x[0],x[1],x[2],x[3],x[4],x[5],x[6],x[7]])}
pub fn message_type_name(v:u8)->&'static str { match v {0x00=>"Control Message",0x01=>"Status Device",0x02=>"Status Bus/IVN",0x03=>"Logging Stream",0x04=>"Status Configuration",0x0A=>"Replay Data",_=>"Unknown"} }

pub fn parse_ethernet_tecmp(raw:&[u8])->Vec<TecmpRecord>{
    if raw.len()<26{return vec![]} let mut et_off=12usize; let mut et=be16(&raw[et_off..et_off+2]);
    while [0x8100,0x88A8,0x9100].contains(&et) { et_off+=4; if raw.len()<et_off+2{return vec![]} et=be16(&raw[et_off..et_off+2]); }
    if !TECMP_ETHERTYPES.contains(&et){return vec![]}
    let h1=et_off+2; if raw.len()<h1+12{return vec![]}
    let cm_id=be16(&raw[h1..h1+2]); let counter=be16(&raw[h1+2..h1+4]); let version=raw[h1+4]; let mt=raw[h1+5]; let dt=be16(&raw[h1+6..h1+8]);
    let mut p=h1+12; let mut out=Vec::new();
    while p+16<=raw.len(){ let ch=be32(&raw[p..p+4]); let tsraw=be64(&raw[p+4..p+12]); let len=be16(&raw[p+12..p+14]) as usize; let flags=be16(&raw[p+14..p+16]); let s=p+16; let e=s+len; if e>raw.len(){break}
        if len==0{p=e;continue}
        out.push(TecmpRecord{timestamp_ns:tsraw&((1u64<<62)-1),channel_id:ch,payload:raw[s..e].to_vec(),meta:TecmpMeta{cm_id,counter,version,message_type:mt,message_type_name:message_type_name(mt).to_string(),data_type:dt,data_flags:flags,timestamp_flags:(tsraw>>62) as u8,device_flags:be16(&raw[h1+10..h1+12]),source_timestamp_ns:Some(tsraw&((1u64<<62)-1)),capture_timestamp_ns:None,timeline_projected:false}}); p=e; }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(ch:u32,payload:&[u8])->Vec<u8>{
        let mut v=Vec::new(); v.extend(ch.to_be_bytes()); v.extend(0u64.to_be_bytes());
        v.extend((payload.len() as u16).to_be_bytes()); v.extend(0u16.to_be_bytes()); v.extend(payload); v
    }
    fn packet(entries:&[Vec<u8>])->Vec<u8>{
        let mut v=vec![0u8;12]; v.extend(0x99FEu16.to_be_bytes());
        v.extend([0,1,0,1,1,0x03,0,0x02,0,0,0,0]); // cm_id,counter,version,msg type 3,data type CAN
        for e in entries{v.extend(e)} v
    }
    #[test]
    fn zero_length_entry_does_not_drop_following_entries(){
        let p=packet(&[entry(1,&[0,0,1,0,1,0xAA]),entry(2,&[]),entry(3,&[0,0,2,0,1,0xBB])]);
        let recs=parse_ethernet_tecmp(&p);
        assert_eq!(recs.iter().map(|r|r.channel_id).collect::<Vec<_>>(),vec![1,3]);
    }
}
