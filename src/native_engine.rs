use crate::model::{BusType, CapturePacket, NormalizedFrame};
use anyhow::{bail, Result};
use std::{ffi::{CStr, CString}, os::raw::{c_char, c_int}, path::Path, slice};

#[repr(C)]
#[derive(Clone, Copy)]
struct LLPacket { timestamp_ns:u64, linktype:u32, interface_id:u32, data_offset:u32, data_length:u32 }
#[repr(C)]
struct LLCapture { packets:*mut LLPacket, data:*mut u8, packet_count:usize, data_size:usize }
#[repr(C)]
#[derive(Clone, Copy)]
struct LLFrameKey {
    timestamp_ns:u64, channel_id:u32, identifier:u32, bus_type:u8, payload_length:u8, payload:[u8;64]
}
#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
pub struct NativeStats { pub total_frames:u64, pub changed_frames:u64, pub static_frames:u64, pub unique_keys:u64 }

unsafe extern "C" {
    fn ll_engine_version()->*const c_char;
    fn ll_read_capture(path:*const c_char,max_packets:usize,out:*mut LLCapture)->c_int;
    fn ll_free_capture(capture:*mut LLCapture);
    fn ll_analyze_frames(frames:*const LLFrameKey,count:usize,out:*mut NativeStats)->c_int;
    fn ll_downsample_minmax(t:*const f64,v:*const f64,n:usize,target:usize,ot:*mut f64,ov:*mut f64,cap:usize)->usize;
}

pub fn version()->String {
    unsafe {
        let p=ll_engine_version();
        if p.is_null() {"C++ Native Engine".into()} else {CStr::from_ptr(p).to_string_lossy().into_owned()}
    }
}
pub fn read_capture(path:&Path,max_packets:usize)->Result<Vec<CapturePacket>> {
    let cpath=CString::new(path.to_string_lossy().as_bytes())?;
    let mut c=LLCapture{packets:std::ptr::null_mut(),data:std::ptr::null_mut(),packet_count:0,data_size:0};
    let rc=unsafe{ll_read_capture(cpath.as_ptr(),max_packets,&mut c)};
    if rc!=0 { bail!("C++ capture reader failed with status {rc}") }
    let result=(||{
        let packets=unsafe{slice::from_raw_parts(c.packets,c.packet_count)};
        let data=unsafe{slice::from_raw_parts(c.data,c.data_size)};
        let mut out=Vec::with_capacity(c.packet_count);
        for p in packets {
            let a=p.data_offset as usize; let z=a.saturating_add(p.data_length as usize);
            if z>data.len(){bail!("C++ capture packet points outside native data buffer")}
            out.push(CapturePacket{timestamp_ns:p.timestamp_ns,linktype:p.linktype,interface_id:p.interface_id,data:data[a..z].to_vec()});
        }
        Ok(out)
    })();
    unsafe{ll_free_capture(&mut c)}
    result
}
fn bus_code(b:BusType)->u8 { match b { BusType::Can=>1,BusType::CanFd=>2,BusType::Lin=>3,BusType::Ethernet=>4,BusType::Analog=>5,BusType::Tecmp=>6,BusType::Unknown=>0 } }
pub fn analyze_frames(frames:&[NormalizedFrame])->Result<NativeStats> {
    let mut compact=Vec::with_capacity(frames.len());
    for f in frames {
        let mut payload=[0u8;64]; let n=f.payload.len().min(64); payload[..n].copy_from_slice(&f.payload[..n]);
        compact.push(LLFrameKey{timestamp_ns:f.timestamp_ns,channel_id:f.channel_id,identifier:if matches!(f.bus_type,BusType::Can|BusType::CanFd){
            let mut id=f.identifier.unwrap_or(0);
            if f.frame_key.contains("EXT:"){id|=0x8000_0000;}
            if f.frame_key.contains(":RTR:"){id|=0x4000_0000;}
            if f.frame_key.contains(":ERROR:"){id|=0x2000_0000;}
            id
        }else{f.identifier.unwrap_or(u32::MAX)},bus_type:bus_code(f.bus_type),payload_length:n as u8,payload});
    }
    let mut out=NativeStats::default();
    let rc=unsafe{ll_analyze_frames(compact.as_ptr(),compact.len(),&mut out)};
    if rc!=0 {bail!("C++ frame analyzer failed with status {rc}")} else {Ok(out)}
}
pub fn downsample(points:&[[f64;2]],target:usize)->Vec<[f64;2]> {
    if points.len()<=target||target<4{return points.to_vec()}
    let t:Vec<f64>=points.iter().map(|p|p[0]).collect(); let v:Vec<f64>=points.iter().map(|p|p[1]).collect();
    let cap=target.saturating_add(4); let mut ot=vec![0.0;cap]; let mut ov=vec![0.0;cap];
    let n=unsafe{ll_downsample_minmax(t.as_ptr(),v.as_ptr(),t.len(),target,ot.as_mut_ptr(),ov.as_mut_ptr(),cap)}.min(cap);
    let mut out=(0..n).map(|i|[ot[i],ov[i]]).collect::<Vec<_>>();
    if out.first()!=points.first(){out.insert(0,points[0]);}
    if out.last()!=points.last(){out.push(*points.last().unwrap());}
    out
}
