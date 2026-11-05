use crate::model::NormalizedFrame;
use std::{collections::{BTreeMap,BTreeSet,HashMap}, sync::Arc};

#[derive(Default,Clone)]
pub struct FrameIndex {
    pub by_key: HashMap<String,Vec<usize>>,
    pub channels: BTreeSet<u32>,
    // Precomputed once at load. The Project Tree must never scan every frame on repaint.
    pub networks: BTreeMap<String,BTreeSet<u32>>,
    pub project_channel_keys: BTreeSet<String>,
    // v0.21: direct frame membership per Project Tree bus/channel node.
    pub by_project_channel: HashMap<String,Vec<usize>>,
    // v0.52.1: lowercase frame keys are prepared once at load so text filtering
    // never allocates/lowercases every frame key on each committed query.
    pub frame_key_lower: Vec<Arc<str>>,
}
impl FrameIndex {
    pub fn build(frames:&[NormalizedFrame])->Self {
        Self::build_with_catalog(frames,&BTreeMap::new())
    }
    /// Topology may include selected channels appearing after a bounded data prefix.
    /// Only actual loaded records are inserted into frame membership indexes.
    pub fn build_with_catalog(frames:&[NormalizedFrame],catalog:&BTreeMap<u32,BTreeSet<String>>)->Self {
        let mut x=Self::default();
        x.frame_key_lower.reserve(frames.len());
        let mut lower_by_key:HashMap<String,Arc<str>>=HashMap::new();

        // Pass 1: discover the known protocol for every channel.
        // If a channel contains decoded CAN/LIN/ETH/etc. plus some undecoded
        // TECMP records, the Project Tree should keep that channel under its
        // known network instead of duplicating it under UNKNOWN.
        let mut dominant:HashMap<u32,String>=catalog.iter().filter_map(|(ch,buses)|
            buses.iter().find(|bus|bus.as_str()!="UNKNOWN" && bus.as_str()!="TECMP").map(|bus|(*ch,bus.clone()))).collect();
        for f in frames {
            let bus=f.bus_type.to_string();
            if bus!="UNKNOWN" && bus!="TECMP" {
                dominant.entry(f.channel_id).or_insert(bus);
            }
        }

        // Pass 2: build indexes using the Project presentation key.
        for (i,f) in frames.iter().enumerate() {
            let lower=if let Some(v)=lower_by_key.get(f.frame_key.as_str()) {
                Arc::clone(v)
            } else {
                let v:Arc<str>=Arc::from(f.frame_key.to_ascii_lowercase());
                lower_by_key.insert(f.frame_key.clone(),Arc::clone(&v));
                v
            };
            x.frame_key_lower.push(lower);
            if let Some(ix)=x.by_key.get_mut(f.frame_key.as_str()) { ix.push(i); }
            else { x.by_key.insert(f.frame_key.clone(),vec![i]); }
            x.channels.insert(f.channel_id);

            let raw_bus=f.bus_type.to_string();
            let project_bus=if raw_bus=="UNKNOWN" || raw_bus=="TECMP" {
                dominant.get(&f.channel_id).cloned().unwrap_or(raw_bus.clone())
            } else {
                raw_bus.clone()
            };

            x.networks.entry(project_bus.clone()).or_default().insert(f.channel_id);
            let project_key=format!("{}|{}",project_bus,f.channel_id);
            x.project_channel_keys.insert(project_key.clone());
            x.by_project_channel.entry(project_key).or_default().push(i);
        }
        for (ch,buses) in catalog {
            x.channels.insert(*ch);
            for bus in buses {
                let bus=if bus=="UNKNOWN" || bus=="TECMP" {dominant.get(ch).unwrap_or(bus)}else{bus};
                x.networks.entry(bus.clone()).or_default().insert(*ch);
                let key=format!("{bus}|{ch}");x.project_channel_keys.insert(key.clone());
                x.by_project_channel.entry(key).or_default();
            }
        }
        x
    }
}
