use crate::model::NormalizedFrame;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct ControlTrigger {
    pub frame_index: usize,
    pub time_s: f64,
    pub control_message_id: u16,
    pub tecmp_cm_id: u16,
    pub occurrence: usize,
    pub label: String,
}

/// Return every TECMP Control Message as an independent point marker.
/// No pairing, no min-gap suppression, and repeated IDs remain separate occurrences.
pub fn control_triggers(frames: &[NormalizedFrame], _legacy_min_gap_s: f64) -> Vec<ControlTrigger> {
    let mut occurrences = BTreeMap::<u16, usize>::new();
    let mut out = Vec::new();

    for (frame_index, f) in frames.iter().enumerate() {
        let Some(meta) = f.tecmp.as_ref() else { continue; };
        if meta.message_type != 0x00 { continue; }

        // TECMP Control Message payload carries the Control Message ID in bytes 2..4.
        // Fall back to the outer TECMP CM ID only for malformed/short payloads.
        let control_message_id = if f.payload.len() >= 4 {
            u16::from_be_bytes([f.payload[2], f.payload[3]])
        } else if f.raw_payload.len() >= 4 {
            u16::from_be_bytes([f.raw_payload[2], f.raw_payload[3]])
        } else {
            meta.cm_id
        };

        let occurrence = occurrences.entry(control_message_id).or_insert(0);
        *occurrence += 1;

        out.push(ControlTrigger {
            frame_index,
            time_s: f.relative_time_s,
            control_message_id,
            tecmp_cm_id: meta.cm_id,
            occurrence: *occurrence,
            label: format!("CM 0x{control_message_id:04X}"),
        });
    }
    out
}
