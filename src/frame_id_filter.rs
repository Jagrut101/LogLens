use eframe::egui;
use std::collections::BTreeSet;

/// None means unrestricted; Some(empty) deliberately shows no IDs.
#[derive(Default,Clone,serde::Serialize,serde::Deserialize)]
pub struct FrameIdFilter {
    pub selected: Option<BTreeSet<String>>,
    pub search: String,
}

impl FrameIdFilter {
    pub fn matches(&self, id: &str) -> bool {
        self.selected.as_ref().is_none_or(|ids| ids.contains(id))
    }

    pub fn reveal(&mut self, id: &str) {
        if let Some(ids) = &mut self.selected { ids.insert(id.to_owned()); }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, catalog: &BTreeSet<String>) {
        let count = self.selected.as_ref().map(|ids| catalog.intersection(ids).count());
        let label = match count {
            None => "Frame IDs: All".to_owned(),
            Some(0) => "Frame IDs: None".to_owned(),
            Some(n) if n == catalog.len() => "Frame IDs: All".to_owned(),
            Some(1) => "Frame IDs: 1 selected".to_owned(),
            Some(n) => format!("Frame IDs: {n} selected"),
        };
        egui::containers::menu::MenuButton::new(label)
            .config(egui::containers::menu::MenuConfig::new().close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside))
            .ui(ui, |ui| {
            ui.set_min_width(260.0);
            ui.label("Frame IDs in enabled channels");
            ui.small(format!("{} selected · {} available", count.unwrap_or(catalog.len()), catalog.len()));
            ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Search Frame IDs"));
            if ui.button("Select none").clicked() { self.selected = Some(BTreeSet::new()); }
            if ui.button("Clear filter (all IDs)").clicked() {
                self.selected = None;
                self.search.clear();
            }
            let search = self.search.to_lowercase();
            let visible: Vec<_> = catalog.iter().filter(|id| crate::model::frame_key_display(id).to_lowercase().contains(&search) || id.to_lowercase().contains(&search)).collect();
            let mut all = !visible.is_empty() && visible.iter().all(|id| self.matches(id));
            let any = visible.iter().any(|id| self.matches(id));
            let text = if search.is_empty() { "Select all" } else { "Select all matching" };
            let mixed = any && !all;
            if ui.add(egui::Checkbox::new(&mut all, text).indeterminate(mixed)).changed() {
                let ids = self.selected.get_or_insert_with(|| catalog.clone());
                for id in &visible {
                    if all { ids.insert((*id).clone()); } else { ids.remove(*id); }
                }
            }
            ui.separator();
            if visible.is_empty() { ui.weak("No matching Frame IDs"); }
            egui::ScrollArea::vertical().id_salt("frame_id_checklist").max_height(300.0)
                .show_rows(ui, ui.spacing().interact_size.y, visible.len(), |ui, rows| {
                    for row in rows {
                        let id = visible[row];
                        let mut checked = self.matches(id);
                        if ui.checkbox(&mut checked, crate::model::frame_key_display(id)).changed() {
                            let ids = self.selected.get_or_insert_with(|| catalog.clone());
                            if checked { ids.insert(id.clone()); } else { ids.remove(id); }
                        }
                    }
                });
            ui.small("Selections apply immediately to the message list.");
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checklist_pointer_selection_search_select_all_and_clear() {
        let catalog: BTreeSet<String> = ["CAN:0x123", "CAN:0x1234", "LIN:0x23"].map(str::to_owned).into_iter().collect();
        let mut filter = FrameIdFilter::default();
        let ctx = egui::Context::default();
        let mut time = 0.0;
        let mut draw = |filter: &mut FrameIdFilter, events| {
            time += 0.2;
            ctx.run(egui::RawInput { time: Some(time), screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0,700.0))), events, ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| filter.ui(ui, &catalog));
            })
        };
        let position = |output: &egui::FullOutput, label: &str| {
            output.shapes.iter().find_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) if t.galley.job.text == label => Some(t.pos + egui::vec2(8.0,7.0)),
                _ => None,
            }).unwrap_or_else(|| panic!("Missing UI label: {label}"))
        };
        let pointer = |pos, pressed| vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }];
        let output = draw(&mut filter,vec![]);
        let pos = position(&output,"Frame IDs: All");
        draw(&mut filter,pointer(pos,true));draw(&mut filter,pointer(pos,false));
        let output=draw(&mut filter,vec![]);
        let pos=position(&output,"CAN:0x123");
        draw(&mut filter,pointer(pos,true));draw(&mut filter,pointer(pos,false));
        assert!(!filter.matches("CAN:0x123"));
        assert!(filter.matches("CAN:0x1234"),"exact ID matching must preserve longer IDs");
        filter.search="lin".into();
        let output=draw(&mut filter,vec![]);
        assert!(!output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.job.text=="CAN:0x1234")));
        let pos=position(&output,"Select all matching");
        draw(&mut filter,pointer(pos,true));draw(&mut filter,pointer(pos,false));
        assert!(!filter.matches("LIN:0x23"));assert!(filter.matches("CAN:0x1234"));
        filter.search.clear();let output=draw(&mut filter,vec![]);
        let pos=position(&output,"Select all");
        draw(&mut filter,pointer(pos,true));draw(&mut filter,pointer(pos,false));
        assert!(catalog.iter().all(|id|filter.matches(id)));
        let output=draw(&mut filter,vec![]);let pos=position(&output,"Select all");
        draw(&mut filter,pointer(pos,true));draw(&mut filter,pointer(pos,false));
        assert!(catalog.iter().all(|id|!filter.matches(id)),"empty selection means show none");
        let output=draw(&mut filter,vec![]);let pos=position(&output,"Clear filter (all IDs)");
        draw(&mut filter,pointer(pos,true));draw(&mut filter,pointer(pos,false));
        assert!(filter.selected.is_none());assert!(catalog.iter().all(|id|filter.matches(id)));
    }
}
