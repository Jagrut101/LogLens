use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};
use uuid::Uuid;
use crate::context::TableData;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: String,
    pub filename: String,
    pub source_path: String,
    #[serde(default)]
    pub original_source_path: String,
    pub created_unix: u64,
    #[serde(default)]
    pub last_opened_unix: u64,
    pub deleted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppTheme {
    Dark,
    Light,
}

impl Default for AppTheme {
    fn default() -> Self { Self::Dark }
}

impl AppTheme {
    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AppPreferences {
    #[serde(default)]
    pub theme: AppTheme,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SessionConfiguration {
    #[serde(default)] pub project: TableData,
    #[serde(default)] pub channels: TableData,
    #[serde(default)] pub messages: TableData,
    #[serde(default)] pub signals: TableData,
    #[serde(default)] pub events: TableData,
    #[serde(default = "default_show_raw_ids")] pub show_raw_ids: bool,
    // Pre-load channel selection is persisted per Recent Session.
    // BTreeSet serializes deterministically and old sessions remain compatible via serde(default).
    #[serde(default)] pub preload_channels: std::collections::BTreeSet<u32>,
}


#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    #[serde(default)] pub schema_version: u32,
    #[serde(default)] pub analysis_settings: std::collections::BTreeMap<String,serde_json::Value>,
    #[serde(default)] pub project_channels: std::collections::BTreeSet<String>,
    #[serde(default)] pub selected_keys: std::collections::BTreeSet<String>,
    #[serde(default)] pub selected_frame_key: String,
    #[serde(default)] pub selected_channel: Option<u32>,
    #[serde(default)] pub selected_frame_bytes: std::collections::HashMap<String,std::collections::BTreeSet<usize>>,
    #[serde(default)] pub graphics_sources: std::collections::BTreeSet<String>,
    #[serde(default)] pub graphics_signals: std::collections::BTreeSet<String>,
    #[serde(default)] pub graphics_bits: std::collections::BTreeSet<String>,
    #[serde(default)] pub graphics_whole_frames: std::collections::BTreeSet<String>,
    #[serde(default)] pub graphics_lane_order: Vec<String>,
    #[serde(default)] pub graphics_multi_cursors: std::collections::HashMap<String,Vec<f64>>,
    #[serde(default)] pub cursor_a_s: f64,
    #[serde(default)] pub cursor_b_s: f64,
    #[serde(default)] pub graphics_range_start_s: Option<f64>,
    #[serde(default)] pub graphics_range_end_s: Option<f64>,
    #[serde(default)] pub graphics_view_start_s: f64,
    #[serde(default)] pub graphics_view_end_s: f64,
    #[serde(default)] pub graphics_show_event_markers: bool,
    #[serde(default)] pub graphics_hide_constant: bool,
    #[serde(default)] pub graphics_hide_repetitive: bool,
    #[serde(default)] pub graphics_stacked: bool,
    #[serde(default)] pub graphics_auto_y: bool,
    #[serde(default)] pub graphics_per_waveform_cursors: bool,
}

fn default_show_raw_ids() -> bool { true }

fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn preferred_root() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(base) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(base).join("LogLens");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join("Library").join("Application Support").join("LogLens");
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(base) = std::env::var_os("XDG_DATA_HOME") {
            return PathBuf::from(base).join("loglens");
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".local").join("share").join("loglens");
        }
    }
    PathBuf::from("runtime")
}

#[cfg(test)]
thread_local! { static TEST_ROOT: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) }; }

#[cfg(test)]
pub(crate) fn with_test_storage(test: impl FnOnce(&Path)) {
    let dir=tempfile::tempdir().unwrap();
    TEST_ROOT.with(|root| *root.borrow_mut()=Some(dir.path().join("storage")));
    struct Reset;
    impl Drop for Reset { fn drop(&mut self) { TEST_ROOT.with(|root| *root.borrow_mut()=None); } }
    let _reset=Reset;
    test(dir.path());
}

fn root() -> PathBuf {
    #[cfg(test)]
    if let Some(root)=TEST_ROOT.with(|root|root.borrow().clone()) { return root; }
    preferred_root()
}

// Replace a complete snapshot only after its contents have been written successfully.
fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    use std::io::Write;
    let mut file=tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}
fn outputs() -> PathBuf { root().join("outputs") }
fn uploads() -> PathBuf { root().join("uploads") }
fn recycle_outputs() -> PathBuf { root().join("recycle_bin").join("outputs") }
fn recycle_uploads() -> PathBuf { root().join("recycle_bin").join("uploads") }

fn migrate_legacy_runtime_if_needed() {
    let dst = root();
    let legacy = PathBuf::from("runtime");
    if dst == legacy || dst.exists() || !legacy.exists() { return; }
    if let Some(parent) = dst.parent() { let _ = fs::create_dir_all(parent); }
    let _ = fs::rename(&legacy, &dst);
}

pub fn storage_root() -> PathBuf { root() }

pub fn load_app_preferences() -> Result<AppPreferences> {
    init()?;
    let path=root().join("settings.json");
    if !path.exists() { return Ok(AppPreferences::default()); }
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

pub fn save_app_preferences(preferences:&AppPreferences) -> Result<()> {
    init()?;
    write_json(&root().join("settings.json"), preferences)
}

pub fn init() -> Result<()> {
    migrate_legacy_runtime_if_needed();
    for p in [outputs(), uploads(), recycle_outputs(), recycle_uploads()] { fs::create_dir_all(p)?; }
    Ok(())
}

fn canonical_text(p: &Path) -> String {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf()).to_string_lossy().to_string()
}

fn normalized_path_text(p:&Path)->String {
    let mut text=canonical_text(p).replace('\\', "/");
    #[cfg(target_os = "windows")]
    { text=text.to_ascii_lowercase(); }
    text
}

fn managed_upload_original(p:&Path)->Option<String>{
    // Old sessions may have accidentally stored LogLens' own uploads/<uuid>/<file>
    // as original_source_path. Recover the real identity by reading that UUID's metadata.
    let normalized=normalized_path_text(p);
    let uploads_root=normalized_path_text(&uploads());
    let prefix=format!("{}/",uploads_root.trim_end_matches('/'));
    let rest=normalized.strip_prefix(&prefix)?;
    let id=rest.split('/').next()?;
    if id.is_empty(){return None;}
    let meta_path=outputs().join(id).join("metadata.json");
    let bytes=fs::read(meta_path).ok()?;
    let meta=serde_json::from_slice::<SessionMeta>(&bytes).ok()?;
    if meta.original_source_path.is_empty(){return None;}
    let original=PathBuf::from(&meta.original_source_path);
    let recovered=normalized_path_text(&original);
    // Avoid recursion/loops from malformed legacy metadata.
    if recovered.starts_with(&prefix){None}else{Some(recovered)}
}

fn source_identity(p:&Path)->String {
    managed_upload_original(p).unwrap_or_else(||normalized_path_text(p))
}

fn meta_identity(m:&SessionMeta)->String {
    if !m.original_source_path.is_empty() {
        source_identity(Path::new(&m.original_source_path))
    } else {
        source_identity(Path::new(&m.source_path))
    }
}

fn write_meta(meta: &SessionMeta, deleted: bool) -> Result<()> {
    let dir = if deleted { recycle_outputs().join(&meta.id) } else { outputs().join(&meta.id) };
    fs::create_dir_all(&dir)?;
    write_json(&dir.join("metadata.json"), meta)?;
    Ok(())
}

pub fn save_configuration(id:&str, project:&TableData, channels:&TableData, messages:&TableData, signals:&TableData, events:&TableData, show_raw_ids:bool, preload_channels:&std::collections::BTreeSet<u32>) -> Result<()> {
    init()?;
    let dir=outputs().join(id).join("context");
    fs::create_dir_all(&dir)?;
    let snapshot=SessionConfiguration {
        project:project.clone(), channels:channels.clone(), messages:messages.clone(),
        signals:signals.clone(), events:events.clone(), show_raw_ids,
        preload_channels:preload_channels.clone(),
    };
    write_json(&dir.join("configuration.json"), &snapshot)?;
    Ok(())
}

pub fn save_workspace(id:&str, snapshot:&WorkspaceSnapshot) -> Result<()> {
    init()?;
    let dir=outputs().join(id).join("context");
    fs::create_dir_all(&dir)?;
    write_json(&dir.join("workspace.json"), snapshot)?;
    Ok(())
}

pub fn load_workspace(id:&str) -> Result<Option<WorkspaceSnapshot>> {
    init()?;
    let path=outputs().join(id).join("context").join("workspace.json");
    if !path.exists(){return Ok(None);}
    Ok(Some(serde_json::from_slice(&fs::read(path)?)?))
}

pub fn load_configuration(id:&str) -> Result<Option<SessionConfiguration>> {
    init()?;
    let path=outputs().join(id).join("context").join("configuration.json");
    if !path.exists(){return Ok(None);}
    Ok(Some(serde_json::from_slice(&fs::read(path)?)?))
}

pub fn create(source: &Path) -> Result<SessionMeta> {
    init()?;
    let identity = source_identity(source);
    // Never promote LogLens' managed uploads/<uuid>/... copy to a new source identity.
    // If this path came from an existing session, preserve its true original source.
    let original = managed_upload_original(source).unwrap_or_else(||canonical_text(source));

    // Reopening the same capture refreshes the existing Recent Session instead of
    // creating a duplicate card every time. Compare normalized source identities
    // so Windows slash/case differences cannot create another session.
    if let Some(mut existing) = list(true).into_iter().find(|m| meta_identity(m) == identity) {
        let saved = uploads().join(&existing.id).join(&existing.filename);
        if let Some(parent) = saved.parent() { fs::create_dir_all(parent)?; }
        // Reopening a managed upload must never copy the file onto itself.
        if source.canonicalize()? != saved.canonicalize().unwrap_or_else(|_| saved.clone()) {
            let file=tempfile::NamedTempFile::new_in(saved.parent().unwrap())?;
            fs::copy(source,file.path())?;
            file.as_file().sync_all()?;
            file.persist(&saved)?;
        }
        existing.source_path = canonical_text(&saved);
        existing.last_opened_unix = now_unix();
        existing.deleted = false;
        write_meta(&existing, false)?;
        return Ok(existing);
    }

    let id = Uuid::new_v4().to_string();
    let filename = source.file_name().and_then(|x| x.to_str()).unwrap_or("log").to_string();
    let upload_dir = uploads().join(&id);
    fs::create_dir_all(&upload_dir)?;
    let saved = upload_dir.join(&filename);
    fs::copy(source, &saved)?;
    let now = now_unix();
    let meta = SessionMeta {
        id: id.clone(),
        filename,
        source_path: canonical_text(&saved),
        original_source_path: original,
        created_unix: now,
        last_opened_unix: now,
        deleted: false,
    };
    fs::create_dir_all(outputs().join(&id).join("context"))?;
    write_meta(&meta, false)?;
    Ok(meta)
}

pub fn list(active: bool) -> Vec<SessionMeta> {
    let _ = init();
    let base = if active { outputs() } else { recycle_outputs() };
    let Ok(rd) = fs::read_dir(base) else { return vec![] };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let p = e.path().join("metadata.json");
        if let Ok(b) = fs::read(&p) {
            if let Ok(mut m) = serde_json::from_slice::<SessionMeta>(&b) {
                m.deleted = !active;
                // Always rebuild the copied-log location from the current persistent
                // storage root. This fixes old relative paths and app upgrades.
                let saved = if active {
                    uploads().join(&m.id).join(&m.filename)
                } else {
                    recycle_uploads().join(&m.id).join(&m.filename)
                };
                m.source_path = canonical_text(&saved);
                if m.last_opened_unix == 0 { m.last_opened_unix = m.created_unix; }
                // Keep metadata visible; opening can fall back to original_source_path.
                out.push(m);
            }
        }
    }
    out.sort_by_key(|m| std::cmp::Reverse((m.last_opened_unix, m.created_unix)));

    // Older versions could already have written duplicate metadata directories.
    // Collapse them at read time so the Start page shows one card per source.
    // Sorting first means the most recently opened session wins.
    let mut seen=std::collections::BTreeSet::new();
    let mut seen_legacy_files=std::collections::BTreeSet::new();
    out.retain(|m|{
        let identity=meta_identity(m);
        if !seen.insert(identity){return false;}
        // Legacy duplicate sessions sometimes recorded another LogLens upload copy as
        // the "original". Those copies have different UUID paths but the same capture
        // filename. Collapse only these managed-upload legacy records by filename.
        let original=PathBuf::from(&m.original_source_path);
        let is_managed=!m.original_source_path.is_empty() &&
            normalized_path_text(&original).starts_with(&format!("{}/",normalized_path_text(&uploads()).trim_end_matches('/')));
        if is_managed {
            let file_key={
                #[cfg(target_os="windows")] { m.filename.to_ascii_lowercase() }
                #[cfg(not(target_os="windows"))] { m.filename.clone() }
            };
            if !seen_legacy_files.insert(file_key){return false;}
        }
        true
    });
    out
}

pub fn touch(id: &str) -> Result<()> {
    init()?;
    let path = outputs().join(id).join("metadata.json");
    let b = fs::read(&path)?;
    let mut m: SessionMeta = serde_json::from_slice(&b)?;
    m.last_opened_unix = now_unix();
    m.deleted = false;
    let saved = uploads().join(&m.id).join(&m.filename);
    m.source_path = canonical_text(&saved);
    write_meta(&m, false)
}

fn move_dir(src: PathBuf, dst: PathBuf) -> Result<()> {
    if !src.exists() { return Ok(()); }
    if let Some(p) = dst.parent() { fs::create_dir_all(p)?; }
    if dst.exists() { fs::remove_dir_all(&dst)?; }
    fs::rename(src, dst)?;
    Ok(())
}

pub fn remove(id: &str) -> Result<()> {
    init()?;
    move_dir(outputs().join(id), recycle_outputs().join(id))?;
    move_dir(uploads().join(id), recycle_uploads().join(id))?;
    Ok(())
}

pub fn restore(id: &str) -> Result<()> {
    init()?;
    move_dir(recycle_outputs().join(id), outputs().join(id))?;
    move_dir(recycle_uploads().join(id), uploads().join(id))?;
    touch(id)?;
    Ok(())
}

pub fn purge(id: &str) -> Result<()> {
    for p in [recycle_outputs().join(id), recycle_uploads().join(id)] {
        if p.exists() { fs::remove_dir_all(p)?; }
    }
    Ok(())
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    #[test]
    fn reopening_managed_copy_preserves_bytes_and_session_identity() {
        with_test_storage(|dir| {
            let path=dir.join("capture.pcap"); fs::write(&path,b"capture bytes").unwrap();
            let first=create(&path).unwrap();
            let reopened=create(Path::new(&first.source_path)).unwrap();
            assert_eq!(first.id,reopened.id);
            assert_eq!(fs::read(&reopened.source_path).unwrap(),b"capture bytes");
            assert_eq!(list(true).len(),1);
        });
    }
    #[test]
    fn app_theme_defaults_to_dark_and_persists_globally() {
        with_test_storage(|_| {
            assert_eq!(load_app_preferences().unwrap().theme, AppTheme::Dark);
            save_app_preferences(&AppPreferences { theme:AppTheme::Light }).unwrap();
            assert_eq!(load_app_preferences().unwrap().theme, AppTheme::Light);
            let raw=fs::read_to_string(root().join("settings.json")).unwrap();
            assert!(raw.contains("\"theme\": \"Light\""));
        });
    }
    #[test]
    fn legacy_workspace_still_loads_and_atomic_save_replaces_it() {
        with_test_storage(|_| {
            init().unwrap(); let dir=outputs().join("legacy").join("context");
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("workspace.json"),br#"{"schema_version":1,"cursor_a_s":3.0}"#).unwrap();
            let mut snapshot=load_workspace("legacy").unwrap().unwrap();
            assert_eq!(snapshot.cursor_a_s,3.0); assert!(snapshot.analysis_settings.is_empty());
            snapshot.cursor_a_s=4.0; save_workspace("legacy",&snapshot).unwrap();
            assert_eq!(load_workspace("legacy").unwrap().unwrap().cursor_a_s,4.0);
        });
    }
}
