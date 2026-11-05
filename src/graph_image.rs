use anyhow::{bail, Context, Result};
use ab_glyph::{Font,FontArc,ScaleFont,PxScale,point};
use tiny_skia::{Pixmap,Paint,PathBuilder,Stroke,Transform,Color,Rect};
use std::path::{Path,PathBuf};
use image::ImageEncoder;

#[derive(Clone,Debug)]
pub struct Cursor {pub label:String,pub time:f64,pub value:String,pub y:Option<f64>}
#[derive(Clone,Debug)]
pub struct Lane {
    pub name:String,pub unit:String,pub points:Vec<[f64;2]>,pub color:[u8;3],
    pub y_range:Option<(f64,f64)>,pub cursors:Vec<Cursor>,pub delta:Option<String>,
}
#[derive(Clone,Debug)]
pub struct Report {
    pub title:String,pub origin:String,pub start:f64,pub end:f64,
    pub lanes:Vec<Lane>,pub markers:Vec<(f64,String)>,pub multi:bool,
    pub overlay:bool,pub grid:bool,pub dark:bool,
}
#[derive(Default)]
pub struct GraphImageExport {pub status:Option<(bool,String)>}
impl GraphImageExport {
    pub fn save(&mut self,mut path:PathBuf,report:&Report)->Result<PathBuf> {
        if path.extension().is_none(){path.set_extension("png");}
        let result=render_png(&path,report).map(|()|path.clone());
        self.status=Some(match &result {
            Ok(path)=>(true,format!("Saved graph image: {}",path.display())),
            Err(error)=>(false,format!("Could not save graph image: {error:#}")),
        });result
    }
}
const W:u32=2000;
const LEFT:f32=160.0;
const RIGHT:f32=1950.0;
#[derive(Clone,Copy)]
struct Palette {background:[u8;3],ink:[u8;3],muted:[u8;3],grid:[u8;3],band:[u8;3],cursor_a:[u8;3],cursor_b:[u8;3]}
fn palette(dark:bool)->Palette {
    if dark {Palette{background:[24,27,32],ink:[232,237,243],muted:[167,180,195],grid:[59,68,80],band:[37,44,54],cursor_a:[255,207,64],cursor_b:[255,115,140]}}
    else{Palette{background:[255,255,255],ink:[35,43,52],muted:[101,113,125],grid:[222,227,232],band:[239,243,247],cursor_a:[174,111,0],cursor_b:[194,48,77]}}
}
/// Native OS appearance takes precedence over an application theme override.
/// If a platform cannot report it, use the currently effective app appearance.
pub fn system_dark(ctx:&eframe::egui::Context)->bool {
    ctx.system_theme().map(|theme|theme==eframe::egui::Theme::Dark).unwrap_or_else(||ctx.style().visuals.dark_mode)
}
struct Canvas {image:Pixmap,font:FontArc}
impl Canvas {
    fn new(height:u32)->Result<Self>{
        let definitions=eframe::egui::FontDefinitions::default();
        let bytes=definitions.font_data.get("Ubuntu-Light").context("Export font unavailable")?.font.to_vec();
        let font=FontArc::try_from_vec(bytes).map_err(|_|anyhow::anyhow!("Invalid export font"))?;
        let mut image=Pixmap::new(W,height).context("Cannot allocate graph image")?;image.fill(Color::WHITE);
        Ok(Self{image,font})
    }
    fn text_width(&self,text:&str,size:f32)->f32 {
        let f=self.font.as_scaled(PxScale::from(size));let mut width=0.0;let mut previous=None;
        for ch in text.chars(){let id=f.glyph_id(ch);if let Some(prev)=previous{width+=f.kern(prev,id);}width+=f.h_advance(id);previous=Some(id);}width
    }
    fn text(&mut self,text:&str,x:f32,y:f32,size:f32,color:[u8;3]) {
        let f=self.font.as_scaled(PxScale::from(size));let mut pen=x;let mut previous=None;
        let baseline=y+f.ascent();let width=self.image.width();let height=self.image.height();
        for ch in text.chars(){
            let id=f.glyph_id(ch);if let Some(prev)=previous{pen+=f.kern(prev,id);}
            let glyph=id.with_scale_and_position(size,point(pen,baseline));
            if let Some(outline)=self.font.outline_glyph(glyph){let bounds=outline.px_bounds();let pixels=self.image.data_mut();
                outline.draw(|gx,gy,coverage|{
                    let px=bounds.min.x as i32+gx as i32;let py=bounds.min.y as i32+gy as i32;
                    if px>=0 && py>=0 && px<width as i32 && py<height as i32 {
                        let i=(py as usize*width as usize+px as usize)*4;
                        for c in 0..3{pixels[i+c]=(color[c] as f32*coverage+pixels[i+c] as f32*(1.0-coverage)).round() as u8;}
                        pixels[i+3]=255;
                    }
                });
            }
            pen+=f.h_advance(id);previous=Some(id);
        }
    }
    fn right_text(&mut self,text:&str,x:f32,y:f32,size:f32,color:[u8;3]) {let width=self.text_width(text,size);self.text(text,x-width,y,size,color);}
    fn line(&mut self,x:f32,y:f32,x2:f32,y2:f32,color:[u8;3],width:f32) {
        let mut p=PathBuilder::new();p.move_to(x,y);p.line_to(x2,y2);
        if let Some(path)=p.finish(){let mut paint=Paint::default();paint.set_color_rgba8(color[0],color[1],color[2],255);
            self.image.stroke_path(&path,&paint,&Stroke{width,..Default::default()},Transform::identity(),None);}
    }
    fn fill(&mut self,x:f32,y:f32,w:f32,h:f32,color:[u8;3]) {
        if let Some(rect)=Rect::from_xywh(x,y,w,h){let mut paint=Paint::default();paint.set_color_rgba8(color[0],color[1],color[2],255);self.image.fill_rect(rect,&paint,Transform::identity(),None);}
    }
    fn wrap(&self,text:&str,width:f32,size:f32)->Vec<String> {
        let mut lines=Vec::new();let mut current=String::new();
        for ch in text.chars(){
            if ch=='\n'{lines.push(std::mem::take(&mut current));continue;}
            current.push(ch);
            if self.text_width(&current,size)>width && current.chars().count()>1 {
                current.pop();lines.push(std::mem::take(&mut current));current.push(ch);
            }
        }
        if !current.is_empty(){lines.push(current);}if lines.is_empty(){lines.push(String::new());}lines
    }
}
fn range(lane:&Lane)->(f64,f64) {
    if let Some((lo,hi))=lane.y_range{if lo.is_finite() && hi.is_finite() && hi>lo{return(lo,hi);}}
    let lo=lane.points.iter().map(|p|p[1]).filter(|v|v.is_finite()).fold(f64::INFINITY,f64::min);
    let hi=lane.points.iter().map(|p|p[1]).filter(|v|v.is_finite()).fold(f64::NEG_INFINITY,f64::max);
    if !lo.is_finite(){return(0.0,1.0);}
    let padding=if (hi-lo).abs()<1e-12{(lo.abs()*0.05).max(0.5)}else{(hi-lo)*0.05};(lo-padding,hi+padding)
}
fn number(value:f64)->String {
    if value.abs()>=1e7 || (value!=0.0 && value.abs()<0.0001){format!("{value:.3e}")}
    else{format!("{value:.3}")}
}
fn plot(c:&mut Canvas,report:&Report,lanes:&[&Lane],top:f32,height:f32) {
    let palette=palette(report.dark);
    let (lo,hi)=if report.overlay{(0.0,255.0)}else{range(lanes[0])};
    let x=|t:f64|LEFT+((t-report.start)/(report.end-report.start)) as f32*(RIGHT-LEFT);
    let y=|v:f64|top+height-((v-lo)/(hi-lo)) as f32*height;
    if report.grid {
        for i in 0..=10{let xx=LEFT+(RIGHT-LEFT)*i as f32/10.0;c.line(xx,top,xx,top+height,palette.grid,1.0);}
        for i in 0..=4{let yy=top+height*i as f32/4.0;c.line(LEFT,yy,RIGHT,yy,palette.grid,1.0);}
    }
    for i in 0..=4 {let value=hi-(hi-lo)*i as f64/4.0;c.right_text(&number(value),LEFT-14.0,top+height*i as f32/4.0-9.0,18.0,palette.muted);}
    c.line(LEFT,top,LEFT,top+height,palette.muted,1.2);c.line(LEFT,top+height,RIGHT,top+height,palette.muted,1.2);
    for (time,label) in &report.markers {
        if *time>=report.start && *time<=report.end {let xx=x(*time);c.line(xx,top,xx,top+height,[150,163,174],1.0);c.text(label,xx.min(RIGHT-100.0)+4.0,top+4.0,16.0,palette.muted);}
    }
    for lane in lanes {
        let mut builder=PathBuilder::new();let mut previous=None;
        for p in &lane.points {
            if !p[0].is_finite() || !p[1].is_finite() || p[0]<report.start || p[0]>report.end {continue;}
            let xx=x(p[0]).clamp(LEFT,RIGHT);let yy=y(p[1]);
            if let Some((px,py))=previous {
                if py>=top && py<=top+height {builder.move_to(px,py);builder.line_to(xx,py);}
                if py.min(yy)<=top+height && py.max(yy)>=top {
                    builder.move_to(xx,py.clamp(top,top+height));builder.line_to(xx,yy.clamp(top,top+height));
                }
            }
            previous=Some((xx,yy));
        }
        if let Some(path)=builder.finish(){let mut paint=Paint::default();paint.set_color_rgba8(lane.color[0],lane.color[1],lane.color[2],255);
            c.image.stroke_path(&path,&paint,&Stroke{width:2.4,..Default::default()},Transform::identity(),None);}
    }
    for (li,lane) in lanes.iter().enumerate(){
        if li>0 && !report.multi{continue;}
        for (i,cursor) in lane.cursors.iter().enumerate(){
            if !cursor.time.is_finite() || cursor.time<report.start || cursor.time>report.end{continue;}
            let color=if i%2==0{palette.cursor_a}else{palette.cursor_b};let xx=x(cursor.time);
            // Dashed measurement cursors stay distinct from waveform transitions.
            let mut yy=top;while yy<top+height{c.line(xx,yy,xx,(yy+8.0).min(top+height),color,1.4);yy+=14.0;}
            c.text(&cursor.label,xx.min(RIGHT-30.0)+5.0,top+23.0+li as f32*22.0,18.0,color);
            if let Some(value)=cursor.y.filter(|v|v.is_finite() && *v>=lo && *v<=hi){c.fill(xx-3.0,y(value)-3.0,6.0,6.0,color);}
        }
    }
}
pub fn render_png(path:&Path,report:&Report)->Result<()> {
    if !report.start.is_finite() || !report.end.is_finite() || report.end<=report.start{bail!("Invalid graph time range");}
    if report.lanes.is_empty(){bail!("Select at least one graph that matches the current filters");}
    let palette=palette(report.dark);
    let mut layout=Canvas::new(1)?;
    let titles:Vec<_>=report.lanes.iter().map(|lane|layout.wrap(&lane.name,RIGHT-LEFT,23.0)).collect();
    let graph_height=if report.overlay {50.0+titles.iter().map(|t|t.len() as f32*28.0).sum::<f32>()+280.0}
        else{titles.iter().map(|t|t.len() as f32*28.0+290.0).sum()};
    let table_rows:Vec<_>=report.lanes.iter().map(|lane|layout.wrap(&lane.name,report_name_width(report.multi),20.0)).collect();
    let table_height: f32=report.lanes.iter().enumerate().map(|(i,lane)| {
        let rows=if report.multi{lane.cursors.len().max(1)}else{1};
        (table_rows[i].len() as f32*25.0+12.0).max(rows as f32*32.0+12.0)
    }).sum();
    let title=layout.wrap(&report.title,RIGHT-50.0,28.0);
    let header=title.len() as f32*34.0+95.0;
    let total=(header+graph_height+table_height+170.0).ceil();
    if total>24000.0{bail!("Too many graphs for one PNG. Select fewer graphs and export again");}
    layout.image=Pixmap::new(W,total as u32).context("Cannot allocate graph image")?;layout.image.fill(Color::from_rgba8(palette.background[0],palette.background[1],palette.background[2],255));
    let c=&mut layout;let mut yy=26.0;
    for line in title {c.text(&line,50.0,yy,28.0,palette.ink);yy+=34.0;}
    c.text(&format!("Time window  {:.6} to {:.6} s",report.start,report.end),50.0,yy+8.0,20.0,palette.muted);
    c.text(&report.origin,50.0,yy+35.0,18.0,palette.muted);yy=header;
    if report.overlay {
        for (i,lane) in report.lanes.iter().enumerate(){for line in &titles[i]{c.text(line,LEFT,yy,23.0,lane.color);yy+=28.0;}}
        plot(c,report,&report.lanes.iter().collect::<Vec<_>>(),yy+12.0,240.0);yy+=290.0;
    }else{
        for (i,lane) in report.lanes.iter().enumerate(){
            for line in &titles[i]{c.text(line,LEFT,yy,23.0,lane.color);yy+=28.0;}
            if !lane.unit.is_empty(){c.right_text(&lane.unit,LEFT-14.0,yy+3.0,18.0,palette.muted);}
            plot(c,report,&[lane],yy+24.0,220.0);yy+=290.0;
        }
    }
    // One common time axis for the stacked plots.
    for i in 0..=10 {let t=report.start+(report.end-report.start)*i as f64/10.0;let xx=LEFT+(RIGHT-LEFT)*i as f32/10.0;
        let label=format!("{t:.3}");let tw=c.text_width(&label,18.0);c.text(&label,(xx-tw/2.0).clamp(LEFT,RIGHT-tw),yy-35.0,18.0,palette.muted);}
    c.text("Time [s]",(LEFT+RIGHT)/2.0-30.0,yy-9.0,19.0,palette.ink);yy+=30.0;
    c.text("Cursor measurements",50.0,yy,24.0,palette.ink);yy+=35.0;
    if report.multi {c.fill(50.0,yy,1900.0,34.0,palette.band);
        for (label,x) in [("Signal",62.0),("Cursor",1120.0),("Time [s]",1270.0),("Value",1580.0)]{c.text(label,x,yy+7.0,19.0,palette.ink);}
    }else {
        let times=report.lanes.first().map(|l|l.cursors.as_slice()).unwrap_or(&[]);
        let a=times.first().map(|v|v.time).unwrap_or(f64::NAN);let b=times.get(1).map(|v|v.time).unwrap_or(f64::NAN);
        c.text(&format!("A  {a:.6} s      B  {b:.6} s      Δt  {:+.6} s",b-a),50.0,yy,20.0,palette.muted);yy+=30.0;
        c.fill(50.0,yy,1900.0,34.0,palette.band);
        for (label,x) in [("Signal",62.0),("A",1030.0),("B",1340.0),("Δ value",1650.0)]{c.text(label,x,yy+7.0,19.0,palette.ink);}
    }
    yy+=38.0;
    for (i,lane) in report.lanes.iter().enumerate(){
        let rows=if report.multi{lane.cursors.len().max(1)}else{1};
        let h=(table_rows[i].len() as f32*25.0+12.0).max(rows as f32*32.0+12.0);
        for (j,line) in table_rows[i].iter().enumerate(){c.text(line,62.0,yy+4.0+j as f32*25.0,20.0,lane.color);}
        if report.multi {
            if lane.cursors.is_empty(){c.text("No cursors set",1270.0,yy+4.0,20.0,palette.muted);}
            for (j,cursor) in lane.cursors.iter().enumerate(){let y=yy+4.0+j as f32*32.0;c.text(&cursor.label,1120.0,y,20.0,palette.ink);c.text(&format!("{:.6}",cursor.time),1270.0,y,20.0,palette.ink);c.text(&cursor.value,1580.0,y,20.0,palette.ink);}
        }else{c.text(lane.cursors.first().map(|v|v.value.as_str()).unwrap_or("n/a"),1030.0,yy+4.0,20.0,palette.ink);
            c.text(lane.cursors.get(1).map(|v|v.value.as_str()).unwrap_or("n/a"),1340.0,yy+4.0,20.0,palette.ink);
            c.text(lane.delta.as_deref().unwrap_or("n/a"),1650.0,yy+4.0,20.0,palette.ink);}
        yy+=h;c.line(50.0,yy,1950.0,yy,palette.grid,1.0);
    }
    // Trim to the actual drawing, never to a desktop/window's dimensions.
    let height=(yy+26.0).ceil().min(c.image.height() as f32) as u32;
    let parent=path.parent().filter(|p|!p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut temporary=tempfile::NamedTempFile::new_in(parent).context("Cannot create image file")?;
    image::codecs::png::PngEncoder::new(temporary.as_file_mut()).write_image(
        &c.image.data()[..W as usize*height as usize*4],W,height,image::ExtendedColorType::Rgba8)?;
    temporary.as_file().sync_all()?;temporary.persist(path).map_err(|e|e.error).context("Cannot save graph image")?;Ok(())
}
fn report_name_width(multi:bool)->f32 {if multi{1020.0}else{940.0}}

#[cfg(test)]
mod tests {
    use super::*;
    fn report()->Report {Report{title:"Test capture".into(),origin:"Source time".into(),start:0.0,end:2.0,overlay:false,grid:true,dark:false,multi:false,markers:vec![],lanes:vec![Lane{name:"CAN · 1 · 0x123 · Byte 0".into(),unit:"byte".into(),points:vec![[0.0,0.0],[1.0,5.0],[2.0,10.0]],color:[0,105,165],y_range:None,cursors:vec![Cursor{label:"A".into(),time:0.2,value:"0 / 0x00".into(),y:Some(0.0)},Cursor{label:"B".into(),time:1.4,value:"5 / 0x05".into(),y:Some(5.0)}],delta:Some("+5".into())}]}}
    #[test]
    fn os_theme_wins_over_app_override_and_updates_between_exports() {
        use eframe::egui::{Context,Theme,Visuals};
        let ctx=Context::default();
        for (os,app,dark) in [(Some(Theme::Dark),Theme::Light,true),(Some(Theme::Light),Theme::Dark,false),(None,Theme::Dark,true),(None,Theme::Light,false)] {
            let _=ctx.run(eframe::egui::RawInput{system_theme:os,..Default::default()},|ctx| {
                ctx.set_visuals(if app==Theme::Dark{Visuals::dark()}else{Visuals::light()});
                assert_eq!(system_dark(ctx),dark,"OS={os:?}, app={app:?}");
            });
        }
    }
    #[test]
    fn dark_and_light_exports_preserve_layout_with_contrasting_backgrounds_and_text() {
        let dir=tempfile::tempdir().unwrap();let mut report=report();let mut dimensions=None;
        for dark in [false,true] {
            report.dark=dark;report.lanes[0].color=if dark{[86,180,233]}else{[0,105,165]};
            let path=dir.path().join(if dark{"dark.png"}else{"light.png"});render_png(&path,&report).unwrap();
            if let Some(previews)=std::env::var_os("LOGLENS_THEME_PREVIEWS") {
                let previews=PathBuf::from(previews);std::fs::create_dir_all(&previews).unwrap();
                std::fs::copy(&path,previews.join(if dark{"export-dark.png"}else{"export-light.png"})).unwrap();
            }
            let image=image::open(path).unwrap().to_rgba8();let palette=palette(dark);
            assert_eq!(&image.get_pixel(0,0).0[..3],&palette.background);
            assert!(image.pixels().any(|pixel|pixel.0[..3]==palette.ink),"measurement text must use the theme's foreground");
            assert!(image.pixels().any(|pixel|pixel.0[..3]==palette.band),"cursor table header should follow the theme");
            if let Some(size)=dimensions{assert_eq!(image.dimensions(),size);}else{dimensions=Some(image.dimensions());}
        }
    }
    #[test]
    fn report_png_is_compact_and_independent_of_windows_or_screenshots() {
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("plot.png");let mut export=GraphImageExport::default();
        export.save(path.clone(),&report()).unwrap();let image=image::open(&path).unwrap().to_rgba8();
        assert_eq!(image.width(),2000);assert!(image.height()<850 && image.height()>450);
        assert_eq!(image.get_pixel(0,0).0,[255;4]);assert!(image.pixels().any(|p|p.0[..3]==[0,105,165]));
        assert!(export.status.as_ref().unwrap().0);
        let before=std::fs::read(&path).unwrap();let mut invalid=report();invalid.lanes.clear();assert!(export.save(path.clone(),&invalid).is_err());assert_eq!(std::fs::read(path).unwrap(),before);
    }
    #[test]
    fn multi_cursor_rows_and_all_selected_lanes_grow_the_content_not_window_size() {
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("multi.png");let mut report=report();report.multi=true;
        for i in 0..6 {report.lanes[0].cursors.push(Cursor{label:format!("C{}",i+3),time:i as f64/4.0,value:format!("{i} V"),y:Some(i as f64)});}
        report.lanes.push(report.lanes[0].clone());render_png(&path,&report).unwrap();let dimensions=image::image_dimensions(&path).unwrap();assert!(dimensions.1>1000);
    }
}
