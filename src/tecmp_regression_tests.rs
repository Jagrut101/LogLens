use crate::{
    capture, engine,
    model::{Direction, TecmpMeta},
    native_engine, protocols,
    tecmp::{self, TecmpRecord},
};
fn record(dt: u16, mt: u8, flags: u16, payload: Vec<u8>) -> TecmpRecord {
    TecmpRecord {
        timestamp_ns: 1_760_000_000_123_456_789,
        channel_id: 1,
        payload,
        meta: TecmpMeta {
            cm_id: 1,
            counter: 1,
            version: 3,
            message_type: mt,
            message_type_name: tecmp::message_type_name(mt).into(),
            data_type: dt,
            data_flags: flags,
            timestamp_flags: 0,
            device_flags: 7,
            source_timestamp_ns: None,
            capture_timestamp_ns: None, timeline_projected:false,
        },
    }
}
fn ethernet(entries: &[TecmpRecord]) -> Vec<u8> {
    let mut v = vec![0; 12];
    v.extend(0x99feu16.to_be_bytes());
    v.extend([0, 1, 0, 1, 3, 3, 0, 2, 0, 0, 0x80, 0x0f]);
    for r in entries {
        v.extend(r.channel_id.to_be_bytes());
        v.extend(r.timestamp_ns.to_be_bytes());
        v.extend((r.payload.len() as u16).to_be_bytes());
        v.extend(r.meta.data_flags.to_be_bytes());
        v.extend(&r.payload);
    }
    v
}
fn pcap(raw: &[u8], ns: u64) -> tempfile::NamedTempFile {
    let f = tempfile::Builder::new().suffix(".pcap").tempfile().unwrap();
    let mut p = Vec::new();
    p.extend([0x4d, 0x3c, 0xb2, 0xa1]);
    p.extend(2u16.to_le_bytes());
    p.extend(4u16.to_le_bytes());
    p.extend([0; 8]);
    p.extend(65535u32.to_le_bytes());
    p.extend(1u32.to_le_bytes());
    p.extend(((ns / 1_000_000_000) as u32).to_le_bytes());
    p.extend(((ns % 1_000_000_000) as u32).to_le_bytes());
    p.extend((raw.len() as u32).to_le_bytes());
    p.extend((raw.len() as u32).to_le_bytes());
    p.extend(raw);
    std::fs::write(f.path(), p).unwrap();
    f
}
#[test]
fn selected_bus_channels_keep_control_markers_but_not_other_status_channels() {
    let bus=record(2,3,0,vec![0,0,1,0x23,1,0xaa]);
    let input=pcap(&ethernet(&[bus.clone()]),bus.timestamp_ns);
    let mut bytes=std::fs::read(input.path()).unwrap();
    for mt in [1,0,0] {
        let mut r=record(0,mt,0,vec![0,1,0,7]);r.channel_id=99;
        r.timestamp_ns+=1_000_000_000+(mt as u64)*1_000_000;
        let mut raw=ethernet(&[r.clone()]);raw[19]=mt;raw[20..22].copy_from_slice(&0u16.to_be_bytes());
        let next=pcap(&raw,r.timestamp_ns);
        bytes.extend_from_slice(&std::fs::read(next.path()).unwrap()[24..]);
    }
    std::fs::write(input.path(),bytes).unwrap();
    let selected=std::collections::BTreeSet::from([1]);
    for source_time in [false,true] {
        let d=engine::load_selected_with_timeline(input.path(),10,Some(&selected),source_time).unwrap();
        assert_eq!(d.frames.len(),3);
        assert!(d.frames.iter().all(|f|f.channel_id==1 || f.tecmp.as_ref().unwrap().message_type==0));
        let markers=crate::events::control_triggers(&d.frames,0.0);
        assert_eq!(markers.len(),2);
        assert_eq!(markers[0].control_message_id,7);
        assert_eq!(markers[1].occurrence,2);
        assert!((markers[0].time_s-1.0).abs()<1e-9);
    }
    let empty=std::collections::BTreeSet::new();
    assert!(engine::load_selected_with_timeline(input.path(),10,Some(&empty),true).unwrap().frames.is_empty());
}

#[test]
fn unselected_load_budget_counts_decoded_records_not_unsupported_packets() {
    let wanted=record(2,3,0,vec![0,0,1,0x23,1,0xaa]);
    let input=pcap(&[0,0],wanted.timestamp_ns);
    let second=pcap(&ethernet(&[wanted]),1_760_000_001_123_456_789);
    let mut bytes=std::fs::read(input.path()).unwrap();
    bytes.extend_from_slice(&std::fs::read(second.path()).unwrap()[24..]);
    std::fs::write(input.path(),bytes).unwrap();
    let d=engine::load(input.path(),1).unwrap();
    assert_eq!(d.frames.len(),1);assert_eq!(d.frames[0].payload,vec![0xaa]);
}

#[test]
fn selected_channel_scan_reaches_packets_after_the_record_budget() {
    let wanted=record(2,3,0,vec![0,0,1,0x23,1,0xaa]);
    let mut skipped=wanted.clone(); skipped.channel_id=2;
    let first=pcap(&ethernet(&[skipped]),wanted.timestamp_ns);
    let second=pcap(&ethernet(&[wanted]),1_760_000_001_123_456_789);
    let mut bytes=std::fs::read(first.path()).unwrap();
    bytes.extend_from_slice(&std::fs::read(second.path()).unwrap()[24..]);
    std::fs::write(first.path(),bytes).unwrap();
    let channels=std::collections::BTreeSet::from([1]);
    let d=engine::load_selected_with_timeline(first.path(),1,Some(&channels),true).unwrap();
    assert_eq!(d.frames.len(),1);
    assert_eq!(d.frames[0].channel_id,1);
    assert_eq!(d.frames[0].payload,vec![0xaa]);
    let discovered=engine::discover_capture_channels(first.path(),1).unwrap();
    assert_eq!(discovered.iter().map(|(id,_)|*id).collect::<Vec<_>>(),vec![1,2]);
}

#[test]
fn bundled_source_clocks_raw_crc_and_capture_basis_survive_loading() {
    let a = record(2, 3, 0, vec![0, 0, 1, 0x23, 1, 0xaa, 0x12, 0x34]);
    let mut b = a.clone();
    b.timestamp_ns += 2_000_000;
    let outer = a.timestamp_ns + 10_000_000;
    let f = pcap(&ethernet(&[a.clone(), b.clone()]), outer);
    let source = engine::load(f.path(), 10).unwrap();
    assert_eq!(
        source
            .frames
            .iter()
            .map(|f| f.timestamp_ns)
            .collect::<Vec<_>>(),
        vec![a.timestamp_ns, b.timestamp_ns]
    );
    assert!((source.frames[1].relative_time_s - 0.002).abs() < 1e-12);
    for frame in &source.frames {
        let m = frame.tecmp.as_ref().unwrap();
        assert_eq!(m.capture_timestamp_ns, Some(outer));
        assert_eq!(m.source_timestamp_ns, Some(frame.timestamp_ns));
        assert_eq!(m.device_flags, 0x800f);
        assert_eq!(frame.raw_payload, a.payload);
        assert_eq!(frame.network.can_crc, Some(0x1234));
    }
    let capture = engine::load_selected_with_timeline(f.path(), 10, None, false).unwrap();
    assert_eq!(
        capture
            .frames
            .iter()
            .map(|f| f.timestamp_ns)
            .collect::<Vec<_>>(),
        vec![outer, outer]
    );
    assert!(!capture.source_time);
}
#[test]
fn error_remote_and_malformed_data_are_distinct() {
    for dt in [2, 3] {
        let f = protocols::normalize_tecmp(&record(dt, 3, 8, vec![0; 7]), 1);
        assert!(f.frame_key.contains(":ERROR:"));
        assert!(f.payload.is_empty());
        assert_eq!(f.identifier, None);
    }
    let r = protocols::normalize_tecmp(&record(2, 3, 2, vec![0, 0, 1, 0x23, 0, 0, 0]), 1);
    assert_eq!(r.frame_key, "CAN:RTR:0x123");
    assert!(r.payload.is_empty());
    let f = protocols::normalize_tecmp(
        &record(2, 3, 1 << 13, vec![0, 0, 1, 0x23, 1, 0xaa, 0, 0]),
        1,
    );
    assert!(f.frame_key.contains(":ERROR:"));
}
#[test]
fn all_canfd_lengths_follow_the_manual() {
    for n in 0..=65 {
        let mut p = vec![0, 0, 1, 0x23, n];
        p.extend(vec![0xaa; n as usize]);
        p.extend([0x01, 0x02, 0x03]);
        let f = protocols::normalize_tecmp(&record(3, 3, 0, p), 1);
        let valid = matches!(n, 0..=8 | 12 | 16 | 20 | 24 | 32 | 48 | 64);
        assert_eq!(f.frame_key.contains("DECODE_ISSUE"), !valid, "length {n}");
        if valid {
            assert_eq!(f.payload.len(), n as usize);
            assert_eq!(f.network.can_crc, Some(0x010203));
        }
    }
}
#[test]
fn replay_directions_ignore_receive_tx_flags() {
    for (dt, p) in [
        (2, vec![0, 0, 1, 0x23, 1, 0xaa]),
        (3, vec![0, 0, 1, 0x23, 1, 0xaa]),
        (4, vec![0, 1, 0xaa, 0]),
        (0x80, vec![0; 14]),
    ] {
        let f = protocols::normalize_tecmp(&record(dt, 10, 0, p), 1);
        assert_eq!(f.direction, Direction::Tx, "type {dt}");
    }
}
#[test]
fn native_stats_keep_can_id_and_frame_kind_identity() {
    let normal = protocols::normalize_tecmp(&record(2, 3, 0, vec![0, 0, 1, 0x23, 1, 0xaa]), 1);
    let extended = protocols::normalize_tecmp(&record(2, 3, 4, vec![0x80, 0, 1, 0x23, 1, 0xaa]), 2);
    let rtr = protocols::normalize_tecmp(&record(2, 3, 2, vec![0, 0, 1, 0x23, 0]), 3);
    let err = protocols::normalize_tecmp(&record(2, 3, 8, vec![0; 5]), 4);
    assert_eq!(
        native_engine::analyze_frames(&[normal, extended, rtr, err])
            .unwrap()
            .unique_keys,
        4
    );
}
fn offset_capture(le: bool, offset: i64) -> tempfile::NamedTempFile {
    fn u16b(x: u16, le: bool) -> [u8; 2] {
        if le {
            x.to_le_bytes()
        } else {
            x.to_be_bytes()
        }
    }
    fn u32b(x: u32, le: bool) -> [u8; 4] {
        if le {
            x.to_le_bytes()
        } else {
            x.to_be_bytes()
        }
    }
    fn block(t: u32, mut b: Vec<u8>, le: bool) -> Vec<u8> {
        b.resize((b.len() + 3) & !3, 0);
        let n = (b.len() + 12) as u32;
        let mut v = Vec::new();
        v.extend(u32b(t, le));
        v.extend(u32b(n, le));
        v.extend(b);
        v.extend(u32b(n, le));
        v
    }
    let mut sh = Vec::new();
    sh.extend(u32b(0x1a2b3c4d, le));
    sh.extend(u16b(1, le));
    sh.extend(u16b(0, le));
    sh.extend([0xff; 8]);
    let mut id = Vec::new();
    id.extend(u16b(1, le));
    id.extend(u16b(0, le));
    id.extend(u32b(65535, le));
    id.extend(u16b(9, le));
    id.extend(u16b(1, le));
    id.extend([9, 0, 0, 0]);
    id.extend(u16b(14, le));
    id.extend(u16b(8, le));
    id.extend(if le {
        offset.to_le_bytes()
    } else {
        offset.to_be_bytes()
    });
    id.extend([0; 4]);
    let mut ep = Vec::new();
    for v in [0, 0, 2_000_000_000, 60, 60] {
        ep.extend(u32b(v, le));
    }
    ep.extend([0; 60]);
    let mut raw = block(0x0a0d0d0a, sh, le);
    raw.extend(block(1, id, le));
    raw.extend(block(6, ep, le));
    let file = tempfile::Builder::new()
        .suffix(".pcapng")
        .tempfile()
        .unwrap();
    std::fs::write(file.path(), raw).unwrap();
    file
}
#[test]
fn both_readers_apply_signed_offsets_in_both_byte_orders() {
    for le in [true, false] {
        for (offset, expected) in [
            (10, 12_000_000_000),
            (-1, 1_000_000_000),
            (-10, 0),
            (i64::MIN, 0),
            (i64::MAX, u64::MAX),
        ] {
            let f = offset_capture(le, offset);
            assert_eq!(
                capture::read_capture(f.path(), 10).unwrap()[0].timestamp_ns,
                expected
            );
            assert_eq!(
                native_engine::read_capture(f.path(), 10).unwrap()[0].timestamp_ns,
                expected
            );
        }
    }
}
#[test]
fn source_time_segment_export_keeps_original_packet_and_capture_clock() {
    let r = record(2, 3, 0, vec![0, 0, 1, 0x23, 1, 0xaa, 0, 0]);
    let outer = r.timestamp_ns + 60_000_000_000;
    let raw = ethernet(&[r.clone()]);
    let f = pcap(&raw, outer);
    let output = tempfile::NamedTempFile::new().unwrap();
    assert_eq!(
        capture::write_pcapng_segment_with_timeline(
            f.path(),
            output.path(),
            0.0,
            0.001,
            10,
            Some(r.timestamp_ns),
            true
        )
        .unwrap(),
        1
    );
    let p = capture::read_capture(output.path(), 10).unwrap();
    assert_eq!(p[0].timestamp_ns, outer);
    assert_eq!(p[0].data, raw);
}
#[test]
fn legacy_metadata_deserializes_with_safe_defaults() {
    let m:TecmpMeta=serde_json::from_str(r#"{"cm_id":1,"counter":1,"version":3,"message_type":3,"message_type_name":"Logging Stream","data_type":2,"data_flags":0,"timestamp_flags":0}"#).unwrap();
    assert_eq!(m.device_flags, 0);
    assert_eq!(m.capture_timestamp_ns, None);
    assert_eq!(m.source_timestamp_ns, None);
}
#[test]
fn zero_source_time_keeps_zero_evidence_and_capture_fallback() {
    let mut r = record(2, 3, 0, vec![0, 0, 1, 0x23, 1, 0xaa]);
    r.timestamp_ns = 0;
    let file = pcap(&ethernet(&[r]), 123_000_000_000);
    let d = engine::load(file.path(), 10).unwrap();
    assert_eq!(d.frames[0].timestamp_ns, 123_000_000_000);
    assert_eq!(
        d.frames[0].tecmp.as_ref().unwrap().source_timestamp_ns,
        Some(0)
    );
    assert!(d.warnings.iter().any(|s| s.contains("Zero TECMP")));
}
#[test]
fn detailed_exports_preserve_exact_clocks_flags_and_raw_bytes() {
    use calamine::{open_workbook_auto, Reader};
    let r = record(2, 3, 0, vec![0, 0, 1, 0x23, 1, 0xaa, 0x12, 0x34]);
    let outer = r.timestamp_ns + 10_000_000;
    let input = pcap(&ethernet(&[r.clone()]), outer);
    let d = engine::load(input.path(), 10).unwrap();
    let frames = vec![&d.frames[0]];
    let csv = tempfile::NamedTempFile::new().unwrap();
    crate::export::selected_mapped_csv(csv.path(), &frames, &[]).unwrap();
    let mut reader = csv::Reader::from_path(csv.path()).unwrap();
    let headers = reader.headers().unwrap().clone();
    let row = reader.records().next().unwrap().unwrap();
    for (name, expected) in [
        ("TECMP_SourceTimestamp_ns", r.timestamp_ns.to_string()),
        ("CaptureTimestamp_ns", outer.to_string()),
        ("RawPayload", "00 00 01 23 01 AA 12 34".into()),
        ("CAN_CRC", "0x1234".into()),
    ] {
        assert_eq!(
            row.get(headers.iter().position(|h| h == name).unwrap()),
            Some(expected.as_str())
        );
    }
    let xlsx = tempfile::Builder::new().suffix(".xlsx").tempfile().unwrap();
    crate::export::selected_mapped_xlsx(xlsx.path(), &frames, &[]).unwrap();
    let mut workbook = open_workbook_auto(xlsx.path()).unwrap();
    let sheet = workbook.worksheet_range("Selected Frames").unwrap();
    let rows = sheet.rows().collect::<Vec<_>>();
    assert_eq!(rows[1][1].to_string(), r.timestamp_ns.to_string());
    assert_eq!(rows[1][12].to_string(), r.timestamp_ns.to_string());
    assert_eq!(rows[1][13].to_string(), outer.to_string());
}
#[test]
fn uptime_status_and_missing_clocks_do_not_stretch_epoch_source_graphs(){
    let bus=record(2,3,0,vec![0,0,1,0x23,1,0xaa]);let outer=bus.timestamp_ns+60_000_000_000;
    let mut status=bus.clone();status.timestamp_ns=1_000_000;
    let mut raw=ethernet(&[status]);raw[19]=1;raw[20]=0;raw[21]=0;
    let first=pcap(&raw,outer-10_000_000);let data=pcap(&ethernet(&[bus.clone()]),outer);
    let mut missing=bus.clone();missing.timestamp_ns=0;let last=pcap(&ethernet(&[missing]),outer+10_000_000);
    let mut bytes=std::fs::read(first.path()).unwrap();bytes.extend(&std::fs::read(data.path()).unwrap()[24..]);bytes.extend(&std::fs::read(last.path()).unwrap()[24..]);std::fs::write(first.path(),bytes).unwrap();
    let d=engine::load(first.path(),10).unwrap();
    assert!((d.frames.last().unwrap().relative_time_s-0.02).abs()<1e-9);
    assert_eq!(d.frames[0].timestamp_ns,bus.timestamp_ns-10_000_000);
    assert_eq!(d.frames[0].tecmp.as_ref().unwrap().source_timestamp_ns,Some(1_000_000));
    assert!(d.frames[0].tecmp.as_ref().unwrap().timeline_projected);
    assert_eq!(d.frames[1].timestamp_ns,bus.timestamp_ns);
    assert_eq!(d.frames[2].tecmp.as_ref().unwrap().source_timestamp_ns,Some(0));
}

#[test]
fn partial_capture_retains_topology_for_selected_channels_after_record_limit() {
    let mut first=record(tecmp::DT_ANALOG,3,0x5900,1000i16.to_be_bytes().to_vec());first.channel_id=1;
    let mut late=first.clone();late.channel_id=2;late.timestamp_ns+=5_000_000_000;
    let mut unselected=first.clone();unselected.channel_id=3;
    let mut raw=ethernet(&[first,late,unselected]);raw[20..22].copy_from_slice(&tecmp::DT_ANALOG.to_be_bytes());
    let input=pcap(&raw,1_760_000_000_000_000_000);
    let selected=std::collections::BTreeSet::from([1,2]);
    let partial=engine::load_selected_with_timeline(input.path(),1,Some(&selected),true).unwrap();
    assert_eq!(partial.frames.len(),1);assert_eq!(partial.frames[0].channel_id,1);
    assert!(partial.warnings.iter().any(|w|w.starts_with("Safe load limit reached")));
    assert_eq!(partial.index.channels,selected);
    assert!(partial.index.project_channel_keys.contains("ANALOG|2"));
    assert!(partial.index.by_project_channel["ANALOG|2"].is_empty(),"topology must not invent frame data");
    let complete=engine::load_selected_with_timeline(input.path(),2,Some(&selected),true).unwrap();
    assert_eq!(complete.frames.len(),2);
    assert!(!complete.warnings.iter().any(|w|w.starts_with("Safe load limit reached")),"exact EOF at the budget must not report missing data");
}
