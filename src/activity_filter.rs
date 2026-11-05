use eframe::egui;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Activity { Signal, Constant, Counter, Crc, Pattern, Analog, Other }
impl Activity {
    const ALL:[Self;7]=[Self::Signal,Self::Constant,Self::Counter,Self::Crc,Self::Pattern,Self::Analog,Self::Other];
    pub fn label(self)->&'static str {match self {
        Self::Signal=>"Signal",Self::Constant=>"Constant",Self::Counter=>"Counter",
        Self::Crc=>"CRC / Checksum candidate",Self::Pattern=>"Pattern",Self::Analog=>"Analog",Self::Other=>"Other / No data",
    }}
    fn from_class(class:&str)->Self {match class {
        "ACTIVE"=>Self::Signal,"STATIC"=>Self::Constant,"COUNTER4"|"COUNTER8"=>Self::Counter,
        "CRC_CANDIDATE"=>Self::Crc,"PATTERN"=>Self::Pattern,"ANALOG"=>Self::Analog,_=>Self::Other,
    }}
}

/// None selects every category; Some(empty) selects none.
#[derive(Default, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityFilter {pub selected:Option<BTreeSet<Activity>>}
impl ActivityFilter {
    pub fn is_empty(&self)->bool {self.selected.as_ref().is_some_and(BTreeSet::is_empty)}
    pub fn is_all(&self)->bool {self.selected.as_ref().is_none_or(|s|Activity::ALL.iter().all(|a|s.contains(a)))}
    pub fn matches(&self,class:&str)->bool {self.selected.as_ref().is_none_or(|s|s.contains(&Activity::from_class(class)))}
    pub fn ui(&mut self,ui:&mut egui::Ui) {
        let label=match self.selected.as_ref() {
            _ if self.is_all()=>"Activity: All".to_owned(),
            Some(s) if s.is_empty()=>"Activity: None".to_owned(),
            Some(s) if s.len()==1=>format!("Activity: {}",s.first().unwrap().label()),
            Some(s)=>format!("Activity: {} selected",s.len()),None=>unreachable!(),
        };
        egui::containers::menu::MenuButton::new(label)
            .config(egui::containers::menu::MenuConfig::new().close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside))
            .ui(ui,|ui| {
                ui.set_min_width(240.0);
                ui.label("Activity types");
                if ui.button("Select none").clicked(){self.selected=Some(BTreeSet::new());}
                if ui.button("Clear filter (all types)").clicked(){self.selected=None;}
                let mut all=self.is_all();
                if ui.checkbox(&mut all,"Select all").changed(){self.selected=if all{None}else{Some(BTreeSet::new())};}
                ui.separator();
                for activity in Activity::ALL {
                    let mut on=self.selected.as_ref().is_none_or(|s|s.contains(&activity));
                    if ui.checkbox(&mut on,activity.label()).changed(){
                        let selected=self.selected.get_or_insert_with(||Activity::ALL.into_iter().collect());
                        if on{selected.insert(activity);}else{selected.remove(&activity);}
                    }
                }
                ui.small("Applies immediately. Multiple types can be selected.");
                ui.small("Messages match if any byte has a selected type.");
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn multi_selection_none_and_serialization_preserve_class_matching() {
        let mut filter=ActivityFilter::default();assert!(filter.is_all());assert!(filter.matches("STATIC"));
        filter.selected=Some(BTreeSet::from([Activity::Pattern,Activity::Constant]));
        assert!(filter.matches("PATTERN"));assert!(filter.matches("STATIC"));
        for class in ["COUNTER4","COUNTER8","CRC_CANDIDATE","ACTIVE","ANALOG","NO DATA"]{assert!(!filter.matches(class));}
        let restored:ActivityFilter=serde_json::from_str(&serde_json::to_string(&filter).unwrap()).unwrap();assert_eq!(restored,filter);
        filter.selected=Some(BTreeSet::new());assert!(!filter.is_all());assert!(!filter.matches("PATTERN"));
    }
}
