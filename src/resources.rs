use std::{fs, path::Path};

const GIB: u64 = 1024 * 1024 * 1024;
const MIB: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub struct ResourceLimits {
    pub total_memory_bytes: Option<u64>,
    pub available_memory_bytes: Option<u64>,
    pub packet_limit: usize,
    pub native_capture_file_limit: u64,
    pub xlsx_file_limit: u64,
    pub analog_point_limit: usize,
    pub waveform_cache_entries: usize,
}

impl ResourceLimits {
    // Account for decoded payloads and index overhead rather than assuming every
    // capture record consumes the same memory. Other readers retain row limits.
    pub fn capture_memory_budget(&self)->usize {
        let total=self.total_memory_bytes.unwrap_or(8*GIB);
        let budget=total.saturating_mul(2)/5;
        let budget=self.available_memory_bytes.map(|available|budget.min(available.saturating_mul(3)/4)).unwrap_or(budget);
        budget.min(usize::MAX as u64) as usize
    }

    pub fn effective_packet_limit(&self, requested: usize) -> usize {
        requested.min(self.packet_limit)
    }

    pub fn memory_label(&self) -> String {
        match (self.total_memory_bytes, self.available_memory_bytes) {
            (Some(t), Some(a)) => format!("{:.1} GiB RAM, {:.1} GiB available", t as f64 / GIB as f64, a as f64 / GIB as f64),
            (Some(t), None) => format!("{:.1} GiB RAM", t as f64 / GIB as f64),
            _ => "RAM size unavailable".into(),
        }
    }
}

pub fn limits() -> ResourceLimits {
    let (total, available) = memory_status();
    let total_gib = total.unwrap_or(8 * GIB);

    let (mut packet_limit, mut native_limit, mut xlsx_limit, mut analog_points, mut cache_entries) = if total_gib <= 10 * GIB {
        (750_000usize, 192 * MIB, 256 * MIB, 350_000usize, 96usize)
    } else if total_gib <= 20 * GIB {
        (1_500_000usize, 384 * MIB, 512 * MIB, 750_000usize, 160usize)
    } else {
        (2_000_000usize, 768 * MIB, 1024 * MIB, 1_500_000usize, 256usize)
    };

    // Available RAM matters more than installed RAM when Windows is already under pressure.
    if let Some(a) = available {
        if a < 1536 * MIB {
            packet_limit = packet_limit.min(250_000);
            native_limit = native_limit.min(64 * MIB);
            xlsx_limit = xlsx_limit.min(128 * MIB);
            analog_points = analog_points.min(150_000);
            cache_entries = cache_entries.min(48);
        } else if a < 3 * GIB {
            packet_limit = packet_limit.min(500_000);
            native_limit = native_limit.min(128 * MIB);
            xlsx_limit = xlsx_limit.min(192 * MIB);
            analog_points = analog_points.min(250_000);
            cache_entries = cache_entries.min(72);
        }
    }

    ResourceLimits {
        total_memory_bytes: total,
        available_memory_bytes: available,
        packet_limit,
        native_capture_file_limit: native_limit,
        xlsx_file_limit: xlsx_limit,
        analog_point_limit: analog_points,
        waveform_cache_entries: cache_entries,
    }
}

pub fn should_use_native_capture(path: &Path) -> bool {
    let lim = limits();
    fs::metadata(path).map(|m| m.len() <= lim.native_capture_file_limit).unwrap_or(false)
}

pub fn validate_xlsx_size(path: &Path) -> anyhow::Result<()> {
    let lim = limits();
    let size = fs::metadata(path)?.len();
    if size > lim.xlsx_file_limit {
        anyhow::bail!(
            "Spreadsheet is {:.1} MiB, above the safe {:.1} MiB limit for the currently available memory. Convert it to CSV or close other applications and retry.",
            size as f64 / MIB as f64,
            lim.xlsx_file_limit as f64 / MIB as f64
        );
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn memory_status() -> (Option<u64>, Option<u64>) {
    #[repr(C)]
    struct MemoryStatusEx {
        length: u32,
        memory_load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page_file: u64,
        avail_page_file: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended_virtual: u64,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalMemoryStatusEx(buffer: *mut MemoryStatusEx) -> i32;
    }

    let mut s = MemoryStatusEx {
        length: std::mem::size_of::<MemoryStatusEx>() as u32,
        memory_load: 0,
        total_phys: 0,
        avail_phys: 0,
        total_page_file: 0,
        avail_page_file: 0,
        total_virtual: 0,
        avail_virtual: 0,
        avail_extended_virtual: 0,
    };
    let ok = unsafe { GlobalMemoryStatusEx(&mut s) } != 0;
    if ok { (Some(s.total_phys), Some(s.avail_phys)) } else { (None, None) }
}

#[cfg(target_os = "linux")]
fn memory_status() -> (Option<u64>, Option<u64>) {
    let Ok(text) = fs::read_to_string("/proc/meminfo") else { return (None, None); };
    let mut total = None;
    let mut available = None;
    for line in text.lines() {
        let mut p = line.split_whitespace();
        match p.next() {
            Some("MemTotal:") => total = p.next().and_then(|v| v.parse::<u64>().ok()).map(|kb| kb * 1024),
            Some("MemAvailable:") => available = p.next().and_then(|v| v.parse::<u64>().ok()).map(|kb| kb * 1024),
            _ => {}
        }
    }
    (total, available)
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn memory_status() -> (Option<u64>, Option<u64>) {
    (None, None)
}


#[cfg(test)]
mod memory_budget_tests {
    use super::*;
    #[test]
    fn capture_budget_reserves_system_memory_and_obeys_available_ram() {
        let mut limit=limits();limit.total_memory_bytes=Some(8*GIB);limit.available_memory_bytes=None;
        assert_eq!(limit.capture_memory_budget()as u64,8*GIB*2/5);
        limit.available_memory_bytes=Some(512*MIB);assert_eq!(limit.capture_memory_budget()as u64,384*MIB);
        limit.available_memory_bytes=Some(32*MIB);assert_eq!(limit.capture_memory_budget()as u64,24*MIB);
        limit.total_memory_bytes=None;limit.available_memory_bytes=None;assert_eq!(limit.capture_memory_budget()as u64,8*GIB*2/5);
    }
}
