use crate::model::{BusType, NormalizedFrame};
use eframe::egui;
use std::collections::{BTreeMap, BTreeSet};

pub type EndpointPair = (String, String);

#[derive(Default)]
pub struct EthernetEndpoints {
    pub channels: BTreeMap<u32, BTreeSet<EndpointPair>>,
    sources: BTreeMap<String, BTreeSet<EndpointPair>>,
}

impl EthernetEndpoints {
    pub fn build(frames: &[NormalizedFrame]) -> Self {
        let mut out = Self::default();
        for f in frames.iter().filter(|f| f.bus_type == BusType::Ethernet) {
            if let (Some(src), Some(dst)) = (&f.network.ip_src, &f.network.ip_dst) {
                let pair = (src.clone(), dst.clone());
                out.channels.entry(f.channel_id).or_default().insert(pair.clone());
                let key = format!("ETH|{}|{}", f.channel_id, f.frame_key);
                out.sources.entry(key).or_default().insert(pair);
            }
        }
        out
    }

    pub fn matches(&self, source: &str, filter: &EndpointFilter) -> bool {
        filter.is_all() || self.sources.get(source).is_some_and(|pairs|
            pairs.iter().any(|pair| filter.matches_pair(pair)))
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq,serde::Serialize,serde::Deserialize)]
pub struct EndpointFilter {
    pub source: Option<BTreeSet<String>>,
    pub destination: Option<BTreeSet<String>>,
}

impl EndpointFilter {
    pub fn is_all(&self) -> bool { self.source.is_none() && self.destination.is_none() }

    fn matches_pair(&self, pair: &EndpointPair) -> bool {
        self.source.as_ref().is_none_or(|ip| ip.contains(&pair.0))
            && self.destination.as_ref().is_none_or(|ip| ip.contains(&pair.1))
    }

    fn destinations(&self, pairs: &BTreeSet<EndpointPair>) -> BTreeSet<String> {
        pairs.iter().filter(|p| self.source.as_ref().is_none_or(|ip| ip.contains(&p.0)))
            .map(|p| p.1.clone()).collect()
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, channel: u32, pairs: &BTreeSet<EndpointPair>) {
        ui.push_id(("ethernet_endpoints", channel), |ui| {
            if pairs.is_empty() { ui.small("No decoded IP endpoints"); return; }
            let sources: BTreeSet<_> = pairs.iter().map(|p|p.0.clone()).collect();
            ip_selector(ui,"Source IP",&mut self.source,&sources);
            let destinations:BTreeSet<_>=pairs.iter().map(|p|p.1.clone()).collect();
            ip_selector(ui,"Destination IP",&mut self.destination,&destinations);
            if !self.is_all() && ui.small_button("All endpoints").clicked() { *self = Self::default(); }
        });
    }
}

fn ip_selector(ui:&mut egui::Ui,label:&str,value:&mut Option<BTreeSet<String>>,options:&BTreeSet<String>) {
    let summary=match value.as_ref(){None=>"All".into(),Some(ids) if ids.is_empty()=>"None".into(),Some(ids)=>format!("{} selected",ids.len())};
    egui::containers::menu::MenuButton::new(format!("{label}: {summary}"))
        .config(egui::containers::menu::MenuConfig::new().close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside))
        .ui(ui,|ui| {
            let mut all=value.is_none();
            if ui.checkbox(&mut all,"All").changed(){*value=if all{None}else{Some(BTreeSet::new())};}
            egui::ScrollArea::vertical().max_height(240.0).show(ui,|ui|{
                for ip in options {
                    let mut checked=value.as_ref().is_none_or(|ids|ids.contains(ip));
                    if ui.checkbox(&mut checked,ip).changed(){
                        let selected=value.get_or_insert_with(||options.clone());
                        if checked{selected.insert(ip.clone());}else{selected.remove(ip);}
                    }
                }
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn udp(src:[u8;4],dst:[u8;4],port:u16,channel:u32)->NormalizedFrame {
        let mut raw=vec![0u8;12];raw.extend(0x0800u16.to_be_bytes());
        let mut ip=vec![0u8;20];ip[0]=0x45;ip[2..4].copy_from_slice(&30u16.to_be_bytes());ip[9]=17;
        ip[12..16].copy_from_slice(&src);ip[16..20].copy_from_slice(&dst);raw.extend(ip);
        raw.extend(port.to_be_bytes());raw.extend(5000u16.to_be_bytes());raw.extend(10u16.to_be_bytes());raw.extend([0,0,1,2]);
        crate::protocols::parse_ethernet_packet(&raw,0,0,channel).unwrap()
    }
    #[test]
    fn decoded_udp_streams_are_filtered_exactly_and_isolated_by_channel() {
        let mut frames=vec![udp([10,0,0,1],[10,0,0,2],1000,7),udp([10,0,0,1],[10,0,0,2],1001,7),
            udp([10,0,0,2],[10,0,0,1],1000,7),udp([10,0,0,10],[10,0,0,2],1000,8)];
        let mut parent=frames[0].clone();
        parent.payload=[1u32.to_be_bytes().to_vec(),2u32.to_be_bytes().to_vec(),vec![1,2],
            2u32.to_be_bytes().to_vec(),2u32.to_be_bytes().to_vec(),vec![3,4]].concat();
        let children=crate::protocols::detect_container_review(&parent);assert_eq!(children.len(),2);frames.extend(children);
        let catalog=EthernetEndpoints::build(&frames);
        assert_eq!(catalog.channels[&7].len(),2);assert_eq!(catalog.channels[&8].len(),1);
        let filter=EndpointFilter{source:Some(BTreeSet::from(["10.0.0.1".into()])),destination:Some(BTreeSet::from(["10.0.0.2".into()]))};
        let matches:Vec<_>=frames.iter().filter(|f|catalog.matches(&format!("ETH|{}|{}",f.channel_id,f.frame_key),&filter)).collect();
        assert_eq!(matches.len(),4,"two ports plus two child PDUs must match; reverse direction and similar IP prefix must not");
        assert!(matches.iter().all(|f|f.channel_id==7));
    }
    #[test]
    fn checkboxes_select_multiple_sources_and_destinations() {
        use egui::{Pos2,Vec2};
        let ctx=egui::Context::default();let pairs=BTreeSet::from([("10.0.0.1".into(),"10.0.0.2".into()),("10.0.0.3".into(),"10.0.0.4".into())]);
        let mut filter=EndpointFilter::default();
        let mut draw=|events|ctx.run(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(Pos2::ZERO,Vec2::new(500.0,400.0))),events,..Default::default()},|ctx|{
            egui::CentralPanel::default().show(ctx,|ui|filter.ui(ui,7,&pairs));
        });
        let pos=|out:&egui::FullOutput,text:&str|out.shapes.iter().find_map(|s|match &s.shape {
            egui::epaint::Shape::Text(t) if t.galley.job.text==text=>Some(t.pos+Vec2::new(3.0,5.0)),_=>None
        }).unwrap_or_else(||panic!("missing {text}"));
        let click=|p,pressed|vec![egui::Event::PointerMoved(p),egui::Event::PointerButton{pos:p,button:egui::PointerButton::Primary,pressed,modifiers:egui::Modifiers::NONE}];
        for label in ["Source IP: All","Destination IP: All"] {
            let out=draw(vec![]);let p=pos(&out,label);draw(click(p,true));draw(click(p,false));
            let out=draw(vec![]);let p=pos(&out,"All");draw(click(p,true));draw(click(p,false));
            for ip in if label.starts_with("Source"){["10.0.0.1","10.0.0.3"]}else{["10.0.0.2","10.0.0.4"]} {
                let out=draw(vec![]);let p=pos(&out,ip);draw(click(p,true));draw(click(p,false));
            }
            let p=Pos2::new(450.0,350.0);draw(click(p,true));draw(click(p,false));
        }
        assert_eq!(filter.source,Some(BTreeSet::from(["10.0.0.1".into(),"10.0.0.3".into()])));
        assert_eq!(filter.destination,Some(BTreeSet::from(["10.0.0.2".into(),"10.0.0.4".into()])));
        assert!(filter.matches_pair(&("10.0.0.3".into(),"10.0.0.4".into())));
        filter.source=Some(BTreeSet::new());assert!(!filter.matches_pair(&("10.0.0.3".into(),"10.0.0.4".into())));
    }
    #[test]
    fn directional_pair_matching_does_not_combine_unrelated_endpoints() {
        let pairs = BTreeSet::from([("10.0.0.1".into(), "10.0.0.2".into()),
            ("10.0.0.2".into(), "10.0.0.1".into()), ("10.0.0.3".into(), "10.0.0.4".into())]);
        let mut catalog = EthernetEndpoints::default(); catalog.sources.insert("ETH|1|shared".into(), pairs.clone());
        let filter = EndpointFilter { source: Some(BTreeSet::from(["10.0.0.1".into()])), destination: Some(BTreeSet::from(["10.0.0.2".into()])) };
        assert!(catalog.matches("ETH|1|shared", &filter));
        assert!(!catalog.matches("ETH|1|shared", &EndpointFilter { destination: Some(BTreeSet::from(["10.0.0.4".into()])), ..filter.clone() }));
        assert_eq!(filter.destinations(&pairs), BTreeSet::from(["10.0.0.2".into()]));
        assert!(catalog.matches("non-IP", &EndpointFilter::default()));
        assert!(!catalog.matches("non-IP", &filter));
    }
}
