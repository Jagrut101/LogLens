use crate::{model::CapturePacket, resources};
use anyhow::{bail, Context, Result};
use std::{fs::File, io::{BufReader, Read}, path::Path};

fn u16e(b:&[u8],le:bool)->u16 { if le {u16::from_le_bytes([b[0],b[1]])} else {u16::from_be_bytes([b[0],b[1]])} }
fn u32e(b:&[u8],le:bool)->u32 { if le {u32::from_le_bytes([b[0],b[1],b[2],b[3]])} else {u32::from_be_bytes([b[0],b[1],b[2],b[3]])} }

const MAX_CAPTURE_PACKET_BYTES: usize = 16 * 1024 * 1024;
const MAX_PCAPNG_BLOCK_BYTES: usize = MAX_CAPTURE_PACKET_BYTES + 1024 * 1024;

pub fn read_capture(path:&Path, max_packets:usize)->Result<Vec<CapturePacket>> {
    let mut out=Vec::new();
    out.try_reserve(max_packets.min(65_536)).map_err(|_|anyhow::anyhow!("Not enough memory to start capture decoding"))?;
    for_each_capture_packet(path,max_packets,|p|{out.push(p);Ok(true)})?;
    Ok(out)
}

/// Stream capture packets through a callback. Returning false stops the scan cleanly.
/// This is the low-memory path used by large-file channel discovery and normalization.
pub fn for_each_capture_packet<F>(path:&Path,max_packets:usize,mut on_packet:F)->Result<usize>
where F:FnMut(CapturePacket)->Result<bool> {
    use std::io::{Seek, SeekFrom};
    let file=File::open(path).with_context(||format!("open {}",path.display()))?;
    let mut r=BufReader::new(file);
    let mut magic=[0u8;4];
    r.read_exact(&mut magic).context("read capture magic")?;
    r.seek(SeekFrom::Start(0))?;
    if magic==[0x0A,0x0D,0x0D,0x0A] { visit_pcapng_stream(&mut r,max_packets,&mut on_packet) }
    else { visit_pcap_stream(&mut r,max_packets,&mut on_packet) }
}

fn read_exact_or_eof<R:Read>(r:&mut R, buf:&mut [u8])->Result<bool> {
    use std::io::ErrorKind;
    let mut off=0usize;
    while off<buf.len() {
        match r.read(&mut buf[off..]) {
            Ok(0) if off==0 => return Ok(false),
            Ok(0) => bail!("Capture ended in the middle of a record"),
            Ok(n) => off+=n,
            Err(e) if e.kind()==ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Ok(true)
}

fn checked_packet_len(n:usize)->Result<usize> {
    if n>MAX_CAPTURE_PACKET_BYTES {
        bail!("Capture record declares an unsafe packet size of {:.1} MiB (limit {:.1} MiB). The file may be corrupt.", n as f64/1048576.0, MAX_CAPTURE_PACKET_BYTES as f64/1048576.0);
    }
    Ok(n)
}

fn read_payload<R:Read>(r:&mut R, len:usize)->Result<Vec<u8>> {
    let len=checked_packet_len(len)?;
    let mut v=Vec::new();
    v.try_reserve_exact(len).map_err(|_|anyhow::anyhow!("Not enough memory to allocate a {:.1} MiB capture packet",len as f64/1048576.0))?;
    v.resize(len,0);
    r.read_exact(&mut v)?;
    Ok(v)
}

fn visit_pcap_stream<R:Read,F:FnMut(CapturePacket)->Result<bool>>(r:&mut R,max_packets:usize,on_packet:&mut F)->Result<usize> {
    let mut gh=[0u8;24];
    r.read_exact(&mut gh).context("PCAP global header is truncated")?;
    let (le,nano)=match &gh[0..4] {
        [0xd4,0xc3,0xb2,0xa1] => (true,false), [0xa1,0xb2,0xc3,0xd4] => (false,false),
        [0x4d,0x3c,0xb2,0xa1] => (true,true), [0xa1,0xb2,0x3c,0x4d] => (false,true),
        _ => bail!("Unsupported capture magic; expected PCAP or PCAPNG"),
    };
    let linktype=u32e(&gh[20..24],le);
    let mut count=0usize;
    while count<max_packets {
        let mut h=[0u8;16];
        if !read_exact_or_eof(r,&mut h)? { break; }
        let sec=u32e(&h[0..4],le) as u64;
        let frac=u32e(&h[4..8],le) as u64;
        let incl=checked_packet_len(u32e(&h[8..12],le) as usize)?;
        let data=read_payload(r,incl)?;
        let ns=sec.saturating_mul(1_000_000_000)+if nano{frac}else{frac.saturating_mul(1_000)};
        count+=1;
        if !on_packet(CapturePacket{timestamp_ns:ns,linktype,interface_id:0,data})? { break; }
    }
    Ok(count)
}

/// Convert capture ticks to nanoseconds. Integer arithmetic is used whenever the tick
/// rate divides (or is a multiple of) 1 GHz: an f64 cannot represent a 1.7e18 ns epoch
/// exactly and would introduce errors of a few hundred nanoseconds.
pub(crate) fn ticks_to_ns(ticks:u64,units_per_sec:f64)->u64 {
    if units_per_sec>0.0 && units_per_sec.fract()==0.0 && units_per_sec<=u64::MAX as f64 {
        let u=units_per_sec as u128;
        if u>0 {
            return ((ticks as u128).saturating_mul(1_000_000_000)/u).min(u64::MAX as u128) as u64;
        }
    }
    ((ticks as f64/units_per_sec)*1e9) as u64
}

#[derive(Clone,Copy)] struct Iface { linktype:u32, ts_units_per_sec:f64, ts_offset_secs:i64 }
fn visit_pcapng_stream<R:Read+std::io::Seek,F:FnMut(CapturePacket)->Result<bool>>(r:&mut R,max_packets:usize,on_packet:&mut F)->Result<usize> {
    use std::io::{Seek,SeekFrom};
    let mut le=true;
    let mut ifaces:Vec<Iface>=Vec::new();
    let mut count=0usize;
    while count<max_packets {
        let mut header=[0u8;8];
        if !read_exact_or_eof(r,&mut header)? { break; }
        let is_shb=header[0..4]==[0x0A,0x0D,0x0D,0x0A];
        if is_shb {
            let mut bom=[0u8;4]; r.read_exact(&mut bom)?;
            le=match bom {[0x4d,0x3c,0x2b,0x1a]=>true,[0x1a,0x2b,0x3c,0x4d]=>false,_=>bail!("Invalid PCAPNG byte-order magic")};
            let len=u32e(&header[4..8],le) as usize;
            if len<28 { bail!("Invalid PCAPNG section header length {len}"); }
            r.seek(SeekFrom::Current((len-12) as i64))?;
            ifaces.clear();
            continue;
        }
        let block_type=u32e(&header[0..4],le);
        let len=u32e(&header[4..8],le) as usize;
        if len<12 { bail!("Invalid PCAPNG block length {len}"); }
        if block_type!=1 && block_type!=3 && block_type!=6 {
            r.seek(SeekFrom::Current((len-8) as i64))?;
            continue;
        }
        if len>MAX_PCAPNG_BLOCK_BYTES {
            bail!("PCAPNG block is {:.1} MiB, above the safe {:.1} MiB limit. The file may be corrupt.",len as f64/1048576.0,MAX_PCAPNG_BLOCK_BYTES as f64/1048576.0);
        }
        let body_len=len-8;
        let mut body=Vec::new();
        body.try_reserve_exact(body_len).map_err(|_|anyhow::anyhow!("Not enough memory to decode a {:.1} MiB PCAPNG block",body_len as f64/1048576.0))?;
        body.resize(body_len,0);
        r.read_exact(&mut body)?;
        if body.len()<4 { bail!("Truncated PCAPNG block"); }
        let trailing=u32e(&body[body.len()-4..],le) as usize;
        if trailing!=len { bail!("PCAPNG block length mismatch ({len} != {trailing})"); }
        let data_end=body.len()-4;
        match block_type {
            1 => {
                if data_end>=8 {
                    let linktype=u16e(&body[0..2],le) as u32;
                    let mut resol=1_000_000f64; let mut ts_offset_secs=0i64;
                    let mut o=8usize;
                    while o+4<=data_end {
                        let code=u16e(&body[o..o+2],le); let olen=u16e(&body[o+2..o+4],le) as usize; o+=4;
                        if o+olen>data_end { break; }
                        if code==9&&olen>=1 { let v=body[o]; resol=if v&0x80!=0 {2f64.powi((v&0x7f) as i32)} else {10f64.powi(v as i32)}; }
                        if code==14 && olen==8 {
                            let b:[u8;8]=body[o..o+8].try_into().unwrap();
                            ts_offset_secs=if le{i64::from_le_bytes(b)}else{i64::from_be_bytes(b)};
                        }
                        o=o.saturating_add((olen+3)&!3);
                    }
                    ifaces.push(Iface{linktype,ts_units_per_sec:resol,ts_offset_secs});
                }
            }
            6 => {
                if data_end>=20 {
                    let iid=u32e(&body[0..4],le) as usize;
                    let hi=u32e(&body[4..8],le) as u64; let lo=u32e(&body[8..12],le) as u64;
                    let cap=checked_packet_len(u32e(&body[12..16],le) as usize)?;
                    if 20usize.checked_add(cap).is_some_and(|z|z<=data_end) {
                        let iface=ifaces.get(iid).copied().unwrap_or(Iface{linktype:1,ts_units_per_sec:1_000_000.0,ts_offset_secs:0});
                        let ticks=(hi<<32)|lo; let ns=(ticks_to_ns(ticks,iface.ts_units_per_sec) as i128 + iface.ts_offset_secs as i128*1_000_000_000).clamp(0,u64::MAX as i128) as u64;
                        let mut data=Vec::new(); data.try_reserve_exact(cap).map_err(|_|anyhow::anyhow!("Not enough memory to retain capture packet"))?;
                        data.extend_from_slice(&body[20..20+cap]);
                        count+=1;
                        if !on_packet(CapturePacket{timestamp_ns:ns,linktype:iface.linktype,interface_id:iid as u32,data})? { break; }
                    }
                }
            }
            3 => {
                if data_end>=4 {
                    let iface=ifaces.first().copied().unwrap_or(Iface{linktype:1,ts_units_per_sec:1_000_000.0,ts_offset_secs:0});
                    let data=&body[4..data_end]; checked_packet_len(data.len())?;
                    let mut packet=Vec::new(); packet.try_reserve_exact(data.len()).map_err(|_|anyhow::anyhow!("Not enough memory to retain capture packet"))?; packet.extend_from_slice(data);
                    count+=1;
                    if !on_packet(CapturePacket{timestamp_ns:0,linktype:iface.linktype,interface_id:0,data:packet})? { break; }
                }
            }
            _=>{}
        }
    }
    Ok(count)
}

pub fn write_pcapng_segment(source:&Path,dest:&Path,start_rel_s:f64,end_rel_s:f64,max_packets:usize)->Result<usize>{
    write_pcapng_segment_with_timeline(source,dest,start_rel_s,end_rel_s,max_packets,None,false)
}

pub fn write_pcapng_segment_with_timeline(source:&Path,dest:&Path,start_rel_s:f64,end_rel_s:f64,max_packets:usize,origin_ns:Option<u64>,source_time:bool)->Result<usize>{
    use std::io::Write;
    let effective_max=resources::limits().effective_packet_limit(max_packets);
    let packets=read_capture(source,effective_max)?;
    if packets.is_empty(){ bail!("Capture contains no packets"); }
    let first_ts=origin_ns.unwrap_or_else(||packets.iter().find(|p|p.timestamp_ns>0).map(|p|p.timestamp_ns).unwrap_or(0));
    let a=start_rel_s.min(end_rel_s).max(0.0);
    let b=start_rel_s.max(end_rel_s).max(a);
    let start_ns=first_ts.saturating_add((a*1e9) as u64);
    let end_ns=first_ts.saturating_add((b*1e9) as u64);
    let mut selected=packets.into_iter().filter(|p|{
        if source_time && p.linktype==1 {
            let records=crate::tecmp::parse_ethernet_tecmp(&p.data);
            if !records.is_empty(){return records.iter().any(|r|{
                let ns=if r.timestamp_ns==0{p.timestamp_ns}else{r.timestamp_ns};
                ns>=start_ns&&ns<=end_ns
            });}
        }
        p.timestamp_ns==0 || (p.timestamp_ns>=start_ns&&p.timestamp_ns<=end_ns)
    }).collect::<Vec<_>>();
    if selected.is_empty(){
        if effective_max<max_packets { bail!("No packets found between cursors A and B within the resource-safe scan limit of {effective_max} packets. Use a smaller capture/range or close other applications and retry."); }
        bail!("No packets found between cursors A and B");
    }
    let mut out=File::create(dest).with_context(||format!("create {}",dest.display()))?;
    fn block<W:Write>(w:&mut W,typ:u32,body:&[u8])->std::io::Result<()>{
        let pad=(4-body.len()%4)%4; let total=(12+body.len()+pad) as u32;
        w.write_all(&typ.to_le_bytes())?;w.write_all(&total.to_le_bytes())?;w.write_all(body)?;
        if pad>0{w.write_all(&[0u8;3][..pad])?;} w.write_all(&total.to_le_bytes())
    }
    let mut shb=Vec::new();shb.extend(0x1A2B3C4Du32.to_le_bytes());shb.extend(1u16.to_le_bytes());shb.extend(0u16.to_le_bytes());shb.extend(u64::MAX.to_le_bytes());
    block(&mut out,0x0A0D0D0A,&shb)?;
    let mut links=selected.iter().map(|p|p.linktype).collect::<Vec<_>>();links.sort_unstable();links.dedup();
    let mut iface=std::collections::HashMap::new();
    for (i,lt) in links.iter().enumerate(){iface.insert(*lt,i as u32);let mut idb=Vec::new();idb.extend((*lt as u16).to_le_bytes());idb.extend(0u16.to_le_bytes());idb.extend(65535u32.to_le_bytes());idb.extend(9u16.to_le_bytes());idb.extend(1u16.to_le_bytes());idb.push(9);idb.extend([0,0,0]);idb.extend(0u16.to_le_bytes());idb.extend(0u16.to_le_bytes());block(&mut out,1,&idb)?;}
    let count=selected.len();
    for p in selected.drain(..){
        let iid=*iface.get(&p.linktype).unwrap_or(&0);let ts=p.timestamp_ns;let mut epb=Vec::new();
        epb.extend(iid.to_le_bytes());epb.extend(((ts>>32)as u32).to_le_bytes());epb.extend((ts as u32).to_le_bytes());
        epb.extend((p.data.len()as u32).to_le_bytes());epb.extend((p.data.len()as u32).to_le_bytes());epb.extend(&p.data);
        block(&mut out,6,&epb)?;
    }
    Ok(count)
}

#[cfg(test)] mod tests { use super::*;
    #[test] fn ns_ticks_are_exact_at_modern_epoch(){
        let ns=1_760_000_000_123_456_789u64;
        assert_eq!(ticks_to_ns(ns,1e9),ns);
        assert_eq!(ticks_to_ns(1_760_000_000_123_456,1e6),1_760_000_000_123_456_000);
        assert_eq!(ticks_to_ns(2,2f64.powi(1)),1_000_000_000);
    }
    #[test] fn endian_helpers(){assert_eq!(u16e(&[0x12,0x34],false),0x1234); assert_eq!(u32e(&[1,0,0,0],true),1);} }
