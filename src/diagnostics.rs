use chrono::Local;
use std::{
    backtrace::Backtrace,
    cell::RefCell,
    fs::{self, File, OpenOptions},
    io::Write,
    panic,
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, OnceLock},
};

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;
const MAX_LOG_FILES: usize = 5;
const MAX_CRASH_FILES: usize = 5;
const MAX_LOG_MESSAGE_BYTES: usize = 16 * 1024;

thread_local! {
    static CURRENT_CONTEXT: RefCell<String> = RefCell::new("Application startup".to_string());
}

struct DiagnosticLogger {
    dir: PathBuf,
    file_path: PathBuf,
    file: Option<File>,
}

static LOGGER: OnceLock<Mutex<DiagnosticLogger>> = OnceLock::new();

pub fn init() -> PathBuf {
    let logger = LOGGER.get_or_init(|| Mutex::new(create_logger()));
    let dir = logger
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .dir
        .clone();
    install_panic_hook_once();
    dir
}

pub fn log_directory() -> PathBuf {
    init()
}

pub fn set_context(context: impl Into<String>) {
    let context = context.into();
    CURRENT_CONTEXT.with(|slot| *slot.borrow_mut() = context);
}

pub fn log(level: &str, message: impl AsRef<str>) {
    let logger = LOGGER.get_or_init(|| Mutex::new(create_logger()));
    let mut logger = logger.lock().unwrap_or_else(|e| e.into_inner());
    logger.write_line(level, message.as_ref());
}

pub fn open_log_directory() -> Result<(), String> {
    let dir = log_directory();
    #[cfg(target_os = "windows")]
    let status = Command::new("explorer").arg(&dir).status();
    #[cfg(target_os = "macos")]
    let status = Command::new("open").arg(&dir).status();
    #[cfg(all(unix, not(target_os = "macos")))]
    let status = Command::new("xdg-open").arg(&dir).status();
    #[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
    let status: std::io::Result<std::process::ExitStatus> = Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "opening folders is not supported on this platform",
    ));

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("Folder opener returned status {s}")),
        Err(e) => Err(format!("Could not open diagnostics folder: {e}")),
    }
}

impl DiagnosticLogger {
    fn write_line(&mut self, level: &str, message: &str) {
        let mut clean = message.replace('\r', " ").replace('\n', " | ");
        if clean.len() > MAX_LOG_MESSAGE_BYTES {
            let mut end=MAX_LOG_MESSAGE_BYTES;
            while end>0 && !clean.is_char_boundary(end) { end-=1; }
            clean.truncate(end);
            clean.push_str(" ... [diagnostic message truncated]");
        }
        let line = format!(
            "{} {:5} [thread={}] {}\n",
            Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
            normalize_level(level),
            std::thread::current().name().unwrap_or("unnamed"),
            clean
        );
        if self.rotate_if_needed(line.len() as u64).is_err() {
            return;
        }
        if let Some(file) = self.file.as_mut() {
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
    }

    fn rotate_if_needed(&mut self, incoming_bytes: u64) -> std::io::Result<()> {
        let current_len = fs::metadata(&self.file_path).map(|m| m.len()).unwrap_or(0);
        if current_len.saturating_add(incoming_bytes) <= MAX_LOG_BYTES {
            return Ok(());
        }
        if let Some(file) = self.file.as_mut() {
            let _ = file.flush();
        }
        self.file = None;
        self.file_path = unique_regular_log_path(&self.dir);
        self.file = open_append(&self.file_path).ok();
        cleanup_old_files(&self.dir, "loglens_", MAX_LOG_FILES);
        Ok(())
    }
}

fn create_logger() -> DiagnosticLogger {
    let dir = create_log_directory();
    let file_path = choose_regular_log_path(&dir);
    let file = open_append(&file_path).ok();
    cleanup_old_files(&dir, "loglens_", MAX_LOG_FILES);
    cleanup_old_files(&dir, "crash_", MAX_CRASH_FILES);
    DiagnosticLogger { dir, file_path, file }
}

fn create_log_directory() -> PathBuf {
    let preferred = platform_log_directory();
    if fs::create_dir_all(&preferred).is_ok() {
        return preferred;
    }
    let fallback = std::env::temp_dir().join("LogLens").join("logs");
    let _ = fs::create_dir_all(&fallback);
    fallback
}

#[cfg(target_os = "windows")]
fn platform_log_directory() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("LogLens")
        .join("logs")
}

#[cfg(target_os = "macos")]
fn platform_log_directory() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Library")
        .join("Logs")
        .join("LogLens")
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_log_directory() -> PathBuf {
    if let Some(p) = std::env::var_os("XDG_STATE_HOME") {
        PathBuf::from(p).join("loglens").join("logs")
    } else {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(".local")
            .join("state")
            .join("loglens")
            .join("logs")
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
fn platform_log_directory() -> PathBuf {
    std::env::temp_dir().join("LogLens").join("logs")
}

fn choose_regular_log_path(dir: &Path) -> PathBuf {
    let daily = dir.join(format!("loglens_{}.log", Local::now().format("%Y-%m-%d")));
    let len = fs::metadata(&daily).map(|m| m.len()).unwrap_or(0);
    if len < MAX_LOG_BYTES {
        daily
    } else {
        unique_regular_log_path(dir)
    }
}

fn unique_regular_log_path(dir: &Path) -> PathBuf {
    dir.join(format!(
        "loglens_{}_{}.log",
        Local::now().format("%Y-%m-%d"),
        Local::now().format("%H%M%S_%3f")
    ))
}

fn open_append(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

fn normalize_level(level: &str) -> &str {
    match level {
        "ERROR" => "ERROR",
        "WARN" => "WARN",
        "DEBUG" => "DEBUG",
        _ => "INFO",
    }
}

fn install_panic_hook_once() {
    static INSTALLED: OnceLock<()> = OnceLock::new();
    INSTALLED.get_or_init(|| {
        let previous=panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            write_panic_report(info);
            previous(info);
        }));
    });
}

fn write_panic_report(info: &panic::PanicHookInfo<'_>) {
    let dir = log_directory_without_hook();
    let now = Local::now();
    let path = dir.join(format!("crash_{}.log", now.format("%Y-%m-%d_%H%M%S_%3f")));
    let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_string()
    };
    let location = info
        .location()
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
        .unwrap_or_else(|| "unknown".to_string());
    let context = CURRENT_CONTEXT.with(|slot| slot.borrow().clone());
    let thread_name = std::thread::current().name().unwrap_or("unnamed").to_string();
    let report = format!(
        "LogLens panic diagnostic\n\
         Timestamp: {}\n\
         Version: {}\n\
         OS: {}\n\
         Architecture: {}\n\
         Process ID: {}\n\
         Thread: {}\n\
         Operation: {}\n\
         Location: {}\n\
         Panic: {}\n\
         Note: this panic may have been recovered by LogLens panic containment.\n\
         Raw CAN/LIN/ETH payload data is not intentionally written by the diagnostics subsystem.\n\n\
         Backtrace:\n{}\n",
        now.format("%Y-%m-%d %H:%M:%S%.3f"),
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::process::id(),
        thread_name,
        context,
        location,
        payload,
        Backtrace::force_capture(),
    );
    if let Ok(mut f) = File::create(&path) {
        let _ = f.write_all(report.as_bytes());
        let _ = f.flush();
    }
    cleanup_old_files(&dir, "crash_", MAX_CRASH_FILES);
}

fn log_directory_without_hook() -> PathBuf {
    if let Some(logger) = LOGGER.get() {
        if let Ok(guard) = logger.try_lock() {
            return guard.dir.clone();
        }
    }
    create_log_directory()
}

fn cleanup_old_files(dir: &Path, prefix: &str, keep: usize) {
    let Ok(entries) = fs::read_dir(dir) else { return; };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            if !name.starts_with(prefix) || path.extension().and_then(|x| x.to_str()) != Some("log") {
                return None;
            }
            let modified = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            Some((modified, path))
        })
        .collect();
    files.sort_by_key(|(modified, _)| *modified);
    let remove_count = files.len().saturating_sub(keep);
    for (_, path) in files.into_iter().take(remove_count) {
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_normalization_is_bounded() {
        assert_eq!(normalize_level("ERROR"), "ERROR");
        assert_eq!(normalize_level("WARN"), "WARN");
        assert_eq!(normalize_level("anything"), "INFO");
    }

    #[test]
    fn regular_log_name_has_log_extension() {
        let p = choose_regular_log_path(&std::env::temp_dir());
        assert_eq!(p.extension().and_then(|x| x.to_str()), Some("log"));
    }
}
