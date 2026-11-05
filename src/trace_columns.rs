use eframe::egui::{self, FontId, Pos2, Sense, Stroke, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceColumn { Timestamp, Delta, Channel, Protocol, FrameKey, Length, Count, Data }
impl TraceColumn {
    pub fn label(self)->&'static str {match self {
        Self::Timestamp=>"Timestamp",Self::Delta=>"dt",Self::Channel=>"Channel",Self::Protocol=>"Protocol",
        Self::FrameKey=>"ID/Key",Self::Length=>"DLC",Self::Count=>"Count",Self::Data=>"Data",
    }}
    fn index(self)->usize { self as usize }
    fn default_width(self)->f32 { [255.0,110.0,150.0,90.0,190.0,55.0,65.0,440.0][self.index()] }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_data_expands_inline_and_collapses_without_changing_complete_value() {
        let ctx=egui::Context::default();let mut columns=TraceColumns::default();
        for c in &mut columns.columns {c.visible=matches!(c.column,TraceColumn::Channel|TraceColumn::Data);c.width=110.0;}
        let value="AA BB CC DD ".repeat(30);let mut row_height=0.0;
        let mut draw=|columns:&mut TraceColumns,events|ctx.run(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(Pos2::ZERO,Vec2::new(240.0,700.0))),events,..Default::default()},|ctx| {
            egui::CentralPanel::default().show(ctx,|ui|{row_height=columns.row(ui,22.0,0,false,["","","Long channel name that does not fit","","","","",&value]).rect.height();});
        });
        draw(&mut columns,vec![]);let output=draw(&mut columns,vec![]);
        let find=|output:&egui::FullOutput,label:&str|output.shapes.iter().filter_map(|s|match &s.shape {
            egui::epaint::Shape::Text(t) if t.galley.job.text==label=>Some(t.pos+Vec2::new(3.0,5.0)),_=>None
        }).collect::<Vec<_>>();
        let points=find(&output,"… [expand]");assert_eq!(points.len(),1,"only Data gets an expand control");let pos=points[0];assert!(pos.x>110.0);
        let click=|pos,pressed|egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed,modifiers:egui::Modifiers::NONE};
        draw(&mut columns,vec![egui::Event::PointerMoved(pos),click(pos,true)]);draw(&mut columns,vec![click(pos,false)]);
        assert_eq!(columns.expanded_row,Some(0));
        let output=draw(&mut columns,vec![]);
        assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.job.text==value && t.galley.size().y>22.0)),"complete data must wrap inline");
        let pos=find(&output,"[collapse]")[0];draw(&mut columns,vec![egui::Event::PointerMoved(pos),click(pos,true)]);draw(&mut columns,vec![click(pos,false)]);
        assert_eq!(columns.expanded_row,None);draw(&mut columns,vec![]);drop(draw);assert_eq!(row_height,22.0);
    }
    #[test]
    fn expanded_rows_keep_following_rows_separate_and_large_traces_virtual() {
        let ctx=egui::Context::default();let mut columns=TraceColumns::default();
        for c in &mut columns.columns{c.visible=c.column==TraceColumn::Data;c.width=220.0;}
        columns.expanded_row=Some(1);let payload="AA BB CC DD ".repeat(50);
        let mut seen=Vec::new();let mut wanted=0.0;
        for offset in [0.0,1800.0,2_599_600.0] {
            for _ in 0..3 {
                seen.clear();
                let _=ctx.run(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(Pos2::ZERO,Vec2::new(260.0,300.0))),..Default::default()},|ctx| {
                    egui::CentralPanel::default().show(ctx,|ui| {
                        egui::ScrollArea::vertical().max_height(180.0).vertical_scroll_offset(offset).show_viewport(ui,|ui,viewport|{
                            let height=columns.expanded_height(ui,22.0,&payload);wanted=height;
                            trace_viewport_rows(ui,viewport,22.0,100_000,Some((1,height)),|ui,range|{
                                for i in range {
                                    let value=if i==1{payload.as_str()}else{"AA BB"};
                                    let r=columns.row(ui,22.0,i,false,["","","","","","","",value]);seen.push((i,r.rect));
                                }
                            });
                        });
                    });
                });
            }
            assert!(seen.len()<30,"must not render the full capture");
            for pair in seen.windows(2){assert!(pair[1].1.top()>=pair[0].1.bottom(),"expanded row must not overlap the following record");}
            if offset==0.0 {assert_eq!(seen[1].0,1);assert_eq!(seen[1].1.height(),wanted);assert!(wanted>22.0);}
            else {assert!(seen[0].0>1,"scrolling past expanded data must reach subsequent rows");}
        }
    }
    #[test]
    fn header_resize_and_clicking_a_non_timestamp_cell_work() {
        let ctx=egui::Context::default();let mut columns=TraceColumns::default();
        let mut frame=|events| {
            let mut row_rect=egui::Rect::NOTHING;let mut clicked=false;
            let input=egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(Pos2::ZERO,Vec2::new(1600.0,300.0))),events,..Default::default()};
            let _=ctx.run(input,|ctx| {egui::CentralPanel::default().show(ctx,|ui| {
                columns.header(ui,22.0);
                let r=columns.row(ui,22.0,7,false,["Timestamp","dt","Channel","Protocol","ID","8","1","AA BB"]);
                row_rect=r.rect;clicked=r.clicked();
            });});
            (row_rect,clicked,columns.columns[0].width)
        };
        let (row,_,original)=frame(vec![]);
        let edge=Pos2::new(row.left()+original,19.0);
        frame(vec![egui::Event::PointerMoved(edge)]);
        frame(vec![egui::Event::PointerButton{pos:edge,button:egui::PointerButton::Primary,pressed:true,modifiers:egui::Modifiers::NONE}]);
        let moved=edge+Vec2::new(40.0,0.0);
        let (_,_,width)=frame(vec![egui::Event::PointerMoved(moved)]);
        assert!((width-original-40.0).abs()<0.01);
        frame(vec![egui::Event::PointerButton{pos:moved,button:egui::PointerButton::Primary,pressed:false,modifiers:egui::Modifiers::NONE}]);
        let pos=Pos2::new(row.left()+width+30.0,row.center().y);
        frame(vec![egui::Event::PointerMoved(pos)]);
        frame(vec![egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed:true,modifiers:egui::Modifiers::NONE}]);
        let (_,clicked,_)=frame(vec![egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed:false,modifiers:egui::Modifiers::NONE}]);
        assert!(clicked,"row selection must work when clicking any visible column");
    }
    #[test]
    fn row_ranges_and_additive_selection_follow_displayed_order() {
        let order=[9,2,7,1];let mut selected=BTreeSet::new();let mut anchor=None;
        select_items(&mut selected,&mut anchor,2,&order,egui::Modifiers::NONE);
        select_items(&mut selected,&mut anchor,1,&order,egui::Modifiers{shift:true,..Default::default()});
        assert_eq!(selected,BTreeSet::from([2,7,1]));
        select_items(&mut selected,&mut anchor,9,&order,egui::Modifiers{command:true,..Default::default()});
        assert_eq!(selected,BTreeSet::from([9,2,7,1]));
        select_items(&mut selected,&mut anchor,7,&order,egui::Modifiers{ctrl:true,..Default::default()});
        assert_eq!(selected,BTreeSet::from([9,2,1]));
    }
    #[test]
    fn header_clicks_select_multiple_columns_and_ranges() {
        let ctx=egui::Context::default();let mut columns=TraceColumns::default();
        columns.columns[1].visible=false;
        let mut draw=|events:Vec<egui::Event>| {
            let modifiers=events.iter().find_map(|e|if let egui::Event::PointerButton{modifiers,..}=e{Some(*modifiers)}else{None}).unwrap_or_default();
            let input=egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(Pos2::ZERO,Vec2::new(1600.0,300.0))),events,modifiers,..Default::default()};
            let _=ctx.run(input,|ctx|{egui::CentralPanel::default().show(ctx,|ui|{columns.header(ui,22.0);});});
            columns.selected_columns.clone()
        };
        draw(vec![]);
        let mut click=|x,modifiers| {
            let pos=Pos2::new(x,19.0);
            draw(vec![egui::Event::PointerMoved(pos),egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed:true,modifiers}]);
            draw(vec![egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed:false,modifiers}])
        };
        assert_eq!(click(30.0,egui::Modifiers::NONE),BTreeSet::from([0]));
        assert_eq!(click(440.0,egui::Modifiers{shift:true,..Default::default()}),BTreeSet::from([0,2,3]));
        assert_eq!(click(600.0,egui::Modifiers{command:true,..Default::default()}),BTreeSet::from([0,2,3,4]));
    }
    #[test]
    fn hidden_columns_share_header_and_row_geometry_and_are_not_painted() {
        let ctx=egui::Context::default();let mut columns=TraceColumns::default();
        columns.columns[0].visible=false;columns.columns[7].visible=false;
        columns.columns[2].width=240.0;
        let mut row_width=0.0;
        let output=ctx.run(Default::default(),|ctx| {egui::CentralPanel::default().show(ctx,|ui| {
            columns.header(ui,22.0);
            row_width=columns.row(ui,22.0,1,false,["HIDDEN_TIME","dt","channel","CAN","id","8","1","HIDDEN_DATA"]).rect.width();
        });});
        assert_eq!(row_width,columns.total_width());
        for shape in output.shapes {if let egui::epaint::Shape::Text(text)=shape.shape {
            assert!(!text.galley.job.text.contains("HIDDEN_"));
        }}
        let mut saved:TraceColumns=serde_json::from_str(&serde_json::to_string(&columns).unwrap()).unwrap();
        saved.sanitize();assert_eq!(saved.total_width(),row_width);
        assert!(!saved.columns[0].visible);assert_eq!(saved.columns[2].width,240.0);
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TraceColumnSetting { pub column:TraceColumn, pub visible:bool, pub width:f32 }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TraceColumns {
    pub columns:Vec<TraceColumnSetting>,
    #[serde(skip)] expanded_row:Option<usize>,
    #[serde(skip)] pub selected_columns:BTreeSet<usize>,
    #[serde(skip)] column_anchor:Option<usize>,
}
impl Default for TraceColumns {
    fn default()->Self { Self{expanded_row:None,selected_columns:BTreeSet::new(),column_anchor:None,columns:[TraceColumn::Timestamp,TraceColumn::Delta,TraceColumn::Channel,
        TraceColumn::Protocol,TraceColumn::FrameKey,TraceColumn::Length,TraceColumn::Count,TraceColumn::Data]
        .into_iter().map(|column|TraceColumnSetting{column,visible:true,width:column.default_width()}).collect()} }
}
impl TraceColumns {
    pub fn sanitize(&mut self) {
        let defaults=Self::default();let mut seen=Vec::new();
        self.columns.retain(|c|if seen.contains(&c.column){false}else{seen.push(c.column);true});
        for c in &mut self.columns {c.width=if c.width.is_finite(){c.width.clamp(36.0,4096.0)}else{c.column.default_width()};}
        for c in defaults.columns {if !seen.contains(&c.column){self.columns.push(c);}}
        if !self.columns.iter().any(|c|c.visible){self.columns[0].visible=true;}
    }
    pub fn total_width(&self)->f32 {self.columns.iter().filter(|c|c.visible).map(|c|c.width).sum()}
    pub fn menu(&mut self,ui:&mut egui::Ui) {
        ui.menu_button("Columns",|ui| {
            ui.small("Show columns and adjust their width");
            let mut visible=self.columns.iter().filter(|c|c.visible).count();
            for c in &mut self.columns {
                ui.horizontal(|ui| {
                    let old=c.visible;
                    ui.add_enabled(!c.visible || visible>1,egui::Checkbox::new(&mut c.visible,c.column.label()));
                    if old!=c.visible {if c.visible{visible+=1;}else{visible-=1;}}
                    ui.add(egui::DragValue::new(&mut c.width).range(36.0..=4096.0).speed(1.0).suffix(" px"));
                });
            }
            ui.separator();
            if ui.button("Reset columns").clicked(){*self=Self::default();}
            ui.small("Drag a header's right edge to resize. Double-click the edge to reset its width.");
        });
        self.selected_columns.retain(|idx|self.columns.iter().any(|c|c.visible && c.column.index()==*idx));
    }
    pub fn clear_column_selection(&mut self) {
        self.selected_columns.clear();self.column_anchor=None;
    }
    pub fn header(&mut self,ui:&mut egui::Ui,height:f32) {
        let (rect,_)=ui.allocate_exact_size(Vec2::new(self.total_width(),height),Sense::hover());
        let mut x=rect.left();
        let mut clicked=None;
        for c in self.columns.iter_mut().filter(|c|c.visible) {
            let cell=egui::Rect::from_min_size(Pos2::new(x,rect.top()),Vec2::new(c.width,height));
            if self.selected_columns.contains(&c.column.index()) {
                ui.painter().rect_filled(cell,0.0,ui.visuals().selection.bg_fill);
            }
            let label_rect=egui::Rect::from_min_max(cell.left_top(),cell.right_bottom()-Vec2::new(4.0,0.0));
            if ui.interact(label_rect,ui.id().with(("trace_header",c.column.index())),Sense::click()).clicked() {
                clicked=Some(c.column.index());
            }
            ui.painter_at(cell.shrink2(Vec2::new(4.0,0.0))).text(cell.left_center()+Vec2::new(5.0,0.0),
                egui::Align2::LEFT_CENTER,c.column.label(),FontId::proportional(13.0),ui.visuals().strong_text_color());
            let edge=egui::Rect::from_min_max(Pos2::new(cell.right()-3.0,cell.top()),Pos2::new(cell.right()+3.0,cell.bottom()));
            let response=ui.interact(edge,ui.id().with(("trace_resize",c.column.index())),Sense::click_and_drag())
                .on_hover_cursor(egui::CursorIcon::ResizeHorizontal).on_hover_text("Drag to resize; double-click to reset width");
            if response.dragged(){c.width=(c.width+ui.input(|i|i.pointer.delta().x)).clamp(36.0,4096.0);}
            if response.double_clicked(){c.width=c.column.default_width();}
            ui.painter().line_segment([cell.right_top(),cell.right_bottom()],Stroke::new(1.0,ui.visuals().widgets.noninteractive.bg_stroke.color));
            x=cell.right();
        }
        if let Some(column)=clicked {
            let modifiers=ui.input(|i|i.modifiers);
            let visible:Vec<usize>=self.columns.iter().filter(|c|c.visible).map(|c|c.column.index()).collect();
            select_items(&mut self.selected_columns,&mut self.column_anchor,column,&visible,modifiers);
        }
    }
    pub fn expanded_row(&self)->Option<usize>{self.expanded_row}

    fn data_width(&self,ui:&egui::Ui)->f32 {
        let mut left=ui.cursor().left();
        for c in self.columns.iter().filter(|c|c.visible) {
            if c.column==TraceColumn::Data {
                let visible=egui::Rect::from_min_size(Pos2::new(left,ui.cursor().top()),Vec2::new(c.width,1.0));
                let width=(visible.right().min(ui.clip_rect().right())-visible.left().max(ui.clip_rect().left())).max(0.0);
                return if width>16.0{width-10.0}else{(c.width-10.0).max(16.0)};
            }
            left+=c.width;
        }
        0.0
    }
    pub fn expanded_height(&self,ui:&egui::Ui,base:f32,value:&str)->f32 {
        let width=self.data_width(ui);
        if width<=0.0{return base;}
        let galley=ui.painter().layout(value.to_owned(),FontId::monospace(12.0),ui.visuals().text_color(),width);
        base+galley.size().y+6.0
    }
    pub fn row(&mut self,ui:&mut egui::Ui,height:f32,id:usize,selected:bool,values:[&str;8])->egui::Response {
        let expanded=self.expanded_row==Some(id);
        let full_height=if expanded{self.expanded_height(ui,height,values[TraceColumn::Data.index()])}else{height};
        let (rect,mut response)=ui.allocate_exact_size(Vec2::new(self.total_width(),full_height),Sense::click());
        if id%2==1 {ui.painter().rect_filled(rect,0.0,ui.visuals().faint_bg_color);}
        let mut x=rect.left();
        for c in self.columns.iter().filter(|c|c.visible) {
            let cell=egui::Rect::from_min_size(Pos2::new(x,rect.top()),Vec2::new(c.width,full_height));
            let highlighted=selected && (self.selected_columns.is_empty() || self.selected_columns.contains(&c.column.index()));
            if highlighted {ui.painter().rect_filled(cell,0.0,ui.visuals().selection.bg_fill);}
            let color=if highlighted{ui.visuals().selection.stroke.color}else{ui.visuals().text_color()};
            let value=values[c.column.index()];
            let visible=cell.intersect(ui.clip_rect());
            let font=FontId::monospace(12.0);
            let galley=ui.painter().layout_no_wrap(value.to_owned(),font.clone(),color);
            let clipped=c.column==TraceColumn::Data && galley.size().x>(visible.width()-10.0).max(0.0);
            let mut text_rect=visible.shrink2(Vec2::new(4.0,0.0));
            if (clipped || (expanded && c.column==TraceColumn::Data)) && visible.width()>12.0 {
                let more=egui::Rect::from_min_max(Pos2::new((visible.right()-78.0).max(visible.left()),cell.top()),Pos2::new(visible.right(),cell.top()+height));
                text_rect.max.x=more.left()-2.0;
                ui.painter_at(more).text(more.center(),egui::Align2::CENTER_CENTER,if expanded{"[collapse]"}else if more.width()>=76.0{"… [expand]"}else{"…"},FontId::proportional(12.0),ui.visuals().hyperlink_color);
                if ui.interact(more,ui.id().with(("trace_expand",id,c.column.index())),Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(if expanded{"Collapse data"}else{"Expand data inline"}).clicked() {
                    self.expanded_row=if expanded{None}else{Some(id)};
                }
            }
            if expanded && c.column==TraceColumn::Data {
                let wrapped=ui.painter().layout(value.to_owned(),font,color,(visible.width()-10.0).max(16.0));
                ui.painter_at(visible).galley(Pos2::new(visible.left()+5.0,cell.top()+height+2.0),wrapped,color);
            } else {
                text_rect.max.y=text_rect.max.y.min(cell.top()+height);
                ui.painter_at(text_rect).galley(Pos2::new(cell.left()+5.0,cell.top()+(height-galley.size().y)/2.0),galley,color);
            }
            let select_rect=if (clipped || (expanded && c.column==TraceColumn::Data)) && visible.width()>12.0 {egui::Rect::from_min_max(cell.min,Pos2::new(text_rect.right()+4.0,cell.bottom()))}else{cell};
            let cell_response=ui.interact(select_rect,ui.id().with(("trace_cell",id,c.column.index())),Sense::click()).on_hover_text(value);
            response=response.union(cell_response);
            x=cell.right();
        }
        response
    }
}

// Only one expanded data row needs an extra offset. Keep large traces virtual
// without allocating a height array or iterating every record each repaint.
pub fn trace_viewport_rows(ui:&mut egui::Ui,viewport:egui::Rect,base:f32,count:usize,expanded:Option<(usize,f32)>,draw:impl FnOnce(&mut egui::Ui,std::ops::Range<usize>)) {
    let spacing=ui.spacing().item_spacing.y;let pitch=base+spacing;
    let expanded=expanded.filter(|(i,_)|*i<count);
    let extra=expanded.map(|(_,height)|(height-base).max(0.0)).unwrap_or(0.0);
    let top=|i:usize|i as f32*pitch+if expanded.is_some_and(|(row,_)|i>row){extra}else{0.0};
    let row_at=|y:f32| {
        if let Some((row,_))=expanded {
            let start=row as f32*pitch;
            if y>=start && y<start+pitch+extra{return row;}
            if y>=start+pitch+extra{return ((y-extra)/pitch).floor().max(0.0) as usize;}
        }
        (y/pitch).floor().max(0.0) as usize
    };
    ui.set_height((count as f32*pitch+extra-spacing).max(0.0));
    let mut first=row_at(viewport.min.y);
    let mut last=row_at(viewport.max.y).saturating_add(2);
    // After collapsing a row, the old scroll offset can exceed the new end
    // until ScrollArea clamps it. Still render the final viewport's rows.
    if last>count {
        let visible_count=last.saturating_sub(first).max(1);
        last=count;first=count.saturating_sub(visible_count);
    }
    first=first.min(last);
    let origin=ui.max_rect().top();
    let rect=egui::Rect::from_x_y_ranges(ui.max_rect().x_range(),origin+top(first)..=origin+top(last));
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect),|ui| {
        ui.skip_ahead_auto_ids(first);draw(ui,first..last);
    });
}

// Range selection follows displayed order, including non-monotonic Overwrite rows.
pub fn select_items(selected:&mut BTreeSet<usize>, anchor:&mut Option<usize>, item:usize, order:&[usize], modifiers:egui::Modifiers) {
    let additive=modifiers.command || modifiers.ctrl;
    if modifiers.shift {
        if let Some((a,b))=anchor.and_then(|a|order.iter().position(|v|*v==a))
            .zip(order.iter().position(|v|*v==item)) {
            if !additive {selected.clear();}
            selected.extend(order[a.min(b)..=a.max(b)].iter().copied());
            return;
        }
    }
    if additive {
        if !selected.remove(&item) {selected.insert(item);}
    } else {selected.clear();selected.insert(item);}
    *anchor=Some(item);
}
