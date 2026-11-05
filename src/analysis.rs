use crate::model::{BusType, NormalizedFrame};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Filter {
    #[serde(default)]
    pub hide_error_frames: bool,
    pub text: String,
    pub bus: Option<BusType>,
    pub channel: Option<u32>,
    pub id: Option<u32>,
    pub start_s: Option<f64>,
    pub end_s: Option<f64>,
}
impl Filter {
    pub fn matches(&self, f: &NormalizedFrame) -> bool {
        if !crate::protocols::frame_visible(f,self.hide_error_frames){return false;}
        if self.bus.is_some_and(|x| x != f.bus_type) { return false; }
        if self.channel.is_some_and(|x| x != f.channel_id) { return false; }
        if self.id.is_some_and(|x| Some(x) != f.identifier) { return false; }
        if self.start_s.is_some_and(|x| f.relative_time_s < x) { return false; }
        if self.end_s.is_some_and(|x| f.relative_time_s > x) { return false; }
        let q = self.text.trim().to_ascii_lowercase();
        q.is_empty()
            || f.frame_key.to_ascii_lowercase().contains(&q)
            || crate::model::frame_key_display(&f.frame_key).to_ascii_lowercase().contains(&q)
            || crate::model::channel_id_hex(f.channel_id).to_ascii_lowercase().contains(&q)
            || f.payload_hex().to_ascii_lowercase().contains(&q)
    }
    pub fn signature(&self) -> String {
        format!("{}|{:?}|{:?}|{:?}|{:?}|{:?}|hide_errors={}", self.text, self.bus, self.channel, self.id, self.start_s, self.end_s,self.hide_error_frames)
    }
}

#[derive(Clone, Default)]
pub struct Stats {
    pub total: usize,
    pub duration_s: f64,
    pub by_bus: BTreeMap<String, usize>,
    pub unique_keys: usize,
    pub channels: usize,
    pub bytes: usize,
}
pub fn statistics(frames: &[NormalizedFrame]) -> Stats {
    let mut s = Stats::default();
    s.total = frames.len();
    if let (Some(a), Some(b)) = (frames.first(), frames.last()) {
        s.duration_s = (b.timestamp_ns.saturating_sub(a.timestamp_ns)) as f64 / 1e9;
    }
    let mut keys = HashSet::new();
    let mut ch = HashSet::new();
    for f in frames {
        if let Some(meta)=f.analog.as_ref() {
            let count=meta.sample_count.min(f.payload.len()/2);
            if count>0 && meta.sample_period_ns>0 {
                let first=frames.first().map(|f|f.timestamp_ns).unwrap_or(f.timestamp_ns);
                let start=f.timestamp_ns.saturating_sub(first) as f64/1e9;
                s.duration_s=s.duration_s.max(start+(count-1) as f64*meta.sample_period_ns as f64/1e9);
            }
        }
        s.bytes += f.payload.len();
        *s.by_bus.entry(f.bus_type.to_string()).or_default() += 1;
        keys.insert(f.frame_key.clone());
        ch.insert(f.channel_id);
    }
    s.unique_keys = keys.len();
    s.channels = ch.len();
    s
}

#[derive(Clone, Debug)]
pub struct SignalCandidate {
    pub frame_key: String,
    pub byte: usize,
    pub bit: usize,
    pub bit_length: usize,
    pub transitions: usize,
    pub high_pct: f64,
    pub score: f64,
    pub likely_endian: String,
    pub likely_type: String,
    pub min_value: f64,
    pub max_value: f64,
}

fn candidate_unsigned_le(payload:&[u8], start_bit:usize, bit_length:usize)->Option<u64> {
    if bit_length==0 || bit_length>32 { return None; }
    let mut out=0u64;
    for k in 0..bit_length {
        let physical=start_bit+k;
        let byte=physical/8;
        let bit=physical%8;
        let v=*payload.get(byte)?;
        out|=(((v>>bit)&1) as u64)<<k;
    }
    Some(out)
}

fn candidate_unsigned_be(payload:&[u8], start_bit:usize, bit_length:usize)->Option<u64> {
    if bit_length==0 || bit_length>32 { return None; }
    // DBC-free candidate interpretation: contiguous physical bits, MSB first.
    let mut out=0u64;
    for k in 0..bit_length {
        let physical=start_bit+k;
        let byte=physical/8;
        let bit=7-(physical%8);
        let v=*payload.get(byte)?;
        out=(out<<1)|(((v>>bit)&1) as u64);
    }
    Some(out)
}

fn numeric_quality(values:&[u64])->f64 {
    if values.len()<3 { return 0.0; }
    let changes=values.windows(2).filter(|w|w[0]!=w[1]).count();
    if changes==0 { return 0.0; }
    let mut deltas:Vec<u64>=values.windows(2).map(|w|w[1].abs_diff(w[0])).filter(|d|*d>0).collect();
    if deltas.is_empty(){return 0.0;}
    deltas.sort_unstable();
    let med=deltas[deltas.len()/2] as f64;
    let span=values.iter().max().unwrap().saturating_sub(*values.iter().min().unwrap()) as f64;
    let smooth=if span<=0.0{0.0}else{1.0-(med/(span+1.0)).min(1.0)};
    let transition_density=changes as f64/(values.len()-1) as f64;
    (0.65*smooth+0.35*(1.0-(transition_density-0.35).abs().min(0.65)/0.65)).clamp(0.0,1.0)
}

fn automatic_payload_analysis_eligible(f:&NormalizedFrame)->bool {
    // TECMP Control/Status records are system/protocol-management traffic. Keep
    // them visible in Trace/Details, but never treat their payload as vehicle
    // signal bytes, counters, patterns or CRC candidates.
    if let Some(meta)=f.tecmp.as_ref() {
        if matches!(meta.message_type,0x00|0x01|0x02|0x04) { return false; }
    }
    if matches!(f.bus_type,BusType::Can|BusType::CanFd) &&
        (f.frame_key.contains(":ERROR:") || f.frame_key.contains(":RTR:") || f.frame_key.contains(":DECODE_ISSUE:")) {return false;}
    !(f.bus_type==BusType::Lin && (
        f.frame_key.starts_with("LIN:EVENT:") ||
        f.frame_key.starts_with("LIN:ERROR:") ||
        f.frame_key.starts_with("LIN:CONDITION:") ||
        f.frame_key.starts_with("LIN:DECODE_ISSUE:") ||
        f.frame_key.starts_with("LIN:TX:REMOVE_SLAVE_RESPONSE:")
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SignalDiscoveryScope { AllDigital, Can, CanFd, Lin, Ethernet, ReviewPdu, Unknown }

impl SignalDiscoveryScope {
    pub fn label(self)->&'static str { match self {
        Self::AllDigital=>"All Digital", Self::Can=>"CAN", Self::CanFd=>"CAN-FD", Self::Lin=>"LIN",
        Self::Ethernet=>"Ethernet", Self::ReviewPdu=>"Review PDU", Self::Unknown=>"Unknown",
    }}
    fn matches(self,f:&NormalizedFrame)->bool {
        if !automatic_payload_analysis_eligible(f){return false;}
        match self {
        Self::AllDigital=>matches!(f.bus_type,BusType::Can|BusType::CanFd|BusType::Lin|BusType::Ethernet|BusType::Tecmp|BusType::Unknown),
        Self::Can=>f.bus_type==BusType::Can, Self::CanFd=>f.bus_type==BusType::CanFd, Self::Lin=>f.bus_type==BusType::Lin,
        Self::Ethernet=>f.bus_type==BusType::Ethernet && !f.frame_key.starts_with("REVIEW:PDU:"),
        Self::ReviewPdu=>f.frame_key.starts_with("REVIEW:PDU:"),
        Self::Unknown=>matches!(f.bus_type,BusType::Tecmp|BusType::Unknown),
    }}
}

fn signed_value(raw:u64,bits:usize)->i64 {
    if bits==0 || bits>=64 { return raw as i64; }
    let sign=1u64<<(bits-1);
    if raw&sign!=0 {(raw as i128-(1i128<<bits)) as i64}else{raw as i64}
}
fn numeric_quality_i64(values:&[i64])->f64 {
    if values.len()<3{return 0.0}
    let changes=values.windows(2).filter(|w|w[0]!=w[1]).count();
    if changes==0{return 0.0}
    let mut ds:Vec<u64>=values.windows(2).map(|w|w[1].abs_diff(w[0])).filter(|d|*d>0).collect();
    if ds.is_empty(){return 0.0} ds.sort_unstable();
    let med=ds[ds.len()/2] as f64;
    let (mn,mx)=values.iter().fold((i64::MAX,i64::MIN),|(mn,mx),v|(mn.min(*v),mx.max(*v)));
    let span=(mx as i128-mn as i128).unsigned_abs() as f64;
    let smooth=if span<=0.0{0.0}else{1.0-(med/(span+1.0)).min(1.0)};
    let density=changes as f64/(values.len()-1) as f64;
    (0.68*smooth+0.32*(1.0-(density-0.35).abs().min(0.65)/0.65)).clamp(0.0,1.0)
}
fn candidate_housekeeping(g:&[&NormalizedFrame],byte:usize)->Option<&'static str>{
    let vals:Vec<u8>=g.iter().filter_map(|f|f.payload.get(byte).copied()).collect();
    if vals.len()<4{return None}
    if vals.windows(2).all(|w|w[0]==w[1]) {return Some("CONSTANT")}
    let n=vals.len()-1;
    let c8=vals.windows(2).filter(|w|w[1].wrapping_sub(w[0])==1).count() as f64/n as f64;
    let c4=vals.windows(2).filter(|w|(w[1]&15).wrapping_sub(w[0]&15)&15==1).count() as f64/n as f64;
    if c8>=0.82||c4>=0.88{return Some("COUNTER")}
    if detect_repeating_pattern(&vals,4,512).is_some(){return Some("PATTERN")}
    None
}
fn overlap(a:&SignalCandidate,b:&SignalCandidate)->f64{
    if a.frame_key!=b.frame_key{return 0.0}
    let as_=a.byte*8+a.bit; let ae=as_+a.bit_length; let bs=b.byte*8+b.bit; let be=bs+b.bit_length;
    let inter=ae.min(be).saturating_sub(as_.max(bs));
    if inter==0{0.0}else{inter as f64/a.bit_length.min(b.bit_length) as f64}
}

pub fn detect_signals(frames:&[NormalizedFrame],limit:usize)->Vec<SignalCandidate>{
    detect_signals_scoped_range(frames,limit,None,None,SignalDiscoveryScope::AllDigital)
}
pub fn detect_signals_range(frames:&[NormalizedFrame],limit:usize,start_s:Option<f64>,end_s:Option<f64>)->Vec<SignalCandidate>{
    detect_signals_scoped_range(frames,limit,start_s,end_s,SignalDiscoveryScope::AllDigital)
}
pub fn detect_lin_signals_range(frames:&[NormalizedFrame],limit:usize,start_s:Option<f64>,end_s:Option<f64>)->Vec<SignalCandidate>{
    detect_signals_scoped_range(frames,limit,start_s,end_s,SignalDiscoveryScope::Lin)
}
pub fn detect_signals_scoped_range(frames:&[NormalizedFrame],limit:usize,start_s:Option<f64>,end_s:Option<f64>,scope:SignalDiscoveryScope)->Vec<SignalCandidate>{
    let lo=start_s.map(|x|frames.partition_point(|f|f.relative_time_s<x)).unwrap_or(0);
    let hi=end_s.map(|x|frames.partition_point(|f|f.relative_time_s<=x)).unwrap_or(frames.len());
    let mut groups:HashMap<(String,u32,String),Vec<&NormalizedFrame>>=HashMap::new();
    for f in &frames[lo..hi] {
        if !scope.matches(f){continue}
        groups.entry((f.bus_type.to_string(),f.channel_id,f.frame_key.clone())).or_default().push(f);
    }
    let mut out=Vec::new();
    for ((bus,channel,frame_key),g) in groups {
        if g.len()<4{continue}
        let max_bytes=g.iter().map(|f|f.payload.len()).max().unwrap_or(0); if max_bytes==0{continue}
        let display_key=format!("{} / CH{} / {}",bus,channel,frame_key);
        let mut sigs=Vec::<Vec<bool>>::with_capacity(max_bytes*8);
        let mut occ=Vec::<f64>::with_capacity(max_bytes*8);
        for physical in 0..max_bytes*8 {
            let byte=physical/8;let bit=physical%8;let mut s=Vec::new();let mut prev=None;let mut high=0;let mut n=0;
            for f in &g {if let Some(v)=f.payload.get(byte).map(|v|(v>>bit)&1){if let Some(p)=prev{s.push(p!=v)}prev=Some(v);high+=v as usize;n+=1}}
            sigs.push(s);occ.push(if n==0{0.0}else{high as f64/n as f64});
        }
        let changing:Vec<bool>=sigs.iter().map(|s|s.iter().any(|x|*x)).collect();
        let mut p=0usize;
        while p<changing.len() {
            if !changing[p]{p+=1;continue}
            let start=p;let mut end=p+1;
            while end<changing.len()&&changing[end]&&end-start<32 {
                let aa=&sigs[end-1];let bb=&sigs[end];let n=aa.len().min(bb.len());if n==0{break}
                let agree=(0..n).filter(|&i|aa[i]==bb[i]).count() as f64/n as f64;
                // Carrying multi-byte values need not transition simultaneously. A lower threshold
                // creates candidate runs; numeric scoring below decides the actual field boundary.
                if agree<0.48{break} end+=1;
            }
            let run=end-start;
            let mut starts=vec![start];
            for off in 1..run.min(8){if changing[start+off]{starts.push(start+off)}}
            for fs in starts {
                let available=end-fs;
                let mut widths=vec![available];
                for w in [1usize,2,4,8,12,16,24,32]{if w<=available&&!widths.contains(&w){widths.push(w)}}
                for width in widths {
                    let first_byte=fs/8;let last_byte=(fs+width-1)/8;
                    let mut hk=None;
                    for b in first_byte..=last_byte {if let Some(k)=candidate_housekeeping(&g,b){hk=Some(k);break}}
                    if hk.is_some(){continue}
                    let le:Vec<u64>=g.iter().filter_map(|f|candidate_unsigned_le(&f.payload,fs,width)).collect();
                    let be:Vec<u64>=g.iter().filter_map(|f|candidate_unsigned_be(&f.payload,fs,width)).collect();
                    if le.len()<4||be.len()<4{continue}
                    let mut qle=numeric_quality(&le);let qbe=numeric_quality(&be);
                    if bus=="LIN"{qle=(qle+0.04).min(1.0)}
                    let (raw,endian,q)=if qbe>qle+0.06{(&be,"Big",qbe)}else{(&le,"Little",qle)};
                    let signed:Vec<i64>=raw.iter().map(|v|signed_value(*v,width)).collect();
                    let qs=numeric_quality_i64(&signed);
                    let use_signed=width>1&&qs>q+0.07;
                    let transitions=raw.windows(2).filter(|w|w[0]!=w[1]).count();if transitions==0{continue}
                    let o=occ[fs..fs+width].iter().sum::<f64>()/width as f64;
                    let balance=(1.0-(o-0.5).abs()*2.0).max(0.0);
                    let width_bonus=(width as f64/16.0).min(1.0);
                    let bestq=if use_signed{qs}else{q};
                    let score=(0.68*bestq+0.20*balance+0.12*width_bonus).clamp(0.0,1.0);
                    if score<0.35{continue}
                    let (mn,mx)=if use_signed{
                        (*signed.iter().min().unwrap() as f64,*signed.iter().max().unwrap() as f64)
                    }else{(*raw.iter().min().unwrap() as f64,*raw.iter().max().unwrap() as f64)};
                    out.push(SignalCandidate{frame_key:display_key.clone(),byte:fs/8,bit:fs%8,bit_length:width,
                        transitions,high_pct:o*100.0,score,likely_endian:endian.into(),
                        likely_type:if use_signed{"Signed".into()}else{"Unsigned".into()},min_value:mn,max_value:mx});
                }
            }
            p=end;
        }
    }
    out.sort_by(|a,b|b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal).then_with(||b.bit_length.cmp(&a.bit_length)));
    let mut kept=Vec::<SignalCandidate>::new();
    for c in out {
        // Non-maximum suppression: hide near-duplicate field interpretations while retaining
        // adjacent independent fields. A materially stronger alternative is already first.
        if kept.iter().any(|k|overlap(k,&c)>=0.80 && k.score>=c.score-0.03){continue}
        kept.push(c);if kept.len()>=limit{break}
    }
    kept
}

pub fn candidate_series(frames:&[NormalizedFrame],c:&SignalCandidate,start_s:Option<f64>,end_s:Option<f64>,limit:usize)->Vec<[f64;2]>{
    let mut parts=c.frame_key.splitn(3," / ");
    let bus=parts.next().unwrap_or(""); let chs=parts.next().unwrap_or(""); let fk=parts.next().unwrap_or("");
    let ch=chs.strip_prefix("CH").and_then(|x|x.parse::<u32>().ok());
    let physical=c.byte*8+c.bit;
    let mut out=Vec::new();
    for f in frames {
        if f.bus_type.to_string()!=bus||f.frame_key!=fk||ch.is_some_and(|x|x!=f.channel_id){continue}
        if start_s.is_some_and(|x|f.relative_time_s<x)||end_s.is_some_and(|x|f.relative_time_s>x){continue}
        let raw=if c.likely_endian.eq_ignore_ascii_case("Big"){candidate_unsigned_be(&f.payload,physical,c.bit_length)}else{candidate_unsigned_le(&f.payload,physical,c.bit_length)};
        if let Some(v)=raw {
            let y=if c.likely_type.eq_ignore_ascii_case("Signed"){signed_value(v,c.bit_length) as f64}else{v as f64};
            out.push([f.relative_time_s,y]); if out.len()>=limit{break}
        }
    }
    out
}

#[derive(Clone, Debug)]
pub struct ByteActivity {
    pub byte: usize,
    pub transitions: usize,
    pub unique_values: usize,
    pub min: u8,
    pub max: u8,
    pub activity_pct: f64,
}
pub fn byte_activity(frames: &[NormalizedFrame], key: &str, channel: Option<u32>) -> Vec<ByteActivity> {
    let g: Vec<&NormalizedFrame> = frames.iter().filter(|f| f.frame_key == key && channel.map(|c| c == f.channel_id).unwrap_or(true)).collect();
    let max_len = g.iter().map(|f| f.payload.len()).max().unwrap_or(0);
    let mut out = Vec::new();
    for byte in 0..max_len {
        let vals: Vec<u8> = g.iter().filter_map(|f| f.payload.get(byte).copied()).collect();
        if vals.is_empty() { continue; }
        let transitions = vals.windows(2).filter(|w| w[0] != w[1]).count();
        let unique_values = vals.iter().copied().collect::<HashSet<_>>().len();
        let min = *vals.iter().min().unwrap_or(&0);
        let max = *vals.iter().max().unwrap_or(&0);
        let activity_pct = 100.0 * transitions as f64 / vals.len().saturating_sub(1).max(1) as f64;
        out.push(ByteActivity { byte, transitions, unique_values, min, max, activity_pct });
    }
    out
}

pub fn byte_value_counts(frames: &[NormalizedFrame], key: &str, channel: Option<u32>, byte: usize) -> Vec<(u8, usize, f64)> {
    let vals: Vec<u8> = frames.iter().filter(|f| f.frame_key == key && channel.map(|c| c == f.channel_id).unwrap_or(true)).filter_map(|f| f.payload.get(byte).copied()).collect();
    let mut m: BTreeMap<u8, usize> = BTreeMap::new();
    for v in &vals { *m.entry(*v).or_default() += 1; }
    m.into_iter().map(|(v, n)| (v, n, if vals.is_empty() { 0.0 } else { 100.0 * n as f64 / vals.len() as f64 })).collect()
}

pub fn payload_change_events(frames: &[NormalizedFrame], key: &str, channel: Option<u32>, max_events: usize) -> Vec<(f64, String, String)> {
    let g: Vec<&NormalizedFrame> = frames.iter().filter(|f| f.frame_key == key && channel.map(|c| c == f.channel_id).unwrap_or(true)).collect();
    let mut out = Vec::new();
    for w in g.windows(2) {
        let (a, b) = (w[0], w[1]);
        let mut changes = Vec::new();
        let n = a.payload.len().max(b.payload.len());
        for i in 0..n {
            let before = a.payload.get(i).copied();
            let after = b.payload.get(i).copied();
            if before != after {
                changes.push(format!("B{}: {}->{}", i, before.map(|v| format!("{v:02X}")).unwrap_or_else(|| "--".into()), after.map(|v| format!("{v:02X}")).unwrap_or_else(|| "--".into())));
            }
        }
        if !changes.is_empty() {
            out.push((b.relative_time_s, b.payload_hex(), changes.join("; ")));
            if out.len() >= max_events { break; }
        }
    }
    out
}

pub fn bit_activity(frames: &[NormalizedFrame], key: &str, channel: Option<u32>) -> Vec<(usize, usize, usize, f64, f64)> {
    let g: Vec<&NormalizedFrame> = frames.iter().filter(|f| f.frame_key == key && channel.map(|c| c == f.channel_id).unwrap_or(true)).collect();
    let max = g.iter().map(|f| f.payload.len()).max().unwrap_or(0);
    let mut out = Vec::new();
    for byte in 0..max {
        for bit in 0..8 {
            let mut prev = None;
            let mut trans = 0usize;
            let mut high = 0usize;
            let mut n = 0usize;
            for f in &g {
                if let Some(v) = f.payload.get(byte) {
                    let b = (v >> bit) & 1;
                    high += b as usize;
                    n += 1;
                    if prev.is_some_and(|p| p != b) { trans += 1; }
                    prev = Some(b);
                }
            }
            if n > 0 {
                out.push((byte, bit, trans, 100.0 * trans as f64 / n.saturating_sub(1).max(1) as f64, high as f64 * 100.0 / n as f64));
            }
        }
    }
    out
}

pub fn byte_waveform(frames: &[NormalizedFrame], key: &str, byte: usize) -> Vec<[f64; 2]> {
    frames.iter().filter(|f| f.frame_key == key).filter_map(|f| f.payload.get(byte).map(|v| [f.relative_time_s, *v as f64])).collect()
}

pub fn bit_waveform(frames: &[NormalizedFrame], key: &str, byte: usize, bit: usize) -> Vec<[f64; 2]> {
    frames.iter().filter(|f| f.frame_key == key).filter_map(|f| f.payload.get(byte).map(|v| [f.relative_time_s, ((v >> bit) & 1) as f64])).collect()
}

pub fn downsample_minmax(points: &[[f64; 2]], max_points: usize) -> Vec<[f64; 2]> {
    if points.len() <= max_points || max_points < 4 { return points.to_vec(); }
    let buckets = (max_points / 2).max(1);
    let bucket_size = (points.len() as f64 / buckets as f64).ceil() as usize;
    let mut out = Vec::with_capacity(max_points + 2);
    for chunk in points.chunks(bucket_size) {
        if chunk.is_empty() { continue; }
        let mut min = chunk[0];
        let mut max = chunk[0];
        for p in chunk.iter().copied() {
            if p[1] < min[1] { min = p; }
            if p[1] > max[1] { max = p; }
        }
        if min[0] <= max[0] { out.push(min); if max != min { out.push(max); } }
        else { out.push(max); if max != min { out.push(min); } }
    }
    // Preserve held values at both viewport boundaries, even when neither is
    // a bucket extremum. These points also keep the last decoded state visible.
    for point in [points[0],*points.last().unwrap()] {
        if !out.contains(&point) {out.push(point);}
    }
    out.sort_by(|a, b| a[0].total_cmp(&b[0]));
    out
}

/// Convert a configuration endian label into the decoder byte-order flag.
///
/// The configuration UI accepts the automotive names as well as common aliases:
/// Intel / Little / LE -> little-endian, Motorola / Big / BE / Network -> big-endian.
/// Blank values retain the historic LogLens default of little-endian. Unknown labels
/// return `None` so Configuration validation can reject ambiguous definitions.
pub fn parse_little_endian(endian: &str) -> Option<bool> {
    let token = endian
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>();
    if token.is_empty() {
        return Some(true);
    }
    if token == "little"
        || token == "le"
        || token.contains("littleendian")
        || token.contains("intel")
    {
        return Some(true);
    }
    if token == "big"
        || token == "be"
        || token.contains("bigendian")
        || token.contains("motorola")
        || token == "network"
        || token.contains("networkbyteorder")
    {
        return Some(false);
    }
    None
}

pub fn endian_is_little(endian: &str) -> bool {
    parse_little_endian(endian).unwrap_or(true)
}

pub fn decode_signal(payload: &[u8], start_byte: usize, start_bit: usize, bit_length: usize, little_endian: bool, signed: bool, scale: f64, offset: f64) -> Option<f64> {
    // v0.47.9 uses one physical bit-numbering convention for every protocol and endian:
    // bit_position 0 is the byte LSB and bit_position 7 is the byte MSB.
    //
    // Intel/Little: start_bit is the least-significant signal bit and traversal is
    // 0 -> 7, then the next byte 0 -> 7.
    // Motorola/Big: start_bit is the most-significant signal bit and traversal is
    // 7 -> 0, then the next byte 7 -> 0. Therefore a byte-aligned 8/16/32-bit
    // Motorola signal starts at Bit 7, while an Intel signal starts at Bit 0.
    if bit_length == 0 || bit_length > 64 || start_bit > 7 { return None; }
    let mut raw = 0u64;
    if little_endian {
        for i in 0..bit_length {
            let pos = start_bit + i;
            let b = start_byte + pos / 8;
            let bit = pos % 8;
            let v = *payload.get(b)?;
            raw |= (((v >> bit) & 1) as u64) << i;
        }
    } else {
        let mut byte = start_byte;
        let mut bit = start_bit;
        for _ in 0..bit_length {
            let v = *payload.get(byte)?;
            raw = (raw << 1) | (((v >> bit) & 1) as u64);
            if bit == 0 {
                byte += 1;
                bit = 7;
            } else {
                bit -= 1;
            }
        }
    }
    let val = if signed {
        if bit_length == 64 {
            (raw as i64) as f64
        } else if (raw & (1u64 << (bit_length - 1))) != 0 {
            (raw as i128 - (1i128 << bit_length)) as f64
        } else {
            raw as f64
        }
    } else {
        raw as f64
    };
    Some(val * scale + offset)
}

/// Decode a configured signal using an explicit data type. Legacy integer
/// configurations continue to use the signed flag when data_type is empty.
pub fn decode_signal_typed(payload: &[u8], start_byte: usize, start_bit: usize, bit_length: usize, little_endian: bool, signed: bool, data_type: &str, scale: f64, offset: f64) -> Option<f64> {
    let kind=data_type.trim().to_ascii_lowercase();
    let aligned_start_bit=if little_endian { 0 } else { 7 };
    if kind=="float32" || kind=="float 32" || kind=="f32" {
        if bit_length!=32 || start_bit!=aligned_start_bit { return None; }
        let bytes=payload.get(start_byte..start_byte+4)?;
        let a:[u8;4]=bytes.try_into().ok()?;
        let raw=if little_endian { f32::from_le_bytes(a) } else { f32::from_be_bytes(a) } as f64;
        return Some(raw*scale+offset);
    }
    if kind=="float64" || kind=="float 64" || kind=="f64" || kind=="double" {
        if bit_length!=64 || start_bit!=aligned_start_bit { return None; }
        let bytes=payload.get(start_byte..start_byte+8)?;
        let a:[u8;8]=bytes.try_into().ok()?;
        let raw=if little_endian { f64::from_le_bytes(a) } else { f64::from_be_bytes(a) };
        return Some(raw*scale+offset);
    }
    let typed_signed=if kind.contains("unsigned") { false } else if kind.contains("signed") { true } else { signed };
    decode_signal(payload,start_byte,start_bit,bit_length,little_endian,typed_signed,scale,offset)
}

#[derive(Clone, Debug, Default)]
pub struct ScopedByteActivity {
    pub byte:usize,pub samples:usize,pub transitions:usize,pub unique_values:usize,
    pub min:u8,pub max:u8,pub first:u8,pub last:u8,pub activity_pct:f64,
    pub class:&'static str,pub confidence:f64,pub reason:String,
}

#[derive(Clone, Debug, Default)]
pub struct PatternDetection {
    pub period:usize,
    pub comparisons:usize,
    pub matches:usize,
    pub confidence:f64,
    pub kind:&'static str,
    pub cycles:usize,
    pub delta_confidence:f64,
}

/// Generic, byte-position-independent periodicity detector.
///
/// It searches both the value sequence and the first-difference sequence.  This catches
/// exact/near-exact repeated sequences, alternating/toggle data and ramp/reset waveforms.
/// The search is bounded (<=512 lags over <=4096 samples) so Ethernet/Review PDU analysis
/// stays interactive.  Period 1 is excluded because CONSTANT is classified separately.
pub fn detect_repeating_pattern(values:&[u8], min_cycles:usize, max_period:usize)->Option<PatternDetection>{
    if min_cycles<2 || values.len()<min_cycles*2 { return None; }
    let max_p=(values.len()/min_cycles).min(max_period.max(2)).min(512);
    let mut best:Option<PatternDetection>=None;
    for p in 2..=max_p {
        let cycles=values.len()/p;
        if cycles<min_cycles { continue; }
        let comparisons=values.len()-p;
        if comparisons<12 { continue; }

        let matches=(p..values.len()).filter(|&i|values[i]==values[i-p]).count();
        let value_conf=matches as f64/comparisons as f64;

        // Compare modular deltas at the same phase. A ramp/reset pattern can remain highly
        // periodic even with an occasional absolute-value disturbance or capture anomaly.
        let mut delta_matches=0usize;
        let mut delta_n=0usize;
        if values.len()>p+1 {
            for i in (p+1)..values.len() {
                let d=values[i].wrapping_sub(values[i-1]);
                let dp=values[i-p].wrapping_sub(values[i-p-1]);
                delta_n+=1;
                if d==dp { delta_matches+=1; }
            }
        }
        let delta_conf=if delta_n>0{delta_matches as f64/delta_n as f64}else{0.0};

        // Require strong structural evidence. Value repetition is primary; delta repetition
        // permits a little more noise but must still be very stable over >=4 cycles.
        let confidence=value_conf.max(delta_conf*0.985);
        if value_conf<0.90 && delta_conf<0.94 { continue; }
        if cycles<4 && confidence<0.98 { continue; }

        let phase=&values[..p];
        let unique=phase.iter().copied().collect::<std::collections::HashSet<_>>().len();
        let deltas=phase.windows(2).map(|w|w[1].wrapping_sub(w[0])).collect::<Vec<_>>();
        let same_delta=deltas.first().map(|d|deltas.iter().all(|x|x==d)).unwrap_or(false);
        let kind=if unique==2 && p<=4 {"Alternating / Toggle"}
            else if same_delta && !deltas.is_empty() {"Ramp / Reset"}
            else if delta_conf>value_conf+0.02 {"Delta Sequence"}
            else {"Repeating Sequence"};

        let candidate=PatternDetection{period:p,comparisons,matches,confidence,kind,cycles,delta_confidence:delta_conf};
        let replace=best.as_ref().map(|b|
            confidence>b.confidence+0.005 ||
            ((confidence-b.confidence).abs()<=0.005 && p<b.period)
        ).unwrap_or(true);
        if replace { best=Some(candidate); }
    }
    best
}

fn crc8_normal_skip(payload:&[u8],skip:usize,poly:u8,init:u8,xorout:u8)->u8{
    let mut crc=init;
    for (i,&b) in payload.iter().enumerate(){
        if i==skip { continue; }
        crc^=b;
        for _ in 0..8 { crc=if crc&0x80!=0 {(crc<<1)^poly}else{crc<<1}; }
    }
    crc^xorout
}

fn checksum8_skip(payload:&[u8],skip:usize)->(u8,u8,u8){
    let mut xor=0u8; let mut sum=0u8;
    for (i,&b) in payload.iter().enumerate(){ if i!=skip { xor^=b; sum=sum.wrapping_add(b); } }
    (xor,sum,!sum)
}

/// Try a compact set of common 8-bit checksum/CRC algorithms against a candidate
/// byte. This is evidence only: message IDs/Data IDs and OEM-specific CRC setup are
/// not available here, so results remain CRC_CANDIDATE rather than "proven CRC".
fn common_crc8_candidate(frames:&[NormalizedFrame],indices:&[usize],byte:usize)->Option<(&'static str,f64,usize)>{
    let mut counts=[0usize;6];
    let mut n=0usize;
    for &i in indices.iter().take(384) {
        let Some(f)=frames.get(i) else{continue};
        if f.payload.len()<2 || byte>=f.payload.len(){continue;}
        let actual=f.payload[byte];
        let (xor,sum,sum_inv)=checksum8_skip(&f.payload,byte);
        let predicted=[
            xor,
            sum,
            sum_inv,
            crc8_normal_skip(&f.payload,byte,0x07,0x00,0x00),      // CRC-8
            crc8_normal_skip(&f.payload,byte,0x1D,0xFF,0xFF),      // SAE J1850 style
            crc8_normal_skip(&f.payload,byte,0x2F,0xFF,0xFF),      // AUTOSAR style
        ];
        for j in 0..counts.len(){if predicted[j]==actual{counts[j]+=1;}}
        n+=1;
    }
    if n<16{return None;}
    let (idx,best)=counts.iter().copied().enumerate().max_by_key(|(_,c)|*c)?;
    let confidence=best as f64/n as f64;
    if confidence<0.90{return None;}
    let name=match idx{0=>"XOR8",1=>"SUM8",2=>"SUM8_INV",3=>"CRC8_0x07",4=>"CRC8_SAE_J1850",_=>"CRC8_AUTOSAR"};
    Some((name,confidence,n))
}

/// Find the useful testcase interval without deleting any raw frames.
///
/// Sources are isolated by bus + channel + frame key. Byte classification is learned from
/// the complete selected-channel dataset first; housekeeping bytes classified as STATIC,
/// COUNTER, CRC/checksum candidate or stable PATTERN do not extend the activity window.
/// The first/last transition of the remaining ACTIVE bytes defines the useful interval.
pub fn auto_activity_range(frames:&[NormalizedFrame], guard_s:f64)->Option<(f64,f64)> {
    if frames.len()<2 { return None; }

    let mut groups:BTreeMap<(String,u32,String),Vec<usize>>=BTreeMap::new();
    for (i,f) in frames.iter().enumerate() {
        if automatic_payload_analysis_eligible(f) && matches!(f.bus_type, BusType::Can|BusType::CanFd|BusType::Lin|BusType::Ethernet|BusType::Tecmp|BusType::Unknown) {
            groups.entry((f.bus_type.to_string(),f.channel_id,f.frame_key.clone())).or_default().push(i);
        }
    }

    let mut first:Option<f64>=None;
    let mut last:Option<f64>=None;

    for (_source,indices) in groups {
        if indices.len()<2 { continue; }
        let classes=byte_activity_indexed(frames,&indices,None,None,None);
        let active:HashSet<usize>=classes.iter()
            .filter(|r| r.class=="ACTIVE")
            .map(|r|r.byte).collect();
        if active.is_empty() { continue; }

        let mut prev=indices[0];
        for &idx in indices.iter().skip(1) {
            let Some(a)=frames.get(prev) else { prev=idx; continue; };
            let Some(b)=frames.get(idx) else { prev=idx; continue; };
            let changed=active.iter().any(|&byte| a.payload.get(byte)!=b.payload.get(byte));
            if changed {
                let t=b.relative_time_s;
                first=Some(first.map(|x|x.min(t)).unwrap_or(t));
                last=Some(last.map(|x|x.max(t)).unwrap_or(t));
            }
            prev=idx;
        }
    }

    let (first,last)=(first?,last?);
    let full_end=frames.last().map(|f|f.relative_time_s).unwrap_or(last).max(0.0);
    let start=(first-guard_s.max(0.0)).max(0.0);
    let end=(last+guard_s.max(0.0)).min(full_end);
    if end>start { Some((start,end)) } else { None }
}

pub fn friendly_byte_class(class:&str)->&'static str {
    match class {
        "STATIC"=>"CONSTANT",
        "COUNTER4"=>"ALIVE COUNTER (4-bit)",
        "COUNTER8"=>"ALIVE COUNTER (8-bit)",
        "CRC_CANDIDATE"=>"CRC / CHECKSUM CANDIDATE",
        "PATTERN"=>"REPEATING PATTERN",
        "ACTIVE"=>"SIGNAL ACTIVITY",
        _=>"NO DATA",
    }
}

pub fn byte_activity_indexed(frames:&[NormalizedFrame],indices:&[usize],channel:Option<u32>,start_s:Option<f64>,end_s:Option<f64>)->Vec<ScopedByteActivity>{
    #[derive(Clone)] struct Acc{
        samples:usize,transitions:usize,seen:[bool;256],unique:usize,min:u8,max:u8,first:u8,last:u8,prev:Option<u8>,
        plus1:usize,nibble_plus1:usize,high_nibble_plus1:usize,forward8:usize,forward4:usize,high_forward4:usize,
        pattern_sample:Vec<u8>,
    }
    let mut acc:Vec<Acc>=Vec::new();
    let mut crc_sample_indices=Vec::<usize>::new();
    for &i in indices{
        let Some(f)=frames.get(i) else{continue};
        if channel.is_some_and(|c|c!=f.channel_id){continue}
        if start_s.is_some_and(|x|f.relative_time_s<x)||end_s.is_some_and(|x|f.relative_time_s>x){continue}
        if crc_sample_indices.len()<384 { crc_sample_indices.push(i); }
        if acc.len()<f.payload.len(){acc.resize_with(f.payload.len(),||Acc{
            samples:0,transitions:0,seen:[false;256],unique:0,min:255,max:0,first:0,last:0,prev:None,
            plus1:0,nibble_plus1:0,high_nibble_plus1:0,forward8:0,forward4:0,high_forward4:0,pattern_sample:Vec::new()
        });}
        for(b,&v)in f.payload.iter().enumerate(){
            let x=&mut acc[b];
            if x.samples==0{x.first=v;x.min=v;x.max=v}
            if let Some(p)=x.prev{
                if p!=v{x.transitions+=1}
                let d8=v.wrapping_sub(p);
                let d4=(v&0x0f).wrapping_sub(p&0x0f)&0x0f;
                let dh=((v>>4).wrapping_sub(p>>4))&0x0f;
                if d8==1{x.plus1+=1}
                if d4==1{x.nibble_plus1+=1}
                if dh==1{x.high_nibble_plus1+=1}
                // A capture can miss one or two cyclic messages. Treat a small positive modular
                // jump as counter continuity instead of rejecting the alive counter.
                if (1..=3).contains(&d8){x.forward8+=1}
                if (1..=3).contains(&d4){x.forward4+=1}
                if (1..=3).contains(&dh){x.high_forward4+=1}
            }
            if !x.seen[v as usize]{x.seen[v as usize]=true;x.unique+=1}
            x.min=x.min.min(v);x.max=x.max.max(v);x.last=v;x.prev=Some(v);x.samples+=1;
            if x.pattern_sample.len()<4096{x.pattern_sample.push(v);}
        }
    }

    let pattern_samples=acc.iter().map(|x|x.pattern_sample.clone()).collect::<Vec<_>>();
    let mut rows:Vec<ScopedByteActivity>=acc.into_iter().enumerate().filter(|(_,x)|x.samples>0).map(|(byte,x)|{
        let denom=x.samples.saturating_sub(1).max(1) as f64;
        let activity=100.0*x.transitions as f64/denom;
        let exact8=x.plus1 as f64/denom;
        let exact4=x.nibble_plus1 as f64/denom;
        let exact4_hi=x.high_nibble_plus1 as f64/denom;
        let tolerant8=x.forward8 as f64/denom;
        let tolerant4=x.forward4 as f64/denom;
        let tolerant4_hi=x.high_forward4 as f64/denom;

        // Protocol-independent counter classifier. This runs on normalized payloads, so the
        // same logic applies to CAN, CAN-FD, LIN, Ethernet, TECMP and Unknown payload sources.
        let (class,confidence,reason)=if x.transitions==0{
            ("STATIC",1.0,format!("No value change across {} samples",x.samples))
        }else if exact8>=0.88 && tolerant8>=0.92 && x.unique>=8{
            ("COUNTER8",((exact8*0.65+tolerant8*0.35).min(0.999)),
             format!("8-bit rolling sequence: {:.1}% exact +1, {:.1}% including small capture gaps",100.0*exact8,100.0*tolerant8))
        }else if exact4>=0.88 && tolerant4>=0.92 && x.unique>=8{
            ("COUNTER4",((exact4*0.65+tolerant4*0.35).min(0.999)),
             format!("Low-nibble 4-bit rolling sequence: {:.1}% exact +1, {:.1}% including small capture gaps",100.0*exact4,100.0*tolerant4))
        }else if exact4_hi>=0.88 && tolerant4_hi>=0.92 && x.unique>=8{
            ("COUNTER4",((exact4_hi*0.65+tolerant4_hi*0.35).min(0.999)),
             format!("High-nibble 4-bit rolling sequence: {:.1}% exact +1, {:.1}% including small capture gaps",100.0*exact4_hi,100.0*tolerant4_hi))
        }else{
            ("ACTIVE",1.0,format!("Changing data: {:.1}% activity, {} unique values",activity,x.unique))
        };
        ScopedByteActivity{byte,samples:x.samples,transitions:x.transitions,unique_values:x.unique,min:x.min,max:x.max,first:x.first,last:x.last,activity_pct:activity,class,confidence,reason}
    }).collect();

    // Automotive E2E layout prior: a verified counter near a rapidly changing byte makes
    // that neighbor a strong CRC/checksum candidate, regardless of the normalized protocol.
    let counter_bytes=rows.iter().filter(|r|(r.class=="COUNTER4"||r.class=="COUNTER8")&&r.confidence>=0.88).map(|r|r.byte).collect::<Vec<_>>();
    for &counter_byte in &counter_bytes {
        for candidate in counter_byte.saturating_sub(2)..=(counter_byte+2).min(rows.len().saturating_sub(1)) {
            if candidate==counter_byte{continue;}
            if let Some(r)=rows.get_mut(candidate){
                if r.class=="ACTIVE" && r.samples>=24 && r.activity_pct>=85.0 && r.unique_values>=12 {
                    let diversity=(r.unique_values as f64/64.0).min(1.0);
                    r.class="CRC_CANDIDATE";
                    r.confidence=(0.70+0.18*(r.activity_pct/100.0)+0.10*diversity).min(0.96);
                    r.reason=format!("High-activity byte near verified counter B{}; strong CRC/checksum layout candidate",counter_byte);
                }
            }
        }
    }

    // Exact/common-algorithm verification on a compact candidate set: first/last bytes,
    // counter neighbors, plus the strongest high-diversity active bytes. This keeps long
    // Ethernet payload analysis bounded while still covering typical automotive layouts.
    let mut crc_candidates=HashSet::<usize>::new();
    let nbytes=rows.len();
    for b in 0..nbytes.min(4){crc_candidates.insert(b);}
    for b in nbytes.saturating_sub(4)..nbytes{crc_candidates.insert(b);}
    for &c in &counter_bytes{
        for b in c.saturating_sub(2)..=(c+2).min(nbytes.saturating_sub(1)){crc_candidates.insert(b);}
    }
    let mut ranked=rows.iter().filter(|r|r.class=="ACTIVE"&&r.samples>=16&&r.unique_values>=8)
        .map(|r|(r.byte,r.activity_pct*(r.unique_values as f64).ln_1p())).collect::<Vec<_>>();
    ranked.sort_by(|a,b|b.1.total_cmp(&a.1));
    for (b,_) in ranked.into_iter().take(8){crc_candidates.insert(b);}
    for b in crc_candidates{
        let Some(r)=rows.get_mut(b) else{continue};
        if r.class!="ACTIVE"{continue;}
        if let Some((algorithm,score,n))=common_crc8_candidate(frames,&crc_sample_indices,b){
            r.class="CRC_CANDIDATE";
            r.confidence=(0.80+0.20*score).min(0.995);
            r.reason=format!("{} matches candidate byte in {:.1}% of {} sampled frames; CRC/checksum candidate",algorithm,score*100.0,n);
        }
    }

    // Stable short-period pattern classification. Counter and CRC decisions have priority.
    for r in &mut rows {
        if r.class!="ACTIVE"{continue;}
        if let Some(p)=pattern_samples.get(r.byte).and_then(|v|detect_repeating_pattern(v,4,512)){
            r.class="PATTERN";
            r.confidence=p.confidence.min(0.999);
            r.reason=format!("{}: period {} frames, {} cycles, {:.1}% confidence (value matches {}/{}, delta {:.1}%)",p.kind,p.period,p.cycles,p.confidence*100.0,p.matches,p.comparisons,p.delta_confidence*100.0);
        }
    }

    // Generic CRC/checksum fallback for layouts not matched by a common algorithm.
    for r in &mut rows {
        if r.class=="ACTIVE" && r.samples>=64 && r.activity_pct>=98.0 && r.unique_values>=64 {
            r.class="CRC_CANDIDATE";
            r.confidence=(0.70+0.20*(r.activity_pct/100.0)+0.10*(r.unique_values as f64/256.0)).min(0.94);
            r.reason=format!("High-change/high-diversity byte: {:.1}% activity, {} unique values; CRC/checksum candidate, algorithm not proven",r.activity_pct,r.unique_values);
        }
    }
    rows
}


#[derive(Clone, Debug, Default)]
pub struct TimingAnalysis {
    pub samples:usize,
    pub mean_ms:f64,
    pub min_ms:f64,
    pub max_ms:f64,
    pub median_ms:f64,
    pub jitter_ms:f64,
    pub late_frames:usize,
    pub gap_events:Vec<(f64,f64)>,
}
pub fn timing_analysis(frames:&[NormalizedFrame],key:&str,channel:Option<u32>)->TimingAnalysis{
    let g=frames.iter().filter(|f|f.frame_key==key&&channel.map(|c|c==f.channel_id).unwrap_or(true)).collect::<Vec<_>>();
    if g.len()<2{return TimingAnalysis{samples:g.len(),..Default::default()};}
    let mut dt=g.windows(2).map(|w|(w[1].timestamp_ns.saturating_sub(w[0].timestamp_ns))as f64/1e6).collect::<Vec<_>>();
    let mean=dt.iter().sum::<f64>()/dt.len()as f64;
    let min=dt.iter().copied().fold(f64::INFINITY,f64::min);
    let max=dt.iter().copied().fold(f64::NEG_INFINITY,f64::max);
    let mut sorted=dt.clone(); sorted.sort_by(|a,b|a.total_cmp(b));
    let median=sorted[sorted.len()/2];
    let threshold=(median*1.5).max(median+0.1);
    let mut gaps=Vec::new();
    for (i,v) in dt.iter().enumerate(){if *v>threshold{gaps.push((g[i+1].relative_time_s,*v));}}
    TimingAnalysis{samples:g.len(),mean_ms:mean,min_ms:min,max_ms:max,median_ms:median,jitter_ms:max-min,late_frames:gaps.len(),gap_events:gaps}
}

#[derive(Clone, Debug, Default)]
pub struct CounterIntegrity {
    pub byte:usize,
    pub width:u8,
    pub samples:usize,
    pub valid_transitions:usize,
    pub discontinuities:Vec<(f64,u8,u8)>,
    pub confidence:f64,
}
pub fn counter_integrity(frames:&[NormalizedFrame],key:&str,channel:Option<u32>,byte:usize,width:u8)->CounterIntegrity{
    counter_integrity_range(frames,key,channel,byte,width,0,None,None)
}

pub fn counter_integrity_range(frames:&[NormalizedFrame],key:&str,channel:Option<u32>,byte:usize,width:u8,shift:u8,start_s:Option<f64>,end_s:Option<f64>)->CounterIntegrity{
    let vals=frames.iter().filter(|f|f.frame_key==key&&channel.map(|c|c==f.channel_id).unwrap_or(true))
        .filter(|f|!start_s.is_some_and(|x|f.relative_time_s<x)&&!end_s.is_some_and(|x|f.relative_time_s>x))
        .filter_map(|f|f.payload.get(byte).map(|v|(f.relative_time_s,*v))).collect::<Vec<_>>();
    let mask=if width==4{0x0f}else{0xff};
    let mut valid=0usize; let mut bad=Vec::new();
    for w in vals.windows(2){
        let a=(w[0].1>>shift)&mask; let b=(w[1].1>>shift)&mask;
        let expected=if width==4{(a+1)&0x0f}else{a.wrapping_add(1)};
        if b==expected{valid+=1}else{bad.push((w[1].0,a,b));}
    }
    let denom=vals.len().saturating_sub(1).max(1);
    CounterIntegrity{byte,width,samples:vals.len(),valid_transitions:valid,confidence:valid as f64/denom as f64,discontinuities:bad}
}

pub fn ab_byte_compare(frames:&[NormalizedFrame],key:&str,channel:Option<u32>,a:f64,b:f64)->Vec<(usize,Option<u8>,Option<u8>)>{
    let mut fa=None; let mut fb=None;
    // Frames are time-ordered, so scan until both cursors are past; do not stop at the
    // earlier cursor (A may be placed after B).
    let last=a.max(b);
    for f in frames.iter().filter(|f|f.frame_key==key&&channel.map(|c|c==f.channel_id).unwrap_or(true)){
        if f.relative_time_s>last{break;}
        if f.relative_time_s<=a{fa=Some(f);}
        if f.relative_time_s<=b{fb=Some(f);}
    }
    let n=fa.map(|f|f.payload.len()).unwrap_or(0).max(fb.map(|f|f.payload.len()).unwrap_or(0));
    (0..n).map(|i|(i,fa.and_then(|f|f.payload.get(i).copied()),fb.and_then(|f|f.payload.get(i).copied()))).collect()
}


#[cfg(test)]
mod pattern_detection_regression_tests {
    use super::detect_repeating_pattern;

    #[test]
    fn detects_long_repeating_sequence_independent_of_byte_position() {
        let cycle=(0..73).map(|i|((i*17+3)%251) as u8).collect::<Vec<_>>();
        let mut v=Vec::new(); for _ in 0..8 { v.extend_from_slice(&cycle); }
        let p=detect_repeating_pattern(&v,4,512).expect("long pattern");
        assert_eq!(p.period,73);
        assert!(p.confidence>0.99);
    }

    #[test]
    fn detects_ramp_reset() {
        let mut v=Vec::new(); for _ in 0..20 { v.extend(0u8..16u8); }
        let p=detect_repeating_pattern(&v,4,512).expect("ramp/reset");
        assert_eq!(p.period,16);
        assert!(p.kind.contains("Ramp") || p.kind.contains("Repeating"));
    }

    #[test]
    fn tolerates_sparse_anomalies_but_rejects_nonperiodic_data() {
        let mut v=Vec::new(); for _ in 0..12 { v.extend_from_slice(&[3,9,27,81,7,21,63,189]); }
        v[35]^=0x55; v[70]^=0x11;
        assert!(detect_repeating_pattern(&v,4,512).is_some());
        let randomish=(0..300).map(|i|(((i*73)^(i*i*19)+(i>>1))&255) as u8).collect::<Vec<_>>();
        assert!(detect_repeating_pattern(&randomish,4,128).is_none());
    }
}

#[cfg(test)]
mod signal_decode_regression_tests {
    use super::{decode_signal, decode_signal_typed, endian_is_little, parse_little_endian};

    #[test]
    fn two_zero_bytes_decode_to_zero_little_endian() {
        assert_eq!(decode_signal(&[0x00, 0x00], 0, 0, 16, true, false, 1.0, 0.0), Some(0.0));
    }

    #[test]
    fn two_zero_bytes_decode_to_zero_big_endian() {
        assert_eq!(decode_signal(&[0x00, 0x00], 0, 7, 16, false, false, 1.0, 0.0), Some(0.0));
    }

    #[test]
    fn adjacent_byte_is_not_part_of_byte0_byte1_little_signal() {
        // Byte 2 deliberately contains 0x01. A 16-bit Intel signal starting at
        // Byte 0 / Bit 0 uses only Byte 0 and Byte 1.
        assert_eq!(decode_signal(&[0x00, 0x00, 0x01], 0, 0, 16, true, false, 1.0, 0.0), Some(0.0));
    }

    #[test]
    fn adjacent_byte_is_not_part_of_byte0_byte1_big_signal() {
        // A byte-aligned 16-bit Motorola signal starts at Byte 0 / physical Bit 7.
        assert_eq!(decode_signal(&[0x00, 0x00, 0x01], 0, 7, 16, false, false, 1.0, 0.0), Some(0.0));
    }

    #[test]
    fn known_little_endian_values_are_stable() {
        assert_eq!(decode_signal(&[0x01, 0x00], 0, 0, 16, true, false, 1.0, 0.0), Some(1.0));
        assert_eq!(decode_signal(&[0x00, 0x01], 0, 0, 16, true, false, 1.0, 0.0), Some(256.0));
    }

    #[test]
    fn full_byte_uses_physical_start_bit_for_each_endian() {
        assert_eq!(decode_signal(&[0xA5], 0, 0, 8, true, false, 1.0, 0.0), Some(165.0));
        assert_eq!(decode_signal(&[0xA5], 0, 7, 8, false, false, 1.0, 0.0), Some(165.0));
    }

    #[test]
    fn aligned_16_and_32_bit_endian_values() {
        assert_eq!(decode_signal(&[0x12, 0x34], 0, 0, 16, true, false, 1.0, 0.0), Some(0x3412 as f64));
        assert_eq!(decode_signal(&[0x12, 0x34], 0, 7, 16, false, false, 1.0, 0.0), Some(0x1234 as f64));
        assert_eq!(decode_signal(&[0x01, 0x02, 0x03, 0x04], 0, 0, 32, true, false, 1.0, 0.0), Some(0x04030201u32 as f64));
        assert_eq!(decode_signal(&[0x01, 0x02, 0x03, 0x04], 0, 7, 32, false, false, 1.0, 0.0), Some(0x01020304u32 as f64));
    }

    #[test]
    fn unaligned_motorola_crosses_to_next_byte_msb() {
        // Byte0 bits 3..0 = 1111; Byte1 bits 7..0 = 1010_0101 -> 0xFA5.
        assert_eq!(decode_signal(&[0x0F, 0xA5], 0, 3, 12, false, false, 1.0, 0.0), Some(0xFA5 as f64));
    }

    #[test]
    fn endian_aliases_are_supported() {
        for e in ["Little", "Little Endian", "LE", "Intel", "Intel / Little"] {
            assert!(endian_is_little(e), "{e}");
        }
        for e in ["Big", "Big Endian", "BE", "Motorola", "Motorola / Big", "Network Byte Order"] {
            assert!(!endian_is_little(e), "{e}");
        }
    }

    #[test]
    fn unknown_endian_label_is_rejected_by_parser() {
        assert_eq!(parse_little_endian("mystery"), None);
    }

    #[test]
    fn big_float_requires_physical_msb_start() {
        let one=1.0f32.to_be_bytes();
        assert_eq!(decode_signal_typed(&one, 0, 7, 32, false, false, "Float32", 1.0, 0.0), Some(1.0));
        assert_eq!(decode_signal_typed(&one, 0, 0, 32, false, false, "Float32", 1.0, 0.0), None);
    }

    #[test]
    fn signed_64_bit_is_sign_extended() {
        assert_eq!(decode_signal(&[0xFF; 8], 0, 0, 64, true, true, 1.0, 0.0), Some(-1.0));
        assert_eq!(decode_signal(&[0xFF; 8], 0, 7, 64, false, true, 1.0, 0.0), Some(-1.0));
    }

    #[test]
    fn invalid_intra_byte_bit_position_is_rejected() {
        assert_eq!(decode_signal(&[0x00, 0x00, 0x01], 0, 8, 16, true, false, 1.0, 0.0), None);
        assert_eq!(decode_signal_typed(&[0x00, 0x00, 0x01], 0, 8, 16, true, false, "Unsigned Integer", 1.0, 0.0), None);
    }
}

#[cfg(test)] mod tecmp_eligibility_tests {
    use super::*;
    #[test] fn errors_remote_and_invalid_can_records_are_excluded(){
        let base=crate::protocols::parse_socketcan(&[0,0,1,0x23,1,0,0,0,0xaa,0,0,0,0,0,0,0],1,1).unwrap();
        for bus in [BusType::Can,BusType::CanFd]{
            for key in ["CAN:ERROR:FLAGS_0x0008:CH1","CAN:RTR:0x123","CANFD:DECODE_ISSUE:BAD_LENGTH_9:0x123"]{
                let mut f=base.clone();f.bus_type=bus;f.frame_key=key.into();assert!(!automatic_payload_analysis_eligible(&f));
            }
        }
        assert!(automatic_payload_analysis_eligible(&base));
    }
}
