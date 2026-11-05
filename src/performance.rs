use crate::model::NormalizedFrame;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub struct TimeRangeIndex {
    pub by_frame: BTreeMap<String, Vec<usize>>,
}
impl TimeRangeIndex {
    pub fn build(frames:&[NormalizedFrame])->Self {
        let mut by_frame:BTreeMap<String,Vec<usize>>=BTreeMap::new();
        for (i,f) in frames.iter().enumerate() {
            by_frame.entry(f.frame_key.clone()).or_default().push(i);
        }
        Self{by_frame}
    }
    pub fn range<'a>(&self, frames:&'a [NormalizedFrame], key:&str, start:f64, end:f64)->Vec<&'a NormalizedFrame> {
        let Some(ix)=self.by_frame.get(key) else{return Vec::new();};
        let lo=ix.partition_point(|i| frames[*i].relative_time_s < start);
        let hi=ix.partition_point(|i| frames[*i].relative_time_s <= end);
        ix[lo..hi].iter().map(|i|&frames[*i]).collect()
    }
}

#[derive(Clone, Debug, Default)]
pub struct LodSeries {
    pub levels:Vec<Vec<[f64;2]>>,
}
impl LodSeries {
    pub fn build(points:&[[f64;2]], max_levels:usize)->Self {
        let mut levels=vec![points.to_vec()];
        while levels.len()<max_levels {
            let prev=levels.last().unwrap();
            if prev.len()<=2048 {break;}
            let mut next=Vec::with_capacity((prev.len()+3)/4*2);
            for chunk in prev.chunks(4) {
                if chunk.is_empty(){continue;}
                let mut min=chunk[0]; let mut max=chunk[0];
                for p in chunk.iter().skip(1) {
                    if p[1]<min[1]{min=*p;}
                    if p[1]>max[1]{max=*p;}
                }
                if min[0]<=max[0]{next.push(min); if max!=min{next.push(max);}}
                else {next.push(max); if max!=min{next.push(min);}}
            }
            levels.push(next);
        }
        Self{levels}
    }
    pub fn level_for_width(&self, pixel_width:f32)->&[[f64;2]] {
        let budget=(pixel_width.max(64.0) as usize)*2;
        for l in self.levels.iter().rev() {
            if l.len()<=budget {return l;}
        }
        self.levels.first().map(Vec::as_slice).unwrap_or(&[])
    }
}

#[derive(Clone, Debug, Default)]
pub struct PerformanceCounters {
    pub visible_frames:usize,
    pub visible_points:usize,
    pub lod_level:usize,
    pub cache_hits:u64,
    pub cache_misses:u64,
}
