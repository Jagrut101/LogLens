use crate::{analysis::endian_is_little, context::SignalDefinition, model::{NormalizedFrame, channel_id_hex, channel_text_hex, frame_key_display, parse_channel_id}};
use anyhow::Result;
use std::{fs::File, path::Path};

pub fn frames_csv(path:&Path, frames:&[&NormalizedFrame])->Result<()> {
    let mut w=csv::Writer::from_writer(File::create(path)?);
    w.write_record(["Sequence","Time_s","Bus","Channel","Direction","FrameID","FrameKey","Length","Payload"])?;
    for f in frames { w.write_record([f.sequence.to_string(),format!("{:.9}",f.relative_time_s),f.bus_type.to_string(),channel_id_hex(f.channel_id),f.direction.to_string(),f.id_text(),frame_key_display(&f.frame_key),f.payload.len().to_string(),f.payload_hex()])?; }
    w.flush()?; Ok(())
}

pub fn signals_csv(path:&Path, signals:&[SignalDefinition])->Result<()> {
    let mut w=csv::Writer::from_writer(File::create(path)?);
    w.write_record(["Protocol","SignalName","Channel","FrameID","StartByte","StartBit","BitLength","Endian","DataType","Signed","Scale","Offset","Unit"])?;
    for s in signals { w.write_record([s.protocol.clone(),s.name.clone(),channel_text_hex(&s.channel),s.frame_id.clone(),s.start_byte.to_string(),s.start_bit.to_string(),s.bit_length.to_string(),s.endian.clone(),s.data_type.clone(),s.signed.to_string(),s.scale.to_string(),s.offset.to_string(),s.unit.clone()])?; }
    w.flush()?; Ok(())
}


fn tecmp_type(f:&NormalizedFrame)->String { f.tecmp.as_ref().map(|m|format!("0x{:04X}",m.data_type)).unwrap_or_default() }
const TECMP_HEADERS:[&str;6]=["TECMP_SourceTimestamp_ns","CaptureTimestamp_ns","TECMP_DeviceFlags","TECMP_DataFlags","TECMP_TimestampFlags","CAN_CRC"];
fn tecmp_columns(f:&NormalizedFrame)->[String;6]{
    let m=f.tecmp.as_ref();
    [m.and_then(|m|m.source_timestamp_ns).map(|v|v.to_string()).unwrap_or_default(),
     m.and_then(|m|m.capture_timestamp_ns).map(|v|v.to_string()).unwrap_or_default(),
     m.map(|m|format!("0x{:04X}",m.device_flags)).unwrap_or_default(),
     m.map(|m|format!("0x{:04X}",m.data_flags)).unwrap_or_default(),
     m.map(|m|m.timestamp_flags.to_string()).unwrap_or_default(),
     f.network.can_crc.map(|v|format!("0x{v:X}")).unwrap_or_default()]
}
fn write_tecmp_columns(ws:&mut rust_xlsxwriter::Worksheet,row:u32,f:&NormalizedFrame)->Result<()>{
    for (i,value) in tecmp_columns(f).into_iter().enumerate(){ws.write_string(row,12+i as u16,value)?;}
    Ok(())
}
fn raw_hex(f:&NormalizedFrame)->String { f.raw_payload.iter().map(|b|format!("{b:02X}")).collect::<Vec<_>>().join(" ") }

pub fn frames_csv_detailed(path:&Path, frames:&[&NormalizedFrame])->Result<()> {
    let mut w=csv::Writer::from_writer(File::create(path)?);
    w.write_record(["Sequence","Timestamp_ns","RelativeTime_s","Protocol","Channel","Direction","FrameID","FrameKey","DLC","TECMP_DataType","Payload","RawPayload"].into_iter().chain(TECMP_HEADERS))?;
    for f in frames {
        w.write_record([
            f.sequence.to_string(), f.timestamp_ns.to_string(), format!("{:.9}",f.relative_time_s),
            f.bus_type.to_string(), channel_id_hex(f.channel_id), f.direction.to_string(), f.id_text(),
            frame_key_display(&f.frame_key), f.payload.len().to_string(), tecmp_type(f), f.payload_hex(), raw_hex(f)
        ].into_iter().chain(tecmp_columns(f)))?;
    }
    w.flush()?; Ok(())
}

pub fn frames_xlsx(path:&Path, frames:&[&NormalizedFrame])->Result<()> {
    use rust_xlsxwriter::Workbook;
    let mut wb=Workbook::new();

    {
        let ws=wb.add_worksheet(); ws.set_name("Summary")?;
        ws.write_string(0,0,"LogLens Export")?;
        ws.write_string(1,0,"Records")?; ws.write_number(1,1,frames.len() as f64)?;
        if let Some(f)=frames.first(){ws.write_string(2,0,"Start Relative Time (s)")?;ws.write_number(2,1,f.relative_time_s)?;}
        if let Some(f)=frames.last(){ws.write_string(3,0,"End Relative Time (s)")?;ws.write_number(3,1,f.relative_time_s)?;}
        let mut counts=std::collections::BTreeMap::<String,usize>::new();
        for f in frames {*counts.entry(f.bus_type.to_string()).or_default()+=1;}
        ws.write_string(5,0,"Protocol")?;ws.write_string(5,1,"Count")?;
        for (i,(k,v)) in counts.into_iter().enumerate(){ws.write_string((i+6) as u32,0,&k)?;ws.write_number((i+6) as u32,1,v as f64)?;}
    }

    fn fill(ws:&mut rust_xlsxwriter::Worksheet, rows:&[&NormalizedFrame])->Result<()> {
        let headers=["Sequence","Timestamp_ns","RelativeTime_s","Protocol","Channel","Direction","FrameID","FrameKey","DLC","TECMP_DataType","Payload","RawPayload"];
        for (c,h) in headers.iter().chain(TECMP_HEADERS.iter()).enumerate(){ws.write_string(0,c as u16,*h)?;}
        for (r,f) in rows.iter().enumerate(){
            let rr=(r+1) as u32;
            ws.write_number(rr,0,f.sequence as f64)?; ws.write_string(rr,1,f.timestamp_ns.to_string())?;
            ws.write_number(rr,2,f.relative_time_s)?; ws.write_string(rr,3,f.bus_type.to_string())?;
            ws.write_string(rr,4,channel_id_hex(f.channel_id))?; ws.write_string(rr,5,f.direction.to_string())?;
            ws.write_string(rr,6,f.id_text())?; ws.write_string(rr,7,frame_key_display(&f.frame_key))?;
            ws.write_number(rr,8,f.payload.len() as f64)?; ws.write_string(rr,9,tecmp_type(f))?;
            ws.write_string(rr,10,f.payload_hex())?; ws.write_string(rr,11,raw_hex(f))?;write_tecmp_columns(ws,rr,f)?;
        } Ok(())
    }

    { let ws=wb.add_worksheet(); ws.set_name("Frames")?; fill(ws,frames)?; }
    for (name,bus) in [("CAN",crate::model::BusType::Can),("CAN-FD",crate::model::BusType::CanFd),("LIN",crate::model::BusType::Lin),("Ethernet",crate::model::BusType::Ethernet),("Analog",crate::model::BusType::Analog),("TECMP",crate::model::BusType::Tecmp),("Unknown",crate::model::BusType::Unknown)] {
        let rows=frames.iter().copied().filter(|f|f.bus_type==bus).collect::<Vec<_>>();
        if !rows.is_empty(){let ws=wb.add_worksheet();ws.set_name(name)?;fill(ws,&rows)?;}
    }
    wb.save(path)?; Ok(())
}


#[derive(Clone, Debug)]
pub struct MappedSignalRow {
    pub sequence:u64,
    pub timestamp_ns:u64,
    pub relative_time_s:f64,
    pub protocol:String,
    pub channel:u32,
    pub frame_id:String,
    pub frame_key:String,
    pub signal:String,
    pub raw_value:f64,
    pub physical_value:f64,
    pub unit:String,
}

fn protocol_token(v:&str)->String {
    let t=v.trim().to_ascii_lowercase().chars().filter(|c|!matches!(*c,' '|'_'|'-')).collect::<String>();
    match t.as_str(){
        "canfd"=>"canfd".into(),
        "automotiveethernet"|"ethernet"|"eth"=>"eth".into(),
        "can"=>"can".into(),
        "lin"=>"lin".into(),
        "analog"=>"analog".into(),
        "tecmp"=>"tecmp".into(),
        "unknown"|"none"=>"unknown".into(),
        _=>t,
    }
}
fn signal_matches_frame(f:&NormalizedFrame,s:&SignalDefinition)->bool {
    let configured_protocol=s.protocol.trim();
    if !configured_protocol.is_empty() && configured_protocol!="*" && !configured_protocol.eq_ignore_ascii_case("all")
        && protocol_token(configured_protocol)!=protocol_token(&f.bus_type.to_string()) { return false; }
    let channel=parse_channel_id(&s.channel);
    let id=s.frame_id.trim().to_ascii_lowercase();
    let fid=f.id_text().to_ascii_lowercase();
    let fkey=f.frame_key.to_ascii_lowercase();
    let id_match=id==fid || id.trim_start_matches("0x")==fid.trim_start_matches("0x")
        || fkey==id || fkey.ends_with(&format!(":{}",id)) || fkey.contains(&format!(":{}:",id));
    id_match && channel.map(|c|c==f.channel_id).unwrap_or(true)
}

pub fn map_selected_signals(frames:&[&NormalizedFrame],signals:&[SignalDefinition])->Vec<MappedSignalRow>{
    use crate::analysis::decode_signal_typed;
    let mut out=Vec::new();
    for f in frames {
        for sig in signals {
            if !signal_matches_frame(f,sig){continue;}
            let little=endian_is_little(&sig.endian);
            // Raw engineering extraction before factor/offset, then configured physical value.
            let raw=decode_signal_typed(&f.payload,sig.start_byte,sig.start_bit,sig.bit_length,little,sig.signed,&sig.data_type,1.0,0.0);
            let physical=decode_signal_typed(&f.payload,sig.start_byte,sig.start_bit,sig.bit_length,little,sig.signed,&sig.data_type,sig.scale,sig.offset);
            if let (Some(raw_value),Some(physical_value))=(raw,physical){
                out.push(MappedSignalRow{
                    sequence:f.sequence,timestamp_ns:f.timestamp_ns,relative_time_s:f.relative_time_s,
                    protocol:f.bus_type.to_string(),channel:f.channel_id,frame_id:f.id_text(),frame_key:f.frame_key.clone(),
                    signal:sig.name.clone(),raw_value,physical_value,unit:sig.unit.clone()
                });
            }
        }
    }
    out
}

pub fn selected_mapped_csv(path:&Path,frames:&[&NormalizedFrame],signals:&[SignalDefinition])->Result<(usize,usize)>{
    // Export every selected normalized frame, including protocols that have no configured signal.
    // If one or more configured signals map to a frame, emit one row per mapped signal;
    // otherwise emit a frame-only row with blank signal/value fields.
    let mapped=map_selected_signals(frames,signals);
    let mut by_sequence=std::collections::BTreeMap::<u64,Vec<&MappedSignalRow>>::new();
    for r in &mapped { by_sequence.entry(r.sequence).or_default().push(r); }
    let mut w=csv::Writer::from_writer(File::create(path)?);
    w.write_record(["Sequence","Timestamp_ns","RelativeTime_s","Protocol","Channel","Direction","FrameID","FrameKey","DLC","TECMP_DataType","Payload","RawPayload","Signal","RawValue","PhysicalValue","Unit"].into_iter().chain(TECMP_HEADERS))?;
    for f in frames {
        if let Some(rows)=by_sequence.get(&f.sequence) {
            for r in rows {
                w.write_record([f.sequence.to_string(),f.timestamp_ns.to_string(),format!("{:.9}",f.relative_time_s),f.bus_type.to_string(),
                    channel_id_hex(f.channel_id),f.direction.to_string(),f.id_text(),frame_key_display(&f.frame_key),f.payload.len().to_string(),tecmp_type(f),
                    f.payload_hex(),raw_hex(f),r.signal.clone(),format!("{}",r.raw_value),format!("{}",r.physical_value),r.unit.clone()].into_iter().chain(tecmp_columns(f)))?;
            }
        } else {
            w.write_record([f.sequence.to_string(),f.timestamp_ns.to_string(),format!("{:.9}",f.relative_time_s),f.bus_type.to_string(),
                channel_id_hex(f.channel_id),f.direction.to_string(),f.id_text(),frame_key_display(&f.frame_key),f.payload.len().to_string(),tecmp_type(f),
                f.payload_hex(),raw_hex(f),String::new(),String::new(),String::new(),String::new()].into_iter().chain(tecmp_columns(f)))?;
        }
    }
    w.flush()?; Ok((frames.len(),mapped.len()))
}

pub fn selected_mapped_xlsx(path:&Path,frames:&[&NormalizedFrame],signals:&[SignalDefinition])->Result<(usize,usize)>{
    use rust_xlsxwriter::Workbook;
    let mapped=map_selected_signals(frames,signals);
    let mut wb=Workbook::new();
    {
        let ws=wb.add_worksheet();ws.set_name("Summary")?;
        ws.write_string(0,0,"LogLens Selected Export")?;
        ws.write_string(1,0,"Selected frames")?;ws.write_number(1,1,frames.len() as f64)?;
        ws.write_string(2,0,"Mapped signal samples")?;ws.write_number(2,1,mapped.len() as f64)?;
        ws.write_string(3,0,"Configured signals")?;ws.write_number(3,1,signals.len() as f64)?;
        let mut counts=std::collections::BTreeMap::<String,usize>::new();
        for f in frames {*counts.entry(f.bus_type.to_string()).or_default()+=1;}
        ws.write_string(5,0,"Protocol")?;ws.write_string(5,1,"Frame count")?;
        for (i,(protocol,count)) in counts.into_iter().enumerate(){
            ws.write_string((i+6) as u32,0,&protocol)?;ws.write_number((i+6) as u32,1,count as f64)?;
        }
    }
    {
        let ws=wb.add_worksheet();ws.set_name("Selected Frames")?;
        let h=["Sequence","Timestamp_ns","RelativeTime_s","Protocol","Channel","Direction","FrameID","FrameKey","DLC","TECMP_DataType","Payload","RawPayload"];
        for(c,v)in TECMP_HEADERS.iter().enumerate(){ws.write_string(0,12+c as u16,*v)?;}
        for(c,v)in h.iter().enumerate(){ws.write_string(0,c as u16,*v)?;}
        for(r,f)in frames.iter().enumerate(){let rr=(r+1)as u32;
            ws.write_number(rr,0,f.sequence as f64)?;ws.write_string(rr,1,f.timestamp_ns.to_string())?;ws.write_number(rr,2,f.relative_time_s)?;
            ws.write_string(rr,3,f.bus_type.to_string())?;ws.write_string(rr,4,channel_id_hex(f.channel_id))?;ws.write_string(rr,5,f.direction.to_string())?;
            ws.write_string(rr,6,f.id_text())?;ws.write_string(rr,7,frame_key_display(&f.frame_key))?;ws.write_number(rr,8,f.payload.len() as f64)?;
            ws.write_string(rr,9,tecmp_type(f))?;ws.write_string(rr,10,f.payload_hex())?;ws.write_string(rr,11,raw_hex(f))?;write_tecmp_columns(ws,rr,f)?;
        }
    }
    // Protocol-specific frame sheets. These are created dynamically only when the
    // selected range contains that protocol. UNKNOWN is intentionally preserved.
    for (name,bus) in [
        ("CAN",crate::model::BusType::Can),
        ("CAN-FD",crate::model::BusType::CanFd),
        ("LIN",crate::model::BusType::Lin),
        ("Ethernet",crate::model::BusType::Ethernet),
        ("Analog",crate::model::BusType::Analog),
        ("TECMP",crate::model::BusType::Tecmp),
        ("Unknown",crate::model::BusType::Unknown)
    ] {
        let protocol_rows=frames.iter().copied().filter(|f|f.bus_type==bus).collect::<Vec<_>>();
        if protocol_rows.is_empty(){continue;}
        let ws=wb.add_worksheet();ws.set_name(name)?;
        let h=["Sequence","Timestamp_ns","RelativeTime_s","Protocol","Channel","Direction","FrameID","FrameKey","DLC","TECMP_DataType","Payload","RawPayload"];
        for(c,v)in TECMP_HEADERS.iter().enumerate(){ws.write_string(0,12+c as u16,*v)?;}
        for(c,v)in h.iter().enumerate(){ws.write_string(0,c as u16,*v)?;}
        for(r,f)in protocol_rows.iter().enumerate(){let rr=(r+1)as u32;
            ws.write_number(rr,0,f.sequence as f64)?;ws.write_string(rr,1,f.timestamp_ns.to_string())?;ws.write_number(rr,2,f.relative_time_s)?;
            ws.write_string(rr,3,f.bus_type.to_string())?;ws.write_string(rr,4,channel_id_hex(f.channel_id))?;ws.write_string(rr,5,f.direction.to_string())?;
            ws.write_string(rr,6,f.id_text())?;ws.write_string(rr,7,frame_key_display(&f.frame_key))?;ws.write_number(rr,8,f.payload.len() as f64)?;
            ws.write_string(rr,9,tecmp_type(f))?;ws.write_string(rr,10,f.payload_hex())?;ws.write_string(rr,11,raw_hex(f))?;write_tecmp_columns(ws,rr,f)?;
        }
    }
    {
        let ws=wb.add_worksheet();ws.set_name("Mapped Signals")?;
        let h=["Sequence","Timestamp_ns","RelativeTime_s","Protocol","Channel","FrameID","FrameKey","Signal","RawValue","PhysicalValue","Unit"];
        for(c,v)in h.iter().enumerate(){ws.write_string(0,c as u16,*v)?;}
        for(r,v)in mapped.iter().enumerate(){let rr=(r+1)as u32;
            ws.write_number(rr,0,v.sequence as f64)?;ws.write_string(rr,1,v.timestamp_ns.to_string())?;ws.write_number(rr,2,v.relative_time_s)?;
            ws.write_string(rr,3,&v.protocol)?;ws.write_string(rr,4,channel_id_hex(v.channel))?;ws.write_string(rr,5,&v.frame_id)?;
            ws.write_string(rr,6,frame_key_display(&v.frame_key))?;ws.write_string(rr,7,&v.signal)?;ws.write_number(rr,8,v.raw_value)?;
            ws.write_number(rr,9,v.physical_value)?;ws.write_string(rr,10,&v.unit)?;
        }
    }
    {
        let ws=wb.add_worksheet();ws.set_name("Signal Configuration")?;
        let h=["Protocol","Signal","Channel","FrameID","StartByte","StartBit","BitLength","Endian","DataType","Signed","Factor","Offset","Unit"];
        for(c,v)in h.iter().enumerate(){ws.write_string(0,c as u16,*v)?;}
        for(r,v)in signals.iter().enumerate(){let rr=(r+1)as u32;
            ws.write_string(rr,0,&v.protocol)?;ws.write_string(rr,1,&v.name)?;ws.write_string(rr,2,channel_text_hex(&v.channel))?;ws.write_string(rr,3,&v.frame_id)?;
            ws.write_number(rr,4,v.start_byte as f64)?;ws.write_number(rr,5,v.start_bit as f64)?;ws.write_number(rr,6,v.bit_length as f64)?;
            ws.write_string(rr,7,&v.endian)?;ws.write_string(rr,8,&v.data_type)?;ws.write_boolean(rr,9,v.signed)?;ws.write_number(rr,10,v.scale)?;ws.write_number(rr,11,v.offset)?;ws.write_string(rr,12,&v.unit)?;
        }
    }
    wb.save(path)?;Ok((frames.len(),mapped.len()))
}


#[cfg(test)]
mod channel_hex_tests {
    use super::*;
    use calamine::{open_workbook_auto,Reader};
    #[test]
    fn hex_channels_survive_exports_reimport_and_signal_mapping() {
        crate::session::with_test_storage(|dir| {
            let input=dir.join("channels.csv");
            std::fs::write(&input,"time,channel,id,payload\n0,32785,123,01\n1,36881,123,02\n").unwrap();
            let data=crate::engine::load(&input,100).unwrap();
            let frames=data.frames.iter().collect::<Vec<_>>();
            let signal=SignalDefinition{protocol:"CAN".into(),name:"Test".into(),channel:"0x8011".into(),frame_id:"0x123".into(),bit_length:8,scale:1.0,..Default::default()};
            let mapped=map_selected_signals(&frames,&[signal.clone()]);
            assert_eq!(mapped.len(),1,"hexadecimal channel must not map to other channels");
            assert_eq!(mapped[0].channel,32785);
            for variant in 0..3 {
                let path=dir.join(format!("export{variant}.csv"));
                match variant {
                    0=>frames_csv(&path,&frames).unwrap(),
                    1=>frames_csv_detailed(&path,&frames).unwrap(),
                    _=>{selected_mapped_csv(&path,&frames,&[signal.clone()]).unwrap();}
                }
                let mut reader=csv::Reader::from_path(&path).unwrap();
                let column=reader.headers().unwrap().iter().position(|h|h=="Channel").unwrap();
                let values=reader.records().map(|r|r.unwrap()[column].to_string()).collect::<Vec<_>>();
                assert_eq!(values,vec!["0x8011","0x9011"]);
                let reloaded=crate::engine::load(&path,100).unwrap();
                assert_eq!(reloaded.frames.iter().map(|f|f.channel_id).collect::<Vec<_>>(),vec![32785,36881]);
            }
            for selected in [false,true] {
                let path=dir.join(format!("export{selected}.xlsx"));
                if selected {selected_mapped_xlsx(&path,&frames,&[signal.clone()]).unwrap();}
                else {frames_xlsx(&path,&frames).unwrap();}
                let mut workbook=open_workbook_auto(&path).unwrap();
                let range=workbook.worksheet_range(if selected{"Selected Frames"}else{"Frames"}).unwrap();
                assert_eq!(range.get_value((1,4)).unwrap().to_string(),"0x8011");
                assert_eq!(range.get_value((2,4)).unwrap().to_string(),"0x9011");
            }
        });
    }
}
