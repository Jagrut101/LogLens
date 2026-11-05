use crate::{
    analysis::{
        bit_activity, byte_activity, byte_activity_indexed, byte_value_counts,
        decode_signal, decode_signal_typed, endian_is_little, parse_little_endian, detect_repeating_pattern, downsample_minmax, payload_change_events, statistics, auto_activity_range,
        Filter, SignalCandidate, SignalDiscoveryScope, Stats, timing_analysis, counter_integrity_range, ab_byte_compare,
    },
    context::{read_table_for, signal_definitions, SignalDefinition, TableData},
    engine::{discover_capture_channels, Dataset},
    events::{control_triggers, ControlTrigger},
    export,
    model::{BusType, channel_id_hex, channel_text_hex, channel_display, frame_key_display},
    capture,
    native_engine,
    resources,
    diagnostics,
    session::{self, AppPreferences, AppTheme, SessionMeta, WorkspaceSnapshot},
};
use eframe::egui::{self, Color32, Pos2, Sense, Stroke, Vec2};
use std::{collections::{BTreeMap, BTreeSet, HashMap, VecDeque}, path::PathBuf, sync::{Arc, mpsc::{self, Receiver}}, thread, panic::{self, AssertUnwindSafe}, time::{Duration, Instant}};

#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Workspace { Measurement, Analysis, Configuration }
#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum AnalysisPage { FrameByte, SignalDiscovery, Timing, CountersCrc, Events, AbCompare }

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
enum GraphicsDensity { Compact, Standard, Detailed }

#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum ConfigPage { Project, Channels, Messages, Signals, Events }
#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum ActivityLevel { Bytes, Bits }
#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum ExplorerMode { Activity, Values, Changes, Waveform }
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
enum InspectorPane { None, Details, Graphics, Events }
#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum InspectorDock { Bottom, Right }
#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum ExportRange { CursorAB, Visible, Full }
#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum ExportContent { CurrentSelection, SelectedChannels, AllData }
#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum ExportFormat { Excel, Csv, Pcapng }

#[derive(Clone)]
struct ToolLogEntry {
    time: String,
    level: String,
    message: String,
}

enum LoadMessage {
    Stage(&'static str, u8),
    Finished(Result<Dataset, String>),
}

struct BackgroundLoad {
    path: PathBuf,
    target_session: Option<SessionMeta>,
    channels: Option<BTreeSet<u32>>,
    preserve_graphics: bool,
    retain_channel_scope: bool,
    rx: Receiver<LoadMessage>,
    stage: String,
    progress: u8,
}

#[derive(Clone)]
struct TraceDisplayRow {
    timestamp: String,
    dt: String,
    channel: String,
    protocol: String,
    frame_key: String,
    dlc: String,
    data: String,
}

// Focused text fields retain their own select-all and copy behavior.
fn trace_selection_command(ui:&mut egui::Ui)->Option<bool> {
    if ui.ctx().wants_keyboard_input() {return None;}
    ui.input_mut(|input| {
        if input.consume_key(egui::Modifiers::NONE,egui::Key::Escape) {Some(false)}
        else if input.consume_key(egui::Modifiers::COMMAND,egui::Key::A)
            || input.consume_key(egui::Modifiers::CTRL,egui::Key::A) {Some(true)}
        else {None}
    })
}

fn tsv_field(value:&str)->String {
    if value.contains(['\t','\n','\r','"']) {format!("\"{}\"",value.replace('"',"\"\""))}
    else {value.to_string()}
}

impl TraceDisplayRow {
    fn clipboard_row(&self, count: usize) -> String {
        // TSV can be pasted into a spreadsheet; retain complete, untruncated fields.
        let count = count.to_string();
        let values = [&self.timestamp, &self.dt, &self.channel, &self.protocol,
            &self.frame_key, &self.dlc, &count, &self.data];
        let fields: Vec<String> = values.into_iter().map(|v| {
            if v.contains(['\t', '\n', '\r', '"']) {
                format!("\"{}\"", v.replace('"', "\"\""))
            } else { v.clone() }
        }).collect();
        format!("Timestamp\tdt\tChannel\tProtocol\tID/Key\tDLC\tCount\tData\n{}", fields.join("\t"))
    }

    fn copy_menu(&self, ui: &mut egui::Ui, count: usize) {
        if ui.button("Copy row").clicked() {
            ui.ctx().copy_text(self.clipboard_row(count));
            ui.close();
        }
        if ui.button("Copy data (HEX)").clicked() {
            ui.ctx().copy_text(self.data.clone());
            ui.close();
        }
    }
}

pub struct LogLensApp {
    dataset: Option<Arc<Dataset>>,
    path: Option<PathBuf>,
    session: Option<SessionMeta>,
    error: Option<String>,
    info: Option<String>,
    workspace: Workspace,
    analysis_page: AnalysisPage,
    graphics_density: GraphicsDensity,
    graphics_show_filters: bool,
    graphics_show_help: bool,
    graphics_show_signal_browser: bool,
    // v0.51.9: native viewer split widths; dragged with the same manual splitter pattern as Project/Trace.
    graphics_signal_panel_width: f32,
    graphics_cursor_panel_width: f32,
    // Dedicated clean waveform workspace. Graphics tab remains a lightweight launcher.
    graphics_viewer_open: bool,
    graph_channel_names: BTreeMap<u32,String>,
    graph_lane_names: BTreeMap<String,String>,
    graph_rename: Option<(Option<u32>,String,String)>,
    graph_image_export: crate::graph_image::GraphImageExport,
    graphics_activity_filter: crate::activity_filter::ActivityFilter,
    graphics_frame_id_filter: crate::frame_id_filter::FrameIdFilter,
    graphics_eth_endpoints: crate::eth_filter::EthernetEndpoints,
    graphics_eth_filters: BTreeMap<u32,crate::eth_filter::EndpointFilter>,
    graphics_selected_source: Option<String>,
    graphics_selected_signal: Option<String>,
    graphics_reveal_source: Option<String>,
    show_raw_ids: bool,
    config_page: ConfigPage,
    filter: Filter,
    max_packets: usize,
    sessions: Vec<SessionMeta>,
    deleted: Vec<SessionMeta>,
    show_recycle: bool,
    show_tool_log: bool,
    show_settings: bool,
    show_graph_settings: bool,
    theme: AppTheme,
    graph_use_colors: bool,
    graph_show_grid: bool,
    graph_show_markers: bool,
    graph_show_cursor_labels: bool,
    // v0.53.2: choose between the existing global A/B pair and independent per-waveform measurement cursors.
    graphics_per_waveform_cursors: bool,
    graphics_multi_cursors: HashMap<String, Vec<f64>>,
    graphics_multi_cursor_armed: Option<String>,
    // v0.53.4: active draggable per-waveform cursor: (stable lane id, cursor index).
    graphics_multi_cursor_drag: Option<(String, usize)>,
    graph_line_width: f32,
    tool_log: VecDeque<ToolLogEntry>,
    background_load: Option<BackgroundLoad>,
    active_preload_channels: BTreeSet<u32>,
    pending_log_path: Option<PathBuf>,
    pending_channels: Vec<(u32,String)>,
    pending_channel_selection: BTreeSet<u32>,
    // Ephemeral channel-picker controls; never change committed capture selection until Load.
    channel_picker_search: String,
    channel_picker_filter: String,
    tecmp_source_time: bool,
    perf_trace_query_ms: f32,
    perf_trace_render_ms: f32,
    show_perf_hud: bool,
    trace_filter_pending: bool,
    trace_filter_changed_at: Option<Instant>,

    project_config: TableData,
    vehicle_db: TableData,
    event_log: TableData,
    channel_map: TableData,
    signal_table: TableData,
    signals: Vec<SignalDefinition>,

    selected_channels: BTreeSet<u32>,
    // Project Tree global scope uses bus + channel identity so CAN1, LIN1 and ETH1 never collide.
    project_channels: BTreeSet<String>,
    selected_keys: BTreeSet<String>,
    activity_level: ActivityLevel,
    explorer_mode: ExplorerMode,
    selected_frame_key: String,
    selected_channel: Option<u32>,
    inspect_byte: usize,
    inspect_bit: usize,
    reference_value: u8,
    compare_ids: bool,
    compare_keys: BTreeSet<String>,
    multi_frame_keys:BTreeSet<String>,
    selected_frame_bytes: HashMap<String,BTreeSet<usize>>,
    multi_hide_static:bool,
    multi_hide_counter:bool,
    multi_hide_crc_candidates:bool,
    multi_activity_signature:String,
    multi_activity_cache:Vec<(String,Vec<crate::analysis::ScopedByteActivity>)>,

    waveform_keys: BTreeSet<String>,
    waveform_byte: usize,
    waveform_bit: usize,
    raw_waveform_bytes: BTreeSet<usize>,
    raw_waveform_stacked: bool,
    candidates: Vec<SignalCandidate>,
    candidates_ready: bool,
    signal_discovery_scope: SignalDiscoveryScope,
    candidate_preview: Option<SignalCandidate>,

    time_start: f64,
    time_end: f64,
    use_manual_window: bool,
    selected_event_row: Option<usize>,
    selected_ecus: BTreeSet<String>,

    custom_name: String,
    custom_frame: String,
    custom_start_byte: usize,
    custom_start_bit: usize,
    custom_bit_length: usize,
    custom_scale: f64,
    custom_offset: f64,
    custom_signed: bool,
    custom_little_endian: bool,

    signal_search: String,
    waveform_overlap: bool,

    cached_filter_signature: String,
    filtered_indices: Vec<usize>,
    cached_stats: Option<Stats>,
    // Performance caches. These prevent full-log scans on every egui repaint.
    cached_scope_signature: String,
    cached_scoped_indices: Vec<usize>,
    cached_graphics_sources: Vec<(String,String,u32,String,usize)>,
    cached_waveforms: HashMap<String, Arc<Vec<[f64; 2]>>>,
    cached_decoded_signal_waveforms: HashMap<String, Arc<Vec<[f64; 2]>>>,
    cached_decoded_signal_indices: HashMap<String,Arc<Vec<usize>>>,
    // v0.52.0: TECMP analog decoding is expensive; retain decoded samples + metadata across repaints.
    cached_analog_waveforms: HashMap<String, (Arc<Vec<[f64; 2]>>, String, usize, u64)>,
    // Zoom-aware Analog display points: one entry per lane (view start bits, view end bits, points).
    cached_analog_view: HashMap<String, (u64, u64, Arc<Vec<[f64; 2]>>)>,
    // v0.52.0: cursor measurement strings are recomputed only when cursors/lanes/filter state changes.
    cached_cursor_rows_key: String,
    cached_cursor_rows: Vec<(String,String,String,String)>,
    signal_config_dirty: bool,
    cached_trace_scope_signature: String,
    cached_trace_processed_count: usize,
    cached_trace_positions: HashMap<String, usize>,
    cached_trace_rows: Arc<Vec<TraceRow>>,
    // v0.18: cache only the currently rendered Trace viewport strings.
    // Prevents timestamp/protocol/payload formatting on every repaint.
    trace_view_cache: HashMap<usize, TraceDisplayRow>,
    trace_view_cache_scope: String,
    trace_columns: crate::trace_columns::TraceColumns,
    cached_change_scope_signature: String,
    cached_changing_trace_sources: BTreeSet<String>,
    cached_changing_graphics_sources: BTreeSet<String>,

    // CANoe-style workspace state
    measurement_running: bool,
    measurement_paused: bool,
    replay_speed: f32,
    selected_trace_row: Option<usize>,
    selected_trace_rows: BTreeSet<usize>,
    trace_row_anchor: Option<usize>,
    graphics_bytes: BTreeSet<usize>,
    graphics_sources: BTreeSet<String>,
    graphics_signals: BTreeSet<String>,
    graphics_bits: BTreeSet<String>,
    // Whole-frame waveform selections. Each source is rendered as one deterministic payload signature trace.
    graphics_whole_frames: BTreeSet<String>,
    // Display-only filter: hide STATIC bytes and hide a frame when every payload byte is STATIC in the active Analysis Range.
    graphics_hide_constant: bool,
    // Display-only filter for stable short-period byte patterns in the committed Analysis Range.
    graphics_hide_repetitive: bool,
    graphics_repetitive_cache: HashMap<String,bool>,
    graphics_stacked: bool,
    // Auto-fit each engineering/raw-byte lane to the values inside the current visible time viewport.
    // Binary bit lanes intentionally remain fixed at 0..1.
    graphics_auto_y: bool,
    graphics_lane_height: f32,
    // v0.52.4: authoritative vertical offset shared by waveform and Cursor Values.
    graphics_lane_scroll_y: f32,
    graphics_y_scale: HashMap<String,f64>,
    graphics_y_offset: HashMap<String,f64>,
    graphics_lane_order: Vec<String>,
    graphics_drag_lane: Option<String>,
    graphics_drag_cursor: Option<char>,
    graphics_active_cursor: char,
    graphics_show_active: bool,
    graphics_show_static: bool,
    graphics_show_counter: bool,
    graphics_show_crc: bool,
    graphics_show_pattern: bool,
    graphics_class_cache: HashMap<String,(&'static str,f64)>,
    graphics_source_class_ready: BTreeSet<String>,
    graphics_class_window: Option<(f64,f64)>,
    graphics_tree_open: BTreeSet<String>,
    graphics_show_event_markers: bool,
    cursor_a_s: f64,
    cursor_b_s: f64,
    show_network_tree: bool,
    // Revision 5: tree filtering is display-only; it never alters the selected channels.
    explorer_search: String,
    explorer_protocol: String,
    inspector_pane: InspectorPane,
    inspector_height: f32,
    inspector_width: f32,
    project_tree_width: f32,
    inspector_dock: InspectorDock,
    floating_panes: BTreeSet<InspectorPane>,

    // Shared graphics viewport + offline replay
    graphics_view_start_s: f64,
    graphics_range_start_s: Option<f64>,
    graphics_range_end_s: Option<f64>,
    // Manual Analysis Range editor. Draft text is isolated from the committed range so
    // typing never triggers classification; Apply commits once.
    graphics_range_manual_start: String,
    graphics_range_manual_end: String,
    graphics_range_manual_dirty: bool,
    graphics_range_dragging: bool,
    graphics_range_changed_only: bool,
    // Persistent XViewer-style analysis-window interaction:
    // 0 idle, 1 create, 2 resize-left, 3 resize-right, 4 move.
    graphics_range_interaction: u8,
    graphics_range_last_pointer_s: Option<f64>,
    graphics_range_cached_bounds: Option<(f64,f64)>,
    graphics_range_cached_bytes: BTreeMap<String,BTreeSet<usize>>,
    graphics_view_end_s: f64,
    graphics_follow_replay: bool,
    replay_position_s: f64,
    replay_anchor: Option<Instant>,
    trace_follow_replay: bool,
    trace_overwrite: bool,
    trace_only_changing: bool,
    graphics_only_changing: bool,
    trace_freeze_view: bool,
    trace_display_position_s: f64,
    // v0.36 integrated workflow
    show_export_manager: bool,
    export_range: ExportRange,
    export_content: ExportContent,
    export_format: ExportFormat,
    export_skip_constant_frames: bool,
    // v0.54.0: debounced persistent per-log workspace state.
    workspace_cache_signature: String,
    workspace_cache_last_save: Instant,
    show_start_page: bool,
    home_session_search: String,
}

fn selection_tile(ui:&mut egui::Ui,on:&mut bool,label:impl Into<egui::WidgetText>)->egui::Response {
    let selected=*on;
    let resp=ui.add_sized([72.0,30.0],egui::Button::new(label).selected(selected));
    if resp.clicked(){*on=!*on;}
    resp
}

#[derive(Clone, Copy)]
struct HomePalette {
    dark: bool,
    backdrop: Color32,
    surface: Color32,
    hero: Color32,
    text: Color32,
    muted: Color32,
    border: Color32,
    hero_border: Color32,
    accent: Color32,
    accent_button: Color32,
    pill: Color32,
}
impl HomePalette {
    fn new(dark: bool) -> Self {
        if dark {
            Self { dark, backdrop:Color32::from_rgb(11,18,24), surface:Color32::from_rgb(21,30,39),
                hero:Color32::from_rgb(25,37,46), text:Color32::from_rgb(234,244,246),
                muted:Color32::from_rgb(158,175,184), border:Color32::from_rgb(42,59,69),
                hero_border:Color32::from_rgb(55,108,104), accent:Color32::from_rgb(76,200,189),
                accent_button:Color32::from_rgb(0,140,133), pill:Color32::from_rgb(23,56,58) }
        } else {
            Self { dark, backdrop:Color32::from_rgb(244,247,248), surface:Color32::WHITE,
                hero:Color32::from_rgb(237,246,246), text:Color32::from_rgb(23,40,44),
                muted:Color32::from_rgb(94,117,124), border:Color32::from_rgb(217,228,231),
                hero_border:Color32::from_rgb(99,163,158), accent:Color32::from_rgb(8,123,120),
                accent_button:Color32::from_rgb(0,140,133), pill:Color32::from_rgb(223,245,239) }
        }
    }
}
/// Display-only Capture Explorer filter: protocol + network + decimal/HEX channel/name search.
/// Filtering intentionally does not change `project_channels` or force a new capture load.
fn explorer_matches_channel(bus: &str, channel_id: u32, label: &str, query: &str, protocol: &str) -> bool {
    let bus_upper = bus.to_ascii_uppercase();
    let protocol_ok = match protocol {
        "All" => true,
        "Ethernet" => bus_upper == "ETH" || bus_upper == "ETHERNET",
        "CAN FD" => bus_upper == "CAN-FD" || bus_upper == "CAN FD" || bus_upper == "CANFD",
        "Other" => !matches!(bus_upper.as_str(), "CAN" | "CAN-FD" | "CAN FD" | "CANFD" | "LIN" | "ETH" | "ETHERNET" | "ANALOG" | "TECMP"),
        other => bus_upper == other.to_ascii_uppercase(),
    };
    if !protocol_ok { return false; }
    let q = query.trim().to_ascii_lowercase();
    q.is_empty()
        || bus.to_ascii_lowercase().contains(&q)
        || label.to_ascii_lowercase().contains(&q)
        || channel_id.to_string().contains(&q)
        || format!("0x{channel_id:X}").to_ascii_lowercase().contains(&q)
}

/// Reserve useful trace space while resizing the Capture Explorer.
fn explorer_panel_width(available: f32, requested: f32) -> f32 {
    let width = available.max(0.0);
    let maximum = (width - 360.0).max(120.0).min(width * 0.48);
    requested.clamp(280.0_f32.min(maximum), maximum)
}

/// Constrain the docked inspector so the main trace remains interactive.
fn docked_pane_extent(available: f32, requested: f32, main_min: f32, pane_min: f32) -> f32 {
    let maximum = (available - main_min - 5.0).max(0.0);
    requested.clamp(pane_min.min(maximum), maximum)
}

fn home_control_style(ui: &mut egui::Ui, p: HomePalette) {
    let style = ui.style_mut();
    style.spacing.item_spacing = Vec2::new(10.0, 9.0);
    style.spacing.button_padding = Vec2::new(13.0, 9.0);
    style.spacing.interact_size.y = 35.0;
    style.visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(9);
    style.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(9);
    style.visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(9);
    style.visuals.widgets.active.corner_radius = egui::CornerRadius::same(9);
    style.visuals.widgets.inactive.bg_fill = p.surface;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, p.border);
}

impl LogLensApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let _ = session::init();
        let theme = match session::load_app_preferences() {
            Ok(preferences) => preferences.theme,
            Err(error) => {
                diagnostics::log("WARN", format!("Could not load app settings; using Dark theme: {error:#}"));
                AppTheme::Dark
            }
        };
        cc.egui_ctx.set_visuals(match theme {
            AppTheme::Dark => egui::Visuals::dark(),
            AppTheme::Light => egui::Visuals::light(),
        });
        // Slightly larger desktop controls improve readability and click targets across all workspaces.
        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.button_padding = Vec2::new(5.0, 3.0);
        style.spacing.interact_size.y = 24.0;
        style.spacing.item_spacing = Vec2::new(7.0, 6.0);
        style.spacing.window_margin = egui::Margin::same(10);
        style.visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(5);
        style.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(5);
        style.visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(5);
        style.visuals.widgets.active.corner_radius = egui::CornerRadius::same(5);
        cc.egui_ctx.set_style(style);
        let mut app=Self::initial_state();
        app.theme=theme;
        app
    }

    fn initial_state() -> Self {
        Self {
            dataset: None,
            path: None,
            session: None,
            error: None,
            info: None,
            workspace: Workspace::Measurement,
            analysis_page: AnalysisPage::FrameByte,
            
            graphics_density: GraphicsDensity::Standard,
            graphics_show_filters: false,
            graphics_show_help: false,
            graphics_show_signal_browser: true,
            graphics_signal_panel_width: 260.0,
            graphics_cursor_panel_width: 400.0,
            graphics_viewer_open: false,
            graph_channel_names: BTreeMap::new(),
            graph_lane_names: BTreeMap::new(),
            graph_rename: None,
            graph_image_export: Default::default(),
            graphics_activity_filter: Default::default(),
            graphics_frame_id_filter: Default::default(),
            graphics_eth_endpoints: Default::default(),
            graphics_eth_filters: BTreeMap::new(),
            graphics_selected_source: None,
            graphics_selected_signal: None,
            graphics_reveal_source: None,
            show_raw_ids: true,
            config_page: ConfigPage::Project,
            filter: Filter::default(),
            max_packets: usize::MAX,
            sessions: session::list(true),
            deleted: session::list(false),
            show_recycle: false,
            show_tool_log: false,
            show_settings: false,
            show_graph_settings: false,
            theme: AppTheme::Dark,
            graph_use_colors: true,
            graph_show_grid: true,
            graph_show_markers: true,
            graph_show_cursor_labels: true,
            graphics_per_waveform_cursors: false,
            graphics_multi_cursors: HashMap::new(),
            graphics_multi_cursor_armed: None,
            graphics_multi_cursor_drag: None,
            graph_line_width: 1.7,
            show_start_page: false,
            home_session_search: String::new(),
            background_load: None,
            active_preload_channels: BTreeSet::new(),
            pending_log_path: None,
            pending_channels: vec![],
            pending_channel_selection: BTreeSet::new(),
            channel_picker_search: String::new(),
            channel_picker_filter: "All".to_string(),
            tecmp_source_time: true,
            perf_trace_query_ms: 0.0,
            perf_trace_render_ms: 0.0,
            show_perf_hud: false,
            trace_filter_pending: false,
            trace_filter_changed_at: None,
            tool_log: VecDeque::from([ToolLogEntry {
                time: tool_log_time(),
                level: "INFO".into(),
                message: format!("LogLens v{} started - {} - {} - diagnostics: {} - session storage: {}", env!("CARGO_PKG_VERSION"), native_engine::version(), resources::limits().memory_label(), diagnostics::log_directory().display(), session::storage_root().display()),
            }]),
            project_config: TableData::empty(),
            vehicle_db: TableData::empty(),
            event_log: TableData::empty(),
            channel_map: TableData::empty(),
            signal_table: TableData::empty(),
            signals: vec![],
            selected_channels: BTreeSet::new(),
            project_channels: BTreeSet::new(),
            selected_keys: BTreeSet::new(),
            activity_level: ActivityLevel::Bytes,
            explorer_mode: ExplorerMode::Activity,
            selected_frame_key: String::new(),
            selected_channel: None,
            inspect_byte: 0,
            inspect_bit: 0,
            reference_value: 0,
            compare_ids: false,
            compare_keys: BTreeSet::new(),
            multi_frame_keys:BTreeSet::new(),
            selected_frame_bytes: HashMap::new(),
            multi_hide_static:true,
            multi_hide_counter:true,
            multi_hide_crc_candidates:true,
            multi_activity_signature:String::new(),
            multi_activity_cache:Vec::new(),
            waveform_keys: BTreeSet::new(),
            waveform_byte: 0,
            waveform_bit: 0,
            raw_waveform_bytes: BTreeSet::new(),
            raw_waveform_stacked: false,
            candidates: vec![],
            candidates_ready: false,
            signal_discovery_scope: SignalDiscoveryScope::AllDigital,
            candidate_preview: None,
            time_start: 0.0,
            time_end: 0.0,
            use_manual_window: false,
            selected_event_row: None,
            selected_ecus: BTreeSet::new(),
            custom_name: "Unverified_CustomSignal".into(),
            custom_frame: String::new(),
            custom_start_byte: 0,
            custom_start_bit: 0,
            custom_bit_length: 8,
            custom_scale: 1.0,
            custom_offset: 0.0,
            custom_signed: false,
            custom_little_endian: true,
            signal_search: String::new(),
            waveform_overlap: true,
            cached_filter_signature: String::new(),
            filtered_indices: vec![],
            cached_stats: None,
            cached_scope_signature: String::new(),
            cached_scoped_indices: vec![],
            cached_graphics_sources: vec![],
            cached_waveforms: HashMap::new(),
            cached_decoded_signal_waveforms: HashMap::new(),
            cached_decoded_signal_indices: HashMap::new(),
            cached_analog_waveforms: HashMap::new(),
            cached_analog_view: HashMap::new(),
            cached_cursor_rows_key: String::new(),
            cached_cursor_rows: Vec::new(),
            signal_config_dirty: false,
            cached_trace_scope_signature: String::new(),
            cached_trace_processed_count: 0,
            cached_trace_positions: HashMap::new(),
            cached_trace_rows: Arc::new(vec![]),
            trace_view_cache: HashMap::new(),
            trace_view_cache_scope: String::new(),
            trace_columns: crate::trace_columns::TraceColumns::default(),
            cached_change_scope_signature: String::new(),
            cached_changing_trace_sources: BTreeSet::new(),
            cached_changing_graphics_sources: BTreeSet::new(),
            measurement_running: false,
            measurement_paused: false,
            replay_speed: 1.0,
            selected_trace_row: None,
            selected_trace_rows: BTreeSet::new(),
            trace_row_anchor: None,
            graphics_bytes: BTreeSet::new(),
            graphics_sources: BTreeSet::new(),
            graphics_signals: BTreeSet::new(),
            graphics_bits: BTreeSet::new(),
            graphics_whole_frames: BTreeSet::new(),
            graphics_hide_constant: false,
            graphics_hide_repetitive: false,
            graphics_repetitive_cache: HashMap::new(),
            graphics_stacked: true,
            graphics_auto_y: true,
            graphics_lane_height: 150.0,
            graphics_lane_scroll_y: 0.0,
            graphics_y_scale: HashMap::new(),
            graphics_y_offset: HashMap::new(),
            graphics_lane_order: Vec::new(),
            graphics_drag_lane: None,
            graphics_drag_cursor: None,
            graphics_active_cursor: 'A',
            graphics_show_active: true,
            graphics_show_static: false,
            graphics_show_counter: false,
            graphics_show_crc: false,
            graphics_show_pattern: true,
            graphics_class_cache: HashMap::new(),
            graphics_source_class_ready: BTreeSet::new(),
            graphics_class_window: None,
            graphics_tree_open: BTreeSet::new(),
            graphics_show_event_markers: true,
            cursor_a_s: 0.0,
            cursor_b_s: 0.0,
            show_network_tree: true,
            explorer_search: String::new(),
            explorer_protocol: "All".into(),
            inspector_pane: InspectorPane::Details,
            inspector_height: 260.0,
            inspector_width: 460.0,
            project_tree_width: 360.0,
            inspector_dock: InspectorDock::Bottom,
            floating_panes: BTreeSet::new(),
            graphics_view_start_s: 0.0,
            graphics_range_start_s: None,
            graphics_range_end_s: None,
            graphics_range_manual_start: String::new(),
            graphics_range_manual_end: String::new(),
            graphics_range_manual_dirty: false,
            graphics_range_dragging: false,
            graphics_range_changed_only: true,
            graphics_range_interaction: 0,
            graphics_range_last_pointer_s: None,
            graphics_range_cached_bounds: None,
            graphics_range_cached_bytes: BTreeMap::new(),
            graphics_view_end_s: 1.0,
            graphics_follow_replay: true,
            replay_position_s: 0.0,
            replay_anchor: None,
            trace_follow_replay: true,
            trace_overwrite: false,
            trace_only_changing: false,
            graphics_only_changing: false,
            trace_freeze_view: false,
            trace_display_position_s: 0.0,
            show_export_manager: false,
            export_range: ExportRange::CursorAB,
            export_content: ExportContent::CurrentSelection,
            export_format: ExportFormat::Excel,
            export_skip_constant_frames: false,
            workspace_cache_signature: String::new(),
            workspace_cache_last_save: Instant::now(),
        }
    }

    fn refresh_sessions(&mut self) {
        self.sessions = session::list(true);
        self.deleted = session::list(false);
    }

    fn log_tool(&mut self, level: &str, message: impl Into<String>) {
        let message=message.into();
        diagnostics::log(level,&message);
        self.tool_log.push_back(ToolLogEntry {
            time: tool_log_time(),
            level: level.to_string(),
            message,
        });
        while self.tool_log.len() > 500 { self.tool_log.pop_front(); }
    }

    fn invalidate_dataset_graphics_caches(&mut self) {
        // Raw waveforms and classification are dataset-derived. Classification readiness
        // must always be invalidated together with its cache (v0.46.1 invariant).
        self.cached_waveforms.clear();
        self.cached_decoded_signal_waveforms.clear();
        self.cached_decoded_signal_indices.clear();
        self.cached_analog_waveforms.clear();
        self.cached_analog_view.clear();
        self.cached_cursor_rows_key.clear();
        self.cached_cursor_rows.clear();
        self.graphics_multi_cursors.clear();
        self.graphics_multi_cursor_armed=None;
        self.graphics_multi_cursor_drag=None;
        self.graphics_class_cache.clear();
        self.graphics_repetitive_cache.clear();
        self.graphics_source_class_ready.clear();
        self.graphics_class_window=None;
    }

    fn trim_graphics_caches_for_resource_limits(&mut self) {
        let limit=resources::limits().waveform_cache_entries.max(16);
        while self.cached_waveforms.len()+self.cached_decoded_signal_waveforms.len()+self.cached_decoded_signal_indices.len()+self.cached_analog_waveforms.len() >= limit {
            if let Some(k)=self.cached_analog_waveforms.keys().next().cloned() { self.cached_analog_waveforms.remove(&k); continue; }
            if let Some(k)=self.cached_decoded_signal_waveforms.keys().next().cloned() { self.cached_decoded_signal_waveforms.remove(&k); continue; }
            if let Some(k)=self.cached_decoded_signal_indices.keys().next().cloned() { self.cached_decoded_signal_indices.remove(&k); continue; }
            if let Some(k)=self.cached_waveforms.keys().next().cloned() { self.cached_waveforms.remove(&k); continue; }
            break;
        }
    }

    fn apply_signal_configuration(&mut self) {
        ensure_signal_position_columns(&mut self.signal_table);
        ensure_signal_data_type_column(&mut self.signal_table);
        self.signals=signal_definitions(&self.signal_table);
        let valid=self.signals.iter().map(|x|x.name.clone()).collect::<BTreeSet<_>>();
        self.graphics_signals.retain(|name|valid.contains(name));
        self.graphics_lane_order.retain(|lane|!lane.starts_with("SIG|") || valid.contains(lane.trim_start_matches("SIG|")));
        // Signal definitions affect only decoded signal waveforms. Raw byte waveforms and
        // byte classification are capture-derived and intentionally remain valid.
        self.cached_decoded_signal_waveforms.clear();
        self.cached_decoded_signal_indices.clear();
        self.signal_config_dirty=false;
    }

    fn appearance_settings_ui(&mut self,ui:&mut egui::Ui) {
                ui.strong("Appearance");
                ui.separator();
                let previous_theme=self.theme;
                ui.horizontal(|ui| {
                    ui.label("Theme");
                    egui::ComboBox::from_id_salt("app_theme")
                        .selected_text(self.theme.label())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.theme, AppTheme::Dark, "Dark");
                            ui.selectable_value(&mut self.theme, AppTheme::Light, "Light");
                        });
                });
                ui.small("Dark is the default theme for new installations.");
                if self.theme != previous_theme {
                    ui.ctx().set_visuals(match self.theme {
                        AppTheme::Dark => egui::Visuals::dark(),
                        AppTheme::Light => egui::Visuals::light(),
                    });
                    match session::save_app_preferences(&AppPreferences { theme:self.theme }) {
                        Ok(()) => self.log_tool("INFO", format!("Theme changed to {}", self.theme.label())),
                        Err(error) => self.log_tool("WARN", format!("Could not persist theme preference: {error:#}")),
                    }
                }
                ui.add_space(10.0);
    }

    fn graphics_settings_ui(&mut self,ui:&mut egui::Ui) {
                ui.strong("Graphics");
                ui.separator();
                ui.checkbox(&mut self.graph_use_colors,"Color graph")
                    .on_hover_text("Use stable per-waveform colors. Disable for a single theme-safe waveform color.");
                ui.checkbox(&mut self.graph_show_grid,"Grid");
                ui.checkbox(&mut self.graph_show_markers,"CM / event markers");
                ui.checkbox(&mut self.graph_show_cursor_labels,"A / B cursor labels");
                ui.add_space(6.0);
                ui.label("Cursor mode");
                ui.radio_value(&mut self.graphics_per_waveform_cursors,false,"Global A/B cursors");
                ui.radio_value(&mut self.graphics_per_waveform_cursors,true,"Per-waveform multi-cursors");
                ui.small(if self.graphics_per_waveform_cursors {
                    "Each waveform owns independent C1..C8 measurement points."
                } else {
                    "A/B cross all waveforms and provide the existing delta measurement."
                });
                if !self.graphics_per_waveform_cursors { self.graphics_multi_cursor_armed=None; }
                else { self.graphics_stacked=true; }
                ui.horizontal(|ui|{
                    ui.label("Line width");
                    ui.add(egui::Slider::new(&mut self.graph_line_width,0.8..=3.5).step_by(0.1));
                });
                ui.add_space(8.0);
                ui.strong("Display");
                ui.separator();
                if ui.checkbox(&mut self.show_raw_ids,"Show raw IDs with configured names").changed(){self.trace_view_cache.clear();}
                ui.checkbox(&mut self.graphics_auto_y,"Auto Y scale");
                ui.horizontal(|ui|{
                    ui.label("Waveform layout");
                    ui.radio_value(&mut self.graphics_stacked,true,"Stacked");
                    ui.radio_value(&mut self.graphics_stacked,false,"Overlay");
                });
                if ui.button("Fit Y now").on_hover_text("Reset manual per-lane Y scale/offset and fit the visible window").clicked(){
                    self.graphics_y_scale.clear();
                    self.graphics_y_offset.clear();
                    self.graphics_auto_y=true;
                }
                ui.add_space(8.0);
                if ui.button("Reset graphics defaults").clicked(){
                    self.graph_use_colors=true;
                    self.graph_show_grid=true;
                    self.graph_show_markers=true;
                    self.graph_show_cursor_labels=true;
                    self.graphics_per_waveform_cursors=false;
                    self.graphics_multi_cursor_armed=None;
                    self.graph_line_width=1.7;
                    self.graphics_density=GraphicsDensity::Standard;self.graphics_lane_height=150.0;
                    self.graphics_auto_y=true;
                    self.graphics_stacked=true;
                }

        ui.separator();ui.label("Detail level");
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.graphics_density,GraphicsDensity::Compact,"Compact");
            ui.radio_value(&mut self.graphics_density,GraphicsDensity::Standard,"Standard");
            ui.radio_value(&mut self.graphics_density,GraphicsDensity::Detailed,"Detailed");
        });
        ui.add(egui::Slider::new(&mut self.graphics_lane_height,72.0..=180.0).text("Lane height"));
    }

    fn graph_settings_panel(&mut self,ctx:&egui::Context) {
        if !self.show_graph_settings{return;}
        let mut open=true;
        egui::Window::new("Graph Settings").id(egui::Id::new("graph_settings"))
            .open(&mut open).default_width(350.0).resizable(true).show(ctx,|ui| {
                egui::ScrollArea::vertical().max_height(620.0).show(ui,|ui| {
                    self.appearance_settings_ui(ui);self.graphics_settings_ui(ui);
                });
            });
        self.show_graph_settings=open;
    }

    fn settings_panel(&mut self, ctx:&egui::Context) {
        if !self.show_settings{return;}
        let mut open=self.show_settings;
        egui::Window::new("Settings")
            .id(egui::Id::new("loglens_settings"))
            .open(&mut open)
            .default_width(330.0)
            .resizable(true)
            .anchor(egui::Align2::RIGHT_TOP,[-12.0,46.0])
            .show(ctx,|ui|{
                self.appearance_settings_ui(ui);
                self.graphics_settings_ui(ui);
            });
        self.show_settings=open;
    }

    fn tool_log_panel(&mut self, ctx: &egui::Context) {
        if !self.show_tool_log { return; }
        egui::TopBottomPanel::bottom("tool_log_panel")
            .resizable(true)
            .default_height(155.0)
            .min_height(90.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.strong("Tool Log");
                    ui.weak(format!("{} entries", self.tool_log.len()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Close").clicked() { self.show_tool_log = false; }
                        if ui.button("Clear").clicked() { self.tool_log.clear(); }
                        if ui.button("Open Diagnostics Folder").clicked() {
                            match diagnostics::open_log_directory() {
                                Ok(()) => self.log_tool("INFO",format!("Opened diagnostics folder: {}",diagnostics::log_directory().display())),
                                Err(e) => { self.log_tool("WARN",&e); self.error=Some(e); }
                            }
                        }
                    });
                });
                ui.separator();
                egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                    for entry in &self.tool_log {
                        ui.horizontal(|ui| {
                            ui.monospace(&entry.time);
                            let color = match entry.level.as_str() {
                                "ERROR" => Color32::LIGHT_RED,
                                "WARN" => Color32::YELLOW,
                                _ => ui.visuals().text_color(),
                            };
                            ui.colored_label(color, format!("{:5}", entry.level));
                            ui.label(&entry.message);
                        });
                    }
                });
            });
    }

    fn open_log(&mut self) {
        if let Some(p) = rfd::FileDialog::new()
            .add_filter("Vehicle log", &["pcap", "pcapng", "csv", "xlsx", "xls", "txt", "log"])
            .pick_file()
        {
            self.log_tool("INFO", format!("Open requested: {}", p.display()));
            let ext=p.extension().and_then(|x|x.to_str()).unwrap_or("").to_ascii_lowercase();
            if ext=="pcap" || ext=="pcapng" {
                self.prepare_capture_selection(p,None);
            } else if self.load_path(p, None) { self.show_start_page = false; }
        }
    }

    fn can_change_channels(&self) -> bool {
        self.dataset.is_some() && self.background_load.is_none() && self.path.as_ref()
            .and_then(|p|p.extension()).and_then(|e|e.to_str())
            .is_some_and(|e|e.eq_ignore_ascii_case("pcap") || e.eq_ignore_ascii_case("pcapng"))
    }

    fn change_capture_channels(&mut self) -> bool {
        if !self.can_change_channels() { return false; }
        let path=self.path.clone().unwrap();
        let selection=if self.active_preload_channels.is_empty(){None}else{Some(self.active_preload_channels.clone())};
        self.prepare_capture_selection(path,selection)
    }

    fn prepare_capture_selection(&mut self,path:PathBuf,selection:Option<BTreeSet<u32>>) -> bool {
        // Discover from the complete capture, not the loaded subset: deselected
        // channels must remain available when returning to the selector.
        match discover_capture_channels(&path,self.max_packets) {
            Ok(channels)=>{
                if self.path.as_ref()!=Some(&path){self.tecmp_source_time=true;}
                let available=channels.iter().map(|(id,_)|*id).collect::<BTreeSet<_>>();
                self.pending_channel_selection=selection.map(|s|s.intersection(&available).copied().collect()).unwrap_or(available);
                self.pending_channels=channels;
                self.pending_log_path=Some(path);
                self.channel_picker_search.clear();
                self.channel_picker_filter = "All".to_string();
                // Keep auxiliary settings from covering the dedicated channel-selection screen.
                self.show_settings=false;
                self.show_start_page=true;
                self.info=Some("Change the channels, then click Load Selected Channels. Return to Workspace keeps the current loaded data.".into());
                self.error=None;
                true
            },
            Err(e)=>{self.error=Some(format!("Channel scan failed: {e:#}"));false},
        }
    }

    fn return_to_workspace(&mut self) {
        self.show_start_page=false;
        self.pending_log_path=None; self.pending_channels.clear(); self.pending_channel_selection.clear();
        if let Some(d)=self.dataset.as_ref(){self.tecmp_source_time=d.source_time;}
    }

    fn load_path(&mut self, p: PathBuf, target_session: Option<SessionMeta>) -> bool {
        self.load_path_with_scope(p,target_session,false)
    }
    fn load_path_with_scope(&mut self, p: PathBuf, target_session: Option<SessionMeta>, retain_channel_scope:bool) -> bool {
        if self.background_load.is_some() {
            self.info=Some("A capture is already loading.".into());
            return false;
        }
        let preserve_graphics=self.path.as_ref().map(|old| old==&p).unwrap_or(false);
        let max_packets=self.max_packets;
        let (tx,rx)=mpsc::channel::<LoadMessage>();
        let worker_path=p.clone();
        let load_channel_filter=if self.pending_log_path.as_ref()==Some(&p) { Some(self.pending_channel_selection.clone()) } else { None };
        self.persist_session_configuration();
        self.persist_workspace_if_changed(true);
        let channels=load_channel_filter.clone();
        let source_time=target_session.as_ref().and_then(|m|session::load_workspace(&m.id).ok().flatten())
            .and_then(|w|w.analysis_settings.get("tecmp_source_time").and_then(|v|v.as_bool())).unwrap_or(self.tecmp_source_time);
        self.log_tool("INFO",format!("Background load started: {}",p.display()));
        let resource_snapshot=resources::limits();
        let file_mib=std::fs::metadata(&p).map(|m|m.len() as f64/(1024.0*1024.0)).unwrap_or(0.0);
        self.log_tool("INFO",format!(
            "Load diagnostics: file={:.1} MiB, {}, capture memory budget={:.0} MiB; text/spreadsheet row cap={}",
            file_mib, resource_snapshot.memory_label(), resource_snapshot.capture_memory_budget() as f64/1048576.0, resource_snapshot.effective_packet_limit(max_packets)
        ));
        self.error=None;
        self.info=Some(format!("Loading {}...",p.file_name().and_then(|x|x.to_str()).unwrap_or("capture")));
        let worker_start = thread::Builder::new().name("loglens-loader".into()).spawn(move || {
            diagnostics::set_context(format!("Loading/decoding {}",worker_path.file_name().and_then(|x|x.to_str()).unwrap_or("capture")));
            diagnostics::log("INFO",format!("Loader worker entered: {}",worker_path.display()));
            let _=tx.send(LoadMessage::Stage("Reading and decoding capture",10));
            let result=panic::catch_unwind(AssertUnwindSafe(|| crate::engine::load_selected_with_timeline(&worker_path,max_packets,load_channel_filter.as_ref(),source_time)));
            let converted=match result {
                Ok(Ok(d)) => { diagnostics::log("INFO",format!("Loader worker decoded {} normalized frames",d.frames.len())); Ok(d) },
                Ok(Err(e)) => { let msg=format!("{e:#}"); diagnostics::log("ERROR",format!("Loader worker failed: {msg}")); Err(msg) },
                Err(_) => { diagnostics::log("ERROR","Loader worker panic was contained while decoding the capture"); Err("Loader worker panicked while decoding the capture".into()) },
            };
            let _=tx.send(LoadMessage::Stage("Finalizing dataset",90));
            let _=tx.send(LoadMessage::Finished(converted));
        }).map_err(|e|e.to_string());

        if let Err(e) = worker_start {
            let msg=format!("Could not start loader thread: {e}");
            self.log_tool("ERROR",&msg);
            self.error=Some(msg);
        }
        if self.error.is_none() {
            self.background_load=Some(BackgroundLoad {
                path:p, target_session, channels, preserve_graphics, retain_channel_scope, rx,
                stage:"Starting loader".into(), progress:1,
            });
            true
        } else {
            false
        }
    }

    fn poll_background_load(&mut self, ctx:&egui::Context) {
        let mut finished=None;
        if let Some(job)=self.background_load.as_mut() {
            loop {
                match job.rx.try_recv() {
                    Ok(LoadMessage::Stage(stage,p)) => { job.stage=stage.into(); job.progress=p; }
                    Ok(LoadMessage::Finished(result)) => { finished=Some(result); break; }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        finished=Some(Err("Loader worker stopped unexpectedly. LogLens recovered without closing; retry with fewer channels or a smaller capture.".into()));
                        break;
                    }
                }
            }
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        if let Some(result)=finished {
            let Some(job)=self.background_load.take() else {
                let msg="Loader completion arrived without an active load job; the inconsistent state was ignored safely.".to_string();
                self.log_tool("ERROR",&msg);
                self.error=Some(msg);
                return;
            };
            match result {
                Ok(d)=>self.apply_loaded_dataset_with_scope(job.path,job.preserve_graphics,d,job.target_session,job.channels,job.retain_channel_scope),
                Err(msg)=>{
                    self.log_tool("ERROR",format!("Failed to load {}: {}",job.path.display(),msg));
                    self.tecmp_source_time=self.dataset.as_ref().map(|d|d.source_time).unwrap_or(self.tecmp_source_time);
                    self.error=Some(msg); self.info=None;
                }
            }
        }
    }

    fn loading_overlay(&mut self,ctx:&egui::Context) {
        let Some(job)=self.background_load.as_ref() else{return;};
        let stage=job.stage.clone(); let progress=job.progress;
        egui::Window::new("Loading capture")
            .collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER,[0.0,0.0])
            .show(ctx,|ui|{
                ui.set_min_width(380.0);
                ui.strong(stage);
                ui.add_space(8.0);
                ui.add(egui::ProgressBar::new(progress as f32/100.0).show_percentage());
                ui.add_space(6.0);
                ui.weak("Loading runs on a background worker. LogLens remains responsive.");
            });
    }

    fn apply_loaded_dataset(&mut self,p:PathBuf,preserve_graphics:bool,d:Dataset,target_session:Option<SessionMeta>,channels:Option<BTreeSet<u32>>) {
        self.apply_loaded_dataset_with_scope(p,preserve_graphics,d,target_session,channels,false);
    }
    fn apply_loaded_dataset_with_scope(&mut self,p:PathBuf,preserve_graphics:bool,d:Dataset,target_session:Option<SessionMeta>,channels:Option<BTreeSet<u32>>,retain_channel_scope:bool) {
        if retain_channel_scope && d.warnings.iter().any(|w|w.starts_with("Safe load limit reached")) && self.dataset.as_ref().is_some_and(|old|
            d.frames.len()<old.frames.len() || (old.source_time==d.source_time && d.frames.last().map(|f|f.timestamp_ns)<old.frames.last().map(|f|f.timestamp_ns))) {
            self.pending_log_path=None;self.pending_channels.clear();self.pending_channel_selection.clear();
            self.tecmp_source_time=self.dataset.as_ref().map(|d|d.source_time).unwrap_or(self.tecmp_source_time);
            self.info=Some("Reload reached a smaller load limit. Kept the current data and channel selections; no analysis changes were reset.".into());
            self.log_tool("WARN","Partial reload would reduce loaded coverage; retained current dataset and selections");return;
        }
        let previous_sources:BTreeSet<_>=self.cached_graphics_sources.iter().map(|s|s.0.clone()).collect();
        let preload_scope_changed=channels.as_ref().is_some_and(|selected|selected!=&self.active_preload_channels);
        let explicit_channel_selection=target_session.is_none() && channels.is_some();
        let preserved_workspace=preserve_graphics.then(||self.workspace_snapshot());
        let loaded_channels=d.index.project_channel_keys.clone();
        let load_warnings=d.warnings.clone();
        // Save the outgoing capture before changing any dataset or selection state.
        self.persist_session_configuration();
        self.persist_workspace_if_changed(true);
        let next_session=match target_session {
            Some(meta)=> { let _=session::touch(&meta.id); Some(meta) },
            None=>match session::create(&p) {
                Ok(meta)=>Some(meta),
                Err(e)=> { self.log_tool("WARN",format!("Could not save Recent Session: {e:#}")); None }
            },
        };
        let had_dataset=self.dataset.is_some();
        if !preserve_graphics { self.reset_analysis_settings(); }
        self.tecmp_source_time=d.source_time;
        if had_dataset && !preserve_graphics {
            self.project_config=TableData::empty(); self.channel_map=TableData::empty();
            self.vehicle_db=TableData::empty(); self.signal_table=TableData::empty();
            self.event_log=TableData::empty(); self.signals.clear(); self.signal_config_dirty=false;
        }
        self.session=next_session;
        self.workspace_cache_signature.clear();
        let saved_graphics_sources=self.graphics_sources.clone();
        let saved_graphics_bytes=self.graphics_bytes.clone();
        let saved_graphics_signals=self.graphics_signals.clone();
        let saved_graphics_bits=self.graphics_bits.clone();
        let saved_graphics_stacked=self.graphics_stacked;
        let saved_graphics_lane_height=self.graphics_lane_height;
        let saved_graphics_show_event_markers=self.graphics_show_event_markers;
        let saved_graphics_view=(self.graphics_view_start_s,self.graphics_view_end_s);
        let saved_graphics_follow_replay=self.graphics_follow_replay;
        let saved_graphics_only_changing=self.graphics_only_changing;
        let saved_graphics_range=(self.graphics_range_start_s,self.graphics_range_end_s,self.graphics_range_changed_only);
        self.log_tool("INFO",format!("Background decode completed: {}",p.display()));

                let stats = statistics(&d.frames);
                if d.frames.len() <= 250_000 {
                    match native_engine::analyze_frames(&d.frames) {
                        Ok(ns) => self.log_tool("INFO", format!(
                            "{} - {} frames, {} unique frame keys, {} payload changes, {} repeated/static frames",
                            native_engine::version(), ns.total_frames, ns.unique_keys, ns.changed_frames, ns.static_frames
                        )),
                        Err(e) => self.log_tool("WARN", format!("Native analyzer fallback: {e:#}")),
                    }
                } else {
                    // Large-log fast path: do not duplicate every payload into an FFI
                    // statistics buffer during open. The persistent FrameIndex already
                    // contains the unique frame-key index needed by the UI.
                    self.log_tool("INFO", format!(
                        "{} - large-log fast path ACTIVE: {} frames, {} indexed frame keys; deferred payload statistics",
                        native_engine::version(), d.frames.len(), d.index.by_key.len()
                    ));
                }
                // Keep analysis/replay coordinates relative internally for numeric stability.
                // UI timestamps are rendered from the first/last recorded absolute timestamps.
                self.time_start = 0.0;
                self.time_end = stats.duration_s;
                // Analysis-window results are dataset-derived and must never survive a log swap.
                self.graphics_range_cached_bounds=None;
                self.graphics_range_cached_bytes.clear();
                if preserve_graphics {
                    self.graphics_view_start_s = saved_graphics_view.0.max(0.0).min(stats.duration_s.max(0.001));
                    self.graphics_view_end_s = saved_graphics_view.1.max(self.graphics_view_start_s + 0.001).min(stats.duration_s.max(0.001));
                } else {
                    self.graphics_view_start_s = 0.0;
                    self.graphics_view_end_s = stats.duration_s.max(0.001);
                }
                self.replay_position_s = 0.0;
                self.replay_anchor = None;
                self.trace_display_position_s = 0.0;
                self.trace_freeze_view = false;
                self.measurement_running = false;
                self.measurement_paused = false;
                self.selected_frame_key = d.frames.first().map(|f| f.frame_key.clone()).unwrap_or_default();
                if preserve_graphics {
                    self.graphics_sources = saved_graphics_sources;
                    self.graphics_bytes = saved_graphics_bytes;
                    self.graphics_signals = saved_graphics_signals;
                    self.graphics_bits = saved_graphics_bits;
                    self.graphics_stacked = saved_graphics_stacked;
                    self.graphics_lane_height = saved_graphics_lane_height;
                    self.graphics_show_event_markers = saved_graphics_show_event_markers;
                    self.graphics_follow_replay = saved_graphics_follow_replay;
                    self.graphics_only_changing = saved_graphics_only_changing;
                    self.graphics_range_start_s=saved_graphics_range.0;
                    self.graphics_range_end_s=saved_graphics_range.1;
                    self.graphics_range_changed_only=saved_graphics_range.2;
                } else {
                    // v0.48.7: automatically focus analysis on meaningful testcase activity.
                    // Opening a capture shows its complete loaded span. Activity
                    // cropping remains an explicit Auto Activity action.
                    self.graphics_range_start_s=None;self.graphics_range_end_s=None;
                    self.graphics_sources.clear();
                    self.selected_frame_bytes.clear();
                    self.graphics_lane_order.clear();
                    if let Some(f) = d.frames.first() {
                        self.graphics_sources.insert(graphics_source_key(&f.bus_type.to_string(), f.channel_id, &f.frame_key));
                    }
                    self.graphics_bytes.clear();
                    self.graphics_bits.clear();
                    self.graphics_signals.clear();
                }
                self.custom_frame = self.selected_frame_key.clone();
                self.waveform_keys.clear();
                if !self.selected_frame_key.is_empty() { self.waveform_keys.insert(self.selected_frame_key.clone()); }
                self.project_channels = d.index.project_channel_keys.clone();
                self.cached_graphics_sources = graphics_available_sources(&d.frames);
                self.graphics_eth_endpoints=crate::eth_filter::EthernetEndpoints::build(&d.frames);
                self.graphics_eth_filters.clear();
                self.graphics_frame_id_filter = Default::default();
                // Analog is already a physical sampled signal. On a new capture make discovered
                // TECMP Analog channels visible immediately instead of requiring byte-style setup.
                if !preserve_graphics {
                    for (source_key,bus,_ch,_frame_key,_dlc) in &self.cached_graphics_sources {
                        if bus=="ANALOG" {
                            self.graphics_sources.insert(source_key.clone());
                            self.selected_frame_bytes.insert(source_key.clone(),BTreeSet::from([0usize]));
                            let lane=format!("SRC|{}|0",source_key);
                            if !self.graphics_lane_order.contains(&lane){self.graphics_lane_order.push(lane);}
                        }
                    }
                }
                self.invalidate_dataset_graphics_caches();
                self.cached_scope_signature.clear();
                self.cached_scoped_indices.clear();
                self.cached_trace_scope_signature.clear();
                self.cached_trace_processed_count = 0;
                self.cached_trace_positions.clear();
                Arc::make_mut(&mut self.cached_trace_rows).clear();
                self.trace_view_cache.clear();self.trace_view_cache_scope.clear();
                self.cached_change_scope_signature.clear();
                self.cached_changing_trace_sources.clear();
                self.cached_changing_graphics_sources.clear();
                self.dataset = Some(Arc::new(d));
                self.path = Some(p.clone());
                self.workspace = Workspace::Measurement;
                self.show_network_tree = true;
                // New captures start with Frame Inspector; reloading a running
                // graphics session keeps the existing selected inspector pane.
                if !preserve_graphics { self.inspector_pane = InspectorPane::Details; }
                self.cached_stats = Some(stats);
                self.candidates.clear();
                self.candidates_ready = false;
                self.cached_filter_signature.clear();
                self.filtered_indices.clear();
                self.error = None;
                self.info = Some(format!("Decoded {}", p.file_name().and_then(|x| x.to_str()).unwrap_or("log")));
                self.log_tool("INFO", format!("Decoded {} frames from {}", self.cached_stats.as_ref().map(|s| s.total).unwrap_or(0), p.file_name().and_then(|x| x.to_str()).unwrap_or("log")));
                for warning in &load_warnings { self.log_tool("WARN",warning); }
                self.info=Some(load_completion_summary(&load_warnings,self.cached_stats.as_ref().map(|s|s.duration_s).unwrap_or(0.0)));

                if let Some(id)=self.session.as_ref().map(|m|m.id.clone()) {
                    if !preserve_graphics {
                        self.restore_session_configuration(&id);
                        self.restore_workspace(&id);
                    }
                }
                if let Some(w)=preserved_workspace {self.apply_workspace_snapshot(w);}
                if explicit_channel_selection {
                    if !retain_channel_scope {self.project_channels=loaded_channels.clone();}
                    let plotted_in_scope=self.graphics_sources.iter().any(|source|parse_graphics_source_key(source).is_some_and(|(bus,ch,_)|self.project_channels.contains(&project_channel_key(&bus,ch))))
                        || self.signals.iter().any(|signal|self.graphics_signals.contains(&signal.name) && self.cached_graphics_sources.iter().any(|(_,bus,ch,key,_)|self.project_channels.contains(&project_channel_key(bus,*ch)) && signal_matches_source(bus,key,*ch,signal)));
                    for (key,bus,ch,_,_) in &self.cached_graphics_sources {
                        if !retain_channel_scope && bus=="ANALOG" && self.project_channels.contains(&project_channel_key(bus,*ch)) && (!self.active_preload_channels.contains(ch) || !previous_sources.contains(key) || (preload_scope_changed && !plotted_in_scope)) {
                            self.graphics_sources.insert(key.clone());
                            self.selected_frame_bytes.entry(key.clone()).or_insert_with(||BTreeSet::from([0]));
                        }
                    }
                }
                if explicit_channel_selection && !preserve_graphics && !retain_channel_scope {
                    self.project_channels=loaded_channels;
                    for (key,bus,_,_,_) in &self.cached_graphics_sources {
                        if bus=="ANALOG" {
                            self.graphics_sources.insert(key.clone());
                            self.selected_frame_bytes.insert(key.clone(),BTreeSet::from([0usize]));
                            let lane=format!("SRC|{key}|0");
                            if !self.graphics_lane_order.contains(&lane){self.graphics_lane_order.push(lane);}
                        }
                    }
                    self.graphics_view_start_s=0.0;
                    self.graphics_view_end_s=self.cached_stats.as_ref().map(|s|s.duration_s).unwrap_or(0.001).max(0.001);
                    self.time_start=0.0; self.time_end=self.graphics_view_end_s;
                    self.use_manual_window=false;
                    self.graphics_range_start_s=None; self.graphics_range_end_s=None;
                    self.graphics_range_cached_bounds=None; self.graphics_range_cached_bytes.clear();
                    self.cursor_a_s=0.0; self.cursor_b_s=self.graphics_view_end_s;
                    self.graphics_multi_cursors.clear();
                }
                self.active_preload_channels=channels.unwrap_or_default();
                self.pending_log_path=None; self.pending_channels.clear(); self.pending_channel_selection.clear();
                self.show_start_page=false;
                self.persist_session_configuration();
                self.persist_workspace_if_changed(true);
                self.refresh_sessions();
    }

    fn load_saved_session(&mut self, m: SessionMeta) {
        let saved=PathBuf::from(&m.source_path);
        let original=PathBuf::from(&m.original_source_path);
        let p=if saved.exists(){saved}
            else if !m.original_source_path.is_empty() && original.exists(){
                self.log_tool("WARN",format!("Persistent session copy missing; using original capture: {}",original.display()));
                original
            } else {
                let msg=format!("Recent Session is listed, but its capture is unavailable: {}",m.filename);
                self.log_tool("ERROR",&msg); self.error=Some(msg); self.refresh_sessions(); return;
            };
        if self.background_load.is_some() { self.info=Some("A capture is already loading.".into()); return; }
        self.log_tool("INFO",format!("Opening Recent Session: {}",m.filename));
        // Reopen directly with the saved channel scope. Do not change active configuration
        // or workspace until the worker successfully returns the new dataset.
        let selection=match session::load_configuration(&m.id) {
            Ok(Some(config))=>config.preload_channels,
            Ok(None)=>BTreeSet::new(),
            Err(e)=> { self.error=Some(format!("Could not read saved session: {e:#}")); return; }
        };
        self.pending_log_path=None;
        if !selection.is_empty() {
            self.pending_log_path=Some(p.clone());
            self.pending_channel_selection=selection;
        }
        if self.load_path(p,Some(m)) { self.show_start_page=false; }
    }

    fn new_session(&mut self) {
        self.persist_session_configuration();
        self.persist_workspace_if_changed(true);
        self.log_tool("INFO", "New Session");
        self.background_load=None;
        self.pending_log_path=None; self.pending_channels.clear(); self.pending_channel_selection.clear();
        self.active_preload_channels.clear(); self.reset_analysis_settings();
        self.dataset = None;
        self.path = None;
        self.session = None;
        self.project_config = TableData::empty();
        self.vehicle_db = TableData::empty();
        self.event_log = TableData::empty();
        self.channel_map = TableData::empty();
        self.signal_table = TableData::empty();
        self.signals.clear();
        self.signal_config_dirty = false;
        self.error = None;
        self.info = None;
        self.selected_channels.clear();
        self.project_channels.clear();
        self.selected_keys.clear();
        self.waveform_keys.clear();
        self.graphics_bytes.clear();
        self.graphics_sources.clear();
        self.selected_frame_bytes.clear();
        self.graphics_bits.clear();
        self.graphics_lane_order.clear();
        self.graphics_signals.clear();
        self.filtered_indices.clear();
        self.cached_filter_signature.clear();
        self.cached_stats = None;
        self.cached_scope_signature.clear();
        self.cached_scoped_indices.clear();
        self.cached_graphics_sources.clear();
        self.graphics_eth_endpoints=Default::default();self.graphics_eth_filters.clear();
                self.graphics_frame_id_filter = Default::default();
        self.invalidate_dataset_graphics_caches();
        self.cached_trace_scope_signature.clear();
        self.cached_trace_processed_count = 0;
        self.cached_trace_positions.clear();
        Arc::make_mut(&mut self.cached_trace_rows).clear();
        self.cached_change_scope_signature.clear();
        self.cached_changing_trace_sources.clear();
        self.cached_changing_graphics_sources.clear();
        self.graphics_range_cached_bounds=None;
        self.graphics_range_cached_bytes.clear();
        self.candidates.clear();
        self.candidates_ready = false;
    }

    fn analysis_settings(&self) -> BTreeMap<String, serde_json::Value> {
        let mut settings=BTreeMap::new();
        settings.insert("graph_use_colors".into(),serde_json::json!(self.graph_use_colors));
        settings.insert("graph_show_grid".into(),serde_json::json!(self.graph_show_grid));
        settings.insert("graph_show_markers".into(),serde_json::json!(self.graph_show_markers));
        settings.insert("graph_show_cursor_labels".into(),serde_json::json!(self.graph_show_cursor_labels));
        settings.insert("graph_line_width".into(),serde_json::json!(self.graph_line_width));
        settings.insert("graphics_activity_filter".into(),serde_json::json!(self.graphics_activity_filter));
        settings.insert("graph_channel_names".into(),serde_json::json!(self.graph_channel_names));
        settings.insert("graph_lane_names".into(),serde_json::json!(self.graph_lane_names));
        settings.insert("graphics_eth_filters".into(),serde_json::json!(self.graphics_eth_filters));
        settings.insert("graphics_frame_id_filter".into(),serde_json::json!(self.graphics_frame_id_filter));
        settings.insert("tecmp_source_time".into(),serde_json::json!(self.dataset.as_ref().map(|d|d.source_time).unwrap_or(self.tecmp_source_time)));
        settings.insert("trace_columns".into(),serde_json::to_value(&self.trace_columns).expect("serializable trace columns"));
        settings.insert("workspace".into(), serde_json::to_value(&self.workspace).expect("serializable analysis setting"));
        settings.insert("analysis_page".into(), serde_json::to_value(&self.analysis_page).expect("serializable analysis setting"));
        settings.insert("config_page".into(), serde_json::to_value(&self.config_page).expect("serializable analysis setting"));
        settings.insert("filter".into(), serde_json::to_value(&self.filter).expect("serializable analysis setting"));
        settings.insert("selected_channels".into(), serde_json::to_value(&self.selected_channels).expect("serializable analysis setting"));
        settings.insert("activity_level".into(), serde_json::to_value(&self.activity_level).expect("serializable analysis setting"));
        settings.insert("explorer_mode".into(), serde_json::to_value(&self.explorer_mode).expect("serializable analysis setting"));
        settings.insert("inspect_byte".into(), serde_json::to_value(&self.inspect_byte).expect("serializable analysis setting"));
        settings.insert("inspect_bit".into(), serde_json::to_value(&self.inspect_bit).expect("serializable analysis setting"));
        settings.insert("reference_value".into(), serde_json::to_value(&self.reference_value).expect("serializable analysis setting"));
        settings.insert("compare_ids".into(), serde_json::to_value(&self.compare_ids).expect("serializable analysis setting"));
        settings.insert("compare_keys".into(), serde_json::to_value(&self.compare_keys).expect("serializable analysis setting"));
        settings.insert("multi_frame_keys".into(), serde_json::to_value(&self.multi_frame_keys).expect("serializable analysis setting"));
        settings.insert("multi_hide_static".into(), serde_json::to_value(&self.multi_hide_static).expect("serializable analysis setting"));
        settings.insert("multi_hide_counter".into(), serde_json::to_value(&self.multi_hide_counter).expect("serializable analysis setting"));
        settings.insert("multi_hide_crc_candidates".into(), serde_json::to_value(&self.multi_hide_crc_candidates).expect("serializable analysis setting"));
        settings.insert("waveform_keys".into(), serde_json::to_value(&self.waveform_keys).expect("serializable analysis setting"));
        settings.insert("waveform_byte".into(), serde_json::to_value(&self.waveform_byte).expect("serializable analysis setting"));
        settings.insert("waveform_bit".into(), serde_json::to_value(&self.waveform_bit).expect("serializable analysis setting"));
        settings.insert("raw_waveform_bytes".into(), serde_json::to_value(&self.raw_waveform_bytes).expect("serializable analysis setting"));
        settings.insert("raw_waveform_stacked".into(), serde_json::to_value(&self.raw_waveform_stacked).expect("serializable analysis setting"));
        settings.insert("signal_discovery_scope".into(), serde_json::to_value(&self.signal_discovery_scope).expect("serializable analysis setting"));
        settings.insert("time_start".into(), serde_json::to_value(&self.time_start).expect("serializable analysis setting"));
        settings.insert("time_end".into(), serde_json::to_value(&self.time_end).expect("serializable analysis setting"));
        settings.insert("use_manual_window".into(), serde_json::to_value(&self.use_manual_window).expect("serializable analysis setting"));
        settings.insert("selected_event_row".into(), serde_json::to_value(&self.selected_event_row).expect("serializable analysis setting"));
        settings.insert("selected_ecus".into(), serde_json::to_value(&self.selected_ecus).expect("serializable analysis setting"));
        settings.insert("custom_name".into(), serde_json::to_value(&self.custom_name).expect("serializable analysis setting"));
        settings.insert("custom_frame".into(), serde_json::to_value(&self.custom_frame).expect("serializable analysis setting"));
        settings.insert("custom_start_byte".into(), serde_json::to_value(&self.custom_start_byte).expect("serializable analysis setting"));
        settings.insert("custom_start_bit".into(), serde_json::to_value(&self.custom_start_bit).expect("serializable analysis setting"));
        settings.insert("custom_bit_length".into(), serde_json::to_value(&self.custom_bit_length).expect("serializable analysis setting"));
        settings.insert("custom_scale".into(), serde_json::to_value(&self.custom_scale).expect("serializable analysis setting"));
        settings.insert("custom_offset".into(), serde_json::to_value(&self.custom_offset).expect("serializable analysis setting"));
        settings.insert("custom_signed".into(), serde_json::to_value(&self.custom_signed).expect("serializable analysis setting"));
        settings.insert("custom_little_endian".into(), serde_json::to_value(&self.custom_little_endian).expect("serializable analysis setting"));
        settings.insert("signal_search".into(), serde_json::to_value(&self.signal_search).expect("serializable analysis setting"));
        settings.insert("waveform_overlap".into(), serde_json::to_value(&self.waveform_overlap).expect("serializable analysis setting"));
        settings.insert("graphics_bytes".into(), serde_json::to_value(&self.graphics_bytes).expect("serializable analysis setting"));
        settings.insert("graphics_density".into(), serde_json::to_value(&self.graphics_density).expect("serializable analysis setting"));
        settings.insert("graphics_show_filters".into(), serde_json::to_value(&self.graphics_show_filters).expect("serializable analysis setting"));
        settings.insert("graphics_show_signal_browser".into(), serde_json::to_value(&self.graphics_show_signal_browser).expect("serializable analysis setting"));
        settings.insert("graphics_signal_panel_width".into(), serde_json::to_value(&self.graphics_signal_panel_width).expect("serializable analysis setting"));
        settings.insert("graphics_cursor_panel_width".into(), serde_json::to_value(&self.graphics_cursor_panel_width).expect("serializable analysis setting"));
        settings.insert("graphics_viewer_open".into(), serde_json::to_value(&self.graphics_viewer_open).expect("serializable analysis setting"));
        settings.insert("graphics_lane_height".into(), serde_json::to_value(&self.graphics_lane_height).expect("serializable analysis setting"));
        settings.insert("graphics_lane_scroll_y".into(), serde_json::to_value(&self.graphics_lane_scroll_y).expect("serializable analysis setting"));
        settings.insert("graphics_y_scale".into(), serde_json::to_value(&self.graphics_y_scale).expect("serializable analysis setting"));
        settings.insert("graphics_y_offset".into(), serde_json::to_value(&self.graphics_y_offset).expect("serializable analysis setting"));
        settings.insert("graphics_tree_open".into(), serde_json::to_value(&self.graphics_tree_open).expect("serializable analysis setting"));
        settings.insert("graphics_show_active".into(), serde_json::to_value(&self.graphics_show_active).expect("serializable analysis setting"));
        settings.insert("graphics_show_static".into(), serde_json::to_value(&self.graphics_show_static).expect("serializable analysis setting"));
        settings.insert("graphics_show_counter".into(), serde_json::to_value(&self.graphics_show_counter).expect("serializable analysis setting"));
        settings.insert("graphics_show_crc".into(), serde_json::to_value(&self.graphics_show_crc).expect("serializable analysis setting"));
        settings.insert("graphics_show_pattern".into(), serde_json::to_value(&self.graphics_show_pattern).expect("serializable analysis setting"));
        settings.insert("graphics_range_changed_only".into(), serde_json::to_value(&self.graphics_range_changed_only).expect("serializable analysis setting"));
        settings.insert("graphics_follow_replay".into(), serde_json::to_value(&self.graphics_follow_replay).expect("serializable analysis setting"));
        settings.insert("graphics_only_changing".into(), serde_json::to_value(&self.graphics_only_changing).expect("serializable analysis setting"));
        settings.insert("show_network_tree".into(), serde_json::to_value(&self.show_network_tree).expect("serializable analysis setting"));
        settings.insert("inspector_pane".into(), serde_json::to_value(&self.inspector_pane).expect("serializable analysis setting"));
        settings.insert("inspector_height".into(), serde_json::to_value(&self.inspector_height).expect("serializable analysis setting"));
        settings.insert("inspector_width".into(), serde_json::to_value(&self.inspector_width).expect("serializable analysis setting"));
        settings.insert("inspector_dock".into(), serde_json::to_value(&self.inspector_dock).expect("serializable analysis setting"));
        settings.insert("project_tree_width".into(), serde_json::to_value(&self.project_tree_width).expect("serializable analysis setting"));
        settings.insert("replay_position_s".into(), serde_json::to_value(&self.replay_position_s).expect("serializable analysis setting"));
        settings.insert("replay_speed".into(), serde_json::to_value(&self.replay_speed).expect("serializable analysis setting"));
        settings.insert("trace_follow_replay".into(), serde_json::to_value(&self.trace_follow_replay).expect("serializable analysis setting"));
        settings.insert("trace_overwrite".into(), serde_json::to_value(&self.trace_overwrite).expect("serializable analysis setting"));
        settings.insert("trace_only_changing".into(), serde_json::to_value(&self.trace_only_changing).expect("serializable analysis setting"));
        settings.insert("trace_display_position_s".into(), serde_json::to_value(&self.trace_display_position_s).expect("serializable analysis setting"));
        settings.insert("export_range".into(), serde_json::to_value(&self.export_range).expect("serializable analysis setting"));
        settings.insert("export_content".into(), serde_json::to_value(&self.export_content).expect("serializable analysis setting"));
        settings.insert("export_format".into(), serde_json::to_value(&self.export_format).expect("serializable analysis setting"));
        settings.insert("export_skip_constant_frames".into(), serde_json::to_value(&self.export_skip_constant_frames).expect("serializable analysis setting"));
        settings
    }

    fn restore_analysis_settings(&mut self, settings: &BTreeMap<String, serde_json::Value>) {
        if let Some(v)=settings.get("graph_use_colors").and_then(|v|v.as_bool()){self.graph_use_colors=v;}
        if let Some(v)=settings.get("graph_show_grid").and_then(|v|v.as_bool()){self.graph_show_grid=v;}
        if let Some(v)=settings.get("graph_show_markers").and_then(|v|v.as_bool()){self.graph_show_markers=v;}
        if let Some(v)=settings.get("graph_show_cursor_labels").and_then(|v|v.as_bool()){self.graph_show_cursor_labels=v;}
        if let Some(v)=settings.get("graph_line_width").and_then(|v|v.as_f64()){if v.is_finite(){self.graph_line_width=(v as f32).clamp(0.8,3.5);}}
        self.graphics_activity_filter=settings.get("graphics_activity_filter").and_then(|v|serde_json::from_value(v.clone()).ok()).unwrap_or_default();
        if let Some(v)=settings.get("graph_channel_names") {if let Ok(v)=serde_json::from_value(v.clone()){self.graph_channel_names=v;}}
        if let Some(v)=settings.get("graph_lane_names") {if let Ok(v)=serde_json::from_value(v.clone()){self.graph_lane_names=v;}}
        if let Some(v)=settings.get("graphics_eth_filters") {if let Ok(v)=serde_json::from_value(v.clone()){self.graphics_eth_filters=v;}}
        if let Some(v)=settings.get("graphics_frame_id_filter") {if let Ok(v)=serde_json::from_value(v.clone()){self.graphics_frame_id_filter=v;}}
        if let Some(value)=settings.get("trace_columns") {
            if let Ok(mut columns)=serde_json::from_value::<crate::trace_columns::TraceColumns>(value.clone()) {
                columns.sanitize();self.trace_columns=columns;
            }
        }
        if let Some(value)=settings.get("tecmp_source_time").and_then(|v|v.as_bool()){self.tecmp_source_time=value;}
        if let Some(value)=settings.get("workspace") { if let Ok(value)=serde_json::from_value(value.clone()) { self.workspace=value; } }
        if let Some(value)=settings.get("analysis_page") { if let Ok(value)=serde_json::from_value(value.clone()) { self.analysis_page=value; } }
        if let Some(value)=settings.get("config_page") { if let Ok(value)=serde_json::from_value(value.clone()) { self.config_page=value; } }
        if let Some(value)=settings.get("filter") { if let Ok(value)=serde_json::from_value(value.clone()) { self.filter=value; } }
        if let Some(value)=settings.get("selected_channels") { if let Ok(value)=serde_json::from_value(value.clone()) { self.selected_channels=value; } }
        if let Some(value)=settings.get("activity_level") { if let Ok(value)=serde_json::from_value(value.clone()) { self.activity_level=value; } }
        if let Some(value)=settings.get("explorer_mode") { if let Ok(value)=serde_json::from_value(value.clone()) { self.explorer_mode=value; } }
        if let Some(value)=settings.get("inspect_byte") { if let Ok(value)=serde_json::from_value(value.clone()) { self.inspect_byte=value; } }
        if let Some(value)=settings.get("inspect_bit") { if let Ok(value)=serde_json::from_value(value.clone()) { self.inspect_bit=value; } }
        if let Some(value)=settings.get("reference_value") { if let Ok(value)=serde_json::from_value(value.clone()) { self.reference_value=value; } }
        if let Some(value)=settings.get("compare_ids") { if let Ok(value)=serde_json::from_value(value.clone()) { self.compare_ids=value; } }
        if let Some(value)=settings.get("compare_keys") { if let Ok(value)=serde_json::from_value(value.clone()) { self.compare_keys=value; } }
        if let Some(value)=settings.get("multi_frame_keys") { if let Ok(value)=serde_json::from_value(value.clone()) { self.multi_frame_keys=value; } }
        if let Some(value)=settings.get("multi_hide_static") { if let Ok(value)=serde_json::from_value(value.clone()) { self.multi_hide_static=value; } }
        if let Some(value)=settings.get("multi_hide_counter") { if let Ok(value)=serde_json::from_value(value.clone()) { self.multi_hide_counter=value; } }
        if let Some(value)=settings.get("multi_hide_crc_candidates") { if let Ok(value)=serde_json::from_value(value.clone()) { self.multi_hide_crc_candidates=value; } }
        if let Some(value)=settings.get("waveform_keys") { if let Ok(value)=serde_json::from_value(value.clone()) { self.waveform_keys=value; } }
        if let Some(value)=settings.get("waveform_byte") { if let Ok(value)=serde_json::from_value(value.clone()) { self.waveform_byte=value; } }
        if let Some(value)=settings.get("waveform_bit") { if let Ok(value)=serde_json::from_value(value.clone()) { self.waveform_bit=value; } }
        if let Some(value)=settings.get("raw_waveform_bytes") { if let Ok(value)=serde_json::from_value(value.clone()) { self.raw_waveform_bytes=value; } }
        if let Some(value)=settings.get("raw_waveform_stacked") { if let Ok(value)=serde_json::from_value(value.clone()) { self.raw_waveform_stacked=value; } }
        if let Some(value)=settings.get("signal_discovery_scope") { if let Ok(value)=serde_json::from_value(value.clone()) { self.signal_discovery_scope=value; } }
        if let Some(value)=settings.get("time_start") { if let Ok(value)=serde_json::from_value(value.clone()) { self.time_start=value; } }
        if let Some(value)=settings.get("time_end") { if let Ok(value)=serde_json::from_value(value.clone()) { self.time_end=value; } }
        if let Some(value)=settings.get("use_manual_window") { if let Ok(value)=serde_json::from_value(value.clone()) { self.use_manual_window=value; } }
        if let Some(value)=settings.get("selected_event_row") { if let Ok(value)=serde_json::from_value(value.clone()) { self.selected_event_row=value; } }
        if let Some(value)=settings.get("selected_ecus") { if let Ok(value)=serde_json::from_value(value.clone()) { self.selected_ecus=value; } }
        if let Some(value)=settings.get("custom_name") { if let Ok(value)=serde_json::from_value(value.clone()) { self.custom_name=value; } }
        if let Some(value)=settings.get("custom_frame") { if let Ok(value)=serde_json::from_value(value.clone()) { self.custom_frame=value; } }
        if let Some(value)=settings.get("custom_start_byte") { if let Ok(value)=serde_json::from_value(value.clone()) { self.custom_start_byte=value; } }
        if let Some(value)=settings.get("custom_start_bit") { if let Ok(value)=serde_json::from_value(value.clone()) { self.custom_start_bit=value; } }
        if let Some(value)=settings.get("custom_bit_length") { if let Ok(value)=serde_json::from_value(value.clone()) { self.custom_bit_length=value; } }
        if let Some(value)=settings.get("custom_scale") { if let Ok(value)=serde_json::from_value(value.clone()) { self.custom_scale=value; } }
        if let Some(value)=settings.get("custom_offset") { if let Ok(value)=serde_json::from_value(value.clone()) { self.custom_offset=value; } }
        if let Some(value)=settings.get("custom_signed") { if let Ok(value)=serde_json::from_value(value.clone()) { self.custom_signed=value; } }
        if let Some(value)=settings.get("custom_little_endian") { if let Ok(value)=serde_json::from_value(value.clone()) { self.custom_little_endian=value; } }
        if let Some(value)=settings.get("signal_search") { if let Ok(value)=serde_json::from_value(value.clone()) { self.signal_search=value; } }
        if let Some(value)=settings.get("waveform_overlap") { if let Ok(value)=serde_json::from_value(value.clone()) { self.waveform_overlap=value; } }
        if let Some(value)=settings.get("graphics_bytes") { if let Ok(value)=serde_json::from_value(value.clone()) { self.graphics_bytes=value; } }
        if let Some(value)=settings.get("graphics_density") { if let Ok(value)=serde_json::from_value(value.clone()) { self.graphics_density=value; } }
        if let Some(value)=settings.get("graphics_show_filters") { if let Ok(value)=serde_json::from_value(value.clone()) { self.graphics_show_filters=value; } }
        if let Some(value)=settings.get("graphics_show_signal_browser") { if let Ok(value)=serde_json::from_value(value.clone()) { self.graphics_show_signal_browser=value; } }
        if let Some(value)=settings.get("graphics_signal_panel_width") { if let Ok(value)=serde_json::from_value(value.clone()) { self.graphics_signal_panel_width=value; } }
        if let Some(value)=settings.get("graphics_cursor_panel_width") { if let Ok(value)=serde_json::from_value(value.clone()) { self.graphics_cursor_panel_width=value; } }
        if let Some(value)=settings.get("graphics_viewer_open") { if let Ok(value)=serde_json::from_value(value.clone()) { self.graphics_viewer_open=value; } }
        if let Some(value)=settings.get("graphics_lane_height") { if let Ok(value)=serde_json::from_value(value.clone()) { self.graphics_lane_height=value; } }
        if let Some(value)=settings.get("graphics_lane_scroll_y") { if let Ok(value)=serde_json::from_value(value.clone()) { self.graphics_lane_scroll_y=value; } }
        if let Some(value)=settings.get("graphics_y_scale") { if let Ok(value)=serde_json::from_value(value.clone()) { self.graphics_y_scale=value; } }
        if let Some(value)=settings.get("graphics_y_offset") { if let Ok(value)=serde_json::from_value(value.clone()) { self.graphics_y_offset=value; } }
        if let Some(value)=settings.get("graphics_tree_open") { if let Ok(value)=serde_json::from_value(value.clone()) { self.graphics_tree_open=value; } }
        if let Some(value)=settings.get("graphics_show_active") { if let Ok(value)=serde_json::from_value(value.clone()) { self.graphics_show_active=value; } }
        if let Some(value)=settings.get("graphics_show_static") { if let Ok(value)=serde_json::from_value(value.clone()) { self.graphics_show_static=value; } }
        if let Some(value)=settings.get("graphics_show_counter") { if let Ok(value)=serde_json::from_value(value.clone()) { self.graphics_show_counter=value; } }
        if let Some(value)=settings.get("graphics_show_crc") { if let Ok(value)=serde_json::from_value(value.clone()) { self.graphics_show_crc=value; } }
        if let Some(value)=settings.get("graphics_show_pattern") { if let Ok(value)=serde_json::from_value(value.clone()) { self.graphics_show_pattern=value; } }
        if let Some(value)=settings.get("graphics_range_changed_only") { if let Ok(value)=serde_json::from_value(value.clone()) { self.graphics_range_changed_only=value; } }
        if let Some(value)=settings.get("graphics_follow_replay") { if let Ok(value)=serde_json::from_value(value.clone()) { self.graphics_follow_replay=value; } }
        if let Some(value)=settings.get("graphics_only_changing") { if let Ok(value)=serde_json::from_value(value.clone()) { self.graphics_only_changing=value; } }
        if let Some(value)=settings.get("show_network_tree") { if let Ok(value)=serde_json::from_value(value.clone()) { self.show_network_tree=value; } }
        if let Some(value)=settings.get("inspector_pane") { if let Ok(value)=serde_json::from_value(value.clone()) { self.inspector_pane=value; } }
        if let Some(value)=settings.get("inspector_height") { if let Ok(value)=serde_json::from_value(value.clone()) { self.inspector_height=value; } }
        if let Some(value)=settings.get("inspector_width") { if let Ok(value)=serde_json::from_value(value.clone()) { self.inspector_width=value; } }
        if let Some(value)=settings.get("inspector_dock") { if let Ok(value)=serde_json::from_value(value.clone()) { self.inspector_dock=value; } }
        if let Some(value)=settings.get("project_tree_width") { if let Ok(value)=serde_json::from_value(value.clone()) { self.project_tree_width=value; } }
        if let Some(value)=settings.get("replay_position_s") { if let Ok(value)=serde_json::from_value(value.clone()) { self.replay_position_s=value; } }
        if let Some(value)=settings.get("replay_speed") { if let Ok(value)=serde_json::from_value(value.clone()) { self.replay_speed=value; } }
        if let Some(value)=settings.get("trace_follow_replay") { if let Ok(value)=serde_json::from_value(value.clone()) { self.trace_follow_replay=value; } }
        if let Some(value)=settings.get("trace_overwrite") { if let Ok(value)=serde_json::from_value(value.clone()) { self.trace_overwrite=value; } }
        if let Some(value)=settings.get("trace_only_changing") { if let Ok(value)=serde_json::from_value(value.clone()) { self.trace_only_changing=value; } }
        if let Some(value)=settings.get("trace_display_position_s") { if let Ok(value)=serde_json::from_value(value.clone()) { self.trace_display_position_s=value; } }
        if let Some(value)=settings.get("export_range") { if let Ok(value)=serde_json::from_value(value.clone()) { self.export_range=value; } }
        if let Some(value)=settings.get("export_content") { if let Ok(value)=serde_json::from_value(value.clone()) { self.export_content=value; } }
        if let Some(value)=settings.get("export_format") { if let Ok(value)=serde_json::from_value(value.clone()) { self.export_format=value; } }
        if let Some(value)=settings.get("export_skip_constant_frames") { if let Ok(value)=serde_json::from_value(value.clone()) { self.export_skip_constant_frames=value; } }
    }

    fn reset_analysis_settings(&mut self) {
        let defaults=Self::initial_state();
        self.restore_analysis_settings(&defaults.analysis_settings());
        self.graphics_hide_constant=defaults.graphics_hide_constant;
        self.graphics_hide_repetitive=defaults.graphics_hide_repetitive;
        self.graphics_auto_y=defaults.graphics_auto_y;
        self.graphics_stacked=defaults.graphics_stacked;
        self.graphics_show_event_markers=defaults.graphics_show_event_markers;
        self.graphics_per_waveform_cursors=defaults.graphics_per_waveform_cursors;
        self.selected_keys.clear(); self.selected_channel=None;
        self.graphics_whole_frames.clear(); self.cursor_a_s=0.0; self.cursor_b_s=0.0;
        self.graphics_range_manual_dirty=false; self.multi_activity_signature.clear();
        self.trace_view_cache.clear(); self.trace_view_cache_scope.clear();
        self.selected_trace_row=None;self.selected_trace_rows.clear();self.trace_row_anchor=None;
        self.graphics_selected_source=None;self.graphics_selected_signal=None;self.graphics_reveal_source=None;
        self.graphics_frame_id_filter = Default::default();
    }

    fn workspace_snapshot(&self) -> WorkspaceSnapshot {
        WorkspaceSnapshot {
            schema_version: 2,
            analysis_settings: self.analysis_settings(),
            project_channels: self.project_channels.clone(),
            selected_keys: self.selected_keys.clone(),
            selected_frame_key: self.selected_frame_key.clone(),
            selected_channel: self.selected_channel,
            selected_frame_bytes: self.selected_frame_bytes.clone(),
            graphics_sources: self.graphics_sources.clone(),
            graphics_signals: self.graphics_signals.clone(),
            graphics_bits: self.graphics_bits.clone(),
            graphics_whole_frames: self.graphics_whole_frames.clone(),
            graphics_lane_order: self.graphics_lane_order.clone(),
            graphics_multi_cursors: self.graphics_multi_cursors.clone(),
            cursor_a_s: self.cursor_a_s, cursor_b_s: self.cursor_b_s,
            graphics_range_start_s: self.graphics_range_start_s, graphics_range_end_s: self.graphics_range_end_s,
            graphics_view_start_s: self.graphics_view_start_s, graphics_view_end_s: self.graphics_view_end_s,
            graphics_show_event_markers: self.graphics_show_event_markers,
            graphics_hide_constant: self.graphics_hide_constant, graphics_hide_repetitive: self.graphics_hide_repetitive,
            graphics_stacked: self.graphics_stacked, graphics_auto_y: self.graphics_auto_y,
            graphics_per_waveform_cursors: self.graphics_per_waveform_cursors,
        }
    }

    fn persist_workspace_if_changed(&mut self, force: bool) {
        let Some(id)=self.session.as_ref().map(|m|m.id.clone()) else { return; };
        if !force && self.workspace_cache_last_save.elapsed() < Duration::from_millis(900) { return; }
        let snapshot=self.workspace_snapshot();
        let Ok(bytes)=serde_json::to_vec(&snapshot) else { return; };
        let signature=String::from_utf8_lossy(&bytes).into_owned();
        if !force && signature==self.workspace_cache_signature { self.workspace_cache_last_save=Instant::now(); return; }
        match session::save_workspace(&id,&snapshot) {
            Ok(()) => { self.workspace_cache_signature=signature; self.workspace_cache_last_save=Instant::now(); },
            Err(e) => { self.workspace_cache_last_save=Instant::now(); self.log_tool("WARN",format!("Could not persist workspace: {e:#}")); }
        }
    }

    fn restore_workspace(&mut self, id:&str) {
        match session::load_workspace(id) {
            Ok(Some(w)) => self.apply_workspace_snapshot(w),
            Ok(None)=>{},
            Err(e)=>self.log_tool("WARN",format!("Could not restore saved workspace: {e:#}")),
        }
    }

    fn apply_workspace_snapshot(&mut self,w:WorkspaceSnapshot) {

                let loaded_clock=self.dataset.as_ref().map(|d|d.source_time).unwrap_or(self.tecmp_source_time);
                let saved_clock=w.analysis_settings.get("tecmp_source_time").and_then(|v|v.as_bool()).unwrap_or(false);
                self.restore_analysis_settings(&w.analysis_settings);
                self.tecmp_source_time=loaded_clock;
                let available_channels=self.project_channels.clone();
                if w.schema_version >= 1 {
                    self.project_channels=w.project_channels.intersection(&available_channels).cloned().collect();
                    // Old partial sessions may know a physical channel only as TECMP/UNKNOWN.
                    // Resolve that enabled choice to its newly discovered payload network.
                    let unresolved:BTreeSet<u32>=w.project_channels.iter().filter(|key|!available_channels.contains(*key))
                        .filter_map(|key|key.split_once('|')).filter(|(bus,_)|matches!(*bus,"TECMP"|"UNKNOWN"))
                        .filter_map(|(_,ch)|ch.parse().ok()).collect();
                    self.project_channels.extend(available_channels.iter().filter(|key|key.rsplit_once('|').and_then(|(_,ch)|ch.parse::<u32>().ok()).is_some_and(|ch|unresolved.contains(&ch))).cloned());
                }
                self.selected_keys=w.selected_keys;
                self.selected_frame_key=w.selected_frame_key; self.selected_channel=w.selected_channel;
                self.selected_frame_bytes=w.selected_frame_bytes; self.graphics_sources=w.graphics_sources;
                self.graphics_signals=w.graphics_signals; self.graphics_bits=w.graphics_bits;
                self.graphics_whole_frames=w.graphics_whole_frames; self.graphics_lane_order=w.graphics_lane_order;
                self.graphics_multi_cursors=w.graphics_multi_cursors; self.cursor_a_s=w.cursor_a_s; self.cursor_b_s=w.cursor_b_s;
                self.graphics_range_start_s=w.graphics_range_start_s; self.graphics_range_end_s=w.graphics_range_end_s;
                let duration=self.cached_stats.as_ref().map(|x|x.duration_s).unwrap_or(0.0).max(0.001);
                let restored=restored_graph_view(w.graphics_view_start_s,w.graphics_view_end_s,duration,saved_clock==loaded_clock);
                (self.graphics_view_start_s,self.graphics_view_end_s)=restored.unwrap_or((0.0,duration));
                if restored.is_none() {
                    self.graphics_range_start_s=None; self.graphics_range_end_s=None;
                    self.graphics_multi_cursors.clear(); self.cursor_a_s=0.0; self.cursor_b_s=duration;
                    self.time_start=0.0; self.time_end=duration; self.use_manual_window=false;
                }
                self.graphics_show_event_markers=w.graphics_show_event_markers;
                self.graphics_hide_constant=w.graphics_hide_constant; self.graphics_hide_repetitive=w.graphics_hide_repetitive;
                self.graphics_stacked=w.graphics_stacked; self.graphics_auto_y=w.graphics_auto_y;
                self.graphics_per_waveform_cursors=w.graphics_per_waveform_cursors;
                self.graphics_range_manual_dirty=false;
                self.graphics_range_manual_start=self.graphics_range_start_s.map(|v|format!("{v:.6}")).unwrap_or_default();
                self.graphics_range_manual_end=self.graphics_range_end_s.map(|v|format!("{v:.6}")).unwrap_or_default();
                self.workspace_cache_signature=serde_json::to_string(&self.workspace_snapshot()).unwrap_or_default();
                self.workspace_cache_last_save=Instant::now();
                self.log_tool("INFO","Saved workspace restored: channels, frames, bytes/bits, markers, lanes, ranges and cursors");
    }

    fn persist_session_configuration(&mut self) {
        let Some(id)=self.session.as_ref().map(|m|m.id.clone()) else { return; };
        if let Err(e)=session::save_configuration(&id, &self.project_config, &self.channel_map, &self.vehicle_db, &self.signal_table, &self.event_log, self.show_raw_ids, &self.active_preload_channels) {
            self.log_tool("WARN", format!("Could not persist session configuration: {e:#}"));
        }
    }

    fn restore_session_configuration(&mut self, id:&str) {
        match session::load_configuration(id) {
            Ok(Some(c)) => {
                self.project_config=c.project;
                self.channel_map=c.channels;
                self.vehicle_db=c.messages;
                self.signal_table=c.signals;
                self.event_log=c.events;
                self.show_raw_ids=c.show_raw_ids;
                self.active_preload_channels=c.preload_channels;
                self.apply_signal_configuration();
                self.log_tool("INFO", "Session configuration restored automatically");
            }
            Ok(None) => {}
            Err(e) => self.log_tool("WARN", format!("Could not restore session configuration: {e:#}")),
        }
    }

    fn open_context(&mut self, kind: &str) {
        if let Some(p) = rfd::FileDialog::new().add_filter("Configuration table", &["csv", "xlsx", "xls"]).pick_file() {
            let sheet_hint = match kind {
                "project" => Some("Project"),
                "channel" | "channels" => Some("Channels"),
                "vehicle" | "messages" => Some("Messages"),
                "signals" => Some("Signals"),
                "event" | "events" => Some("CM Markers"),
                _ => None,
            };
            match read_table_for(&p, sheet_hint) {
                Ok(t) => {
                    match kind {
                        "project" => self.project_config = t,
                        "vehicle" | "messages" => self.vehicle_db = t,
                        "event" | "events" => self.event_log = t,
                        "channel" | "channels" => self.channel_map = t,
                        "signals" => { self.signal_table = t; self.apply_signal_configuration(); }
                        _ => {}
                    }
                    let loaded_name = p.file_name().and_then(|x| x.to_str()).unwrap_or("context").to_string();
                    self.info = Some(format!("Loaded {}", loaded_name));
                    self.log_tool("INFO", format!("Configuration loaded: {}", loaded_name));
                    self.persist_session_configuration();
                }
                Err(e) => {
                    let msg = format!("{e:#}");
                    self.log_tool("ERROR", format!("Configuration load failed: {}", msg));
                    self.error = Some(msg);
                },
            }
        }
    }

    fn side_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("sidebar").resizable(true).default_width(235.0).min_width(180.0).show(ctx, |ui| {
            ui.add_space(8.0);
            ui.heading("LogLens");
            ui.small("Automotive log analysis");
            ui.separator();
            if ui.button("New Session").clicked() { self.new_session(); }
            if ui.button("Open Vehicle Log").clicked() { self.open_log(); }
            ui.add_space(8.0);
            ui.strong("Sessions");
            let sessions = self.sessions.clone();
            for m in sessions {
                let selected = self.session.as_ref().map(|s| s.id == m.id).unwrap_or(false);
                ui.horizontal(|ui| {
                    if ui.selectable_label(selected, truncate(&m.filename, 21)).clicked() { self.load_saved_session(m.clone()); }
                    if ui.small_button("X").on_hover_text("Move to recycle bin").clicked() {
                        let _ = session::remove(&m.id);
                        if selected { self.new_session(); }
                        self.refresh_sessions();
                    }
                });
            }
            ui.separator();
            if ui.selectable_label(self.show_recycle, "Recycle Bin").clicked() { self.show_recycle = !self.show_recycle; }
            if self.show_recycle {
                let deleted = self.deleted.clone();
                for m in deleted {
                    ui.horizontal(|ui| {
                        ui.label(truncate(&m.filename, 15));
                        if ui.small_button("Restore").on_hover_text("Restore").clicked() { let _ = session::restore(&m.id); self.refresh_sessions(); }
                        if ui.small_button("Delete").on_hover_text("Delete permanently").clicked() { let _ = session::purge(&m.id); self.refresh_sessions(); }
                    });
                }
            }
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| { ui.small("CANoe-style workspace v0.12.3"); });
        });
    }

    fn header(&mut self, ctx: &egui::Context) {
        let palette = HomePalette::new(ctx.style().visuals.dark_mode);
        let capture_name = self.path.as_ref()
            .and_then(|p| p.file_name()).map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "No capture".into());
        egui::TopBottomPanel::top("header")
            .frame(egui::Frame::new().fill(palette.surface)
                .stroke(Stroke::new(1.0, palette.border))
                .inner_margin(egui::Margin::symmetric(18, 9)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("LogLens").size(22.0).strong().color(palette.accent));
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("AUTOMOTIVE NETWORK ANALYSIS").size(10.0).strong().color(palette.muted));
                        ui.label(egui::RichText::new(truncate(&capture_name, 72)).size(13.0).color(palette.text));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Tool Log").clicked() { self.show_tool_log = !self.show_tool_log; }
                        if ui.button("Settings").clicked() { self.show_settings = !self.show_settings; }
                    });
                });
                if let Some(e) = &self.error { ui.colored_label(Color32::RED, e); }
                else if let Some(i) = &self.info { ui.colored_label(Color32::from_rgb(80, 160, 110), i); }
            });
    }

    /// Home dashboard, styled to match LogLens_Home_Page_Preview.html.
    /// Capture/import handlers remain unchanged; this UI does not re-decode frames.
    fn start_page(&mut self, ctx: &egui::Context) {
        let palette = HomePalette::new(ctx.style().visuals.dark_mode);
        egui::TopBottomPanel::top("start_header")
            .frame(egui::Frame::new()
                .fill(palette.surface)
                .stroke(Stroke::new(1.0, palette.border))
                .inner_margin(egui::Margin::symmetric(28, 12)))
            .show(ctx, |ui| {
                home_control_style(ui, palette);
                ui.horizontal(|ui| {
                    let (logo_rect, _) = ui.allocate_exact_size(Vec2::new(38.0, 38.0), Sense::hover());
                    ui.painter().rect_filled(logo_rect, 10.0, palette.accent_button);
                    let pts = [
                        Pos2::new(logo_rect.left()+6.0, logo_rect.top()+22.0),
                        Pos2::new(logo_rect.left()+13.0, logo_rect.top()+22.0),
                        Pos2::new(logo_rect.left()+17.0, logo_rect.top()+10.0),
                        Pos2::new(logo_rect.left()+23.0, logo_rect.top()+29.0),
                        Pos2::new(logo_rect.left()+28.0, logo_rect.top()+16.0),
                        Pos2::new(logo_rect.left()+32.0, logo_rect.top()+22.0),
                    ];
                    ui.painter().add(egui::Shape::line(pts.to_vec(), Stroke::new(2.0, Color32::WHITE)));
                    ui.add_space(5.0);
                    ui.label(egui::RichText::new("LogLens").size(23.0).strong().color(palette.text));
                    ui.add_space(9.0);
                    ui.separator();
                    ui.label(egui::RichText::new("AUTOMOTIVE NETWORK ANALYSIS")
                        .size(11.0).color(palette.muted));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_sized([85.0, 37.0], egui::Button::new("Tool Log"))
                            .clicked() { self.show_tool_log = !self.show_tool_log; }
                        if ui.add_sized([88.0, 37.0], egui::Button::new("Settings"))
                            .clicked() { self.show_settings = !self.show_settings; }
                        let theme_text = if palette.dark { "Light mode" } else { "Dark mode" };
                        if ui.add_sized([96.0, 37.0], egui::Button::new(theme_text))
                            .on_hover_text("Switch the application theme")
                            .clicked() {
                            self.theme = if palette.dark { AppTheme::Light } else { AppTheme::Dark };
                            ctx.set_visuals(match self.theme {
                                AppTheme::Dark => egui::Visuals::dark(),
                                AppTheme::Light => egui::Visuals::light(),
                            });
                            if let Err(error) = session::save_app_preferences(&AppPreferences {theme:self.theme}) {
                                self.log_tool("WARN", format!("Could not persist theme preference: {error:#}"));
                            }
                        }
                        ui.label(egui::RichText::new(if self.pending_log_path.is_some() { "SELECT CHANNELS" } else { "HOME" })
                            .size(13.0).strong().color(palette.accent));
                    });
                });
                if let Some(e) = &self.error {
                    ui.colored_label(Color32::from_rgb(235, 115, 115), e);
                } else if let Some(i) = &self.info {
                    ui.colored_label(palette.accent, i);
                }
            });

        egui::TopBottomPanel::bottom("start_status")
            .frame(egui::Frame::new()
                .fill(palette.surface)
                .stroke(Stroke::new(1.0, palette.border))
                .inner_margin(egui::Margin::symmetric(28, 12)))
            .show(ctx, |ui| {
                home_control_style(ui, palette);
                ui.horizontal(|ui| {
                    let status = if let Some(job) = &self.background_load {
                        format!("Loading capture  ·  {} ({}%)", job.stage, job.progress)
                    } else if self.pending_log_path.is_some() {
                        "Channel selection in progress".to_string()
                    } else if self.dataset.is_some() {
                        "Capture loaded  ·  Ready to analyze".to_string()
                    } else { "Ready  ·  No capture open".to_string() };
                    ui.label(egui::RichText::new(status).size(12.0).color(palette.muted));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(format!("LogLens v{}  ·  Channel UI revision 4", env!("CARGO_PKG_VERSION")))
                            .size(12.0).color(palette.muted));
                    });
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(palette.backdrop)
                .inner_margin(egui::Margin::symmetric(28, 20)))
            .show(ctx, |ui| {
                home_control_style(ui, palette);
                // Channel selection is a separate workflow page, not a block embedded
                // between the Home hero and recent-session cards.
                if self.pending_log_path.is_some() {
                    self.start_channel_selection_page(ui, palette);
                    return;
                }
                let viewport_h = ui.available_height();
                egui::ScrollArea::vertical()
                    .id_salt("home_dashboard_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let content_w = ui.available_width().max(320.0);
                        ui.set_width(content_w);

                        egui::Frame::new()
                            .fill(palette.hero)
                            .stroke(Stroke::new(1.0, palette.hero_border))
                            .corner_radius(egui::CornerRadius::same(16))
                            .inner_margin(egui::Margin::symmetric(32, 26))
                            .show(ui, |ui| {
                                home_control_style(ui, palette);
                                ui.set_width((content_w - 66.0).max(240.0));
                                ui.label(egui::RichText::new("CAPTURE  /  DECODE  /  ANALYZE")
                                    .size(11.0).strong().color(palette.accent));
                                ui.add_space(9.0);
                                ui.label(egui::RichText::new("Your automotive network\nanalysis workspace")
                                    .size(30.0).strong().color(palette.text));
                                ui.add_space(5.0);
                                ui.label(egui::RichText::new("Import a vehicle log, select the relevant channels, inspect frames and signals, then export your engineering results.")
                                    .size(14.0).color(palette.muted));
                                ui.add_space(22.0);
                                ui.horizontal_wrapped(|ui| {
                                    if ui.add_enabled(self.background_load.is_none(),
                                        egui::Button::new(egui::RichText::new("+  Open Capture...").size(14.0).strong().color(Color32::WHITE))
                                            .fill(palette.accent_button)
                                            .min_size(Vec2::new(181.0, 43.0)))
                                        .on_hover_text("PCAP, PCAPNG, CSV, XLSX, TXT or LOG")
                                        .clicked() { self.open_log(); }
                                    if ui.add_enabled(self.dataset.is_some() && self.background_load.is_none(),
                                        egui::Button::new(egui::RichText::new("Resume Analysis").size(14.0))
                                            .min_size(Vec2::new(169.0, 43.0)))
                                        .on_hover_text("Return to the active analysis workspace without reloading")
                                        .clicked() { self.return_to_workspace(); }
                                    if ui.add_enabled(self.can_change_channels(),
                                        egui::Button::new(egui::RichText::new("Change Channels").size(14.0))
                                            .min_size(Vec2::new(163.0, 43.0)))
                                        .on_hover_text("Review the channel checklist for the current capture")
                                        .clicked() { self.change_capture_channels(); }
                                });
                                ui.add_space(20.0);
                                ui.separator();
                                ui.add_space(13.0);
                                ui.horizontal_wrapped(|ui| {
                                    egui::Frame::new().fill(palette.pill)
                                        .corner_radius(egui::CornerRadius::same(7))
                                        .inner_margin(egui::Margin::symmetric(11, 6))
                                        .show(ui, |ui| {
                                            ui.label(egui::RichText::new(if self.dataset.is_some() { "ACTIVE" } else { "READY" })
                                                .size(11.0).strong().color(palette.accent));
                                        });
                                    let current = self.path.as_ref().and_then(|p|p.file_name()).and_then(|n|n.to_str())
                                        .unwrap_or("No active capture");
                                    ui.label(egui::RichText::new(truncate(current, 64))
                                        .size(14.0).strong().color(palette.text));
                                    if let Some(s) = self.cached_stats.as_ref().filter(|_| self.dataset.is_some()) {
                                        ui.label(egui::RichText::new(format!("{} frames  ·  {} channels  ·  {:.3} s", s.total, s.channels, s.duration_s))
                                            .size(12.0).color(palette.muted));
                                    } else {
                                        ui.label(egui::RichText::new("CAN / CAN FD / LIN / Ethernet / Analog")
                                            .size(12.0).color(palette.muted));
                                    }
                                });
                            });

                        ui.add_space(18.0);
                        let cards_height = (viewport_h - 360.0).clamp(300.0, 720.0);
                        if content_w >= 980.0 {
                            let gap = 18.0;
                            let left_w = (content_w - gap) / 2.25;
                            let right_w = (content_w - gap) - left_w;
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                ui.allocate_ui_with_layout(Vec2::new(left_w, cards_height),
                                    egui::Layout::top_down(egui::Align::Min), |ui| {
                                        self.start_config_card(ui, left_w, cards_height, palette);
                                    });
                                ui.add_space(gap);
                                ui.allocate_ui_with_layout(Vec2::new(right_w, cards_height),
                                    egui::Layout::top_down(egui::Align::Min), |ui| {
                                        self.start_recent_card(ui, right_w, cards_height, palette);
                                    });
                            });
                        } else {
                            self.start_config_card(ui, content_w, 225.0, palette);
                            ui.add_space(16.0);
                            self.start_recent_card(ui, content_w, 330.0, palette);
                        }
                        ui.add_space(18.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new("WORKFLOW").size(12.0).color(palette.muted));
                            ui.separator();
                            ui.label(egui::RichText::new(
                                "01  Open capture    >    02  Select channels    >    03  Analyze & graph    >    04  Export")
                                .size(12.0).color(palette.text));
                        });
                        ui.add_space(12.0);
                    });
            });
    }


    /// Step 02 / 04: full-width native channel-selection workspace.
    /// All actions operate on pending_channel_selection until Load is explicitly requested.
    fn start_channel_selection_page(&mut self, ui: &mut egui::Ui, p: HomePalette) {
        let Some(path) = self.pending_log_path.clone() else { return; };
        let full_width = ui.available_width().max(320.0);
        let viewport_height = ui.available_height();
        egui::ScrollArea::vertical()
            .id_salt("channel_picker_page_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(full_width);
                ui.label(egui::RichText::new("WORKFLOW  /  02  SELECT CHANNELS")
                    .size(12.0).strong().color(p.accent));
                ui.add_space(7.0);
                ui.label(egui::RichText::new("Choose channels for analysis")
                    .size(29.0).strong().color(p.text));
                ui.add_space(3.0);
                ui.label(egui::RichText::new("Find a network, review its capture channels, and choose which data to load.")
                    .size(14.0).color(p.muted));
                ui.add_space(14.0);
                ui.horizontal_wrapped(|ui| {
                    let may_load = !self.pending_channel_selection.is_empty() && self.background_load.is_none();
                    if ui.add_enabled(may_load,
                        egui::Button::new(egui::RichText::new("Load selected channels  →").size(14.0).strong().color(Color32::WHITE))
                            .fill(p.accent_button).min_size(Vec2::new(215.0, 43.0)))
                        .on_hover_text("Decode only the selected capture channels")
                        .clicked() {
                        if self.load_path(path.clone(), None) { self.show_start_page = false; }
                    }
                    if ui.add_sized([151.0, 43.0], egui::Button::new(if self.dataset.is_some() {
                        "Back to analysis"
                    } else { "Back to home" }))
                        .on_hover_text("Keep the existing loaded dataset unchanged")
                        .clicked() {
                        if self.dataset.is_some() { self.return_to_workspace(); }
                        else { self.pending_log_path=None; self.pending_channels.clear(); self.pending_channel_selection.clear(); }
                    }
                    ui.add_space(8.0);
                    let selected = self.pending_channel_selection.len();
                    let total = self.pending_channels.len();
                    ui.label(egui::RichText::new(format!("{selected} of {total} channels selected"))
                        .size(14.0).strong().color(p.text));
                });
                ui.add_space(17.0);
                let card_height = (viewport_height - 180.0).clamp(490.0, 960.0);
                if full_width >= 1120.0 {
                    let gap = 18.0;
                    let list_width = ((full_width-gap) * 0.67).max(620.0);
                    let details_width = full_width-gap-list_width;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x=0.0;
                        ui.allocate_ui_with_layout(Vec2::new(list_width, card_height),
                            egui::Layout::top_down(egui::Align::Min), |ui| {
                                self.start_channel_picker_list(ui, list_width, card_height, p);
                            });
                        ui.add_space(gap);
                        ui.allocate_ui_with_layout(Vec2::new(details_width, card_height),
                            egui::Layout::top_down(egui::Align::Min), |ui| {
                                self.start_channel_picker_details(ui, details_width, card_height, p, &path);
                            });
                    });
                } else {
                    self.start_channel_picker_list(ui, full_width, card_height, p);
                    ui.add_space(16.0);
                    self.start_channel_picker_details(ui, full_width, 300.0, p, &path);
                }
                ui.add_space(15.0);
                ui.label(egui::RichText::new("01  Open capture     ›     02  Select channels     ›     03  Analyze & graph     ›     04  Export")
                    .size(12.0).color(p.muted));
                ui.add_space(10.0);
            });
    }

    fn start_channel_picker_list(&mut self, ui: &mut egui::Ui, card_width:f32, card_height:f32, p:HomePalette) {
        let search = self.channel_picker_search.trim().to_ascii_lowercase();
        let channel_names = self.channel_name_map();
        let filtered: Vec<(u32,String)> = self.pending_channels.iter().filter(|(id, protocol)| {
            let name = channel_names.get(id).map(String::as_str).unwrap_or("");
            picker_filter_matches(protocol, &self.channel_picker_filter) &&
                (search.is_empty() || channel_id_hex(*id).to_ascii_lowercase().contains(&search)
                    || id.to_string().contains(&search)
                    || protocol.to_ascii_lowercase().contains(&search)
                    || name.to_ascii_lowercase().contains(&search))
        }).cloned().collect();
        egui::Frame::new().fill(p.surface)
            .stroke(Stroke::new(1.0, p.border))
            .corner_radius(egui::CornerRadius::same(14))
            .inner_margin(egui::Margin::same(22))
            .show(ui, |ui| {
                home_control_style(ui,p);
                ui.set_width((card_width - 46.0).max(240.0));
                ui.set_min_height((card_height - 46.0).max(240.0));
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("CAPTURE CHANNELS").size(17.0).strong().color(p.text));
                    ui.label(egui::RichText::new(format!("{} discovered", self.pending_channels.len()))
                        .size(12.0).color(p.muted));
                });
                ui.add_space(6.0);
                let search_width = ui.available_width();
                ui.add_sized([search_width, 42.0],
                    egui::TextEdit::singleline(&mut self.channel_picker_search)
                        .hint_text("Search channel name, ID or network type"));
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    for filter in ["All", "CAN", "CAN FD", "LIN", "Ethernet", "Analog", "TECMP", "Other"] {
                        let selected = self.channel_picker_filter == filter;
                        if ui.add(egui::Button::new(egui::RichText::new(filter).size(12.0))
                            .selected(selected).min_size(Vec2::new(68.0, 32.0)))
                            .on_hover_text("Filter the checklist without changing which channels are selected")
                            .clicked() { self.channel_picker_filter = filter.into(); }
                    }
                });
                ui.add_space(7.0);
                ui.horizontal_wrapped(|ui| {
                    if ui.add(egui::Button::new("Select visible").min_size(Vec2::new(122.0,35.0)))
                        .on_hover_text("Select every channel matching the current search and filter")
                        .clicked() {
                        self.pending_channel_selection.extend(filtered.iter().map(|(id,_)|*id));
                    }
                    if ui.add(egui::Button::new("Clear visible").min_size(Vec2::new(115.0,35.0)))
                        .on_hover_text("Clear only channels matching the current search and filter")
                        .clicked() {
                        for (id,_) in &filtered { self.pending_channel_selection.remove(id); }
                    }
                    if ui.add(egui::Button::new("Select all").min_size(Vec2::new(92.0,35.0)))
                        .on_hover_text("Select all discovered channels, including those hidden by filters")
                        .clicked() {
                        self.pending_channel_selection = self.pending_channels.iter().map(|(id,_)|*id).collect();
                    }
                    if ui.add(egui::Button::new("Clear all").min_size(Vec2::new(86.0,35.0)))
                        .on_hover_text("Deselect all discovered channels")
                        .clicked() { self.pending_channel_selection.clear(); }
                });
                ui.add_space(5.0);
                ui.label(egui::RichText::new(format!("Showing {} of {} channels · {} selected", filtered.len(),
                    self.pending_channels.len(), self.pending_channel_selection.len()))
                    .size(12.0).color(p.muted));
                ui.separator();
                let list_height = (card_height - 285.0).max(220.0);
                egui::ScrollArea::vertical().id_salt("channel_picker_rows_scroll")
                    .max_height(list_height).auto_shrink([false, false])
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .show(ui, |ui| {
                        if filtered.is_empty() {
                            ui.add_space(15.0);
                            ui.label(egui::RichText::new("No channels match this filter. Try another network or search term.")
                                .size(14.0).color(p.muted));
                        }
                        for group in [StartCaptureGroup::BusData, StartCaptureGroup::TecmpSystem, StartCaptureGroup::Other] {
                            let rows:Vec<_> = filtered.iter().filter(|(_,protocol)| start_capture_group(protocol)==group).collect();
                            if rows.is_empty() { continue; }
                            ui.add_space(7.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.label(egui::RichText::new(format!("{}  ·  {}",group.label(),rows.len()))
                                    .size(13.0).strong().color(p.accent));
                                ui.label(egui::RichText::new(group.description()).size(12.0).color(p.muted));
                            });
                            ui.add_space(4.0);
                            for (id, protocol) in rows {
                                let mut checked = self.pending_channel_selection.contains(id);
                                ui.push_id(("picker_channel",id), |ui| {
                                    ui.horizontal(|ui| {
                                        if ui.add_sized([30.0,36.0],egui::Checkbox::new(&mut checked, "")).changed() {
                                            if checked { self.pending_channel_selection.insert(*id); }
                                            else { self.pending_channel_selection.remove(id); }
                                        }
                                        ui.label(egui::RichText::new(channel_id_hex(*id))
                                            .size(13.0).monospace().color(p.text));
                                        ui.add_space(9.0);
                                        ui.vertical(|ui| {
                                            let configured_name = channel_names.get(id).map(String::as_str)
                                                .filter(|name|!name.trim().is_empty());
                                            let label = configured_name.unwrap_or("Unmapped channel");
                                            ui.label(egui::RichText::new(truncate(label, 70)).size(14.0)
                                                .strong().color(if configured_name.is_some() {p.text} else {p.muted}))
                                                .on_hover_text(label);
                                            let note = if group==StartCaptureGroup::TecmpSystem {
                                                format!("{}  ·  system/control data",protocol)
                                            } else { protocol.to_string() };
                                            ui.label(egui::RichText::new(truncate(&note,90)).size(12.0).color(p.muted))
                                                .on_hover_text(note);
                                        });
                                    });
                                    ui.separator();
                                });
                            }
                        }
                    });
            });
    }

    fn start_channel_picker_details(&mut self, ui:&mut egui::Ui, card_width:f32, card_height:f32,
        p:HomePalette, path:&std::path::Path) {
        egui::Frame::new().fill(p.surface)
            .stroke(Stroke::new(1.0,p.border))
            .corner_radius(egui::CornerRadius::same(14))
            .inner_margin(egui::Margin::same(22))
            .show(ui, |ui| {
                home_control_style(ui,p);
                ui.set_width((card_width-46.0).max(240.0));
                ui.set_min_height((card_height-46.0).max(240.0));
                ui.label(egui::RichText::new("IMPORT SUMMARY").size(17.0).strong().color(p.text));
                ui.add_space(7.0);
                let name = path.file_name().and_then(|x|x.to_str()).unwrap_or("capture");
                ui.label(egui::RichText::new(truncate(name,48)).size(14.0).strong().color(p.text))
                    .on_hover_text(path.display().to_string());
                let size = std::fs::metadata(path).map(|m|m.len() as f64 / 1_048_576.0);
                ui.label(egui::RichText::new(match size {
                    Ok(mib) => format!("Source file  ·  {mib:.1} MiB"),
                    Err(_) => "Source file  ·  size unavailable".into(),
                }).size(12.0).color(p.muted));
                ui.add_space(14.0);
                ui.separator();
                ui.add_space(10.0);
                let selected = self.pending_channel_selection.len();
                let total = self.pending_channels.len();
                ui.label(egui::RichText::new(format!("{selected} / {total} selected"))
                    .size(23.0).strong().color(p.text));
                ui.add_space(5.0);
                let bar_width = ui.available_width();
                ui.add(egui::ProgressBar::new(if total>0 {selected as f32/total as f32} else {0.0})
                    .desired_width(bar_width));
                if total > 0 && selected == 0 {
                    ui.add_space(7.0);
                    ui.colored_label(Color32::from_rgb(227,147,89),"Select at least one channel before loading.");
                }
                ui.add_space(15.0);
                ui.separator();
                ui.add_space(11.0);
                ui.label(egui::RichText::new("TIMESTAMP SOURCE").size(13.0).strong().color(p.text));
                ui.add_space(4.0);
                egui::ComboBox::from_id_salt("channel_picker_timeline")
                    .width(ui.available_width().min(340.0))
                    .selected_text(if self.tecmp_source_time { "TECMP source time" } else { "Wireshark capture time" })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.tecmp_source_time, true, "TECMP source time");
                        ui.selectable_value(&mut self.tecmp_source_time, false, "Wireshark capture time");
                    });
                ui.add_space(6.0);
                ui.label(egui::RichText::new("TECMP source time aligns sensor and bus events. Wireshark time uses packet arrival. Both original timestamps remain available.")
                    .size(12.0).color(p.muted));
                ui.add_space(13.0);
                ui.separator();
                ui.add_space(9.0);
                ui.label(egui::RichText::new("SELECTION BY NETWORK").size(13.0).strong().color(p.text));
                for family in ["CAN", "CAN FD", "LIN", "Ethernet", "Analog", "TECMP", "Other"] {
                    let total_family = self.pending_channels.iter().filter(|(_,kind)|picker_filter_matches(kind,family)).count();
                    if total_family==0 {continue;}
                    let selected_family = self.pending_channels.iter().filter(|(id,kind)|{
                        picker_filter_matches(kind,family) && self.pending_channel_selection.contains(id)
                    }).count();
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(family).size(12.0).color(p.text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui| {
                            ui.label(egui::RichText::new(format!("{selected_family}/{total_family}"))
                                .size(12.0).color(p.muted));
                        });
                    });
                }
                ui.add_space(13.0);
                ui.label(egui::RichText::new("Changing channels reloads the capture. The current analysis remains intact until you confirm Load.")
                    .size(12.0).color(p.muted));
            });
    }

    fn start_config_card(&mut self, ui: &mut egui::Ui, card_w: f32, card_h: f32, p: HomePalette) {
        egui::Frame::new().fill(p.surface)
            .stroke(Stroke::new(1.0, p.border))
            .corner_radius(egui::CornerRadius::same(14))
            .inner_margin(egui::Margin::same(24))
            .show(ui, |ui| {
                home_control_style(ui, p);
                ui.set_width((card_w - 50.0).max(170.0));
                ui.set_min_height((card_h - 50.0).max(120.0));
                ui.label(egui::RichText::new("CONFIGURATION LIBRARY").size(17.0).strong().color(p.text));
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Add optional engineering context before or after opening a capture.")
                    .size(13.0).color(p.muted));
                ui.add_space(16.0);
                let inner_w = ui.available_width();
                let button_w = if inner_w >= 580.0 { ((inner_w - 40.0) / 5.0).max(94.0) }
                    else { ((inner_w - 20.0) / 3.0).max(95.0) };
                ui.horizontal_wrapped(|ui| {
                    for (title, kind) in [
                        ("Project", "project"), ("Channels", "channels"),
                        ("Messages", "messages"), ("Signals", "signals"),
                        ("CM Markers", "events")
                    ] {
                        if ui.add_sized([button_w, 42.0],
                            egui::Button::new(egui::RichText::new(title).size(14.0)))
                            .on_hover_text(format!("Import {title} from CSV or Excel"))
                            .clicked() { self.open_context(kind); }
                    }
                });
            });
    }

    fn start_recent_card(&mut self, ui: &mut egui::Ui, card_w: f32, card_h: f32, p: HomePalette) {
        egui::Frame::new().fill(p.surface)
            .stroke(Stroke::new(1.0, p.border))
            .corner_radius(egui::CornerRadius::same(14))
            .inner_margin(egui::Margin::same(24))
            .show(ui, |ui| {
                home_control_style(ui, p);
                ui.set_width((card_w - 50.0).max(170.0));
                ui.set_min_height((card_h - 50.0).max(180.0));
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("RECENT CAPTURES").size(17.0).strong().color(p.text));
                    ui.label(egui::RichText::new(format!("{} sessions", self.sessions.len()))
                        .size(12.0).color(p.muted));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_sized([98.0, 35.0], egui::Button::new("Refresh"))
                            .clicked() { self.refresh_sessions(); }
                    });
                });
                ui.add_space(10.0);
                let input_w = ui.available_width();
                ui.add_sized([input_w, 42.0],
                    egui::TextEdit::singleline(&mut self.home_session_search)
                        .hint_text("Search by capture name or path")
                        .font(egui::TextStyle::Body));
                ui.add_space(7.0);
                ui.separator();
                let search = self.home_session_search.trim().to_lowercase();
                let sessions = self.sessions.iter().filter(|m| {
                    search.is_empty() || m.filename.to_lowercase().contains(&search)
                        || m.original_source_path.to_lowercase().contains(&search)
                }).take(12).cloned().collect::<Vec<_>>();
                if sessions.is_empty() {
                    ui.add_space(16.0);
                    ui.label(egui::RichText::new(if self.sessions.is_empty() {
                        "No recent captures yet. Use Open Capture to begin."
                    } else { "No recent captures match this search." })
                        .size(13.0).color(p.muted));
                } else {
                    let rows_h = (card_h - 173.0).max(130.0);
                    egui::ScrollArea::vertical().id_salt("home_recents_scroll")
                        .max_height(rows_h).auto_shrink([false, false]).show(ui, |ui| {
                        for m in sessions {
                            let row_w = ui.available_width();
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.set_width((row_w - 155.0).max(120.0));
                                    ui.label(egui::RichText::new(truncate(&m.filename, 62))
                                        .size(14.0).strong().color(p.text))
                                        .on_hover_text(&m.filename);
                                    ui.label(egui::RichText::new(format!("Opened {}", format_session_time(m.last_opened_unix)))
                                        .size(12.0).color(p.muted));
                                    ui.label(egui::RichText::new(truncate(&m.original_source_path, 94))
                                        .size(12.0).color(p.muted))
                                        .on_hover_text(&m.original_source_path);
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.add_sized([35.0, 35.0], egui::Button::new("×"))
                                        .on_hover_text("Remove from recent sessions (recoverable)")
                                        .clicked() {
                                        if let Err(e) = session::remove(&m.id) {
                                            self.log_tool("ERROR", format!("Could not remove Recent Session: {e:#}"));
                                        }
                                        self.refresh_sessions();
                                    }
                                    if ui.add_enabled(self.background_load.is_none(),
                                        egui::Button::new("Open").min_size(Vec2::new(80.0, 35.0)))
                                        .on_hover_text(&m.source_path)
                                        .clicked() { self.load_saved_session(m.clone()); }
                                });
                            });
                            ui.add_space(9.0);
                            ui.separator();
                            ui.add_space(4.0);
                        }
                    });
                }
            });
    }

    fn summary_cards(&self, ui: &mut egui::Ui) {
        let Some(s) = self.cached_stats.as_ref() else { return; };
        ui.horizontal_wrapped(|ui| {
            card(ui, "Decoded Frames", s.total.to_string());
            card(ui, "Frame IDs", s.unique_keys.to_string());
            card(ui, "Channels", s.channels.to_string());
            card(ui, "Trace Duration", format!("{:.3} s", s.duration_s));
        });
        ui.add_space(6.0);
    }

    fn workspace_tabs(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::horizontal().id_salt("workspace_navigation_r5").show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.add_sized([86.0, 35.0], egui::Button::new("Home"))
                    .on_hover_text("Return to Home without unloading the active capture").clicked() {
                    self.refresh_sessions();
                    self.show_start_page = true;
                }
                if ui.add_enabled(self.can_change_channels(), egui::Button::new("Change Channels"))
                    .on_hover_text("Choose a different set of capture channels").clicked() {
                    self.change_capture_channels();
                }
                ui.separator();
                if ui.add_sized([166.0, 35.0], egui::SelectableLabel::new(
                        self.workspace == Workspace::Measurement,
                        egui::RichText::new("Capture Explorer").size(14.0)))
                    .on_hover_text("Networks, frame trace, replay and inspector").clicked() {
                    self.workspace = Workspace::Measurement;
                }
                if ui.add_sized([132.0, 35.0], egui::SelectableLabel::new(
                        self.workspace == Workspace::Analysis,
                        egui::RichText::new("Signal Analysis").size(14.0)))
                    .on_hover_text("Byte activity, signal discovery, timing and CRC").clicked() {
                    self.workspace = Workspace::Analysis;
                }
                if ui.add_sized([136.0, 35.0], egui::SelectableLabel::new(
                        self.workspace == Workspace::Configuration,
                        egui::RichText::new("Configuration").size(14.0)))
                    .on_hover_text("Project, Channels, Messages, Signals and Events").clicked() {
                    self.workspace = Workspace::Configuration;
                }
                ui.separator();
                if ui.add_sized([116.0, 35.0], egui::Button::new("Waveforms"))
                    .on_hover_text("Open the dedicated Waveform Viewer using the current capture").clicked() {
                    self.workspace = Workspace::Measurement;
                    self.inspector_pane = InspectorPane::Graphics;
                    self.floating_panes.remove(&InspectorPane::Graphics);
                    self.graphics_viewer_open = true;
                }
                if ui.add_sized([86.0, 35.0], egui::Button::new("Export"))
                    .on_hover_text("Open Export Manager").clicked() {
                    self.show_export_manager = true;
                }
            });
        });
        ui.add_space(6.0);
    }

    /// Capture overview uses only the existing load-time index, not per-frame scans.
    fn capture_explorer_overview(&mut self, ui: &mut egui::Ui, d: &Dataset) {
        let palette = HomePalette::new(ui.visuals().dark_mode);
        let channel_count = d.index.project_channel_keys.len();
        let frame_count = d.frames.len();
        let key_count = d.index.by_key.len();
        let duration = self.cached_stats.as_ref().map(|stats| stats.duration_s)
            .unwrap_or_else(|| d.frames.last().map(|f| f.relative_time_s).unwrap_or(0.0));
        if let Some(warning) = d.warnings.iter().find(|w| w.starts_with("Safe load limit reached")) {
            egui::Frame::new().fill(ui.visuals().faint_bg_color)
                .stroke(Stroke::new(1.0, Color32::from_rgb(211, 147, 45)))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(12, 9))
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(egui::RichText::new("PARTIAL CAPTURE").strong().color(Color32::from_rgb(220, 153, 46)));
                        ui.label(warning);
                        if ui.add_enabled(self.background_load.is_none(),
                            egui::Button::new("Reload selected channels")).clicked() {
                            self.reload_selected_channels();
                        }
                    });
                });
            ui.add_space(6.0);
        }
        egui::Frame::new().fill(palette.hero)
            .stroke(Stroke::new(1.0, palette.hero_border))
            .corner_radius(egui::CornerRadius::same(10))
            .inner_margin(egui::Margin::symmetric(18, 14))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("WORKFLOW / STEP 03").size(10.0).strong().color(palette.accent));
                        ui.label(egui::RichText::new("Capture Explorer & Analysis Workspace").size(22.0).strong().color(palette.text));
                        ui.label(egui::RichText::new("Browse networks, inspect frames, replay events and open waveforms.")
                            .size(13.0).color(palette.muted));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Show Explorer").on_hover_text("Reveal the protocol/channel tree").clicked() {
                            self.show_network_tree = true;
                        }
                    });
                });
                ui.add_space(8.0);
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    let stats = [
                        ("CAPTURE FRAMES", format!("{frame_count}")),
                        ("CHANNELS", channel_count.to_string()),
                        ("FRAME IDs / KEYS", key_count.to_string()),
                        ("DURATION", format!("{duration:.3} s")),
                    ];
                    for (label, value) in stats {
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(label).size(10.0).color(palette.muted));
                            ui.label(egui::RichText::new(value).size(19.0).strong().color(palette.text));
                        });
                        ui.add_space(24.0);
                    }
                });
            });
        ui.add_space(6.0);
    }

    fn ensure_filter_cache(&mut self) {
        // v0.52.1: filtering is a committed visibility operation. While the user is
        // typing, keep the previous result set and do no capture scan at all.
        if self.trace_filter_pending { return; }
        let Some(d) = self.dataset.as_ref() else { return; };
        let sig = self.filter.signature();
        if sig == self.cached_filter_signature { return; }

        // Prepare query/structured fields once per committed filter instead of inside
        // Filter::matches for every frame. In particular, to_ascii_lowercase() used to
        // allocate once per frame on a text query.
        let q = self.filter.text.trim().to_ascii_lowercase();
        let bus = self.filter.bus;
        let channel = self.filter.channel;
        let id = self.filter.id;
        let start_s = self.filter.start_s;
        let end_s = self.filter.end_s;
        let has_text = !q.is_empty();
        let matching_channels:BTreeSet<u32>=if has_text {d.index.channels.iter().copied().filter(|ch|channel_id_hex(*ch).to_ascii_lowercase().contains(&q)).collect()}else{BTreeSet::new()};
        let matching_display_keys:BTreeSet<&str>=if has_text {d.index.by_key.keys().filter(|key|frame_key_display(key).to_ascii_lowercase().contains(&q)).map(String::as_str).collect()}else{BTreeSet::new()};

        self.filtered_indices.clear();
        self.filtered_indices.reserve(d.frames.len().min(64 * 1024));
        for (i, f) in d.frames.iter().enumerate() {
            if bus.is_some_and(|x| x != f.bus_type) { continue; }
            if channel.is_some_and(|x| x != f.channel_id) { continue; }
            if id.is_some_and(|x| Some(x) != f.identifier) { continue; }
            if start_s.is_some_and(|x| f.relative_time_s < x) { continue; }
            if end_s.is_some_and(|x| f.relative_time_s > x) { continue; }
            if has_text {
                // Frame keys are lowercased once when FrameIndex is built. Payload hex
                // formatting is now a fallback only when the frame key did not match.
                let key_match = d.index.frame_key_lower.get(i).is_some_and(|k| k.contains(&q));
                if !key_match && !matching_channels.contains(&f.channel_id) && !matching_display_keys.contains(f.frame_key.as_str()) && !f.payload_hex().to_ascii_lowercase().contains(&q) { continue; }
            }
            self.filtered_indices.push(i);
        }
        self.cached_filter_signature = sig;
        // Only visibility/scope caches depend on an ordinary filter. Raw decode,
        // classification and waveform caches intentionally remain valid.
        self.cached_scope_signature.clear();
        self.cached_trace_scope_signature.clear();
        self.trace_view_cache_scope.clear();
    }

    fn scope_signature(&self) -> String {
        let channels = self.selected_channels.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",");
        let project = self.project_channels.iter().cloned().collect::<Vec<_>>().join(",");
        format!("{}|ch={}|project={}|manual={}|{:.9}|{:.9}", self.cached_filter_signature, channels, project, self.use_manual_window, self.time_start, self.time_end)
    }

    fn ensure_scope_cache(&mut self) {
        let Some(d)=self.dataset.as_ref() else{return;};
        let sig=self.scope_signature();
        if sig==self.cached_scope_signature{return;}

        // v0.21: Project Tree changes no longer scan every frame. Merge the
        // precomputed bus/channel index vectors, then sort back to capture order.
        let mut candidates=Vec::<usize>::new();
        for key in &self.project_channels {
            if let Some(v)=d.index.by_project_channel.get(key) {
                candidates.extend_from_slice(v);
            }
        }
        candidates.sort_unstable();

        let selected_channels=&self.selected_channels;
        let use_manual_window=self.use_manual_window;
        let time_start=self.time_start;
        let time_end=self.time_end;

        // If a text/filter result exists, intersect two sorted index streams rather
        // than testing Filter::matches again for every Project candidate.
        let filter_active=self.filter!=Filter::default();
        let mut scoped=Vec::with_capacity(candidates.len());
        if filter_active {
            let filtered=&self.filtered_indices;
            let (mut a,mut b)=(0usize,0usize);
            while a<candidates.len() && b<filtered.len() {
                let ci=candidates[a]; let fi=filtered[b];
                if ci<fi {a+=1;continue;}
                if fi<ci {b+=1;continue;}
                let f=&d.frames[ci];
                if (selected_channels.is_empty()||selected_channels.contains(&f.channel_id))
                    && (!use_manual_window||(f.relative_time_s>=time_start&&f.relative_time_s<=time_end)) {
                    scoped.push(ci);
                }
                a+=1;b+=1;
            }
        } else {
            for i in candidates {
                let f=&d.frames[i];
                if (selected_channels.is_empty()||selected_channels.contains(&f.channel_id))
                    && (!use_manual_window||(f.relative_time_s>=time_start&&f.relative_time_s<=time_end)) {
                    scoped.push(i);
                }
            }
        }
        self.cached_scoped_indices=scoped;
        self.cached_scope_signature=sig;
        self.cached_change_scope_signature.clear();
    }

    fn ensure_change_cache(&mut self) {
        let Some(d) = self.dataset.as_ref() else { return; };

        // Static/change classification follows the dedicated Graphics analysis selection.
        // A/B remain measurement cursors only (delta/value readout).
        // Use only the COMMITTED analysis range. The live selection rectangle is UI state;
        // it must never invalidate/re-run classification while the pointer is moving.
        // A/B cursors are deliberately absent from this path.
        let window = self.graphics_class_window;
        let change_sig=window.map(|(a,b)|format!("RANGE_STATIC_CLASSIFICATION_V1|{a:.9}|{b:.9}"))
            .unwrap_or_else(||"FULL_CAPTURE_STATIC_CLASSIFICATION_V4".to_string());
        if self.cached_change_scope_signature == change_sig { return; }

        let mut first_payload_by_trace: HashMap<String, Vec<u8>> = HashMap::new();
        let mut changing_trace = BTreeSet::new();
        let mut changing_graphics = BTreeSet::new();

        // engine::load() keeps frames time ordered. Restrict the pass up front so a
        // small committed analysis range is O(frames-in-range), not O(full capture).
        let (lo,hi)=if let Some((a,b))=window {
            (d.frames.partition_point(|f|f.relative_time_s<a),
             d.frames.partition_point(|f|f.relative_time_s<=b))
        } else {(0,d.frames.len())};
        for f in &d.frames[lo..hi] {
            let trace_key = trace_source_key(f);
            if changing_trace.contains(&trace_key) {
                continue;
            }

            match first_payload_by_trace.get(&trace_key) {
                None => {
                    first_payload_by_trace.insert(trace_key, f.payload.clone());
                }
                Some(first_payload) if first_payload != &f.payload => {
                    changing_trace.insert(trace_key.clone());
                    changing_graphics.insert(graphics_source_key(&f.bus_type.to_string(), f.channel_id, &f.frame_key));
                }
                Some(_) => {}
            }
        }

        self.cached_changing_trace_sources = changing_trace;
        self.cached_changing_graphics_sources = changing_graphics;
        self.cached_change_scope_signature = change_sig;
    }

    fn trace_rows_cached(&mut self, d: &Dataset, visible_count: usize, overwrite: bool) -> Arc<Vec<TraceRow>> {
        let mode = if overwrite { "overwrite" } else { "sequential" };
        let scope_sig = format!("{}|mode={}", self.cached_scope_signature, mode);
        if self.cached_trace_scope_signature != scope_sig || visible_count < self.cached_trace_processed_count {
            self.cached_trace_scope_signature = scope_sig;
            self.cached_trace_processed_count = 0;
            self.cached_trace_positions.clear();
            Arc::make_mut(&mut self.cached_trace_rows).clear();
        }

        // Increment only the newly visible portion. This changes live Trace from
        // O(total-visible-frames) per repaint to O(new-frames) per repaint.
        for pos_in_scope in self.cached_trace_processed_count..visible_count {
            let idx = self.cached_scoped_indices[pos_in_scope];
            let f = &d.frames[idx];
            if overwrite {
                let key = format!("{}|{}|{}", f.bus_type, f.channel_id, f.frame_key);
                if let Some(pos) = self.cached_trace_positions.get(&key).copied() {
                    let rows = Arc::make_mut(&mut self.cached_trace_rows);
                    let row = &mut rows[pos];
                    row.idx = idx;
                    row.count += 1;
                    row.dt_s = d.trace_dt_s.get(idx).copied().unwrap_or(0.0);
                } else {
                    let pos = self.cached_trace_rows.len();
                    self.cached_trace_positions.insert(key, pos);
                    Arc::make_mut(&mut self.cached_trace_rows).push(TraceRow {
                        idx, count: 1, dt_s: d.trace_dt_s.get(idx).copied().unwrap_or(0.0)
                    });
                }
            } else {
                Arc::make_mut(&mut self.cached_trace_rows).push(TraceRow {
                    idx, count: 1, dt_s: d.trace_dt_s.get(idx).copied().unwrap_or(0.0)
                });
            }
        }
        self.cached_trace_processed_count = visible_count;
        Arc::clone(&self.cached_trace_rows)
    }

    fn channel_name_map(&self) -> BTreeMap<u32, String> {
        // One display-name resolver for the whole application. A configured name wins;
        // when no mapping exists the raw channel id remains visible.
        let mut out = BTreeMap::new();
        for (table, allow_generic_name) in [
            (&self.project_config, false),
            (&self.channel_map, true),
            (&self.vehicle_db, false),
            (&self.signal_table, false),
            (&self.event_log, false),
        ] {
            for i in 0..table.rows.len() {
                let raw_id = table.col(i, &["channel_id", "ChannelID", "Channel ID", "channel", "Channel"]);
                let mut name = table.col(i, &["channel_name", "ChannelName", "Channel Name", "bus_name", "Bus Name"]);
                if name.trim().is_empty() && allow_generic_name {
                    name = table.col(i, &["name", "Name"]);
                }
                if let (Some(id), false) = (parse_channel_id_text(&raw_id), name.trim().is_empty()) {
                    // Channels table is authoritative; otherwise first configured mapping wins.
                    if allow_generic_name || !out.contains_key(&id) {
                        out.insert(id, name.trim().to_string());
                    }
                }
            }
        }
        out.extend(self.graph_channel_names.clone());
        out
    }

    fn channel_label(&self, id: u32) -> String {
        channel_display(id,self.channel_name_map().get(&id).map(String::as_str),self.show_raw_ids)
    }

    fn frames_filters(&mut self, ui: &mut egui::Ui) {
        let Some(d) = self.dataset.clone() else { return; };
        ui.heading("Network Frames");
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("Search");
            changed |= ui.text_edit_singleline(&mut self.filter.text).changed();
            egui::ComboBox::from_label("Bus Type")
                .selected_text(self.filter.bus.map(|b| b.to_string()).unwrap_or_else(|| "All".into()))
                .show_ui(ui, |ui| {
                    changed |= ui.selectable_value(&mut self.filter.bus, None, "All").changed();
                    for b in [BusType::Can, BusType::CanFd, BusType::Lin, BusType::Ethernet, BusType::Analog, BusType::Tecmp, BusType::Unknown] {
                        changed |= ui.selectable_value(&mut self.filter.bus, Some(b), b.to_string()).changed();
                    }
                });
            if ui.button("Clear Filters").clicked() { self.filter = Filter::default(); changed = true; }
        });
        if changed {
            self.trace_filter_pending=true;
            self.trace_filter_changed_at=Some(Instant::now());
            ui.ctx().request_repaint_after(Duration::from_millis(300));
        }
        // XViewer-style cursor keyboard navigation (global A/B mode only).
        if !self.graphics_per_waveform_cursors && ui.ctx().input(|i|i.key_pressed(egui::Key::A)){self.graphics_active_cursor='A';}
        if !self.graphics_per_waveform_cursors && ui.ctx().input(|i|i.key_pressed(egui::Key::B)){self.graphics_active_cursor='B';}
        let cursor_step=if self.graphics_per_waveform_cursors {0}else{ui.ctx().input(|i|if i.key_pressed(egui::Key::ArrowLeft){-1}else if i.key_pressed(egui::Key::ArrowRight){1}else{0})};
        if cursor_step!=0 && !d.timeline_s.is_empty(){
            let current=if self.graphics_active_cursor=='A'{self.cursor_a_s}else{self.cursor_b_s};
            let idx=d.timeline_s.partition_point(|t|*t<current);
            let next=if cursor_step<0{idx.saturating_sub(1)}else{idx.min(d.timeline_s.len().saturating_sub(1)).saturating_add(1).min(d.timeline_s.len()-1)};
            let t=d.timeline_s[next];
            if self.graphics_active_cursor=='A'{self.cursor_a_s=t}else{self.cursor_b_s=t}
        }

        if self.trace_filter_pending {
            let ready=self.trace_filter_changed_at.map(|t|t.elapsed()>=Duration::from_millis(300)).unwrap_or(true);
            if ready {
                self.cached_filter_signature.clear();
                self.trace_filter_pending=false;
                self.trace_filter_changed_at=None;
            } else {
                ui.ctx().request_repaint_after(Duration::from_millis(50));
            }
        }
        self.ensure_filter_cache();

        let channels = d.index.channels.iter().copied().collect::<Vec<_>>();
        ui.horizontal(|ui| {
            let before = self.show_raw_ids;
            ui.toggle_value(&mut self.show_raw_ids, "Show Raw IDs")
                .on_hover_text("ON: ConfiguredName (RawID). OFF: ConfiguredName only. Unconfigured items always keep the raw ID.");
            if before != self.show_raw_ids { self.trace_view_cache.clear(); self.persist_session_configuration(); }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Channels:");
            for c in channels {
                let mut on = self.selected_channels.contains(&c);
                if ui.checkbox(&mut on, self.channel_label(c)).changed() {
                    if on { self.selected_channels.insert(c); } else { self.selected_channels.remove(&c); }
                }
            }
        });

        egui::CollapsingHeader::new("Context & Filters").default_open(true).show(ui, |ui| {
            ui.label("1. Context Files (Optional)");
            ui.horizontal(|ui| {
                if ui.button("Messages csv/xlsx").clicked() { self.open_context("messages"); }
                if ui.button("Events csv/xlsx").clicked() { self.open_context("event"); }
                if ui.button("Channels csv/xlsx").clicked() { self.open_context("channel"); }
            });
            for (n, t) in [("Messages", &self.vehicle_db), ("CM Markers", &self.event_log), ("Channels", &self.channel_map)] {
                if !t.filename.is_empty() { ui.label(format!("{n}: {} ({} rows)", t.filename, t.rows.len())); }
            }

            if !self.channel_map.is_empty() {
                ui.separator();
                ui.strong("Channel Name Config");
                let mapped = self.channel_name_map();
                ui.label(format!("{} valid channel name mapping(s) applied to this session.", mapped.len()));
                egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                    egui::Grid::new("channel_name_config").striped(true).show(ui, |ui| {
                        ui.strong("Channel ID"); ui.strong("Channel Name"); ui.end_row();
                        for (id, name) in mapped.iter() {
                            ui.monospace(channel_id_hex(*id)); ui.label(name); ui.end_row();
                        }
                    });
                });
                ui.small("Channel IDs display in hexadecimal (for example 0x640A). Decimal and hexadecimal configuration input are supported.");
            }

            if !self.vehicle_db.is_empty() {
                ui.separator();
                ui.label("Message / ECU scope");
                let ecus = self.vehicle_db.unique(&["ECU", "Ecu", "Node", "ECUName", "ECU Name"]);
                ui.horizontal_wrapped(|ui| {
                    for ecu in ecus {
                        let mut on = self.selected_ecus.contains(&ecu);
                        if ui.checkbox(&mut on, &ecu).changed() {
                            if on { self.selected_ecus.insert(ecu); } else { self.selected_ecus.remove(&ecu); }
                        }
                    }
                });
                table_preview(ui, &self.vehicle_db, 8);
            }

            if !self.event_log.is_empty() {
                ui.separator();
                ui.label("Event / use-case windows");
                let labels = (0..self.event_log.rows.len()).map(|i| {
                    let name = self.event_log.col(i, &["Event", "UseCase", "Use Case", "Name", "Description"]);
                    if name.is_empty() { format!("Event {}", i + 1) } else { name }
                }).collect::<Vec<_>>();
                egui::ComboBox::from_label("Use-case/event window")
                    .selected_text(self.selected_event_row.and_then(|i| labels.get(i).cloned()).unwrap_or_else(|| "None".into()))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.selected_event_row, None, "None");
                        for (i, label) in labels.iter().enumerate() { ui.selectable_value(&mut self.selected_event_row, Some(i), label); }
                    });
                if let Some(i) = self.selected_event_row {
                    let start = parse_f64_any(&self.event_log, i, &["Start", "StartTime", "Start Time", "start_s", "EventStart"]);
                    let end = parse_f64_any(&self.event_log, i, &["End", "EndTime", "End Time", "end_s", "EventEnd"]);
                    if let (Some(a), Some(b)) = (start, end) {
                        if ui.button(format!("Apply event window {:.3}-{:.3}s", a, b)).clicked() {
                            self.time_start = a; self.time_end = b; self.use_manual_window = true;
                        }
                    }
                }
            }

            ui.separator();
            ui.checkbox(&mut self.use_manual_window, "Use manual time window");
            if self.use_manual_window {
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut self.time_start).speed(0.01).prefix("Start "));
                    ui.add(egui::DragValue::new(&mut self.time_end).speed(0.01).prefix("End "));
                });
            }
        });

        // Take an owned snapshot so the UI closure can freely mutate `self.info` /
        // `self.error` without keeping an immutable borrow of `self` alive.
        self.ensure_filter_cache();
        self.ensure_scope_cache();
        let indices = self.cached_scoped_indices.clone();
        ui.horizontal(|ui| {
            ui.label(format!("Displayed frames: {}", indices.len()));
            if ui.button("Prepare displayed frames CSV").clicked() {
                if let Some(p) = rfd::FileDialog::new().set_file_name("displayed_frames.csv").save_file() {
                    let rows = indices.iter().map(|i| &d.frames[*i]).collect::<Vec<_>>();
                    match export::frames_csv(&p, &rows) {
                        Ok(_) => self.info = Some(format!("Saved {}", p.display())),
                        Err(e) => self.error = Some(e.to_string()),
                    }
                }
            }
        });
        let channel_names = self.channel_name_map();
        virtual_frame_table(ui, &d.frames, &indices, &channel_names, self.show_raw_ids);
    }

    fn graphics_source_byte_selected(&self,source_key:&str,byte:usize)->bool{
        self.selected_frame_bytes.get(source_key).map(|bytes|bytes.contains(&byte)).unwrap_or(false)
    }
    fn graphics_source_selected_count(&self,source_key:&str,dlc:usize)->usize{
        self.selected_frame_bytes.get(source_key)
            .map(|bytes|bytes.iter().filter(|&&b|b<dlc).count()).unwrap_or(0)
    }
    fn graphics_source_all_selected(&self,source_key:&str,dlc:usize)->bool{
        dlc>0 && self.graphics_source_selected_count(source_key,dlc)==dlc
    }
    fn set_graphics_source_byte(&mut self,source_key:&str,byte:usize,on:bool){
        let bytes=self.selected_frame_bytes.entry(source_key.to_string()).or_default();
        if on{bytes.insert(byte);}else{bytes.remove(&byte);}
        let empty=bytes.is_empty();
        if empty { self.selected_frame_bytes.remove(source_key); self.graphics_sources.remove(source_key); }
        let lane=format!("SRC|{}|{}",source_key,byte);
        if on{
            self.graphics_sources.insert(source_key.to_string());
            if !self.graphics_lane_order.contains(&lane){self.graphics_lane_order.push(lane);}
        }else{
            self.graphics_lane_order.retain(|x|x!=&lane);
        }
        // Selection does not change raw data, decoded-signal definitions, or classification.
    }
    fn set_graphics_source_mask(&mut self,source_key:&str,dlc:usize,on:bool){
        if on {
            self.selected_frame_bytes.insert(source_key.to_string(),(0..dlc).collect());
            self.graphics_sources.insert(source_key.to_string());
        } else {
            self.selected_frame_bytes.remove(source_key);
            self.graphics_sources.remove(source_key);
        }
        let prefix=format!("SRC|{}|",source_key);
        self.graphics_lane_order.retain(|x|!x.starts_with(&prefix));
        if on{for b in 0..dlc{self.graphics_lane_order.push(format!("SRC|{}|{}",source_key,b));}}
    }
    fn frame_payload_len(d:&Dataset,key:&str)->usize{
        d.index.by_key.get(key)
            .and_then(|ix|ix.iter().filter_map(|&i|d.frames.get(i).map(|f|f.payload.len())).max())
            .unwrap_or(0)
    }
    fn selected_byte_count(&self,d:&Dataset,key:&str)->usize{
        let n=Self::frame_payload_len(d,key);
        self.selected_frame_bytes.get(key)
            .map(|bytes|bytes.iter().filter(|&&b|b<n).count())
            .unwrap_or(n)
    }
    fn set_selected_bytes(&mut self,d:&Dataset,key:&str,bytes:BTreeSet<usize>){
        let n=Self::frame_payload_len(d,key);
        self.selected_frame_bytes.insert(key.to_string(),bytes.into_iter().filter(|&b|b<n).collect());
        self.multi_activity_signature.clear();
    }
    fn byte_selected(&self,d:&Dataset,key:&str,byte:usize)->bool{
        let n=Self::frame_payload_len(d,key);
        byte<n && self.selected_frame_bytes.get(key).map(|bytes|bytes.contains(&byte)).unwrap_or(true)
    }
    fn select_signal_bytes(&mut self,d:&Dataset,key:&str){
        let mut bytes=BTreeSet::new();
        if let Some(ix)=d.index.by_key.get(key){
            for r in byte_activity_indexed(&d.frames,ix,self.selected_channel,None,None){
                if r.class=="ACTIVE"{bytes.insert(r.byte);}
            }
        }
        self.set_selected_bytes(d,key,bytes);
    }

    fn ensure_multi_activity_cache(&mut self,d:&Dataset){
        // Byte Analysis uses the same committed Analysis Range as Graphics.
        // Measurement cursors A/B are intentionally not an analysis scope.
        let (start,end)=self.graphics_class_window
            .map(|(a,b)|(Some(a.min(b)),Some(a.max(b))))
            .unwrap_or((None,None));
        let sig=format!("{:?}|{:?}|{:?}|{:?}",self.multi_frame_keys,self.selected_channel,start,end);
        if sig==self.multi_activity_signature{return}
        self.multi_activity_cache.clear();
        for key in &self.multi_frame_keys{if let Some(ix)=d.index.by_key.get(key){self.multi_activity_cache.push((key.clone(),byte_activity_indexed(&d.frames,ix,self.selected_channel,start,end)));}}
        self.multi_activity_signature=sig;
    }

    fn frame_explorer(&mut self, ui: &mut egui::Ui) {
        let Some(d) = self.dataset.clone() else { return; };
        ui.heading("Byte Analysis");
        ui.label("Inspect raw frame activity before creating signal definitions.");

        egui::CollapsingHeader::new("Event / adjacent triggers").default_open(true).show(ui, |ui| {
            if self.event_log.is_empty() { ui.label("Load Events in Configuration to scope analysis around a trigger."); }
            else { table_preview(ui, &self.event_log, 6); }
        });
        egui::CollapsingHeader::new("Message / ECU scope").default_open(true).show(ui, |ui| {
            if self.vehicle_db.is_empty() { ui.label("Load messages.csv in Configuration to relate frame IDs to names/ECUs."); }
            else { table_preview(ui, &self.vehicle_db, 8); }
        });

        let mut keys=d.index.by_key.keys().cloned().collect::<Vec<_>>(); keys.sort();
        egui::CollapsingHeader::new("Byte Analysis").default_open(false).show(ui,|ui|{
            ui.horizontal(|ui|{
                if ui.button("Add current frame").clicked()&&!self.selected_frame_key.is_empty(){self.multi_frame_keys.insert(self.selected_frame_key.clone());self.multi_activity_signature.clear();}
                if ui.button("Clear frames").clicked(){self.multi_frame_keys.clear();self.multi_activity_signature.clear();}
                ui.checkbox(&mut self.multi_hide_static,"Hide constant bytes");
                ui.checkbox(&mut self.multi_hide_counter,"Hide alive counters");
                ui.checkbox(&mut self.multi_hide_crc_candidates,"Hide CRC/checksum candidates");
                if let Some((a,b))=self.graphics_class_window {
                    ui.label(format!("Analysis Range {:.3}-{:.3}s",a,b));
                } else {
                    ui.label("Analysis Range: Full capture");
                }
                ui.label(format!("{} selected",self.multi_frame_keys.len()));
            });
            egui::ScrollArea::vertical().max_height(110.0).show(ui,|ui|{ui.horizontal_wrapped(|ui|{for k in &keys{let mut on=self.multi_frame_keys.contains(k);if ui.checkbox(&mut on,frame_key_display(k)).changed(){if on{self.multi_frame_keys.insert(k.clone());}else{self.multi_frame_keys.remove(k);}self.multi_activity_signature.clear();}}});});
            if !self.multi_frame_keys.is_empty(){
                ui.separator();
                ui.strong("Selected Bytes");
                ui.small("These byte selections affect Byte Analysis and raw-byte Graphics. Trace always shows the complete original frame.");
                let chosen:Vec<String>=self.multi_frame_keys.iter().cloned().collect();
                for key in chosen{
                    let n=Self::frame_payload_len(&d,&key);
                    let selected_count=self.selected_byte_count(&d,&key);
                    ui.group(|ui|{
                        ui.horizontal(|ui|{
                            ui.monospace(frame_key_display(&key)); ui.label(format!("{}/{} selected",selected_count,n));
                            if ui.button("All").clicked(){self.set_selected_bytes(&d,&key,(0..n).collect());}
                            if ui.button("None").clicked(){self.set_selected_bytes(&d,&key,BTreeSet::new());}
                            if ui.button("Signals Only").on_hover_text("Select likely signal bytes and exclude constant, counter and CRC/checksum candidates").clicked(){self.select_signal_bytes(&d,&key);}
                            if ui.button("Invert").clicked(){
                                let inverted=(0..n).filter(|&b|!self.byte_selected(&d,&key,b)).collect();
                                self.set_selected_bytes(&d,&key,inverted);
                            }
                        });
                        ui.horizontal_wrapped(|ui|{
                            for b in 0..n{
                                let mut on=self.byte_selected(&d,&key,b);
                                if ui.checkbox(&mut on,format!("B{}",b)).changed(){
                                    let mut bytes=self.selected_frame_bytes.get(&key).cloned().unwrap_or_else(||(0..n).collect());
                                    if on{bytes.insert(b);}else{bytes.remove(&b);}
                                    self.set_selected_bytes(&d,&key,bytes);
                                }
                            }
                        });
                    });
                }
            }
            self.ensure_multi_activity_cache(&d);
            if !self.multi_frame_keys.is_empty(){
                ui.small("How to read this: SIGNAL ACTIVITY = likely useful data; CONSTANT = unchanged; ALIVE COUNTER = rolling 4/8-bit counter; REPEATING PATTERN = stable short-period sequence; CRC / CHECKSUM CANDIDATE = checksum/CRC evidence. Detection works on normalized CAN/CAN-FD/LIN/ETH/TECMP/UNKNOWN payloads. CRC remains a candidate until the exact OEM algorithm is proven.");
                egui::ScrollArea::vertical().max_height(360.0).show(ui,|ui|{
                    egui::Grid::new("multi_byte_activity").striped(true).show(ui,|ui|{
                        for h in ["Frame","Byte","Meaning","Confidence","Changes","Activity %","Unique","Min","Max","Value","Why?"]{ui.strong(h);}ui.end_row();
                        for(key,rows)in &self.multi_activity_cache{for r in rows{
                            if !self.byte_selected(&d,key,r.byte){continue}
                            if self.multi_hide_static&&r.class=="STATIC"{continue}
                            if self.multi_hide_counter&&(r.class=="COUNTER8"||r.class=="COUNTER4"){continue}
                            if self.multi_hide_crc_candidates&&r.class=="CRC_CANDIDATE"{continue}
                            ui.monospace(frame_key_display(key));ui.label(format!("B{}",r.byte));ui.label(crate::analysis::friendly_byte_class(r.class));ui.label(format!("{:.1}%",100.0*r.confidence));
                            ui.label(r.transitions.to_string());ui.label(format!("{:.2}",r.activity_pct));ui.label(r.unique_values.to_string());
                            ui.monospace(format!("{:02X}",r.min));ui.monospace(format!("{:02X}",r.max));
                            ui.monospace(if r.class=="STATIC"{format!("{:02X}",r.first)}else{format!("{:02X}->{:02X}",r.first,r.last)});ui.small(&r.reason);ui.end_row();
                        }}
                    });
                });
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            egui::ComboBox::from_label("Frame ID / Key").selected_text(if self.selected_frame_key.is_empty() { "Select".to_string() } else { frame_key_display(&self.selected_frame_key) }).show_ui(ui, |ui| {
                for k in &keys { ui.selectable_value(&mut self.selected_frame_key, k.clone(), frame_key_display(k)); }
            });
            let channels = self.selected_frame_channels(&d);
            egui::ComboBox::from_label("Channel").selected_text(self.selected_channel.map(|v| self.channel_label(v)).unwrap_or_else(|| "All".into())).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.selected_channel, None, "All");
                for c in channels { let label = self.channel_label(c); ui.selectable_value(&mut self.selected_channel, Some(c), label); }
            });
        });

        ui.horizontal(|ui| {
            ui.radio_value(&mut self.activity_level, ActivityLevel::Bytes, "Bytes");
            ui.radio_value(&mut self.activity_level, ActivityLevel::Bits, "Bits");
            ui.separator();
            ui.radio_value(&mut self.explorer_mode, ExplorerMode::Activity, "Activity");
            ui.radio_value(&mut self.explorer_mode, ExplorerMode::Values, "Values");
            ui.radio_value(&mut self.explorer_mode, ExplorerMode::Changes, "Payload changes");
            ui.radio_value(&mut self.explorer_mode, ExplorerMode::Waveform, "Waveform");
        });

        if self.selected_frame_key.is_empty() { ui.label("Select a frame ID/key."); return; }
        match (self.activity_level, self.explorer_mode) {
            (ActivityLevel::Bytes, ExplorerMode::Activity) => {
                let rows = byte_activity(&d.frames, &self.selected_frame_key, self.selected_channel);
                egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                    egui::Grid::new("byte_activity").striped(true).show(ui, |ui| {
                        for h in ["Byte", "Transitions", "Activity %", "Unique", "Min", "Max"] { ui.strong(h); } ui.end_row();
                        for r in rows { ui.label(r.byte.to_string()); ui.label(r.transitions.to_string()); ui.label(format!("{:.2}", r.activity_pct)); ui.label(r.unique_values.to_string()); ui.monospace(format!("{:02X}", r.min)); ui.monospace(format!("{:02X}", r.max)); ui.end_row(); }
                    });
                });
            }
            (ActivityLevel::Bits, ExplorerMode::Activity) => {
                let rows = bit_activity(&d.frames, &self.selected_frame_key, self.selected_channel);
                egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                    egui::Grid::new("bit_activity").striped(true).show(ui, |ui| {
                        for h in ["Byte", "Bit", "Transitions", "Activity %", "High %"] { ui.strong(h); } ui.end_row();
                        for (byte, bit, trans, activity, high) in rows { ui.label(byte.to_string()); ui.label(bit.to_string()); ui.label(trans.to_string()); ui.label(format!("{activity:.2}")); ui.label(format!("{high:.2}")); ui.end_row(); }
                    });
                });
            }
            (_, ExplorerMode::Values) => {
                ui.horizontal(|ui| { ui.add(egui::DragValue::new(&mut self.inspect_byte).range(0..=63).prefix("Inspect byte ")); ui.add(egui::DragValue::new(&mut self.reference_value).range(0..=255).prefix("Reference ")); });
                let rows = byte_value_counts(&d.frames, &self.selected_frame_key, self.selected_channel, self.inspect_byte);
                egui::Grid::new("value_counts").striped(true).show(ui, |ui| {
                    for h in ["Value", "Hex", "Count", "%", "vs reference"] { ui.strong(h); } ui.end_row();
                    for (v, n, pct) in rows { ui.label(v.to_string()); ui.monospace(format!("0x{v:02X}")); ui.label(n.to_string()); ui.label(format!("{pct:.2}")); ui.label((v as i16 - self.reference_value as i16).to_string()); ui.end_row(); }
                });
            }
            (_, ExplorerMode::Changes) => {
                let rows = payload_change_events(&d.frames, &self.selected_frame_key, self.selected_channel, 500);
                egui::ScrollArea::vertical().max_height(390.0).show(ui, |ui| {
                    egui::Grid::new("changes").striped(true).show(ui, |ui| {
                        for h in ["Time", "Payload", "Changed bytes"] { ui.strong(h); } ui.end_row();
                        for (t, payload, changes) in rows { ui.label(format!("{t:.6}")); ui.monospace(payload); ui.label(changes); ui.end_row(); }
                    });
                });
            }
            (_, ExplorerMode::Waveform) => {
                if self.activity_level == ActivityLevel::Bits {
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut self.inspect_byte).range(0..=63).prefix("Byte "));
                        ui.add(egui::DragValue::new(&mut self.inspect_bit).range(0..=7).prefix("Bit "));
                    });
                    let raw = scoped_payload_points(&d,&self.selected_frame_key,self.selected_channel,&self.project_channels,self.inspect_byte,Some(self.inspect_bit));
                    let points = downsample_minmax(&raw, 3000);
                    let marks=control_triggers(&d.frames,0.0);
                    draw_canoe_lane(ui,&format!("{} · B{}.b{}",frame_key_display(&self.selected_frame_key),self.inspect_byte,self.inspect_bit),&points,300.0,self.cursor_a_s,self.cursor_b_s,&marks,Some((0.0,1.0)));
                } else {
                    ui.strong("Raw Byte Waveforms");
                    let activities = byte_activity(&d.frames, &self.selected_frame_key, self.selected_channel);
                    let changing = activities.iter().filter(|r| r.transitions > 0).map(|r| r.byte).collect::<Vec<_>>();
                    ui.horizontal(|ui| {
                        if ui.button("Select first 4 changing bytes").clicked() {
                            self.raw_waveform_bytes.clear();
                            for b in changing.iter().take(4) { self.raw_waveform_bytes.insert(*b); }
                        }
                        if ui.button("Select all changing bytes").clicked() {
                            self.raw_waveform_bytes = changing.iter().copied().collect();
                        }
                        if ui.button("Clear").clicked() { self.raw_waveform_bytes.clear(); }
                    });
                    ui.horizontal_wrapped(|ui| {
                        for b in &changing {
                            let mut on = self.raw_waveform_bytes.contains(b);
                            if ui.checkbox(&mut on, format!("Byte {b}")).changed() {
                                if on { self.raw_waveform_bytes.insert(*b); } else { self.raw_waveform_bytes.remove(b); }
                            }
                        }
                    });
                    if changing.is_empty() {
                        ui.label("No changing bytes were found for the selected frame/channel.");
                    } else if self.raw_waveform_bytes.is_empty() {
                        ui.label("Select one or more changing bytes to display their waveforms.");
                    } else {
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut self.raw_waveform_stacked, false, "Overlap");
                            ui.radio_value(&mut self.raw_waveform_stacked, true, "Individual stacked");
                        });
                        let traces = self.raw_waveform_bytes.iter().map(|b| {
                            let raw = scoped_payload_points(&d,&self.selected_frame_key,self.selected_channel,&self.project_channels,*b,None);
                            (format!("Byte {b}"), downsample_minmax(&raw, 2500))
                        }).collect::<Vec<_>>();
                        if self.raw_waveform_stacked {
                            for (name, pts) in &traces {
                                ui.label(egui::RichText::new(name).strong());
                                draw_wave_fixed_range(ui, pts, 170.0, 0.0, 255.0);
                            }
                        } else {
                            draw_multi_byte_wave(ui, &traces, 340.0);
                        }
                    }
                }
            }
        }

        ui.separator();
        ui.checkbox(&mut self.compare_ids, "Compare two frame IDs bit activity");
        if self.compare_ids {
            ui.horizontal_wrapped(|ui| {
                for k in keys.iter().take(100) {
                    let mut on = self.compare_keys.contains(k);
                    if ui.checkbox(&mut on, k).changed() {
                        if on && self.compare_keys.len() < 2 { self.compare_keys.insert(k.clone()); }
                        if !on { self.compare_keys.remove(k); }
                    }
                }
            });
            if self.compare_keys.len() == 2 {
                let selected = self.compare_keys.iter().cloned().collect::<Vec<_>>();
                let a = bit_activity(&d.frames, &selected[0], self.selected_channel);
                let b = bit_activity(&d.frames, &selected[1], self.selected_channel);
                let map_b: BTreeMap<(usize, usize), (usize, f64, f64)> = b.into_iter().map(|(by, bi, tr, ac, hi)| ((by, bi), (tr, ac, hi))).collect();
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    egui::Grid::new("compare_bits").striped(true).show(ui, |ui| {
                        ui.strong("Byte.Bit"); ui.strong(&selected[0]); ui.strong(&selected[1]); ui.end_row();
                        for (by, bi, tr, ac, _) in a {
                            let rhs = map_b.get(&(by, bi)).map(|x| format!("{} / {:.1}%", x.0, x.1)).unwrap_or_default();
                            ui.label(format!("{by}.{bi}")); ui.label(format!("{tr} / {ac:.1}%")); ui.label(rhs); ui.end_row();
                        }
                    });
                });
            }
        }

        egui::CollapsingHeader::new("Custom Signal Preview").default_open(false).show(ui, |ui| {
            ui.text_edit_singleline(&mut self.custom_name);
            egui::ComboBox::from_label("Frame").selected_text(if self.custom_frame.is_empty() { "Select" } else { &self.custom_frame }).show_ui(ui, |ui| {
                for k in &keys { ui.selectable_value(&mut self.custom_frame, k.clone(), k); }
            });
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut self.custom_start_byte).range(0..=63).prefix("Byte "));
                ui.add(egui::DragValue::new(&mut self.custom_start_bit).range(0..=7).prefix("Bit "));
                ui.add(egui::DragValue::new(&mut self.custom_bit_length).range(1..=64).prefix("Length "));
            });
            ui.horizontal(|ui| {
                let was_little=self.custom_little_endian;
                egui::ComboBox::from_label("Endian")
                    .selected_text(if self.custom_little_endian { "Intel / Little" } else { "Motorola / Big" })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.custom_little_endian, true, "Intel / Little");
                        ui.selectable_value(&mut self.custom_little_endian, false, "Motorola / Big");
                    });
                if was_little!=self.custom_little_endian{
                    if self.custom_little_endian && self.custom_start_bit==7{self.custom_start_bit=0;}
                    if !self.custom_little_endian && self.custom_start_bit==0{self.custom_start_bit=7;}
                }
                ui.checkbox(&mut self.custom_signed, "Signed");
                ui.add(egui::DragValue::new(&mut self.custom_scale).speed(0.01).prefix("Scale "));
                ui.add(egui::DragValue::new(&mut self.custom_offset).speed(0.01).prefix("Offset "));
            });
            ui.weak(if self.custom_little_endian {
                "Physical bit numbering: Bit 0 = LSB. Byte-aligned Intel signals start at Bit 0."
            } else {
                "Physical bit numbering: Bit 7 = MSB. Byte-aligned Motorola signals start at Bit 7."
            });
            let mut points = Vec::new();
            if let Some(indices) = d.index.by_key.get(&self.custom_frame) {
                points.reserve(indices.len().min(100_000));
                for &i in indices {
                    let f=&d.frames[i];
                    if self.selected_channel.is_some_and(|ch|ch!=f.channel_id) || !self.project_channels.contains(&project_channel_key(&f.bus_type.to_string(),f.channel_id)){continue;}
                    if let Some(v)=decode_signal(&f.payload, self.custom_start_byte, self.custom_start_bit, self.custom_bit_length, self.custom_little_endian, self.custom_signed, self.custom_scale, self.custom_offset) {
                        points.push([f.relative_time_s,v]);
                    }
                }
            }
            let points = native_engine::downsample(&points, 3000);
            let marks=control_triggers(&d.frames,0.0);
            draw_canoe_lane(ui,&self.custom_name,&points,250.0,self.cursor_a_s,self.cursor_b_s,&marks,None);
        });
    }

    fn measurement_toolbar(&mut self, ui: &mut egui::Ui, d: &Dataset) {
        // Measurement owns capture/replay controls. Analysis and Configuration use purpose-built toolbars.
        let palette = HomePalette::new(ui.visuals().dark_mode);
        egui::Frame::new()
            .fill(palette.surface)
            .stroke(Stroke::new(1.0, palette.border))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::symmetric(9, 7))
            .show(ui, |ui| {
                ui.set_min_width((ui.available_width() - 18.0).max(0.0));
                egui::ScrollArea::horizontal().id_salt("measurement_toolbar_scroll_v121").show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.add_sized([104.0, 34.0], egui::Button::new("Open")).on_hover_text("Open an automotive capture/log").clicked() { self.open_log(); }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    if ui.add_enabled_ui(!self.measurement_running, |ui| ui.add_sized([90.0, 34.0], egui::Button::new("Run"))).inner.on_hover_text("Replay the loaded log using its recorded timestamps").clicked() {
                        let (replay_start, replay_end, _) = self.replay_bounds(d);
                        if self.replay_position_s < replay_start || self.replay_position_s >= replay_end.max(replay_start + 0.001) {
                            self.replay_position_s = replay_start;
                        }
                        self.measurement_running = true;
                        self.measurement_paused = false;
                        self.trace_freeze_view = false;
                        self.trace_display_position_s = self.replay_position_s;
                        self.replay_anchor = Some(Instant::now());
                    }
                    if ui.add_enabled_ui(self.measurement_running, |ui| ui.add_sized([104.0, 34.0], egui::Button::new(if self.measurement_paused { "Resume" } else { "Pause" }))).inner.on_hover_text("Pause or resume offline replay").clicked() {
                        self.measurement_paused = !self.measurement_paused;
                        self.replay_anchor = if self.measurement_paused { None } else { Some(Instant::now()) };
                    }
                    if ui.add_enabled_ui(self.measurement_running, |ui| ui.add_sized([90.0, 34.0], egui::Button::new("Stop"))).inner.on_hover_text("Stop offline replay").clicked() {
                        self.measurement_running = false;
                        self.measurement_paused = false;
                        self.replay_anchor = None;
                    }
                    egui::ComboBox::from_id_salt("shared_replay_speed_combo")
                        .selected_text(format!("{}x", self.replay_speed))
                        .width(68.0)
                        .show_ui(ui, |ui| {
                            for speed in [0.25_f32, 0.5, 1.0, 2.0, 5.0, 10.0] {
                                ui.selectable_value(&mut self.replay_speed, speed, format!("{speed}x"));
                            }
                        });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    if ui.add_sized([104.0, 34.0], egui::SelectableLabel::new(self.workspace == Workspace::Measurement && self.show_network_tree, "Explorer")).on_hover_text("Show the searchable Capture Explorer").clicked() {
                        self.show_network_tree = true;
                        self.workspace = Workspace::Measurement;
                    }
                    let graphics_active = self.graphics_viewer_open
                        || self.floating_panes.contains(&InspectorPane::Graphics);
                    if ui.add_sized([100.0, 34.0], egui::SelectableLabel::new(graphics_active, "Graphics"))
                        .on_hover_text("Open the dedicated waveform viewer; keep Frame Inspector docked").clicked() {
                        self.workspace = Workspace::Measurement;
                        self.inspector_pane = InspectorPane::Details;
                        self.graphics_viewer_open = true;
                    }
                    if ui.add_sized([96.0, 34.0], egui::SelectableLabel::new(self.workspace == Workspace::Measurement && self.inspector_pane == InspectorPane::None, "Trace")).on_hover_text("Maximize Frame Trace").clicked() {
                        self.workspace = Workspace::Measurement;
                        self.inspector_pane = InspectorPane::None;
                    }
                    if ui.add_sized([110.0, 34.0], egui::SelectableLabel::new(
                        self.workspace == Workspace::Measurement && self.inspector_pane == InspectorPane::Details, "Inspector"))
                        .on_hover_text("Inspect the selected frame and payload bytes").clicked() {
                        self.workspace = Workspace::Measurement;
                        self.inspector_pane = InspectorPane::Details;
                    }
                });
                });
            });
        ui.add_space(5.0);

        egui::Frame::new()
            .fill(palette.surface)
            .stroke(Stroke::new(1.0, palette.border))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.set_min_width((ui.available_width() - 24.0).max(0.0));
                ui.horizontal_wrapped(|ui| {
                    ui.strong("REPLAY");
                    let (replay_start, replay_end, replay_label) = self.replay_bounds(d);
                    self.replay_position_s = self.replay_position_s.clamp(replay_start, replay_end.max(replay_start));
                    let slider = egui::Slider::new(&mut self.replay_position_s, replay_start..=replay_end.max(replay_start + 0.000_001)).show_value(false);
                    let slider_width = (ui.available_width() - 285.0).max(170.0);
                    if ui.add_sized([slider_width, 26.0], slider).on_hover_text("Offline replay timeline").changed() {
                        self.replay_anchor = if self.measurement_running && !self.measurement_paused { Some(Instant::now()) } else { None };
                    }
                    let first_ns = d.frames.first().map(|f| f.timestamp_ns).unwrap_or(0);
                    let last_ns = d.frames.last().map(|f| f.timestamp_ns).unwrap_or(first_ns);
                    let replay_ns = timestamp_at_relative(first_ns, self.replay_position_s);
                    let range_start_ns = timestamp_at_relative(first_ns, replay_start);
                    let range_end_ns = timestamp_at_relative(first_ns, replay_end);
                    let prefix = replay_label.as_deref().unwrap_or("Capture");
                    ui.monospace(format!("{prefix} · {:.3} s / {:.3} s", self.replay_position_s, replay_end))
                        .on_hover_text(format!("Replay timestamp: {}\nRange start: {}\nRange end: {}\nCapture start: {}\nCapture end: {}\nRelative replay: {:.6} s", format_timestamp_ns(replay_ns), format_timestamp_ns(range_start_ns), format_timestamp_ns(range_end_ns), format_timestamp_ns(first_ns), format_timestamp_ns(last_ns), self.replay_position_s));
                });
            });
        ui.add_space(5.0);
    }

    fn canoe_workspace(&mut self, ui: &mut egui::Ui) {
        let Some(d) = self.dataset.clone() else { return; };
        let palette = HomePalette::new(ui.visuals().dark_mode);
        // Page 3 only: use the same legible type, spacing and dark surfaces as the approved preview.
        // This local UI style deliberately leaves Home and Channel Selection unchanged.
        {
            let style = ui.style_mut();
            style.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
            style.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
            style.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
            style.text_styles.insert(egui::TextStyle::Monospace, egui::FontId::monospace(12.0));
            style.spacing.item_spacing = Vec2::new(8.0, 7.0);
            style.spacing.button_padding = Vec2::new(11.0, 7.0);
            style.spacing.interact_size.y = 29.0;
            style.visuals.faint_bg_color = palette.surface;
            style.visuals.extreme_bg_color = palette.backdrop;
        }
        ui.painter().rect_filled(ui.max_rect(), egui::CornerRadius::ZERO, palette.backdrop);
        // Historic sessions may have a docked Graphics placeholder. The approved
        // layout uses a Frame Inspector here and a dedicated Waveform Viewer instead.
        if self.inspector_pane == InspectorPane::Graphics {
            self.inspector_pane = InspectorPane::Details;
        }
        if self.selected_trace_row.is_none() && !d.frames.is_empty() {
            self.selected_trace_row = Some(0);
        }
        self.ensure_filter_cache();

        self.capture_explorer_overview(ui, &d);
        self.measurement_toolbar(ui, &d);

        let available = ui.available_rect_before_wrap();
        let splitter = 5.0;
        let left_w = if self.show_network_tree {
            explorer_panel_width(available.width(), self.project_tree_width)
        } else { 0.0 };
        let tree_split_w = if self.show_network_tree { splitter } else { 0.0 };
        let work_left = available.left() + left_w + tree_split_w;
        let work_rect = egui::Rect::from_min_max(egui::pos2(work_left, available.top()), available.max);

        let mut trace_rect = work_rect;
        let mut inspector_rect = egui::Rect::NOTHING;
        // Keep the trace wide enough when the application window is narrow.
        if work_rect.width() < 840.0 && self.inspector_dock == InspectorDock::Right {
            self.inspector_dock = InspectorDock::Bottom;
        }
        let has_docked = self.inspector_pane != InspectorPane::None;
        if has_docked {
            match self.inspector_dock {
                InspectorDock::Bottom => {
                    let h = docked_pane_extent(work_rect.height(), self.inspector_height, 220.0, 160.0);
                    let y = work_rect.bottom() - h;
                    trace_rect.max.y = y - splitter;
                    inspector_rect = egui::Rect::from_min_max(egui::pos2(work_rect.left(), y), work_rect.max);
                }
                InspectorDock::Right => {
                    let w = docked_pane_extent(work_rect.width(), self.inspector_width, 300.0, 240.0);
                    let x = work_rect.right() - w;
                    trace_rect.max.x = x - splitter;
                    inspector_rect = egui::Rect::from_min_max(egui::pos2(x, work_rect.top()), work_rect.max);
                }
            }
        }

        if self.show_network_tree {
            // Resolve configured channel names once per repaint, not for each tree row.
            let explorer_channel_names = self.channel_name_map();
            let explorer_show_raw = self.show_raw_ids;
            let left_rect = egui::Rect::from_min_max(available.min, egui::pos2(available.left() + left_w, available.bottom()));
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(left_rect), |ui| {
                egui::Frame::new().fill(palette.surface)
                .stroke(Stroke::new(1.0, palette.border))
                .corner_radius(egui::CornerRadius::same(8))
                .show(ui, |ui| {
                    ui.set_min_size(left_rect.size());
                    ui.spacing_mut().item_spacing.y = 7.0;
                    ui.spacing_mut().interact_size.y = 26.0;
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("CAPTURE EXPLORER").size(14.0).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("×").on_hover_text("Hide Capture Explorer").clicked() {
                                self.show_network_tree = false;
                            }
                        });
                    });
                    ui.label(egui::RichText::new("PROTOCOLS / CHANNELS").size(10.0).weak());
                    ui.add_space(6.0);
                    ui.add(egui::TextEdit::singleline(&mut self.explorer_search)
                        .hint_text("Search network, channel or 0x ID")
                        .desired_width(ui.available_width().max(140.0)));
                    ui.horizontal_wrapped(|ui| {
                        egui::ComboBox::from_id_salt("capture_explorer_protocol_filter_r5")
                            .selected_text(self.explorer_protocol.as_str())
                            .width(108.0)
                            .show_ui(ui, |ui| {
                                for protocol in ["All", "CAN", "CAN FD", "LIN", "Ethernet", "ANALOG", "TECMP", "Other"] {
                                    ui.selectable_value(&mut self.explorer_protocol, protocol.to_string(), protocol);
                                }
                            });
                        if ui.small_button("Clear search").clicked() {
                            self.explorer_search.clear();
                            self.explorer_protocol = "All".into();
                        }
                    });
                    let visible_matches = d.index.networks.iter().map(|(bus, channels)| {
                        channels.iter().filter(|ch| {
                            let label = channel_display(**ch,
                                explorer_channel_names.get(*ch).map(String::as_str), explorer_show_raw);
                            explorer_matches_channel(bus, **ch, &label,
                                &self.explorer_search, &self.explorer_protocol)
                        }).count()
                    }).sum::<usize>();
                    let selected_count = self.project_channels.len();
                    ui.horizontal_wrapped(|ui| {
                        ui.label(egui::RichText::new(format!("{visible_matches} visible  ·  {selected_count} enabled")).small().weak());
                        let was_showing_raw = self.show_raw_ids;
                        ui.toggle_value(&mut self.show_raw_ids, "Raw IDs")
                            .on_hover_text("Show configured names alongside raw channel IDs");
                        if was_showing_raw != self.show_raw_ids {
                            self.trace_view_cache.clear();
                            self.persist_session_configuration();
                            ui.ctx().request_repaint();
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.small_button("Enable all").clicked() {
                            self.project_channels = d.index.project_channel_keys.clone();
                            self.cached_scope_signature.clear();
                        }
                        if ui.small_button("Disable all").clicked() {
                            self.project_channels.clear();
                            self.cached_scope_signature.clear();
                        }
                    });
                    ui.separator();
                    let channel_list_height = (ui.available_height() - 95.0).max(90.0);
                    egui::ScrollArea::vertical().id_salt("capture_explorer_tree_r5")
                        .max_height(channel_list_height)
                        .auto_shrink([false, false]).show(ui, |ui| {
                            if visible_matches == 0 {
                                ui.add_space(14.0);
                                ui.label("No matching channels");
                                ui.weak("Adjust the search or protocol filter.");
                            }
                            for (bus, channels) in &d.index.networks {
                                let matching: Vec<u32> = channels.iter().copied().filter(|ch| {
                                    let label = channel_display(*ch,
                                        explorer_channel_names.get(ch).map(String::as_str), explorer_show_raw);
                                    explorer_matches_channel(bus, *ch, &label,
                                        &self.explorer_search, &self.explorer_protocol)
                                }).collect();
                                if matching.is_empty() { continue; }
                                egui::CollapsingHeader::new(format!("{bus}  ·  {} channels", matching.len()))
                                    .default_open(true).show(ui, |ui| {
                                        for ch in matching {
                                            let pkey = project_channel_key(bus, ch);
                                            let mut enabled = self.project_channels.contains(&pkey);
                                            let loaded = d.index.by_project_channel.get(&pkey)
                                                .is_some_and(|rows| !rows.is_empty());
                                            let display = channel_display(ch,
                                                explorer_channel_names.get(&ch).map(String::as_str), explorer_show_raw);
                                            let label = if loaded { display.clone() }
                                                else { format!("{display} · not loaded") };
                                            if ui.checkbox(&mut enabled, egui::RichText::new(label).size(13.0))
                                                .on_hover_text(if loaded {
                                                    "Show/hide this channel in the active analysis scope"
                                                } else {
                                                    "No decoded records loaded; its selection is retained"
                                                }).changed() {
                                                if enabled { self.project_channels.insert(pkey); }
                                                else { self.project_channels.remove(&pkey); }
                                                self.cached_scope_signature.clear();
                                            }
                                        }
                                    });
                            }
                        });
                    ui.separator();
                    ui.label(egui::RichText::new("QUICK ANALYSIS").size(11.0).strong().color(palette.muted));
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Frame / Byte Inspector").clicked() {
                            self.analysis_page = AnalysisPage::FrameByte;
                            self.workspace = Workspace::Analysis;
                        }
                        if ui.button("Signal Discovery").clicked() {
                            self.analysis_page = AnalysisPage::SignalDiscovery;
                            self.workspace = Workspace::Analysis;
                        }
                    });
                });
            });

            let split_rect = egui::Rect::from_min_max(
                egui::pos2(available.left() + left_w, available.top()),
                egui::pos2(available.left() + left_w + splitter, available.bottom()),
            );
            let resp = ui.allocate_rect(split_rect, Sense::drag()).on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
            if resp.dragged() {
                self.project_tree_width = explorer_panel_width(available.width(), self.project_tree_width + resp.drag_delta().x);
            }
        }

        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(trace_rect), |ui| {
            egui::Frame::new().fill(palette.surface)
                .stroke(Stroke::new(1.0, palette.border))
                .corner_radius(egui::CornerRadius::same(8))
                .show(ui, |ui| {
                ui.set_min_size(trace_rect.size());
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("FRAME TRACE").size(14.0).strong().color(palette.text));
                    ui.separator();
                    self.trace_columns.menu(ui);
                    ui.add_space(10.0);
                    ui.small("Mode");
                    egui::ComboBox::from_id_salt("trace_mode_combo")
                        .selected_text(if self.trace_overwrite { "Overwrite" } else { "Chronological" })
                        .width(108.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.trace_overwrite, true, "Overwrite")
                                .on_hover_text("One fixed row per Protocol + Channel + Frame ID");
                            ui.selectable_value(&mut self.trace_overwrite, false, "Chronological")
                                .on_hover_text("Every frame reception is a new row");
                        });
                    ui.add_space(10.0);
                    ui.small("View");
                    if ui.selectable_label(self.trace_follow_replay, "Follow").on_hover_text("Keep Trace on the newest replayed frame").clicked() {
                        self.trace_follow_replay = !self.trace_follow_replay;
                    }
                    let freeze_label = if self.trace_freeze_view { "Unfreeze" } else { "Freeze" };
                    if ui.selectable_label(self.trace_freeze_view, freeze_label).on_hover_text("Freeze Trace display while replay continues").clicked() {
                        self.trace_freeze_view = !self.trace_freeze_view;
                        if !self.trace_freeze_view { self.trace_display_position_s = self.replay_position_s; }
                    }
                    ui.add_space(10.0);
                    ui.small("Filter");
                    ui.checkbox(&mut self.trace_only_changing, "Hide static")
                        .on_hover_text("Hide frames whose payload never changes anywhere in the loaded capture");
                    if ui.add_sized([150.0, 20.0], egui::TextEdit::singleline(&mut self.filter.text).hint_text("ID / channel / data")).changed() {
                        // v0.21: typing stays immediate; expensive full-log filtering waits
                        // until the user pauses briefly.
                        self.trace_filter_pending=true;
                        self.trace_filter_changed_at=Some(Instant::now());
                    }
                    if ui.small_button("X").on_hover_text("Clear Trace filter").clicked() {
                        self.filter = Filter::default();
                        self.trace_filter_pending=false;
                        self.trace_filter_changed_at=None;
                        self.cached_filter_signature.clear();
                    }
                    ui.separator();
                    let trace_state = if self.trace_freeze_view { "FROZEN" } else if self.measurement_running { "LIVE" } else { "READY" };
                    ui.small(trace_state);
                });
                if self.trace_filter_pending {
                    let ready=self.trace_filter_changed_at.map(|t|t.elapsed()>=Duration::from_millis(300)).unwrap_or(true);
                    if ready {
                        self.cached_filter_signature.clear();
                        self.trace_filter_pending=false;
                        self.trace_filter_changed_at=None;
                    } else {
                        ui.ctx().request_repaint_after(Duration::from_millis(50));
                    }
                }
                let trace_query_started=Instant::now();
                self.ensure_filter_cache();
                self.ensure_scope_cache();
                if self.trace_only_changing { self.ensure_change_cache(); }
                self.perf_trace_query_ms=self.perf_trace_query_ms*0.85+trace_query_started.elapsed().as_secs_f32()*1000.0*0.15;
                let names = self.channel_name_map();
                // scoped indices preserve capture-time order, so partition_point avoids
                // allocating and rescanning a replay prefix on every refresh.
                let visible_count = if self.measurement_running || self.trace_freeze_view {
                    self.cached_scoped_indices.partition_point(|i| d.timeline_s[*i] <= self.trace_display_position_s)
                } else { self.cached_scoped_indices.len() };
                if self.trace_overwrite {
                    // Overwrite mode intentionally keeps one row per source.
                    let trace_rows = self.trace_rows_cached(&d, visible_count, true);
                    if self.trace_only_changing {
                        let filtered: Vec<TraceRow> = trace_rows.iter()
                            .filter(|r| self.cached_changing_trace_sources.contains(&trace_source_key(&d.frames[r.idx])))
                            .cloned().collect();
                        self.canoe_trace_table(ui, &d, &filtered, &names);
                    } else {
                        self.canoe_trace_table(ui, &d, trace_rows.as_slice(), &names);
                    }
                } else {
                    // Runtime fast path: sequential Trace is virtual. Never construct
                    // millions of TraceRow objects merely because the capture is loaded.
                    self.canoe_trace_table_virtual(ui, &d, visible_count, &names);
                }
            });
        });

        if has_docked {
            let split_rect = match self.inspector_dock {
                InspectorDock::Bottom => egui::Rect::from_min_max(
                    egui::pos2(work_rect.left(), trace_rect.bottom()),
                    egui::pos2(work_rect.right(), trace_rect.bottom() + splitter),
                ),
                InspectorDock::Right => egui::Rect::from_min_max(
                    egui::pos2(trace_rect.right(), work_rect.top()),
                    egui::pos2(trace_rect.right() + splitter, work_rect.bottom()),
                ),
            };
            let cursor = match self.inspector_dock { InspectorDock::Bottom => egui::CursorIcon::ResizeVertical, InspectorDock::Right => egui::CursorIcon::ResizeHorizontal };
            let resp = ui.allocate_rect(split_rect, Sense::drag()).on_hover_cursor(cursor);
            if resp.dragged() {
                match self.inspector_dock {
                    InspectorDock::Bottom => self.inspector_height = docked_pane_extent(work_rect.height(), self.inspector_height - resp.drag_delta().y, 220.0, 160.0),
                    InspectorDock::Right => self.inspector_width = docked_pane_extent(work_rect.width(), self.inspector_width - resp.drag_delta().x, 300.0, 240.0),
                }
            }

            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(inspector_rect), |ui| {
                egui::Frame::new().fill(palette.surface)
                .stroke(Stroke::new(1.0, palette.border))
                .corner_radius(egui::CornerRadius::same(8))
                .show(ui, |ui| {
                    ui.set_min_size(inspector_rect.size());
                    ui.horizontal_wrapped(|ui| {
                        ui.selectable_value(&mut self.inspector_pane, InspectorPane::Details, "Frame Inspector");
                        if ui.button("Open Waveforms").on_hover_text("Open the full waveform viewer without replacing the frame details").clicked() {
                            self.graphics_viewer_open = true;
                        }
                        ui.separator();
                        if ui.selectable_label(self.inspector_dock == InspectorDock::Bottom, "Dock Bottom").clicked() { self.inspector_dock = InspectorDock::Bottom; }
                        if ui.selectable_label(self.inspector_dock == InspectorDock::Right, "Dock Right").clicked() { self.inspector_dock = InspectorDock::Right; }
                        if ui.button("Detach Window").on_hover_text("Detach as a separate operating-system window, like CANoe. Move it to another monitor or dock it back.").clicked() {
                            let pane = self.inspector_pane;
                            if pane != InspectorPane::None { self.floating_panes.insert(pane); }
                            self.inspector_pane = InspectorPane::None;
                        }
                        if ui.button("Close").clicked() { self.inspector_pane = InspectorPane::None; }
                    });
                    ui.separator();
                    match self.inspector_pane {
                        InspectorPane::Details => self.canoe_details_panel(ui, &d),
                        InspectorPane::Graphics => self.canoe_graphics_panel(ui, &d),
                        InspectorPane::Events => self.canoe_events_panel(ui, &d),
                        InspectorPane::None => {}
                    }
                });
            });
        }

        // v0.28: true native OS viewports, CANoe-style.
        // A detached pane is a separate operating-system window (taskbar/window switcher/second monitor),
        // not an egui::Window painted inside the main LogLens window.
        for (pane, title, default_size) in [
            (InspectorPane::Graphics, "LogLens - Graphics", Vec2::new(1100.0, 720.0)),
        ] {
            if self.floating_panes.contains(&pane) {
                let viewport_id=egui::ViewportId::from_hash_of(("native_inspector",pane as u8));
                let builder=egui::ViewportBuilder::default()
                    .with_title(title)
                    .with_inner_size(default_size)
                    .with_min_inner_size(Vec2::new(420.0,220.0))
                    .with_resizable(true);

                let mut dock_request:Option<InspectorDock>=None;
                let mut close_request=false;
                ui.ctx().show_viewport_immediate(viewport_id,builder,|ctx,_class|{
                    // On platforms/backends that cannot create a native child viewport, egui may
                    // fall back to embedded rendering. Normal desktop eframe uses native viewports.
                    egui::CentralPanel::default().show(ctx,|ui|{
                        ui.horizontal(|ui|{
                            ui.strong(match pane {
                                InspectorPane::Details=>"Inspector",
                                InspectorPane::Graphics=>"Graphics",
                                InspectorPane::Events=>"Events / Triggers",
                                InspectorPane::None=>"Window",
                            });
                            ui.separator();
                            if ui.button("Dock Bottom").clicked(){dock_request=Some(InspectorDock::Bottom);}
                            if ui.button("Dock Right").clicked(){dock_request=Some(InspectorDock::Right);}
                        });
                        ui.separator();
                        match pane {
                            InspectorPane::Details=>self.canoe_details_panel(ui,&d),
                            InspectorPane::Graphics=>self.canoe_graphics_panel(ui,&d),
                            InspectorPane::Events=>self.canoe_events_panel(ui,&d),
                            InspectorPane::None=>{}
                        }
                    });
                    if ctx.input(|i|i.viewport().close_requested()){close_request=true;}
                });
                if let Some(dock)=dock_request {
                    self.inspector_dock=dock;
                    self.inspector_pane=pane;
                    self.floating_panes.remove(&pane);
                    ui.ctx().send_viewport_cmd_to(viewport_id,egui::ViewportCommand::Close);
                } else if close_request {
                    self.floating_panes.remove(&pane);
                }
            }
        }

        ui.allocate_rect(available, Sense::hover());
    }

    fn canoe_details_panel(&mut self, ui: &mut egui::Ui, d: &Dataset) {
        let selected = self.selected_trace_row.and_then(|i| d.frames.get(i));
        if let Some(f) = selected {
            ui.columns(2, |cols| {
                cols[0].strong("FRAME");
                cols[0].label(if d.source_time{"Timeline: TECMP source time"}else{"Timeline: Wireshark capture time"});
                cols[0].label(format!("Time: {}", format_timestamp_ns(f.timestamp_ns))).on_hover_text(format!("Raw: {} ns | Relative: {:.9} s", f.timestamp_ns, f.relative_time_s));
                cols[0].label(format!("Channel: {}", self.channel_label(f.channel_id)));
                cols[0].label(format!("Protocol: {}", f.bus_type));
                cols[0].label(format!("ID / Key: {}", frame_key_display(&f.frame_key)));
                cols[0].label(format!("Direction: {}", f.direction));
                if f.bus_type==BusType::Lin {
                    if let Some(meta)=&f.tecmp {
                        cols[0].label(format!("LIN Flags: 0x{:04X} ({})",meta.data_flags,crate::protocols::lin_flags_text(meta.message_type,meta.data_flags)));
                    }
                }
                if let Some(meta)=&f.tecmp {
                    cols[0].label(format!("TECMP source time: {} ns",meta.source_timestamp_ns.unwrap_or(f.timestamp_ns)));
                    if let Some(ns)=meta.capture_timestamp_ns {cols[0].label(format!("Capture time: {}",format_timestamp_ns(ns)));}
                    if meta.timeline_projected{cols[0].label("Graph time aligned from capture clock; original source clock differs.");}
                    cols[0].label(format!("Clock: {}{}",if meta.timestamp_flags&2!=0{"not synchronized"}else{"synchronized/master"},if meta.timestamp_flags&1!=0{"; recalculated"}else{""}));
                    cols[0].label(format!("Device flags: 0x{:04X} ({}); Data flags: 0x{:04X}",meta.device_flags,meta.device_flags_text(),meta.data_flags));
                }
                if let Some(crc)=f.network.can_crc {cols[0].label(format!("CAN CRC: 0x{crc:X}"));}
                // Keep the inspector readable for long Ethernet and analog payloads.
                // Full payload remains available through Trace expansion and byte inspector.
                let raw_preview = f.raw_payload.iter().take(24)
                    .map(|b|format!("{b:02X}")).collect::<Vec<_>>().join(" ");
                cols[0].label(format!("Raw entry ({} B): {}{}",
                    f.raw_payload.len(), raw_preview,
                    if f.raw_payload.len() > 24 { " …" } else { "" }));
                cols[0].label(format!("Length: {}", f.payload.len()));
                cols[1].strong("BYTE INSPECTOR");
                egui::ScrollArea::vertical().max_height(190.0).show(&mut cols[1], |ui| {
                    egui::Grid::new("canoe_byte_inspector_v07").num_columns(8).striped(true).show(ui, |ui| {
                        for (i, b) in f.payload.iter().enumerate() {
                            let selected = self.graphics_bytes.contains(&i);
                            if ui.selectable_label(selected, format!("B{i}: {b:02X}")).clicked() {
                                if selected { self.graphics_bytes.remove(&i); } else { self.graphics_bytes.insert(i); }
                                self.graphics_sources.insert(graphics_source_key(&f.bus_type.to_string(), f.channel_id, &f.frame_key));
                            }
                            if (i + 1) % 8 == 0 { ui.end_row(); }
                        }
                    });
                });
            });
        } else {
            ui.centered_and_justified(|ui| { ui.label("Select a Trace row to inspect frame details."); });
        }
    }

    fn replay_bounds(&self, _d: &Dataset) -> (f64, f64, Option<String>) {
        let duration = self.cached_stats.as_ref().map(|s| s.duration_s).unwrap_or(0.0).max(0.0);
        (0.0, duration, None)
    }

    fn canoe_events_panel(&mut self, ui: &mut egui::Ui, d: &Dataset) {
        let markers = control_triggers(&d.frames, 0.0);
        ui.horizontal(|ui| {
            ui.strong("CONTROL MESSAGE MARKERS");
            ui.separator();
            ui.label(format!("{} marker(s)", markers.len()));
            ui.weak("One Control Message = one timestamp marker");
        });
        ui.separator();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("cm_marker_grid").striped(true).num_columns(5).show(ui, |ui| {
                ui.strong("#"); ui.strong("Time"); ui.strong("Message ID"); ui.strong("TECMP CM ID"); ui.strong("Occurrence"); ui.end_row();
                let capture_start_ns=d.frames.first().map(|f|f.timestamp_ns).unwrap_or(0);
                for (i, marker) in markers.iter().enumerate() {
                    if ui.selectable_label(false, format!("{}",i+1)).clicked() {
                        self.replay_position_s=marker.time_s;
                        self.cursor_a_s=marker.time_s;
                        self.replay_anchor=if self.measurement_running && !self.measurement_paused {Some(Instant::now())} else {None};
                    }
                    ui.monospace(format_timestamp_ns(timestamp_at_relative(capture_start_ns,marker.time_s)));
                    ui.monospace(format!("0x{:04X}",marker.control_message_id));
                    ui.monospace(format!("0x{:04X}",marker.tecmp_cm_id));
                    ui.monospace(format!("{}",marker.occurrence));
                    ui.end_row();
                }
            });
        });
    }

    fn graphics_frame_is_constant(&mut self,d:&Dataset,bus:&str,frame_key:&str,channel:u32,dlc:usize)->bool {
        if bus=="ANALOG" || dlc==0 { return false; }
        (0..dlc).all(|b| self.graphics_byte_class(d,bus,frame_key,channel,b).0=="STATIC")
    }

    fn graphics_byte_is_repetitive(&mut self,d:&Dataset,bus:&str,frame_key:&str,channel:u32,byte:usize)->bool {
        let (wa,wb)=self.graphics_class_window.unwrap_or((0.0,f64::INFINITY));
        let ck=format!("REP|{}|{}|{}|{}|{:.9}|{:.9}",bus,channel,frame_key,byte,wa,wb);
        if let Some(v)=self.graphics_repetitive_cache.get(&ck){return *v;}
        let mut values=Vec::<u8>::new();
        if let Some(ix)=d.index.by_key.get(frame_key){
            let lo=if wa.is_finite(){ix.partition_point(|i|d.frames[*i].relative_time_s<wa)}else{0};
            let hi=if wb.is_finite(){ix.partition_point(|i|d.frames[*i].relative_time_s<=wb)}else{ix.len()};
            for &i in &ix[lo..hi] {
                let f=&d.frames[i];
                if f.channel_id==channel && f.bus_type.to_string()==bus && byte<f.payload.len(){values.push(f.payload[byte]);}
            }
        }
        // Shared tolerant pattern detector used by both classification and Hide Repetitive.
        // Period 1 stays CONSTANT; generic periods up to 512 frames use value/delta periodicity with >=4 cycles.
        let repetitive=detect_repeating_pattern(&values,4,512).is_some();
        self.graphics_repetitive_cache.insert(ck,repetitive);
        repetitive
    }

    fn graphics_frame_is_hidden_by_pattern_filters(&mut self,d:&Dataset,bus:&str,frame_key:&str,channel:u32,dlc:usize)->bool {
        if bus=="ANALOG" || dlc==0{return false;}
        (0..dlc).all(|b| {
            let constant=self.graphics_byte_class(d,bus,frame_key,channel,b).0=="STATIC";
            (self.graphics_hide_constant && constant) ||
            (self.graphics_hide_repetitive && !constant && self.graphics_byte_is_repetitive(d,bus,frame_key,channel,b))
        })
    }

    fn digital_view_waveform(&mut self,d:&Dataset,bus:&str,key:&str,channel:u32,byte:Option<usize>,bit:Option<usize>)->Arc<Vec<[f64;2]>> {
        let cache_key=format!("VIEW|{bus}|{channel}|{key}|{byte:?}|{bit:?}|{}|{}",self.graphics_view_start_s.to_bits(),self.graphics_view_end_s.to_bits());
        if let Some(points)=self.cached_waveforms.get(&cache_key){return Arc::clone(points);}
        let points=Arc::new(digital_view_points(d,bus,key,channel,byte,bit,self.graphics_view_start_s,self.graphics_view_end_s));
        self.trim_graphics_caches_for_resource_limits();
        self.cached_waveforms.insert(cache_key,Arc::clone(&points));points
    }

    fn cached_signal_indices(&mut self,d:&Dataset,signal:&SignalDefinition,scope:&BTreeSet<String>)->Arc<Vec<usize>> {
        let key=serde_json::to_string(&(signal,scope)).expect("serializable signal");
        if let Some(indices)=self.cached_decoded_signal_indices.get(&key){return Arc::clone(indices);}
        let indices=Arc::new(decoded_signal_indices(d,signal,scope));
        self.trim_graphics_caches_for_resource_limits();
        self.cached_decoded_signal_indices.insert(key,Arc::clone(&indices));indices
    }

    fn cached_decoded_signal_waveform(&mut self,d:&Dataset,signal:&SignalDefinition,scope:&BTreeSet<String>)->Arc<Vec<[f64;2]>> {
        let key=serde_json::to_string(&(signal,scope,self.graphics_view_start_s.to_bits(),self.graphics_view_end_s.to_bits())).expect("serializable signal view");
        if let Some(points)=self.cached_decoded_signal_waveforms.get(&key){return Arc::clone(points);}
        let indices=self.cached_signal_indices(d,signal,scope);
        let points=Arc::new(decoded_signal_view_points(d,&indices,signal,self.graphics_view_start_s,self.graphics_view_end_s));
        self.trim_graphics_caches_for_resource_limits();
        self.cached_decoded_signal_waveforms.insert(key,Arc::clone(&points));points
    }

    fn cached_decoded_signal_range(&mut self,d:&Dataset,signal:&SignalDefinition,scope:&BTreeSet<String>)->Option<(f64,f64)> {
        let key=format!("RANGE|{}",serde_json::to_string(&(signal,scope)).expect("serializable signal"));
        if let Some(points)=self.cached_decoded_signal_waveforms.get(&key){return points.first().map(|p|(p[0],p[1]));}
        let indices=self.cached_signal_indices(d,signal,scope);
        let mut low=f64::INFINITY;let mut high=f64::NEG_INFINITY;
        for i in indices.iter(){if let Some(value)=decoded_frame_value(&d.frames[*i],signal){low=low.min(value);high=high.max(value);}}
        let points=if low.is_finite() && high.is_finite(){
            let pad=if (high-low).abs()<1e-12{(low.abs()*0.05).max(0.5)}else{(high-low)*0.05};
            vec![[low-pad,high+pad]]
        }else{Vec::new()};
        let range=points.first().map(|p|(p[0],p[1]));
        self.trim_graphics_caches_for_resource_limits();self.cached_decoded_signal_waveforms.insert(key,Arc::new(points));range
    }

    fn graphics_byte_selector_row(
        &mut self, ui:&mut egui::Ui, d:&Dataset, bus:&str, source_key:&str,
        frame_key:&str, channel:u32, byte:usize, range_filter_active:bool,
        changed_bytes:Option<&BTreeSet<usize>>
    ) {
        if range_filter_active && !changed_bytes.map(|bytes|bytes.contains(&byte)).unwrap_or(false) { return; }
        let mut on=self.graphics_source_byte_selected(source_key,byte);
        let (class,confidence)=self.graphics_byte_class(d,bus,frame_key,channel,byte);
        if !self.graphics_class_visible(class){return;}
        let friendly=match class{"ACTIVE"=>"Signal","STATIC"=>"Constant","COUNTER4"|"COUNTER8"=>"Counter","CRC_CANDIDATE"=>"CRC / Checksum","PATTERN"=>"Pattern",_=>"Review"};
        let label=if class=="COUNTER4"||class=="COUNTER8"||class=="CRC_CANDIDATE"{
            format!("B{}   {} {:.0}%",byte,friendly,confidence*100.0)
        }else{format!("B{}   {}",byte,friendly)};
        egui::CollapsingHeader::new(label)
            .id_salt(format!("graphics_byte_{}_{}",source_key,byte))
            .default_open(false)
            .show(ui,|ui|{
                if ui.checkbox(&mut on,"Raw byte").changed(){
                    self.set_graphics_source_byte(source_key,byte,on);
                }
                ui.horizontal_wrapped(|ui|{
                    for bit in 0..8usize {
                        let bit_key=format!("{}|{}|{}",source_key,byte,bit);
                        let mut bit_on=self.graphics_bits.contains(&bit_key);
                        if ui.checkbox(&mut bit_on,format!("b{}",bit)).changed(){
                            if bit_on {
                                self.graphics_bits.insert(bit_key.clone());
                                self.graphics_sources.insert(source_key.to_string());
                                let lane=format!("BIT|{}",bit_key);
                                if !self.graphics_lane_order.contains(&lane){self.graphics_lane_order.push(lane);}
                            } else {
                                self.graphics_bits.remove(&bit_key);
                                self.graphics_lane_order.retain(|x|x!=&format!("BIT|{}",bit_key));
                            }
                        }
                    }
                });
            });
    }

    fn analog_waveform(&mut self,d:&Dataset,frame_key:&str,channel:u32)->(Arc<Vec<[f64;2]>>,String,usize,u64) {
        let cache_key=format!("ANALOG|{}|{}",channel,frame_key);
        if let Some((wave,unit,samples,period))=self.cached_analog_waveforms.get(&cache_key) {
            return (Arc::clone(wave),unit.clone(),*samples,*period);
        }
        let limit=resources::limits().analog_point_limit.max(10_000);
        let mut unit=String::new(); let mut packet_samples=0usize; let mut period_ns=0u64;
        let mut total_samples=0usize; let mut skipped_packets=0usize;
        for f in d.frames.iter().filter(|f| f.bus_type==BusType::Analog && f.channel_id==channel && f.frame_key==frame_key) {
            let Some(meta)=f.analog.as_ref() else {continue;};
            if meta.sample_period_ns!=0 {
                unit=meta.unit.clone(); packet_samples=meta.sample_count; period_ns=meta.sample_period_ns;
                total_samples=total_samples.saturating_add(f.payload.len()/2);
            } else { skipped_packets+=1; }
        }
        if skipped_packets>0 {
            self.log_tool("WARN",format!("Analog {}: {} packet(s) skipped because the Data Flags carry a reserved/unknown sample time",frame_key,skipped_packets));
        }
        // Keep at most `limit` points. Each bucket keeps its minimum AND maximum sample (in time
        // order) instead of every Nth sample: 1-of-N decimation aliases and can skip the very
        // samples where the value changes, which draws a flat line through real activity.
        let stride=if total_samples<=limit {1} else {((total_samples.saturating_mul(2).saturating_add(limit-1))/limit).max(2)};
        let mut out=Vec::new();
        out.try_reserve(total_samples.min(limit)).ok();
        let mut global_sample=0usize;
        let mut bucket=usize::MAX;
        let mut bmin=[0.0f64;2]; let mut bmax=[0.0f64;2];
        let flush=|out:&mut Vec<[f64;2]>,bmin:[f64;2],bmax:[f64;2]|{
            if bmin==bmax { out.push(bmin); }
            else if bmin[0]<=bmax[0] { out.push(bmin); out.push(bmax); }
            else { out.push(bmax); out.push(bmin); }
        };
        for f in d.frames.iter().filter(|f| f.bus_type==BusType::Analog && f.channel_id==channel && f.frame_key==frame_key) {
            let Some(meta)=f.analog.as_ref() else {continue;};
            if meta.sample_period_ns==0 {continue;}
            for (n,b) in f.payload.chunks_exact(2).enumerate() {
                let raw=i16::from_be_bytes([b[0],b[1]]) as f64;
                let t=f.relative_time_s+(n as f64)*(meta.sample_period_ns as f64)*1e-9;
                let pt=[t,raw*meta.factor];
                if stride==1 { out.push(pt); }
                else {
                    let bi=global_sample/stride;
                    if bi!=bucket {
                        if bucket!=usize::MAX { flush(&mut out,bmin,bmax); }
                        bucket=bi; bmin=pt; bmax=pt;
                    } else {
                        if pt[1]<bmin[1] { bmin=pt; }
                        if pt[1]>bmax[1] { bmax=pt; }
                    }
                }
                global_sample=global_sample.saturating_add(1);
            }
        }
        if stride>1 && bucket!=usize::MAX { flush(&mut out,bmin,bmax); }
        if stride>1 {
            self.log_tool("WARN",format!("Analog cache resource protection: {} samples reduced to {} display/measurement points (1 of every {}).",total_samples,out.len(),stride));
        }
        // Sample blocks can overlap; binary-searching the viewport requires time order.
        out.sort_by(|a,b|a[0].total_cmp(&b[0]));
        let wave=Arc::new(out);
        self.trim_graphics_caches_for_resource_limits();
        self.cached_analog_waveforms.insert(cache_key,(Arc::clone(&wave),unit.clone(),packet_samples,period_ns));
        (wave,unit,packet_samples,period_ns)
    }

    // Zoom-aware Analog display points. The old display cache was a single 6000-point
    // downsample of the whole capture, so zooming in showed only those few points and real
    // value changes inside the window were lost (flat / straight lines). Here the visible
    // window of the decoded samples is min/max-reduced to a pixel-appropriate point count,
    // and cached per lane until the view changes.
    fn analog_view_points(&mut self,d:&Dataset,frame_key:&str,channel:u32,view_start:f64,view_end:f64)->Arc<Vec<[f64;2]>> {
        let key=format!("ANALOG_VIEW|{}|{}",channel,frame_key);
        let (sb,eb)=(view_start.to_bits(),view_end.to_bits());
        if let Some((cs,ce,pts))=self.cached_analog_view.get(&key) {
            if *cs==sb && *ce==eb { return Arc::clone(pts); }
        }
        let pts=Arc::new(original_analog_view_points(d,frame_key,channel,view_start,view_end));
        self.cached_analog_view.insert(key,(sb,eb,Arc::clone(&pts)));
        pts
    }

    // v0.53.7: keep the expensive Analog display downsample out of the repaint path.
    // The decoded Analog cache is resource-bounded; very large channels are uniformly decimated before caching.
    fn cached_analog_display_waveform(&mut self,d:&Dataset,frame_key:&str,channel:u32)->(Arc<Vec<[f64;2]>>,String,usize,u64) {
        let display_key=format!("ANALOG_DISPLAY|{}|{}",channel,frame_key);
        let (raw,unit,samples,period)=self.analog_waveform(d,frame_key,channel);
        if let Some(w)=self.cached_waveforms.get(&display_key) {
            return (Arc::clone(w),unit,samples,period);
        }
        let display=Arc::new(downsample_minmax(raw.as_slice(),6_000));
        self.trim_graphics_caches_for_resource_limits();
        self.cached_waveforms.insert(display_key,Arc::clone(&display));
        (display,unit,samples,period)
    }

    fn canoe_graphics_panel(&mut self, ui: &mut egui::Ui, d: &Dataset) {
        ui.heading("Graphics");
        ui.add_space(6.0);
        ui.label("Waveform rendering and cursor measurement are available in the dedicated viewer.");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_sized([180.0,34.0], egui::Button::new("Open Waveform Viewer")).clicked() {
                self.graphics_viewer_open=true;
            }

        });
        ui.add_space(10.0);
        egui::Grid::new("graphics_launcher_summary").num_columns(2).striped(true).show(ui,|ui| {
            ui.label("Raw sources"); ui.monospace(self.graphics_sources.len().to_string()); ui.end_row();
            ui.label("Decoded signals"); ui.monospace(self.graphics_signals.len().to_string()); ui.end_row();
            ui.label("Bit traces"); ui.monospace(self.graphics_bits.len().to_string()); ui.end_row();
            ui.label("Analysis range");
            if let Some((a,b))=self.graphics_class_window { ui.monospace(format!("{:.3} – {:.3} s",a,b)); }
            else { ui.monospace("Full capture"); }
            ui.end_row();
        });

        if self.graphics_viewer_open {
            // v0.51.4: Waveform Viewer is a true secondary OS viewport, not an egui::Window
            // painted inside the main LogLens viewport. It shares this App instance, dataset,
            // decoded-waveform caches, cursor state and analysis range without reloading the log.
            let viewport_id=egui::ViewportId::from_hash_of("loglens_native_waveform_viewer");
            let builder=egui::ViewportBuilder::default()
                .with_title("LogLens - Waveform Viewer")
                .with_inner_size(Vec2::new(1380.0,820.0))
                .with_min_inner_size(Vec2::new(900.0,520.0))
                .with_resizable(true);
            let mut close_requested=false;
            ui.ctx().show_viewport_immediate(viewport_id,builder,|ctx,_class|{
                egui::CentralPanel::default().show(ctx,|ui|{
                    self.canoe_graphics_viewer_content(ui,d);
                });
                if ctx.input(|i|i.viewport().close_requested()) { close_requested=true; }
            });
            if close_requested {
                self.graphics_viewer_open=false;
            }
        }
    }

    fn enforce_analog_graph_mode(&mut self,project_channels:&BTreeSet<String>) {
        if self.graphics_sources.iter().any(|k|parse_graphics_source_key(k)
            .is_some_and(|(bus,ch,_)|bus=="ANALOG" && project_channels.contains(&project_channel_key(&bus,ch)))) {
            self.graphics_stacked=true;
        }
    }

    fn graph_lane_header_ui(&mut self,ui:&mut egui::Ui,lane:&str,canonical:&str) {
        let label=self.graph_lane_names.get(lane).map(String::as_str).unwrap_or(canonical).to_owned();
        let (rename,close)=draw_graph_lane_header(ui,lane,&label,canonical,&mut self.graphics_drag_lane);
        if rename {self.graph_rename=Some((None,lane.to_owned(),self.graph_lane_names.get(lane).cloned().unwrap_or_default()));}
        if close {self.remove_graphics_lane(lane);}
    }

    fn graph_rename_window(&mut self,ctx:&egui::Context) {
        let Some((channel,lane,mut text))=self.graph_rename.clone() else{return;};
        let mut open=true;let mut save=false;
        egui::Window::new(if channel.is_some(){"Rename channel"}else{"Rename signal / graph"}).id(egui::Id::new("graph_rename"))
            .collapsible(false).resizable(false).open(&mut open).show(ctx,|ui| {
                ui.label("Display name (leave empty to restore the original)");
                let edit=ui.text_edit_singleline(&mut text);
                if edit.lost_focus() && ui.input(|i|i.key_pressed(egui::Key::Enter)){save=true;}
                ui.horizontal(|ui|{if ui.button("Save").clicked(){save=true;}if ui.button("Cancel").clicked(){self.graph_rename=None;}});
            });
        if save {
            let value=text.trim().to_owned();
            if let Some(ch)=channel {
                if value.is_empty(){self.graph_channel_names.remove(&ch);}else{self.graph_channel_names.insert(ch,value);}
                self.trace_view_cache.clear();
            } else if text.trim().is_empty() {self.graph_lane_names.remove(&lane);} else {self.graph_lane_names.insert(lane.clone(),value);}
            self.graph_rename=None;self.persist_workspace_if_changed(true);
        } else if !open {self.graph_rename=None;} else if self.graph_rename.is_some(){self.graph_rename=Some((channel,lane,text));}
    }

    fn reload_selected_channels(&mut self) -> bool {
        if self.background_load.is_some(){return false;}
        let Some(path)=self.path.clone() else{return false;};
        // Reload is a data retry, never a change to the channel checklist.
        self.pending_channels.clear();self.pending_channel_selection.clear();self.pending_log_path=None;
        if !self.active_preload_channels.is_empty() {
            self.pending_log_path=Some(path.clone());
            self.pending_channel_selection=self.active_preload_channels.clone();
        }
        self.load_path_with_scope(path,None,true)
    }

    fn use_tecmp_source_time(&mut self)->bool {
        if self.background_load.is_some() || self.path.is_none() || self.dataset.as_ref().is_none_or(|d|d.source_time){return false;}
        let previous=self.tecmp_source_time;
        self.tecmp_source_time=true;
        if self.reload_selected_channels(){true}else{self.tecmp_source_time=previous;false}
    }

    fn analog_timeline_notice(&mut self,ui:&mut egui::Ui,d:&Dataset) {
        if let Some(message)=d.warnings.iter().find(|w|w.starts_with("Analog capture-time timing:")) {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(ui.visuals().warn_fg_color,message);
                if ui.add_enabled(self.background_load.is_none(),egui::Button::new("Use TECMP source time"))
                    .on_hover_text("Reload the same channels using acquisition timestamps. Selected channels, graphs, names and filters are retained. The view and time cursors reset to the new timeline.")
                    .clicked(){self.use_tecmp_source_time();}
            });
        }
    }

    fn show_full_loaded_range(&mut self,d:&Dataset) {
        let end=self.cached_stats.as_ref().map(|stats|stats.duration_s).unwrap_or_else(||statistics(&d.frames).duration_s).max(0.001);
        self.graphics_view_start_s=0.0;self.graphics_view_end_s=end;
        self.time_start=0.0;self.time_end=end;self.use_manual_window=false;
        self.graphics_range_start_s=None;self.graphics_range_end_s=None;
        self.graphics_range_manual_start.clear();self.graphics_range_manual_end.clear();
        self.graphics_range_manual_dirty=false;self.graphics_range_dragging=false;
        self.graphics_range_interaction=0;self.graphics_range_last_pointer_s=None;
        self.graphics_range_cached_bounds=None;self.graphics_range_cached_bytes.clear();
        self.graphics_class_window=None;self.graphics_class_cache.clear();self.graphics_repetitive_cache.clear();
        self.graphics_source_class_ready.clear();self.cached_change_scope_signature.clear();self.multi_activity_signature.clear();
        self.cached_scope_signature.clear();self.cached_trace_scope_signature.clear();
        self.candidates_ready=false;
    }

    fn canoe_graphics_viewer_content(&mut self, ui: &mut egui::Ui, d: &Dataset) {
        if let Some((ok,message))=&self.graph_image_export.status {
            ui.add(egui::Label::new(egui::RichText::new(message).small().color(if *ok{Color32::LIGHT_GREEN}else{Color32::LIGHT_RED})).truncate()).on_hover_text(message);
        }
        if d.warnings.iter().any(|w|w.starts_with("Safe load limit reached")) {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(Color32::YELLOW,"Partial capture loaded — channel selections are preserved; Fit uses the loaded time span.");
                if ui.add_enabled(self.background_load.is_none(),egui::Button::new("Reload selected channels"))
                    .on_hover_text("Retry the original capture with the same selected channels. Graph selections, channel enable/disable states, filters, ranges and cursors are retained. A memory limit can still restrict loaded records; Change Channels is the only action that changes the capture selection.").clicked() {
                    self.reload_selected_channels();
                }
            });
        }
        self.graph_rename_window(ui.ctx());
        // v0.52.0: resolve configured channel names once per viewer repaint.
        // Previously channel_label() rebuilt this map at every label call.
        let viewer_channel_names=self.channel_name_map();
        let viewer_show_raw_ids=self.show_raw_ids;
        let viewer_channel_label=|id:u32| -> String {
            channel_display(id,viewer_channel_names.get(&id).map(String::as_str),viewer_show_raw_ids)
        };
        let capture_first_ns = d.frames.first().map(|f| f.timestamp_ns).unwrap_or(0);
        let capture_last_ns = d.frames.last().map(|f| f.timestamp_ns).unwrap_or(capture_first_ns);
        // v0.38.1: compact Graphics header. Secondary controls are in the overflow menu.
        ui.horizontal(|ui| {
            ui.strong("GRAPHICS"); ui.separator();
            if ui.small_button("Close viewer").clicked() {self.graphics_viewer_open=false;}

            if ui.add_enabled(self.can_change_channels(),egui::Button::new("Change Channels")).clicked(){self.change_capture_channels();}
            ui.menu_button("Replay",|ui| {
                if ui.add_enabled(!self.measurement_running,egui::Button::new("Run")).clicked(){self.measurement_running=true;self.measurement_paused=false;self.replay_anchor=Some(Instant::now());ui.close();}
                if ui.add_enabled(self.measurement_running,egui::Button::new(if self.measurement_paused{"Resume"}else{"Pause"})).clicked(){self.measurement_paused=!self.measurement_paused;self.replay_anchor=if self.measurement_paused{None}else{Some(Instant::now())};ui.close();}
                if ui.add_enabled(self.measurement_running,egui::Button::new("Stop")).clicked(){self.measurement_running=false;self.measurement_paused=false;self.replay_anchor=None;self.replay_position_s=0.0;ui.close();}
            });
            ui.separator();
            let full_end=self.cached_stats.as_ref().map(|x|x.duration_s).unwrap_or(1.0).max(0.001);
            if ui.small_button("Fit").clicked(){self.graphics_view_start_s=0.0;self.graphics_view_end_s=full_end;}
            if ui.small_button("-").on_hover_text("Zoom out").clicked(){let c=(self.graphics_view_start_s+self.graphics_view_end_s)*0.5;zoom_time_view(&mut self.graphics_view_start_s,&mut self.graphics_view_end_s,1.38,c,0.0,full_end);}
            if ui.small_button("+").on_hover_text("Zoom in").clicked(){let c=(self.graphics_view_start_s+self.graphics_view_end_s)*0.5;zoom_time_view(&mut self.graphics_view_start_s,&mut self.graphics_view_end_s,0.72,c,0.0,full_end);}
            ui.separator();
            ui.checkbox(&mut self.graphics_follow_replay,"Follow");
            let control_count=d.index.by_key.iter().filter(|(key,_)|key.starts_with("TECMP:CONTROL:"))
                .map(|(_,indices)|indices.len()).sum::<usize>();
            ui.checkbox(&mut self.graphics_show_event_markers,format!("CM Markers ({control_count})"))
                .on_hover_text("Show each TECMP Control Message as a vertical timestamp marker. Control messages are retained alongside selected channels.");
            let before_raw = self.show_raw_ids;
            ui.toggle_value(&mut self.show_raw_ids, "Raw IDs")
                .on_hover_text("ON: ConfiguredName (RawID). OFF: ConfiguredName only. Unconfigured IDs remain visible.");
            if before_raw != self.show_raw_ids {
                self.trace_view_cache.clear();
                self.persist_session_configuration();
            }
            if ui.add(egui::Button::new(if self.graphics_show_signal_browser{"Signals <"}else{"Signals >"}))
                .on_hover_text("Show/hide the source browser to maximize waveform space").clicked(){
                self.graphics_show_signal_browser=!self.graphics_show_signal_browser;
            }
            // Dedicated analysis selection. A/B remain measurement cursors only.
            self.graphics_range_changed_only=true;
            let select_label=if self.graphics_range_dragging { "Selecting..." } else { "Select Range" };
            if ui.add(egui::Button::new(select_label).selected(self.graphics_range_dragging))
                .on_hover_text("Enter analysis-range edit mode. Drag outside the current region to create a new range, or drag its edges/body to resize/move it. Exit occurs after the drag. A/B cursors are completely independent.")
                .clicked() {
                self.graphics_range_dragging=!self.graphics_range_dragging;
                if self.graphics_range_dragging { self.graphics_range_manual_dirty=false; }
                self.graphics_range_interaction=0;
                self.graphics_range_last_pointer_s=None;
            }

            ui.toggle_value(&mut self.graphics_hide_constant,"Hide Constant")
                .on_hover_text("Hide STATIC bytes in the active Analysis Range. If every byte of a frame is STATIC, hide the complete frame. A/B cursors do not affect this filter.");
            ui.toggle_value(&mut self.graphics_hide_repetitive,"Hide Repetitive")
                .on_hover_text("Hide bytes with a stable short-period repeating pattern in the committed Analysis Range. Requires at least 3 complete cycles; A/B cursors do not affect this filter.");
            if ui.button("Settings").on_hover_text("Graph appearance, cursors, scale and layout").clicked(){self.show_graph_settings=!self.show_graph_settings;}
            ui.menu_button("Options",|ui|{
                let selected=!self.graphics_sources.is_empty() || !self.graphics_signals.is_empty() || !self.graphics_bits.is_empty() || !self.graphics_whole_frames.is_empty();
                if ui.add_enabled(selected,egui::Button::new("Save graph image…"))
                    .on_hover_text("Export selected graphs from their data, with axes and a cursor table. Includes filtered-in graphs below the scroll area; no window controls or screen capture.").clicked() {
                    ui.close();self.show_graph_settings=false;
                    if let Some(path)=rfd::FileDialog::new().add_filter("PNG image", &["png"]).set_file_name("loglens_graph.png").save_file() {
                        self.save_graph_image(ui.ctx(),d,path);ui.ctx().request_repaint();
                    }
                }
                ui.separator();
                if ui.button("Clear graphs").clicked(){self.graphics_bytes.clear();self.graphics_sources.clear();self.graphics_signals.clear();self.graphics_bits.clear();self.graphics_whole_frames.clear();self.graphics_lane_order.clear();self.graphics_drag_lane=None;ui.close();}
                ui.menu_button("Mouse controls",|ui|{
                    ui.label("Trackpad left/right — pan time (axis locked)");
                    ui.label("Wheel — scroll stacked graphs up/down");
                    ui.label("Ctrl/Cmd + wheel — X zoom at pointer");
                    ui.label("Shift + wheel — pan time left/right");
                    ui.label("Middle drag — pan time");
                    ui.label("Double-click — fit complete log");
                    ui.label("Shift + left-drag — select Changed Data range");
                    ui.label("Drag range edge/center — resize or move Changed Data range");
                    ui.separator();
                    ui.label("A + click — place measurement Cursor A");
                    ui.label("B + click — place measurement Cursor B");
                    ui.label("Select Range + drag — create analysis region");
                    ui.label("Drag A/B line — move cursor");
                    ui.label("Left / Right - previous/next sample");
                });
            });
        });
        self.analog_timeline_notice(ui,d);
        let loaded_end=self.cached_stats.as_ref().map(|s|s.duration_s).unwrap_or_else(||d.frames.last().map(|f|f.relative_time_s).unwrap_or(0.0));
        let completeness=if d.warnings.iter().any(|w|w.starts_with("Safe load limit reached")){"Partial"}else{"Complete selected scope"};
        ui.small(format!("{completeness}: {} records · loaded 0.000–{loaded_end:.3} s · view {:.3}–{:.3} s",d.frames.len(),self.graphics_view_start_s,self.graphics_view_end_s));
        ui.horizontal(|ui|{
            ui.small(if d.source_time{"TECMP source time"}else{"Wireshark capture time"});
            ui.separator();
            if !self.graphics_per_waveform_cursors {
                ui.selectable_value(&mut self.graphics_active_cursor,'A',"A");
                ui.add_sized([86.0,22.0],egui::DragValue::new(&mut self.cursor_a_s).speed(0.001).suffix(" s"));
                ui.selectable_value(&mut self.graphics_active_cursor,'B',"B");
                ui.add_sized([86.0,22.0],egui::DragValue::new(&mut self.cursor_b_s).speed(0.001).suffix(" s"));
                ui.monospace(format!("dt {:.3} ms",(self.cursor_b_s-self.cursor_a_s).abs()*1000.0));
            } else {
                ui.strong("Per-waveform cursors");
                ui.weak("Use + Cursor on a lane, then click that waveform (max 8 per lane)");
            }
            ui.separator();
            if !self.graphics_per_waveform_cursors {
                ui.small(format!("A actual {}",format_timestamp_ns(timestamp_at_relative(capture_first_ns,self.cursor_a_s))));
                ui.small(format!("B actual {}",format_timestamp_ns(timestamp_at_relative(capture_first_ns,self.cursor_b_s))));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                if ui.button("Export...").clicked(){self.show_export_manager=true;}
            });
        });
        {
            // Keep manual fields synchronized with graph selection until the user starts typing.
            // The draft is deliberately separate from the committed range: no analysis/cache
            // invalidation occurs for individual keystrokes.
            if !self.graphics_range_manual_dirty {
                if let Some((a,b))=self.graphics_range_start_s.zip(self.graphics_range_end_s) {
                    self.graphics_range_manual_start=format!("{:.6}",a.min(b));
                    self.graphics_range_manual_end=format!("{:.6}",a.max(b));
                } else {
                    self.graphics_range_manual_start.clear();
                    self.graphics_range_manual_end.clear();
                }
            }
            ui.horizontal_wrapped(|ui|{
                ui.strong("Analysis Range");
                ui.label("Start");
                let sr=ui.add_sized([92.0,22.0],egui::TextEdit::singleline(&mut self.graphics_range_manual_start).hint_text("seconds"));
                ui.label("s  End");
                let er=ui.add_sized([92.0,22.0],egui::TextEdit::singleline(&mut self.graphics_range_manual_end).hint_text("seconds"));
                ui.label("s");
                if sr.changed() || er.changed() { self.graphics_range_manual_dirty=true; }

                let apply=ui.button("Apply").on_hover_text("Commit the typed range once. A/B measurement cursors are not changed.");
                if apply.clicked() {
                    let ps=self.graphics_range_manual_start.trim().parse::<f64>();
                    let pe=self.graphics_range_manual_end.trim().parse::<f64>();
                    if let (Ok(mut a),Ok(mut b))=(ps,pe) {
                        // The toolbar-local `full_end` lives in a different UI closure.
                        // Resolve the capture duration here so manual range validation
                        // does not depend on that closure's scope.
                        let capture_end_s = self.cached_stats.as_ref()
                            .map(|x| x.duration_s)
                            .unwrap_or_else(|| d.frames.last().map(|f| f.relative_time_s).unwrap_or(0.0))
                            .max(0.0);
                        a=a.clamp(0.0,capture_end_s); b=b.clamp(0.0,capture_end_s);
                        if a>b { std::mem::swap(&mut a,&mut b); }
                        if b-a >= 0.000_001 {
                            self.graphics_range_start_s=Some(a); self.graphics_range_end_s=Some(b);
                            self.graphics_range_manual_start=format!("{:.6}",a);
                            self.graphics_range_manual_end=format!("{:.6}",b);
                            self.graphics_range_manual_dirty=false;
                            // Only derived range-analysis state is invalidated. Raw/decoded
                            // waveform caches and A/B measurement state remain untouched.
                            self.graphics_range_cached_bounds=None; self.graphics_range_cached_bytes.clear();
                            self.graphics_class_window=None; self.graphics_class_cache.clear(); self.graphics_repetitive_cache.clear();
                            self.graphics_source_class_ready.clear(); self.cached_change_scope_signature.clear(); self.multi_activity_signature.clear();
                            self.candidates_ready=false;
                        }
                    }
                }
                if ui.small_button("Auto Activity").on_hover_text("Recalculate the useful testcase window. Ignores CONSTANT, counter, CRC/checksum and stable repetitive housekeeping bytes; keeps 1 second before/after activity.").clicked() {
                    if let Some((a,b))=auto_activity_range(&d.frames,1.0) {
                        self.graphics_range_start_s=Some(a); self.graphics_range_end_s=Some(b);
                        self.graphics_range_manual_start=format!("{a:.6}"); self.graphics_range_manual_end=format!("{b:.6}");
                        self.graphics_range_manual_dirty=false;
                        self.graphics_view_start_s=a; self.graphics_view_end_s=b.max(a+0.001);
                        self.graphics_range_cached_bounds=None; self.graphics_range_cached_bytes.clear();
                        self.graphics_class_window=None; self.graphics_class_cache.clear(); self.graphics_repetitive_cache.clear();
                        self.graphics_source_class_ready.clear(); self.cached_change_scope_signature.clear(); self.multi_activity_signature.clear();
                        self.candidates_ready=false;
                    }
                }
                if ui.small_button("Full Range").on_hover_text("Show and analyze the entire loaded time span. Clear the time restriction while retaining channel, frame and graph selections.").clicked() {
                    self.show_full_loaded_range(d);
                }
                if let Some((a,b))=self.graphics_range_start_s.zip(self.graphics_range_end_s) {
                    let rs=a.min(b); let re=a.max(b);
                    ui.monospace(format!("Duration {:.3} ms",(re-rs)*1000.0));
                } else { ui.label("Full capture"); }
            });
            ui.small("Type exact seconds and press Apply, or press Select Range before editing the shaded graph region. Normal graph dragging never edits Analysis Range; A/B remain measurement-only.");
        }
        ui.separator();
        ui.horizontal(|ui|{
            if ui.button("Show all Analog").on_hover_text("Add every detected TECMP Analog channel to Graphics").clicked(){
                let analog_sources=self.cached_graphics_sources.iter()
                    .filter(|(_,bus,_,_,_)|bus=="ANALOG")
                    .map(|(k,_,_,_,_)|k.clone()).collect::<Vec<_>>();
                for source_key in analog_sources {
                    self.graphics_sources.insert(source_key.clone());
                    self.selected_frame_bytes.insert(source_key.clone(),BTreeSet::from([0usize]));
                    let lane=format!("SRC|{}|0",source_key);
                    if !self.graphics_lane_order.contains(&lane){self.graphics_lane_order.push(lane);}
                }
                self.graphics_stacked=true; // Analog uses its engineering-value Y scale.
            }
            let n=self.cached_graphics_sources.iter().filter(|(_,bus,_,_,_)|bus=="ANALOG").count();
            ui.small(format!("Detected Analog channels: {}",n));
        });

        self.ensure_filter_cache();
        self.ensure_scope_cache();
        // Dedicated selection is the classification boundary. A/B are measurement-only.
        // Changing this range invalidates only derived classification/discovery caches.
        let live_range=self.graphics_range_start_s.zip(self.graphics_range_end_s)
            .map(|(a,b)|(a.min(b),a.max(b)))
            .filter(|(a,b)|(b-a).abs()>1e-6);
        // Commit exactly once after create/resize/move finishes. During drag only the
        // shaded rectangle changes; derived analysis caches remain untouched.
        if self.graphics_range_interaction==0 && !self.graphics_range_dragging
            && live_range != self.graphics_class_window {
            self.graphics_class_window=live_range;
            self.graphics_class_cache.clear();
            self.graphics_repetitive_cache.clear();
            self.graphics_source_class_ready.clear();
            self.cached_change_scope_signature.clear();
            self.multi_activity_signature.clear();
            self.graphics_range_cached_bounds=None;
            self.graphics_range_cached_bytes.clear();
            self.candidates_ready=false;
        }
        if self.graphics_only_changing { self.ensure_change_cache(); }
        let available_signals = self.signals.clone();
        // Analyze only after the persistent window is released. Cache the result
        // for the exact [start,end] so repainting Graphics does not rescan the log.
        let range_selection_complete = self.graphics_range_interaction==0
            && self.graphics_class_window.is_some();
        if self.graphics_range_changed_only && range_selection_complete {
            if let Some(bounds)=self.graphics_class_window {
                let stale=self.graphics_range_cached_bounds
                    .map(|old|(old.0-bounds.0).abs()>1e-9 || (old.1-bounds.1).abs()>1e-9)
                    .unwrap_or(true);
                if stale {
                    self.graphics_range_cached_bytes=range_changed_source_bytes(&d.frames,bounds.0,bounds.1);
                    self.graphics_range_cached_bounds=Some(bounds);
                }
            }
        }
        let range_change_bytes=self.graphics_range_cached_bytes.clone();
        // Normal Graphics sources/lanes remain stable. Changed Data is presented
        // separately for the selected analysis window.
        let range_filter_active=false;
        // Project Tree bus/channel selection is the global source scope for both Trace and Graphics.
        // The set is explicit: empty means no network sources are enabled. Graphics selections
        // are retained internally while hidden so they return when the source is re-enabled.
        let project_channels = self.project_channels.clone();
        let frame_id_catalog: BTreeSet<String> = self.cached_graphics_sources.iter()
            .filter(|(_,bus,ch,_,_)| project_channels.contains(&project_channel_key(bus,*ch)))
            .map(|(_,_,_,id,_)| id.clone()).collect();
        let explicit_ids = self.graphics_frame_id_filter.selected.is_some();
        let mut available_sources = self.cached_graphics_sources.iter()
            .filter(|(k,bus,ch,_,_)|
                project_channels.contains(&project_channel_key(bus, *ch))
                && (explicit_ids || bus=="ANALOG" || !self.graphics_only_changing || self.cached_changing_graphics_sources.contains(k))
                && (!range_filter_active || range_change_bytes.contains_key(k))
            )
            .cloned().collect::<Vec<_>>();
        // Hide a complete source/frame only when every payload byte is STATIC in the
        // committed Analysis Range. This is display filtering only; cached raw data stays intact.
        if !explicit_ids && (self.graphics_hide_constant || self.graphics_hide_repetitive) {
            available_sources.retain(|(_,bus,ch,frame_key,dlc)|
                bus=="ANALOG" || !self.graphics_frame_is_hidden_by_pattern_filters(d,bus,frame_key,*ch,*dlc));
        }
        // A decoded graph may remain visible even when its raw message is static.
        // Keep the actively selected graph's message reachable in the browser.
        if let Some(key)=&self.graphics_selected_source {
            if !available_sources.iter().any(|(source,_,_,_,_)|source==key) {
                if let Some(source)=self.cached_graphics_sources.iter().find(|(source,bus,ch,_,_)|
                    source==key && project_channels.contains(&project_channel_key(bus,*ch))) {
                    available_sources.push(source.clone());
                }
            }
        }
        // Graphics markers are individual TECMP Control Messages, not paired Event windows.
        // Keep every occurrence, including repeated Control Message IDs.
        let event_marks = if self.graphics_show_event_markers {
            control_triggers(&d.frames, 0.0)
        } else { Vec::new() };

        // Signals list is width-constrained only. Do NOT reserve ui.available_height() here:
        // doing so makes a detached egui::Window inherit that height as content minimum and
        // effectively prevents vertical resizing. Both columns now follow the floating window.
        ui.horizontal_top(|ui| {
            if self.graphics_show_signal_browser {
            let viewer_h=ui.available_height();
            let left_w=self.graphics_signal_panel_width.clamp(180.0,520.0);
            ui.allocate_ui_with_layout(
                Vec2::new(left_w,viewer_h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                // Manual browser interaction takes priority over graph-triggered reveal.
                // Cancel before building headers: `.open(Some(true))` ignores their clicks.
                let browser_rect=egui::Rect::from_min_size(ui.cursor().min,Vec2::new(left_w,viewer_h)).intersect(ui.clip_rect());
                if ui.input(|i|i.pointer.primary_pressed() && i.pointer.interact_pos().is_some_and(|pos|browser_rect.contains(pos))) {
                    self.graphics_reveal_source=None;
                }
                // Force a vertical browser layout. The parent viewer is horizontal; without
                // this, child sections inherit that layout and GRAPHICS SOURCES expands beside
                // SIGNALS, consuming the waveform workspace.
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_min_width(0.0);
                ui.horizontal(|ui|{ui.strong("SIGNALS"); ui.separator(); ui.small("Sources / Decoded");});
                let previous_ids=self.graphics_frame_id_filter.selected.clone();
                self.graphics_frame_id_filter.ui(ui, &frame_id_catalog);
                let previous_activity=self.graphics_activity_filter.clone();
                self.graphics_activity_filter.ui(ui);
                let activity_changed=previous_activity!=self.graphics_activity_filter;
                if activity_changed {
                    self.graphics_hide_constant=false;self.graphics_hide_repetitive=false;self.graphics_only_changing=false;
                    available_sources=self.cached_graphics_sources.iter().filter(|(_,bus,ch,_,_)|project_channels.contains(&project_channel_key(bus,*ch))).cloned().collect();
                }
                let id_filter_changed=previous_ids!=self.graphics_frame_id_filter.selected || activity_changed;
                let mut browser_sources:Vec<_>=available_sources.iter().filter(|(_,_,_,key,_)|
                    self.graphics_frame_id_filter.matches(key)).cloned().collect();
                browser_sources.retain(|(_,bus,ch,key,dlc)|self.graphics_source_activity_matches(d,bus,key,*ch,*dlc));
                let mut browser_signals:Vec<_>=available_signals.iter().filter(|sig|
                    self.graphics_frame_id_filter.selected.is_none() || self.cached_graphics_sources.iter().any(|(_,bus,ch,key,_)|
                        project_channels.contains(&project_channel_key(bus,*ch)) && self.graphics_frame_id_filter.matches(key) && signal_matches_source(bus,key,*ch,sig))).collect();
                browser_signals.retain(|sig|self.graphics_signal_activity_matches(d,sig));
                let selected_source=self.graphics_selected_source.clone();
                let selected_signal=self.graphics_selected_signal.clone();
                let reveal_source=self.graphics_reveal_source.clone();
                ui.small(format!("{} messages · {} signals",browser_sources.len(),browser_signals.len()));
                if let Some(ids)=&self.graphics_frame_id_filter.selected {
                    if ids.len()<=4 {ui.label(format!("Selected IDs: {}",if ids.is_empty(){"None".to_string()}else{ids.iter().cloned().collect::<Vec<_>>().join(", ")}));}
                }
                if browser_sources.is_empty() && browser_signals.is_empty(){ui.weak("No matching messages or signals");}
                let nav_h = ui.available_height().max(80.0);
                egui::ScrollArea::vertical()
                    .id_salt("canoe_graphics_signal_nav")
                    .auto_shrink([false, true])
                    .max_height(nav_h)
                    .show(ui, |ui| {
                        ui.small(format!("Project scope: {} bus/channel source(s)", project_channels.len()));
                        if range_selection_complete && self.graphics_range_changed_only {
                            let bounds=self.graphics_range_cached_bounds;
                            egui::CollapsingHeader::new(format!("CHANGED DATA - {} source(s)",range_change_bytes.len()))
                                .id_salt("graphics_changed_data_root")
                                .default_open(false).show(ui,|ui|{
                                    if let Some((a,b))=bounds {
                                        ui.monospace(format!("{:.6} -> {:.6} s",a,b));
                                    }
                                    ui.small("Only payload bytes that changed between consecutive frames inside this window.");
                                    for (source,bytes) in &range_change_bytes {
                                        egui::CollapsingHeader::new(source).show(ui,|ui|{
                                            if bytes.is_empty() { ui.small("No changed bytes"); }
                                            else {
                                                ui.label(format!("{} changed byte(s)",bytes.len()));
                                                ui.horizontal_wrapped(|ui|{
                                                    for byte in bytes { ui.label(format!("B{}",byte)); }
                                                });
                                            }
                                        });
                                    }
                                });
                            ui.separator();
                        }
                        egui::CollapsingHeader::new("GRAPHICS SOURCES")
                            .id_salt("graphics_sources_root")
                            .open((id_filter_changed && !browser_sources.is_empty()).then_some(true).or_else(||reveal_source.as_ref().map(|_|true)))
                            .default_open(true)
                            .show(ui, |ui| {
                        ui.small("Protocol > Channel > Frame > Byte > Bit");
                        let mut buses:BTreeMap<String,BTreeMap<u32,Vec<(String,String,usize)>>>=BTreeMap::new();
                        for (source_key,bus,ch,frame_key,dlc) in &browser_sources {
                            buses.entry(bus.clone()).or_default().entry(*ch).or_default()
                                .push((source_key.clone(),frame_key.clone(),*dlc));
                        }
                        for (bus,channels) in buses {
                            let reveal_bus=reveal_source.as_ref().and_then(|k|parse_graphics_source_key(k)).is_some_and(|(b,_,_)|b==bus);
                            egui::CollapsingHeader::new(bus.clone()).id_salt(("viewer_bus",&bus)).open((id_filter_changed || reveal_bus).then_some(true)).default_open(false).show(ui,|ui|{
                                for (ch,frames) in channels {
                                    let reveal_channel=reveal_source.as_ref().and_then(|k|parse_graphics_source_key(k)).is_some_and(|(b,c,_)|b==bus && c==ch);
                                    let channel_header=egui::CollapsingHeader::new(viewer_channel_label(ch)).id_salt(("viewer_channel",&bus,ch)).open((id_filter_changed || reveal_channel).then_some(true)).default_open(false).show(ui,|ui|{
                                        let frames=if bus=="ETH" {
                                            let pairs=self.graphics_eth_endpoints.channels.get(&ch).cloned().unwrap_or_default();
                                            let filter=self.graphics_eth_filters.entry(ch).or_default();
                                            filter.ui(ui,ch,&pairs);
                                            let frames:Vec<_>=frames.into_iter().filter(|(source,_,_)|self.graphics_eth_endpoints.matches(source,filter)).collect();
                                            ui.small(format!("{} matching messages / streams",frames.len()));
                                            if frames.is_empty(){ui.weak("No matching Ethernet streams");}
                                            frames
                                        } else {frames};
                                        let mut render_frame = |ui:&mut egui::Ui, source_key:String, frame_key:String, dlc:usize| {
                                                let changed_bytes=range_change_bytes.get(&source_key);
                                                // Analog is a sampled signal, not a byte tree. Keep its selector visible.
                                                if bus=="ANALOG" {
                                                    let mut on=self.graphics_source_byte_selected(&source_key,0);
                                                    let active=selected_source.as_deref()==Some(source_key.as_str());
                                                    let response=ui.horizontal(|ui|{
                                                        let text=egui::RichText::new(format!("{}  Analog waveform",frame_key_display(&frame_key))).color(if active {ui.visuals().selection.stroke.color}else{ui.visuals().text_color()});
                                                        if ui.checkbox(&mut on,text)
                                                            .on_hover_text("TECMP Analog: INT16 samples decoded using Data Flags sample time, factor and unit").changed(){
                                                            self.set_graphics_source_byte(&source_key,0,on);
                                                            if on {
                                                                self.graphics_sources.insert(source_key.clone());
                                                                self.graphics_stacked=true;
                                                            } else {
                                                                self.graphics_sources.remove(&source_key);
                                                            }
                                                        }
                                                        ui.small(format!("{} B packet",dlc));
                                                    }).response;
                                                    if reveal_source.as_deref()==Some(source_key.as_str()) {response.scroll_to_me(Some(egui::Align::Center));self.graphics_reveal_source=None;}
                                                    if response.clicked(){self.select_graphics_message(&source_key);}
                                                } else {
                                                    let active=selected_source.as_deref()==Some(source_key.as_str());
                                                    let text=egui::RichText::new(graphics_frame_display_label(&frame_key)).color(if active {ui.visuals().selection.stroke.color}else{ui.visuals().text_color()});
                                                    let frame_response=egui::CollapsingHeader::new(text)
                                                        .id_salt(format!("graphics_frame_{}",source_key))
                                                        .open((reveal_source.as_deref()==Some(source_key.as_str())).then_some(true))
                                                        .default_open(false)
                                                        .show(ui,|ui|{
                                                            ui.horizontal(|ui|{
                                                                let mut whole=self.graphics_whole_frames.contains(&source_key);
                                                                if ui.checkbox(&mut whole,"Whole Frame").on_hover_text("Render the complete payload as one signal-like waveform. The Y value is a deterministic payload signature; any payload change changes the trace.").changed(){
                                                                    let lane=format!("FRAME|{}",source_key);
                                                                    if whole {
                                                                        self.graphics_whole_frames.insert(source_key.clone());
                                                                        self.graphics_sources.insert(source_key.clone());
                                                                        if !self.graphics_lane_order.contains(&lane){self.graphics_lane_order.push(lane);}
                                                                    } else {
                                                                        self.graphics_whole_frames.remove(&source_key);
                                                                        self.graphics_lane_order.retain(|x|x!=&lane);
                                                                    }
                                                                }
                                                                let mut all=self.graphics_source_all_selected(&source_key,dlc);
                                                                if ui.checkbox(&mut all,"All bytes").on_hover_text("Select/deselect the complete message/frame").changed(){
                                                                    self.set_graphics_source_mask(&source_key,dlc,all);
                                                                }
                                                                let count=self.graphics_source_selected_count(&source_key,dlc);
                                                                ui.small(if count==0{"None selected".to_string()}else if count==dlc && dlc>0{"All selected".to_string()}else{format!("{}/{} selected",count,dlc)});
                                                            });
                                                            // Long Ethernet payloads are grouped for UI virtualization-by-expansion.
                                                            // This is only a presentation grouping; every payload byte remains addressable.
                                                            if dlc>32 {
                                                                const BYTE_GROUP:usize=32;
                                                                for start_byte in (0..dlc).step_by(BYTE_GROUP) {
                                                                    let end_byte=(start_byte+BYTE_GROUP).min(dlc);
                                                                    if !self.graphics_activity_filter.is_all() && !(start_byte..end_byte).any(|b|{let class=self.graphics_byte_class(d,&bus,&frame_key,ch,b).0;self.graphics_class_visible(class)}) {continue;}
                                                                    egui::CollapsingHeader::new(format!("Bytes {}-{}",start_byte,end_byte.saturating_sub(1)))
                                                                        .id_salt(format!("graphics_byte_group_{}_{}",source_key,start_byte))
                                                                        .default_open(false)
                                                                        .show(ui,|ui|{
                                                                            for b in start_byte..end_byte {
                                                                                self.graphics_byte_selector_row(ui,d,&bus,&source_key,&frame_key,ch,b,range_filter_active,changed_bytes);
                                                                            }
                                                                        });
                                                                }
                                                            } else {
                                                                for b in 0..dlc {
                                                                    self.graphics_byte_selector_row(ui,d,&bus,&source_key,&frame_key,ch,b,range_filter_active,changed_bytes);
                                                                }
                                                            }
                                                        });
                                                    if reveal_source.as_deref()==Some(source_key.as_str()) {frame_response.header_response.scroll_to_me(Some(egui::Align::Center));self.graphics_reveal_source=None;}
                                                    if frame_response.header_response.clicked(){self.select_graphics_message(&source_key);}
                                                } // Analog direct selector / regular frame tree
                                        };

                                        if bus=="ETH" {
                                            // Group each UDP stream with only the auto-detected Review PDUs
                                            // that originated from that exact 4-tuple. This is presentation-only;
                                            // source keys and analysis/cache isolation remain unchanged.
                                            let mut udp_groups:BTreeMap<String,Vec<(String,String,usize)>>=BTreeMap::new();
                                            let mut other_eth:Vec<(String,String,usize)>=Vec::new();
                                            for (source_key,frame_key,dlc) in frames {
                                                if let Some(stream)=graphics_eth_stream_key(&frame_key) {
                                                    udp_groups.entry(stream).or_default().push((source_key,frame_key,dlc));
                                                } else {
                                                    other_eth.push((source_key,frame_key,dlc));
                                                }
                                            }
                                            for (stream,mut stream_frames) in udp_groups {
                                                stream_frames.sort_by_key(|(_,fk,_)| if fk.starts_with("ETH:UDP:"){0}else{1});
                                                egui::CollapsingHeader::new(stream.clone())
                                                    .id_salt(format!("graphics_eth_stream_{}_{}",ch,stream))
                                                    .open(id_filter_changed.then_some(true).or_else(||reveal_source.as_ref().and_then(|k|parse_graphics_source_key(k)).and_then(|(_,_,fk)|graphics_eth_stream_key(&fk)).filter(|key|key==&stream).map(|_|true)))
                                                    .default_open(false)
                                                    .show(ui,|ui|{
                                                        for (source_key,frame_key,dlc) in stream_frames {
                                                            render_frame(ui,source_key,frame_key,dlc);
                                                        }
                                                    });
                                            }
                                            for (source_key,frame_key,dlc) in other_eth {
                                                render_frame(ui,source_key,frame_key,dlc);
                                            }
                                        } else {
                                            for (source_key,frame_key,dlc) in frames {
                                                render_frame(ui,source_key,frame_key,dlc);
                                            }
                                        }
                                    });
                                    channel_header.header_response.context_menu(|ui| {
                                        if ui.button("Rename channel…").clicked() {
                                            self.graph_rename=Some((Some(ch),String::new(),viewer_channel_names.get(&ch).cloned().unwrap_or_default()));ui.close();
                                        }
                                    });
                                }
                            });
                        }
                        }); // GRAPHICS SOURCES root open by default; all child levels start collapsed
                        egui::CollapsingHeader::new(format!("Decoded ({})", browser_signals.len())).id_salt("graphics_decoded_root").open(selected_signal.as_ref().filter(|_|reveal_source.is_some()).map(|_|true)).default_open(true).show(ui, |ui| {
                            for sig in &browser_signals {
                                let mut on=self.graphics_signals.contains(&sig.name);
                                // Use the already-built source catalog instead of scanning every frame
                                // for every configured signal on every repaint.
                                let has_match=self.cached_graphics_sources.iter().any(|(_,bus,ch,frame_key,_)|
                                    (project_channels.is_empty() || project_channels.contains(&project_channel_key(bus,*ch)))
                                    && signal_matches_source(bus,frame_key,*ch,sig));
                                let base=if sig.unit.trim().is_empty(){sig.name.clone()}else{format!("{} [{}]",sig.name,sig.unit)};
                                let base=self.graph_lane_names.get(&format!("SIG|{}",sig.name)).cloned().unwrap_or(base);
                                let label=if has_match{base}else{format!("{}  (no matching frames)",base)};
                                let active=selected_signal.as_deref()==Some(sig.name.as_str());
                                let response=ui.checkbox(&mut on,egui::RichText::new(label).color(if active {ui.visuals().selection.stroke.color}else{ui.visuals().text_color()}));
                                if active && reveal_source.is_some(){response.scroll_to_me(Some(egui::Align::Center));}
                                if response.changed(){
                                    let lane=format!("SIG|{}",sig.name);
                                    if on{
                                        self.graphics_signals.insert(sig.name.clone());
                                        if !self.graphics_lane_order.contains(&lane){self.graphics_lane_order.push(lane);}
                                    }else{
                                        self.graphics_signals.remove(&sig.name);
                                        self.graphics_lane_order.retain(|x|x!=&lane);
                                    }
                                }
                            }
                        });
                    });
                }); // vertical SIGNALS browser
            });
            } // graphics_show_signal_browser
            if self.graphics_show_signal_browser {
                let (split_rect,split_resp)=ui.allocate_exact_size(Vec2::new(6.0,ui.available_height()),Sense::drag());
                ui.painter().vline(split_rect.center().x,split_rect.y_range(),ui.visuals().widgets.noninteractive.bg_stroke);
                let split_resp=split_resp.on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
                if split_resp.dragged(){
                    self.graphics_signal_panel_width=(self.graphics_signal_panel_width+split_resp.drag_delta().x).clamp(180.0,520.0);
                }
            }
            // The center is the flexible pane. Reserve the current right measurement width,
            // exactly like the project's Trace/Inspector splitter reserves its docked pane.
            let pane_spacing=ui.spacing().item_spacing.x;
            let available=ui.available_width();
            let right_w=self.graphics_cursor_panel_width.clamp(280.0,760.0)
                .min((available-6.0-2.0*pane_spacing-160.0).max(120.0));
            let center_w=(available-right_w-6.0-2.0*pane_spacing).max(80.0);
            // v0.52.9: Cursor Values follow the actual rendered lane rectangles.
            // Lane Height, bit-lane caps, control rows and per-lane spacing are therefore
            // measured once on the waveform side instead of re-derived on the value side.
            self.enforce_analog_graph_mode(&project_channels);
            let mut rendered_lane_rects:Vec<(String,f32,f32)>=Vec::new();
            ui.allocate_ui_with_layout(
                Vec2::new(center_w, ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                // Long lane identities must not expand this pane past its divider.
                ui.set_max_width(center_w);
                let center_clip=egui::Rect::from_min_size(ui.cursor().min,Vec2::new(center_w,ui.available_height()));
                ui.set_clip_rect(ui.clip_rect().intersect(center_clip));
                if self.graphics_sources.is_empty() && self.graphics_signals.is_empty() && self.graphics_bits.is_empty() && self.graphics_whole_frames.is_empty() {
                    ui.centered_and_justified(|ui| { ui.label("Select bytes or signals from the list."); });
                } else if self.graphics_stacked {
                    // Repair selections saved by v0.41.2/v0.46.1 where the Analog checkbox
                    // could be checked without adding the source to graphics_sources.
                    let analog_checked=self.cached_graphics_sources.iter()
                        .filter(|(k,bus,_,_,_)|bus=="ANALOG" && self.graphics_source_byte_selected(k,0))
                        .map(|(k,_,_,_,_)|k.clone()).collect::<Vec<_>>();
                    for k in analog_checked { self.graphics_sources.insert(k); }
                    self.sync_graphics_lane_order();
                    let lane_order=self.graphics_lane_order.clone();

                    let (ruler_a,ruler_b)=if self.graphics_per_waveform_cursors {(f64::NAN,f64::NAN)}else{(self.cursor_a_s,self.cursor_b_s)};
                    draw_xviewer_time_ruler(ui,self.graphics_view_start_s,self.graphics_view_end_s,
                        capture_first_ns,ruler_a,ruler_b,&event_marks);
                    // v0.52.3: graph lanes and Cursor Values use the same persistent scroll id.
                    // The CM ruler stays outside/frozen; only the lane body scrolls.
                    let shared_lane_scroll_h=(ui.available_height()-34.0).max(140.0);
                    let graph_scroll_out=graphics_lane_scroll_area(self.graphics_lane_scroll_y,shared_lane_scroll_h).show(ui, |ui| {
                        let plot_width = ui.available_width().max(80.0);
                        ui.set_min_width(plot_width);

                        for lane_id in lane_order.iter() {
                            if !self.graphics_lane_activity_matches(d,lane_id){continue;}
                            let lane_screen_top=ui.cursor().top();
                            let mut rendered=false;

                            if let Some(source_part)=lane_id.strip_prefix("FRAME|") {
                                if let Some((bus,ch,frame_key))=parse_graphics_source_key(source_part) {
                                    if self.graphics_whole_frames.contains(source_part)
                                        && project_channels.contains(&project_channel_key(&bus,ch)) {
                                        let dlc=Self::frame_payload_len(d,&frame_key);
                                        let frame_constant=self.graphics_frame_is_constant(d,&bus,&frame_key,ch,dlc);
                                        if !(self.graphics_hide_constant && frame_constant) {
                                            let raw=self.digital_view_waveform(d,&bus,&frame_key,ch,None,None);
                                            if !raw.is_empty() {
                                                let pts=raw.as_ref().clone();
                                                let label=graphics_lane_header(&bus,&viewer_channel_label(ch),&frame_key,"Whole Frame");
                                                self.graph_lane_header_ui(ui,lane_id,&label);
                                                let multi_cursors=self.graphics_multi_cursors.get(lane_id).cloned().unwrap_or_default();
                                                let multi_armed=self.graphics_multi_cursor_armed.as_deref()==Some(lane_id.as_str());
                                                let multi_drag=self.graphics_multi_cursor_drag.as_ref().and_then(|(id,idx)|if id==lane_id{Some(*idx)}else{None});
                                                let ys=self.graphics_y_scale.entry(lane_id.clone()).or_insert(1.0);
                                                let yo=self.graphics_y_offset.entry(lane_id.clone()).or_insert(0.0);
                                                let cursor_action=draw_canoe_lane_interactive(ui,&label,lane_id,&pts,self.graphics_lane_height,
                                                    &mut self.cursor_a_s,&mut self.cursor_b_s,&mut self.graphics_drag_cursor,&mut self.graphics_active_cursor,"payload signature",
                                                    self.replay_position_s,&event_marks,None,self.graphics_auto_y,
                                                    &mut self.graphics_view_start_s,&mut self.graphics_view_end_s,
                                                    self.cached_stats.as_ref().map(|s|s.duration_s).unwrap_or(1.0).max(0.001),
                                                    ys,yo,self.graphics_density,&mut self.graphics_range_start_s,&mut self.graphics_range_end_s,&mut self.graphics_range_dragging,&mut self.graphics_range_interaction,&mut self.graphics_range_last_pointer_s,self.graph_use_colors,self.graph_show_grid,self.graph_show_markers,self.graph_show_cursor_labels,self.graph_line_width,self.graphics_per_waveform_cursors,multi_armed,&multi_cursors,multi_drag);
                                                self.apply_lane_cursor_action(lane_id,cursor_action);
                                                ui.add_space(4.0); rendered=true;
                                            }
                                        }
                                    }
                                }
                                if rendered {
                                    rendered_lane_rects.push((lane_id.clone(),lane_screen_top,ui.cursor().top()));
                                    continue;
                                }
                            }

                            if let Some(rest)=lane_id.strip_prefix("SRC|") {
                                if let Some((source_part,byte_text))=rest.rsplit_once('|') {
                                    if let Ok(b)=byte_text.parse::<usize>() {
                                        if let Some((bus,ch,frame_key))=parse_graphics_source_key(source_part) {
                                            if self.graphics_sources.contains(source_part)
                                                && self.graphics_source_byte_selected(source_part,b)
                                                && project_channels.contains(&project_channel_key(&bus,ch))
                                                && (bus=="ANALOG" || !self.graphics_only_changing || self.cached_changing_graphics_sources.contains(source_part))
                                                && (!range_filter_active || range_change_bytes.get(source_part).map(|bytes|bytes.contains(&b)).unwrap_or(false))
                                            {
                                                if bus=="ANALOG" {
                                                    let (raw,unit,sample_count,period_ns)=self.cached_analog_display_waveform(d,&frame_key,ch);
                                                    if period_ns==0 {
                                                        ui.horizontal(|ui|{
                                                            ui.strong(format!("{} / {} / Analog",bus,viewer_channel_label(ch)));
                                                            ui.colored_label(egui::Color32::YELLOW,"No valid TECMP Sample Time in Data Flags");
                                                        });
                                                        ui.small(format!("Packet contains {} samples ({} bytes). Waveform needs Sample Time code 0x1..0xF.",sample_count,sample_count*2));
                                                        rendered=true;
                                                    } else if raw.is_empty() {
                                                        ui.colored_label(egui::Color32::YELLOW,
                                                            format!("{} / {} / Analog: selected, but no decoded samples were produced",bus,viewer_channel_label(ch)));
                                                        rendered=true;
                                                    } else {
                                                        let pts=self.analog_view_points(d,&frame_key,ch,self.graphics_view_start_s,self.graphics_view_end_s).as_ref().clone();
                                                        // With Auto Y off keep a stable full-capture range (the global min/max is preserved by the display downsample).
                                                        let analog_fixed_range=if self.graphics_auto_y {None} else {
                                                            let lo=raw.iter().map(|q|q[1]).fold(f64::INFINITY,f64::min);
                                                            let hi=raw.iter().map(|q|q[1]).fold(f64::NEG_INFINITY,f64::max);
                                                            if (hi-lo).abs()<1e-12 {let pad=(lo.abs()*0.05).max(0.5);Some((lo-pad,hi+pad))} else {let pad=(hi-lo)*0.05;Some((lo-pad,hi+pad))}
                                                        };
                                                        let label=format!("{} [{}] · {} samples/packet · {:.3} ms",graphics_lane_header(&bus,&viewer_channel_label(ch),&frame_key,"Analog"),unit,sample_count,period_ns as f64/1_000_000.0);
                                                        // Analog participates in the same lane-order controls as raw bytes, bits and signals.
                                                        self.graph_lane_header_ui(ui,lane_id,&label);
                                                        let multi_cursors=self.graphics_multi_cursors.get(lane_id).cloned().unwrap_or_default();
                                                let multi_armed=self.graphics_multi_cursor_armed.as_deref()==Some(lane_id.as_str());
                                                let multi_drag=self.graphics_multi_cursor_drag.as_ref().and_then(|(id,idx)|if id==lane_id{Some(*idx)}else{None});
                                                let ys=self.graphics_y_scale.entry(lane_id.clone()).or_insert(1.0);
                                                        let yo=self.graphics_y_offset.entry(lane_id.clone()).or_insert(0.0);
                                                        let cursor_action=draw_canoe_lane_interactive(ui,&label,lane_id,&pts,self.graphics_lane_height,
                                                            &mut self.cursor_a_s,&mut self.cursor_b_s,&mut self.graphics_drag_cursor,&mut self.graphics_active_cursor,&unit,
                                                            self.replay_position_s,&event_marks,analog_fixed_range,self.graphics_auto_y,
                                                            &mut self.graphics_view_start_s,&mut self.graphics_view_end_s,
                                                            self.cached_stats.as_ref().map(|s|s.duration_s).unwrap_or(1.0).max(0.001),
                                                            ys,yo,self.graphics_density,&mut self.graphics_range_start_s,&mut self.graphics_range_end_s,&mut self.graphics_range_dragging,&mut self.graphics_range_interaction,&mut self.graphics_range_last_pointer_s,self.graph_use_colors,self.graph_show_grid,self.graph_show_markers,self.graph_show_cursor_labels,self.graph_line_width,self.graphics_per_waveform_cursors,multi_armed,&multi_cursors,multi_drag);
                                                self.apply_lane_cursor_action(lane_id,cursor_action);
                                                        ui.add_space(4.0); rendered=true;
                                                    }
                                                    if rendered { rendered_lane_rects.push((lane_id.clone(),lane_screen_top,ui.cursor().top())); }
                                                    continue;
                                                }
                                                let (class,confidence)=self.graphics_byte_class(d,&bus,&frame_key,ch,b);
                                                if self.graphics_hide_constant && class=="STATIC" {continue;}
                                                if self.graphics_hide_repetitive && self.graphics_byte_is_repetitive(d,&bus,&frame_key,ch,b) {continue;}
                                                if !self.graphics_class_visible(class){continue;}
                                                let raw=self.digital_view_waveform(d,&bus,&frame_key,ch,Some(b),None);
                                                if !raw.is_empty() {
                                                    let pts=downsample_minmax(raw.as_slice(),1200);
                                                    let friendly=crate::analysis::friendly_byte_class(class); let badge=if class=="ACTIVE"||class=="STATIC"{friendly.to_string()}else{format!("{} {:.0}%",friendly,confidence*100.0)};
                                                    let mut label=graphics_lane_header(&bus,&viewer_channel_label(ch),&frame_key,&format!("Byte {b}"));
                                                    if self.graphics_density==GraphicsDensity::Detailed { label.push_str(&format!("   [{badge}]")); }

                                                    self.graph_lane_header_ui(ui,lane_id,&label);

                                                    {
                                                        let multi_cursors=self.graphics_multi_cursors.get(lane_id).cloned().unwrap_or_default();
                                                let multi_armed=self.graphics_multi_cursor_armed.as_deref()==Some(lane_id.as_str());
                                                let multi_drag=self.graphics_multi_cursor_drag.as_ref().and_then(|(id,idx)|if id==lane_id{Some(*idx)}else{None});
                                                let ys=self.graphics_y_scale.entry(lane_id.clone()).or_insert(1.0);
                                                        let yo=self.graphics_y_offset.entry(lane_id.clone()).or_insert(0.0);
                                                        let cursor_action=draw_canoe_lane_interactive(
                                                            ui,&label,lane_id,pts.as_slice(),self.graphics_lane_height,
                                                            &mut self.cursor_a_s,&mut self.cursor_b_s,&mut self.graphics_drag_cursor,&mut self.graphics_active_cursor,"",
                                                            self.replay_position_s,
                                                            &event_marks,if self.graphics_auto_y { None } else { Some((0.0,255.0)) },self.graphics_auto_y,
                                                            &mut self.graphics_view_start_s,&mut self.graphics_view_end_s,
                                                            self.cached_stats.as_ref().map(|s|s.duration_s).unwrap_or(1.0).max(0.001),
                                                            ys,yo,self.graphics_density,
                                                            &mut self.graphics_range_start_s,&mut self.graphics_range_end_s,&mut self.graphics_range_dragging,&mut self.graphics_range_interaction,&mut self.graphics_range_last_pointer_s,self.graph_use_colors,self.graph_show_grid,self.graph_show_markers,self.graph_show_cursor_labels,self.graph_line_width,self.graphics_per_waveform_cursors,multi_armed,&multi_cursors,multi_drag);
                                                self.apply_lane_cursor_action(lane_id,cursor_action);
                                                    }
                                                    ui.add_space(4.0);
                                                    rendered=true;
                                                }
                                            }
                                        }
                                    }
                                }
                            } else if let Some(rest)=lane_id.strip_prefix("BIT|") {
                                if self.graphics_bits.contains(rest) {
                                    if let Some((source_and_byte,bit_text))=rest.rsplit_once('|') {
                                        if let (Some((source_part,byte_text)),Ok(bit))=(source_and_byte.rsplit_once('|'),bit_text.parse::<usize>()) {
                                            if let (Ok(byte),Some((bus,ch,frame_key)))=(byte_text.parse::<usize>(),parse_graphics_source_key(source_part)) {
                                                if project_channels.contains(&project_channel_key(&bus,ch))
                                                    && (!range_filter_active || range_change_bytes.get(source_part).map(|bytes|bytes.contains(&byte)).unwrap_or(false)) {
                                                    let raw=self.digital_view_waveform(d,&bus,&frame_key,ch,Some(byte),Some(bit));
                                                    if !raw.is_empty() {
                                                        let digital=raw.as_ref().clone();
                                                        let label=graphics_lane_header(&bus,&viewer_channel_label(ch),&frame_key,&format!("Byte {byte} · Bit {bit}"));
                                                        self.graph_lane_header_ui(ui,lane_id,&label);
                                                        let multi_cursors=self.graphics_multi_cursors.get(lane_id).cloned().unwrap_or_default();
                                                let multi_armed=self.graphics_multi_cursor_armed.as_deref()==Some(lane_id.as_str());
                                                let multi_drag=self.graphics_multi_cursor_drag.as_ref().and_then(|(id,idx)|if id==lane_id{Some(*idx)}else{None});
                                                let ys=self.graphics_y_scale.entry(lane_id.clone()).or_insert(1.0);
                                                        let yo=self.graphics_y_offset.entry(lane_id.clone()).or_insert(0.0);
                                                        let cursor_action=draw_canoe_lane_interactive(ui,&label,lane_id,&digital,self.graphics_lane_height.min(72.0),
                                                            &mut self.cursor_a_s,&mut self.cursor_b_s,&mut self.graphics_drag_cursor,&mut self.graphics_active_cursor,"",
                                                            self.replay_position_s,&event_marks,Some((0.0,1.0)),self.graphics_auto_y,
                                                            &mut self.graphics_view_start_s,&mut self.graphics_view_end_s,
                                                            self.cached_stats.as_ref().map(|s|s.duration_s).unwrap_or(1.0).max(0.001),
                                                            ys,yo,GraphicsDensity::Detailed,&mut self.graphics_range_start_s,&mut self.graphics_range_end_s,&mut self.graphics_range_dragging,&mut self.graphics_range_interaction,&mut self.graphics_range_last_pointer_s,self.graph_use_colors,self.graph_show_grid,self.graph_show_markers,self.graph_show_cursor_labels,self.graph_line_width,self.graphics_per_waveform_cursors,multi_armed,&multi_cursors,multi_drag);
                                                self.apply_lane_cursor_action(lane_id,cursor_action);
                                                        ui.add_space(4.0); rendered=true;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            } else if let Some(name)=lane_id.strip_prefix("SIG|") {
                                if self.graphics_signals.contains(name) {
                                    if let Some(sig)=self.signals.iter().find(|s|s.name==name).cloned() {
                                        let pts=self.cached_decoded_signal_waveform(d,&sig,&project_channels);
                                        let fixed_range=if self.graphics_auto_y{None}else{self.cached_decoded_signal_range(d,&sig,&project_channels)};
                                        if pts.is_empty() {
                                            ui.horizontal(|ui|{
                                                ui.strong(graphics_signal_header(&sig,&viewer_channel_label));
                                                ui.colored_label(Color32::LIGHT_RED,
                                                    format!("No decoded samples · CH {} · ID {} · {}:{} / {} bit",
                                                        if sig.channel.trim().is_empty(){"*".to_string()}else{channel_text_hex(&sig.channel)},
                                                        sig.frame_id,sig.start_byte,sig.start_bit,sig.bit_length));
                                            });
                                            ui.add_space(4.0); rendered=true;
                                        } else {
                                            let label=graphics_signal_header(&sig,&viewer_channel_label);

                                            self.graph_lane_header_ui(ui,lane_id,&label);

                                            {
                                                let multi_cursors=self.graphics_multi_cursors.get(lane_id).cloned().unwrap_or_default();
                                                let multi_armed=self.graphics_multi_cursor_armed.as_deref()==Some(lane_id.as_str());
                                                let multi_drag=self.graphics_multi_cursor_drag.as_ref().and_then(|(id,idx)|if id==lane_id{Some(*idx)}else{None});
                                                let ys=self.graphics_y_scale.entry(lane_id.clone()).or_insert(1.0);
                                                let yo=self.graphics_y_offset.entry(lane_id.clone()).or_insert(0.0);
                                                let cursor_action=draw_canoe_lane_interactive(
                                                    ui,&label,lane_id,pts.as_slice(),self.graphics_lane_height,
                                                    &mut self.cursor_a_s,&mut self.cursor_b_s,&mut self.graphics_drag_cursor,&mut self.graphics_active_cursor,&sig.unit,
                                                    self.replay_position_s,
                                                    &event_marks,fixed_range,self.graphics_auto_y,
                                                    &mut self.graphics_view_start_s,&mut self.graphics_view_end_s,
                                                    self.cached_stats.as_ref().map(|s|s.duration_s).unwrap_or(1.0).max(0.001),
                                                    ys,yo,self.graphics_density,
                                                    &mut self.graphics_range_start_s,&mut self.graphics_range_end_s,&mut self.graphics_range_dragging,&mut self.graphics_range_interaction,&mut self.graphics_range_last_pointer_s,self.graph_use_colors,self.graph_show_grid,self.graph_show_markers,self.graph_show_cursor_labels,self.graph_line_width,self.graphics_per_waveform_cursors,multi_armed,&multi_cursors,multi_drag);
                                                self.apply_lane_cursor_action(lane_id,cursor_action);
                                            }
                                            ui.add_space(4.0);
                                            rendered=true;
                                        }
                                    }
                                }
                            }

                            if rendered {
                                rendered_lane_rects.push((lane_id.clone(),lane_screen_top,ui.cursor().top()));
                            }
                            if !rendered {
                                // Hidden lanes remain in the user order and return
                                // to the same position when re-enabled.
                            }
                        }
                    });
                    // The scrollbar lies outside inner_rect. Always retain its result,
                    // including thumb dragging and momentum after the pointer leaves the plot.
                    self.graphics_lane_scroll_y=graph_scroll_out.state.offset.y.max(0.0);
                    let move_request=graphics_lane_drop_target(ui,self.graphics_drag_lane.as_deref(),
                        &self.graphics_lane_order,&rendered_lane_rects,graph_scroll_out.inner_rect);


                    // CANoe-style overview navigator: full recording with highlighted visible window.
                    let full_end=self.cached_stats.as_ref().map(|x|x.duration_s).unwrap_or(1.0).max(0.001);
                    ui.add_space(4.0);
                    draw_graphics_overview(ui,full_end,&mut self.graphics_view_start_s,&mut self.graphics_view_end_s,self.replay_position_s);


                    if let Some((moving,target))=move_request {
                        self.move_graphics_lane_to_target(&moving,&target);
                        self.graphics_drag_lane=None;
                    } else if ui.ctx().input(|i|i.pointer.any_released()) {
                        self.graphics_drag_lane=None;
                    }
                } else {
                    // Overlay is a common 0..255 raw-byte view. Analog has an engineering-value
                    // scale and therefore cannot be represented correctly there.
                    let analog_selected=self.graphics_sources.iter().any(|k|
                        parse_graphics_source_key(k).map(|(bus,_,_)|bus=="ANALOG").unwrap_or(false));
                    if analog_selected {
                        ui.colored_label(egui::Color32::YELLOW,"Analog requires Stacked view. Enable Stacked or click 'Show all Analog'.");
                    }
                    let mut traces: Vec<(String, Vec<[f64; 2]>)> = Vec::new();
                    let mut trace_lanes=Vec::new();
                    for source_key in self.graphics_sources.clone() {
                        if self.graphics_only_changing && !self.cached_changing_graphics_sources.contains(&source_key) { continue; }
                        if let Some((bus, ch, frame_key)) = parse_graphics_source_key(&source_key) {
                            if !project_channels.contains(&project_channel_key(&bus, ch)) { continue; }
                            if bus=="ANALOG" {continue;}
                            let selected_bytes: Vec<usize> = self.selected_frame_bytes.get(&source_key)
                                .map(|bytes|bytes.iter().copied().collect())
                                .unwrap_or_else(||self.graphics_bytes.iter().copied().collect());
                            for b in selected_bytes {
                                let (class,confidence)=self.graphics_byte_class(d,&bus,&frame_key,ch,b);
                                if self.graphics_hide_constant && class=="STATIC" {continue;}
                                                if self.graphics_hide_repetitive && self.graphics_byte_is_repetitive(d,&bus,&frame_key,ch,b) {continue;}
                                if !self.graphics_class_visible(class){continue;}
                                let raw=self.digital_view_waveform(d,&bus,&frame_key,ch,Some(b),None);
                                if raw.is_empty(){continue;}
                                let friendly=crate::analysis::friendly_byte_class(class); let badge=if class=="ACTIVE"||class=="STATIC"{friendly.to_string()}else{format!("{} {:.0}%",friendly,confidence*100.0)};
                                trace_lanes.push(format!("SRC|{}|{}",source_key,b));
                                let canonical=format!("{} / {} / {} B{} [{}]",bus,viewer_channel_label(ch),frame_key,b,badge);
                                let label=self.graph_lane_names.get(&format!("SRC|{}|{}",source_key,b)).cloned().unwrap_or(canonical);
                                traces.push((label,downsample_minmax(&raw,1800)));
                            }
                        }
                    }
                    if traces.is_empty() { ui.label("Overlay is limited to raw bytes with the same 0-255 scale."); }
                    else { let action=draw_multi_byte_wave_interactive(ui,&traces,ui.available_height().max(180.0),self.cursor_a_s,self.cursor_b_s,self.replay_position_s,&mut self.graphics_view_start_s,&mut self.graphics_view_end_s,self.cached_stats.as_ref().map(|s|s.duration_s).unwrap_or(1.0).max(0.001),self.graph_use_colors,self.graph_show_grid,self.graph_show_cursor_labels,self.graph_line_width);
                        if let Some((index,close))=action {if let Some(lane)=trace_lanes.get(index) {
                            if close {self.remove_graphics_lane(lane);}else{self.select_graphics_lane(lane);}
                        }}
                    }
                }
            });
            // Cursor values follow the exact Graphics lane order. This keeps the right-side
            // measurement rows aligned conceptually with the visible waveform rows and prevents
            // hidden Constant / Repetitive lanes from appearing in the measurement list.
            let mut cursor_rows_by_lane:HashMap<String,(String,String,String,String)>=HashMap::new();
            let mut multi_rows_by_lane:HashMap<String,(String,Vec<(f64,String)>)>=HashMap::new();
            for lane_id in self.graphics_lane_order.clone() {
                if let Some(source_part)=lane_id.strip_prefix("FRAME|") {
                    if let Some((bus,ch,frame_key))=parse_graphics_source_key(source_part) {
                        if let Some(times)=self.graphics_multi_cursors.get(&lane_id).cloned() {
                            let vals=times.iter().filter_map(|t|digital_value_at(d,&bus,&frame_key,ch,None,None,*t).map(|v|(*t,format!("signature {:.0}",v)))).collect::<Vec<_>>();
                            if !vals.is_empty(){multi_rows_by_lane.insert(lane_id.clone(),(graphics_lane_header(&bus,&viewer_channel_label(ch),&frame_key,"Whole Frame"),vals));}
                        }
                    }
                } else if let Some(rest)=lane_id.strip_prefix("SRC|") {
                    if let Some((source_part,byte_text))=rest.rsplit_once('|') {
                        if let (Ok(byte),Some((bus,ch,frame_key)))=(byte_text.parse::<usize>(),parse_graphics_source_key(source_part)) {
                            if !project_channels.contains(&project_channel_key(&bus,ch)){continue;}
                            if bus=="ANALOG" {
                                let (wave,unit,_,period_ns)=self.analog_waveform(d,&frame_key,ch);
                                if period_ns==0 || wave.is_empty(){continue;}
                                let suffix=if unit.trim().is_empty(){String::new()}else{format!(" {}",unit.trim())};
                                // Multi-cursor measurements are independent of the global A/B positions.
                                // This is important for Analog lanes when A/B are disabled or outside the waveform range.
                                if let Some(times)=self.graphics_multi_cursors.get(&lane_id) {
                                    let vals=times.iter().filter_map(|t|analog_sample_at(d,&frame_key,ch,*t).map(|p|(*t,format!("{:.3}{}",p[1],suffix)))).collect::<Vec<_>>();
                                    if !vals.is_empty(){multi_rows_by_lane.insert(lane_id.clone(),(format!("{} / Analog",viewer_channel_label(ch)),vals));}
                                }
                                let av=analog_sample_at(d,&frame_key,ch,self.cursor_a_s).map(|p|p[1]);
                                let bv=analog_sample_at(d,&frame_key,ch,self.cursor_b_s).map(|p|p[1]);
                                if let (Some(a),Some(b))=(av,bv) {
                                    cursor_rows_by_lane.insert(lane_id.clone(),(
                                        viewer_channel_label(ch),
                                        format!("{:.3}{}",a,suffix),
                                        format!("{:.3}{}",b,suffix),
                                        format!("{:+.3}{}",b-a,suffix)));
                                }
                                continue;
                            }
                            let (class,_)=self.graphics_byte_class(d,&bus,&frame_key,ch,byte);
                            if self.graphics_hide_constant && class=="STATIC" {continue;}
                            if self.graphics_hide_repetitive && self.graphics_byte_is_repetitive(d,&bus,&frame_key,ch,byte){continue;}
                            if !self.graphics_class_visible(class){continue;}
                            if let Some(times)=self.graphics_multi_cursors.get(&lane_id) {
                                let vals=times.iter().filter_map(|t|digital_value_at(d,&bus,&frame_key,ch,Some(byte),None,*t).map(|v|(*t,format!("0x{:02X} / {}",v as u8,v as u8)))).collect::<Vec<_>>();
                                if !vals.is_empty(){multi_rows_by_lane.insert(lane_id.clone(),(graphics_lane_header(&bus,&viewer_channel_label(ch),&frame_key,&format!("Byte {byte}")),vals));}
                            }
                            if let (Some(a),Some(b))=(digital_value_at(d,&bus,&frame_key,ch,Some(byte),None,self.cursor_a_s),digital_value_at(d,&bus,&frame_key,ch,Some(byte),None,self.cursor_b_s)) {
                                cursor_rows_by_lane.insert(lane_id.clone(),(
                                    graphics_lane_header(&bus,&viewer_channel_label(ch),&frame_key,&format!("Byte {byte}")),
                                    format!("0x{:02X} / {}",a as u8,a as u8),
                                    format!("0x{:02X} / {}",b as u8,b as u8),
                                    format!("{:+.0}",b-a)));
                            }
                        }
                    }
                } else if let Some(rest)=lane_id.strip_prefix("BIT|") {
                    if !self.graphics_bits.contains(rest){continue;}
                    if let Some((source_and_byte,bit_text))=rest.rsplit_once('|') {
                        if let (Some((source_part,byte_text)),Ok(bit))=(source_and_byte.rsplit_once('|'),bit_text.parse::<usize>()) {
                            if let (Ok(byte),Some((bus,ch,frame_key)))=(byte_text.parse::<usize>(),parse_graphics_source_key(source_part)) {
                                if !project_channels.contains(&project_channel_key(&bus,ch)){continue;}
                                    let av=digital_value_at(d,&bus,&frame_key,ch,Some(byte),None,self.cursor_a_s).map(|v|(((v as u64)>>bit)&1) as f64);
                                let bv=digital_value_at(d,&bus,&frame_key,ch,Some(byte),None,self.cursor_b_s).map(|v|(((v as u64)>>bit)&1) as f64);
                                if let Some(times)=self.graphics_multi_cursors.get(&lane_id) {
                                    let vals=times.iter().filter_map(|t|digital_value_at(d,&bus,&frame_key,ch,Some(byte),None,*t).map(|v|(*t,format!("{}",(((v as u64)>>bit)&1) as u8)))).collect::<Vec<_>>();
                                    if !vals.is_empty(){multi_rows_by_lane.insert(lane_id.clone(),(graphics_lane_header(&bus,&viewer_channel_label(ch),&frame_key,&format!("Byte {byte} · Bit {bit}")),vals));}
                                }
                                if let (Some(a),Some(b))=(av,bv) {
                                    cursor_rows_by_lane.insert(lane_id.clone(),(
                                        graphics_lane_header(&bus,&viewer_channel_label(ch),&frame_key,&format!("Byte {byte} · Bit {bit}")),
                                        format!("{}",a as u8),format!("{}",b as u8),format!("{:+.0}",b-a)));
                                }
                            }
                        }
                    }
                } else if let Some(name)=lane_id.strip_prefix("SIG|") {
                    if !self.graphics_signals.contains(name){continue;}
                    if let Some(sig)=available_signals.iter().find(|s|s.name==name) {
                        let indices=self.cached_signal_indices(d,sig,&project_channels);
                        let unit=if sig.unit.trim().is_empty(){String::new()}else{format!(" {}",sig.unit.trim())};
                        if let Some(times)=self.graphics_multi_cursors.get(&lane_id) {
                            let vals=times.iter().filter_map(|t|decoded_signal_sample_at(d,&indices,sig,*t).map(|p|p[1]).map(|v|(*t,format!("{:.3}{}",v,unit)))).collect::<Vec<_>>();
                            if !vals.is_empty(){multi_rows_by_lane.insert(lane_id.clone(),(graphics_signal_header(sig,&viewer_channel_label),vals));}
                        }
                        if let (Some(a),Some(b))=(decoded_signal_sample_at(d,&indices,sig,self.cursor_a_s).map(|p|p[1]),decoded_signal_sample_at(d,&indices,sig,self.cursor_b_s).map(|p|p[1])) {
                            cursor_rows_by_lane.insert(lane_id.clone(),(
                                graphics_signal_header(sig,&viewer_channel_label),format!("{:.3}{}",a,unit),format!("{:.3}{}",b,unit),format!("{:+.3}{}",b-a,unit)));
                        }
                    }
                }
            }
            let cursor_rows:Vec<(String,String,String,String,String)>=self.graphics_lane_order.iter()
                .filter_map(|lane|cursor_rows_by_lane.get(lane).cloned().map(|r|(lane.clone(),self.graph_lane_names.get(lane).cloned().unwrap_or(r.0),r.1,r.2,r.3))).collect();
            let multi_rows:Vec<(String,String,Vec<(f64,String)>)>=self.graphics_lane_order.iter()
                .filter_map(|lane|multi_rows_by_lane.get(lane).cloned().map(|r|(lane.clone(),self.graph_lane_names.get(lane).cloned().unwrap_or(r.0),r.1))).collect();

            let (split_rect,split_resp)=ui.allocate_exact_size(Vec2::new(6.0,ui.available_height()),Sense::drag());
            ui.painter().vline(split_rect.center().x,split_rect.y_range(),ui.visuals().widgets.noninteractive.bg_stroke);
            let split_resp=split_resp.on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
            if split_resp.dragged(){
                // Dragging the divider right makes Cursor Values narrower; left makes it wider.
                self.graphics_cursor_panel_width=(self.graphics_cursor_panel_width-split_resp.drag_delta().x).clamp(280.0,760.0);
            }
            // Use the same width reserved for this frame; divider changes apply next frame.
            let cursor_w=right_w;
            ui.allocate_ui_with_layout(
                Vec2::new(cursor_w,ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                // v0.52.6: the frozen Cursor Values header is exactly the same height as
                // draw_xviewer_time_ruler (54 px).  Therefore both scrolling bodies begin
                // on the same Y coordinate; no constant header offset is introduced.
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(),54.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.strong("CURSOR VALUES");
                        if self.graphics_per_waveform_cursors {
                            ui.small("Independent cursors for each waveform");
                        } else {
                            ui.columns(3,|columns| {
                                for (column,(label,value)) in columns.iter_mut().zip([
                                    ("A",self.cursor_a_s),("B",self.cursor_b_s),("Δ",(self.cursor_b_s-self.cursor_a_s).abs())]) {
                                    column.add(egui::Label::new(egui::RichText::new(format!("{label}  {value:.6} s")).monospace().small()).truncate()).on_hover_text(format!("{label}: {value:.9} s"));
                                }
                            });
                        }
                    });
                if self.graphics_per_waveform_cursors {
                    if multi_rows.is_empty(){
                        ui.small("Use + Cursor on any plotted waveform, then click that graph to place C1..C8.");
                    } else {
                        let body_rect=ui.available_rect_before_wrap();
                        let clip=ui.clip_rect().intersect(body_rect);
                        ui.set_clip_rect(clip);
                        for (lane_id,name,values) in &multi_rows {
                            let Some((_,top,bottom))=rendered_lane_rects.iter().find(|(id,_,_)|id==lane_id) else { continue; };
                            let top=*top; let bottom=*bottom;
                            if bottom<=body_rect.top() || top>=body_rect.bottom() { continue; }
                            let row_rect=egui::Rect::from_min_max(egui::pos2(body_rect.left(),top),egui::pos2(body_rect.right(),bottom));
                            #[allow(deprecated)]
                            ui.allocate_ui_at_rect(row_rect,|ui|{
                                ui.set_clip_rect(clip.intersect(row_rect));
                                draw_cursor_value_card(ui,lane_id,name,None,values,row_rect.height());
                            });
                        }
                    }
                } else if cursor_rows.is_empty(){
                    ui.small("Select a plotted byte, bit, analog trace, or decoded signal to see cursor values.");
                }else{
                    let body_rect=ui.available_rect_before_wrap();
                    let clip=ui.clip_rect().intersect(body_rect);
                    ui.set_clip_rect(clip);
                    for (lane_id,name,a,b,delta) in &cursor_rows {
                        let Some((_,top,bottom))=rendered_lane_rects.iter().find(|(id,_,_)|id==lane_id) else { continue; };
                        let top=*top;
                        let bottom=*bottom;
                        if bottom<=body_rect.top() || top>=body_rect.bottom() { continue; }
                        let row_rect=egui::Rect::from_min_max(egui::pos2(body_rect.left(),top),egui::pos2(body_rect.right(),bottom));
                        #[allow(deprecated)]
                        ui.allocate_ui_at_rect(row_rect,|ui|{
                            ui.set_clip_rect(clip.intersect(row_rect));
                            draw_cursor_value_card(ui,lane_id,name,Some([a.as_str(),b.as_str(),delta.as_str()]),&[],row_rect.height());
                        });
                    }
                }
            });
        });
        self.graph_settings_panel(ui.ctx());
    }

    fn save_graph_image(&mut self,ctx:&egui::Context,d:&Dataset,path:PathBuf) {
        let result=self.graph_image_report(d,crate::graph_image::system_dark(ctx)).and_then(|report|self.graph_image_export.save(path,&report));
        match result {
            Ok(path)=>self.log_tool("INFO",format!("Saved data-rendered graph image: {}",path.display())),
            Err(error)=>{self.graph_image_export.status=Some((false,format!("Could not save graph image: {error:#}")));self.log_tool("ERROR",format!("Graph image export failed: {error:#}"));},
        }
    }

    fn graph_image_report(&mut self,d:&Dataset,dark:bool)->anyhow::Result<crate::graph_image::Report> {
        use crate::graph_image::{Report,Lane,Cursor};
        self.sync_graphics_lane_order();
        let start=self.graphics_view_start_s;let end=self.graphics_view_end_s;
        let scope=self.project_channels.clone();self.enforce_analog_graph_mode(&scope);let mut lanes=Vec::new();
        for id in self.graphics_lane_order.clone() {
            if !self.graphics_stacked && !id.starts_with("SRC|"){continue;}
            if !self.graphics_lane_activity_matches(d,&id){continue;}
            let times:Vec<(String,f64)>=if self.graphics_per_waveform_cursors {
                self.graphics_multi_cursors.get(&id).map(|v|v.iter().enumerate().map(|(i,t)|(format!("C{}",i+1),*t)).collect()).unwrap_or_default()
            }else{vec![("A".into(),self.cursor_a_s),("B".into(),self.cursor_b_s)]};
            let (name,unit,points,values,fixed,kind)=if let Some(name)=id.strip_prefix("SIG|") {
                let Some(sig)=self.signals.iter().find(|s|s.name==name).cloned() else{continue;};
                let points=self.cached_decoded_signal_waveform(d,&sig,&scope).as_ref().clone();
                let indices=self.cached_signal_indices(d,&sig,&scope);
                let values=times.iter().map(|(_,t)|decoded_signal_sample_at(d,&indices,&sig,*t).map(|p|p[1])).collect::<Vec<_>>();
                let fixed=if self.graphics_auto_y{None}else{self.cached_decoded_signal_range(d,&sig,&scope)};
                (graphics_signal_header(&sig,&|ch|self.channel_label(ch)),sig.unit.clone(),points,values,fixed,0)
            }else{
                let Some(source)=graphics_lane_source(&id) else{continue;};
                let Some((bus,ch,key))=parse_graphics_source_key(&source) else{continue;};
                if !scope.contains(&project_channel_key(&bus,ch)){continue;}
                let (byte,bit)=if let Some(rest)=id.strip_prefix("SRC|"){(rest.rsplit_once('|').and_then(|(_,b)|b.parse::<usize>().ok()),None)}
                    else if let Some(rest)=id.strip_prefix("BIT|") {
                        let Some((prefix,bit))=rest.rsplit_once('|') else{continue;};
                        (prefix.rsplit_once('|').and_then(|(_,b)|b.parse::<usize>().ok()),bit.parse::<usize>().ok())
                    }else{(None,None)};
                if bus=="ANALOG" {
                    let (raw,unit,count,period)=self.analog_waveform(d,&key,ch);if period==0 || raw.is_empty(){continue;}
                    let points=self.analog_view_points(d,&key,ch,start,end).as_ref().clone();
                    let values=times.iter().map(|(_,t)|analog_sample_at(d,&key,ch,*t).map(|p|p[1])).collect();
                    let fixed=if self.graphics_auto_y{None}else{Some(export_axis_range(raw.as_slice()))};
                    (graphics_lane_header(&bus,&self.channel_label(ch),&key,&format!("Analog [{}] · {} samples/packet",unit,count)),unit,points,values,fixed,0)
                }else{
                    if self.graphics_only_changing && !self.cached_changing_graphics_sources.contains(&source){continue;}
                    if let Some(byte)=byte {
                        let class=self.graphics_byte_class(d,&bus,&key,ch,byte).0;
                        if self.graphics_hide_constant && class=="STATIC"{continue;}
                        if self.graphics_hide_repetitive && self.graphics_byte_is_repetitive(d,&bus,&key,ch,byte){continue;}
                    }else if self.graphics_hide_constant && self.graphics_frame_is_constant(d,&bus,&key,ch,Self::frame_payload_len(d,&key)){continue;}
                    let points=self.digital_view_waveform(d,&bus,&key,ch,byte,bit).as_ref().clone();
                    let values=times.iter().map(|(_,t)|digital_value_at(d,&bus,&key,ch,byte,bit,*t)).collect();
                    let detail=if let Some(bit)=bit{format!("Byte {} · Bit {}",byte.unwrap_or(0),bit)}else if let Some(byte)=byte{format!("Byte {byte}")}else{"Whole Frame".into()};
                    let mut name=graphics_lane_header(&bus,&self.channel_label(ch),&key,&detail);
                    if key.contains("@UDP:") || key.starts_with("ETH:UDP:"){name.push_str(&format!("\n{key}"));}
                    (name,if bit.is_some(){"bit"}else if byte.is_some(){"byte"}else{"signature"}.into(),points,values,bit.map(|_|(0.0,1.0)),if bit.is_some(){2}else if byte.is_some(){1}else{3})
                }
            };
            let points=held_points_in_view(&points,start,end);if points.is_empty(){continue;}
            let base=fixed.unwrap_or_else(||export_axis_range(&points));
            let span=(base.1-base.0).max(1e-12);let scale=self.graphics_y_scale.get(&id).copied().unwrap_or(1.0).max(0.05);
            let mid=(base.0+base.1)*0.5+self.graphics_y_offset.get(&id).copied().unwrap_or(0.0)*span;
            let y_range=Some((mid-span/scale*0.5,mid+span/scale*0.5));
            let format_value=|v:Option<f64>|match v {
                None=>"n/a".into(),Some(v) if kind==1=>format!("0x{:02X} / {}",v as u8,v as u8),Some(v) if kind==2=>format!("{v:.0}"),
                Some(v) if kind==3=>format!("{v:.0}"),Some(v)=>format!("{v:.3}{}",if unit.is_empty(){String::new()}else{format!(" {unit}")}),
            };
            let delta=if !self.graphics_per_waveform_cursors {Some(match (values.first().copied().flatten(),values.get(1).copied().flatten()) {
                (Some(a),Some(b)) if kind>0=>format!("{:+.0}",b-a),
                (Some(a),Some(b))=>format!("{:+.3}{}",b-a,if unit.is_empty(){String::new()}else{format!(" {unit}")}),_=>"n/a".into(),
            })}else{None};
            let cursors=times.iter().zip(values).map(|((label,time),value)|Cursor{label:label.clone(),time:*time,value:format_value(value),y:value}).collect();
            let color=waveform_color(&id,dark);let color=if self.graph_use_colors{[color.r(),color.g(),color.b()]}else if dark{[100,190,240]}else{[0,95,160]};
            lanes.push(Lane{name:self.graph_lane_names.get(&id).cloned().unwrap_or(name),unit,points:downsample_minmax(&points,8000),color,y_range,cursors,delta});
        }
        if lanes.is_empty(){anyhow::bail!("No selected graphs match the current filters and time window");}
        let markers=if self.graphics_show_event_markers && self.graph_show_markers{control_triggers(&d.frames,0.0).into_iter().map(|m|(m.time_s,m.label)).collect()}else{Vec::new()};
        Ok(Report{title:self.path.as_ref().and_then(|p|p.file_name()).map(|s|format!("Waveform | {}",s.to_string_lossy())).unwrap_or("Waveform export".into()),
            origin:format!("{} · Origin {}",if d.source_time{"TECMP source time"}else{"Capture time"},format_timestamp_ns(d.frames.first().map(|f|f.timestamp_ns).unwrap_or(0))),
            start,end,lanes,markers,multi:self.graphics_per_waveform_cursors,overlay:!self.graphics_stacked,grid:self.graph_show_grid,dark})
    }

    fn select_graphics_message(&mut self,source:&str) {
        if let Some((_,channel,key))=parse_graphics_source_key(source) {
            self.selected_frame_key=key;self.selected_channel=Some(channel);
            self.graphics_selected_source=Some(source.to_string());self.graphics_selected_signal=None;
        }
    }

    fn select_graphics_lane(&mut self,lane:&str) {
        let source=if let Some(name)=lane.strip_prefix("SIG|") {
            self.graphics_selected_signal=Some(name.to_string());
            self.signals.iter().find(|s|s.name==name).and_then(|sig|self.cached_graphics_sources.iter().find(|(_,bus,ch,key,_)|
                self.project_channels.contains(&project_channel_key(bus,*ch)) && signal_matches_source(bus,key,*ch,sig))).map(|(key,_,_,_,_)|key.clone())
        } else {
            self.graphics_selected_signal=None;
            graphics_lane_source(lane)
        };
        if let Some(source)=source {
            if let Some((bus,channel,key))=parse_graphics_source_key(&source) {
                if bus=="ETH" && self.graphics_eth_filters.get(&channel).is_some_and(|filter|!self.graphics_eth_endpoints.matches(&source,filter)) {
                    self.graphics_eth_filters.remove(&channel);
                }
                self.graphics_frame_id_filter.reveal(&key);
                self.selected_frame_key=key;self.selected_channel=Some(channel);
            }
            self.graphics_selected_source=Some(source.clone());self.graphics_reveal_source=Some(source);
            self.graphics_show_signal_browser=true;
        }
    }

    fn remove_graphics_lane(&mut self,lane:&str) {
        if let Some(name)=lane.strip_prefix("SIG|") {self.graphics_signals.remove(name);}
        else if let Some(bit)=lane.strip_prefix("BIT|") {self.graphics_bits.remove(bit);}
        else if let Some(source)=lane.strip_prefix("FRAME|") {self.graphics_whole_frames.remove(source);}
        else if let Some(rest)=lane.strip_prefix("SRC|") {
            if let Some((source,byte))=rest.rsplit_once('|') {
                if let Ok(byte)=byte.parse::<usize>() {
                    self.selected_frame_bytes.entry(source.to_string()).or_insert_with(||self.graphics_bytes.clone()).remove(&byte);
                }
            }
        }
        if let Some(source)=graphics_lane_source(lane) {
            let has_bytes=self.selected_frame_bytes.get(&source).is_some_and(|b|!b.is_empty());
            let has_bits=self.graphics_bits.iter().any(|b|b.starts_with(&format!("{}|",source)));
            if !has_bytes && !has_bits && !self.graphics_whole_frames.contains(&source) {
                self.graphics_sources.remove(&source);self.selected_frame_bytes.remove(&source);
            }
        }
        self.graphics_lane_order.retain(|id|id!=lane);
        self.graphics_multi_cursors.remove(lane);self.graphics_y_scale.remove(lane);self.graphics_y_offset.remove(lane);
        if self.graphics_multi_cursor_armed.as_deref()==Some(lane){self.graphics_multi_cursor_armed=None;}
        if self.graphics_multi_cursor_drag.as_ref().is_some_and(|(id,_)|id==lane){self.graphics_multi_cursor_drag=None;}
    }

    fn apply_lane_cursor_action(&mut self,lane_id:&str,action:Option<LaneCursorAction>) {
        let Some(action)=action else { return; };
        if !matches!(action,LaneCursorAction::Remove) {self.select_graphics_lane(lane_id);}
        match action {
            LaneCursorAction::Select => {},
            LaneCursorAction::Remove => self.remove_graphics_lane(lane_id),
            LaneCursorAction::Arm => {
                if self.graphics_multi_cursor_armed.as_deref()==Some(lane_id) { self.graphics_multi_cursor_armed=None; }
                else { self.graphics_multi_cursor_armed=Some(lane_id.to_string()); }
            }
            LaneCursorAction::Add(t) => {
                let cursors=self.graphics_multi_cursors.entry(lane_id.to_string()).or_default();
                if cursors.len()<8 { cursors.push(t); }
                self.graphics_multi_cursor_armed=None;
            }
            LaneCursorAction::DragStart(idx) => {
                self.graphics_multi_cursor_drag=Some((lane_id.to_string(),idx));
            }
            LaneCursorAction::Move(idx,t) => {
                if let Some(cursors)=self.graphics_multi_cursors.get_mut(lane_id) {
                    if let Some(cursor)=cursors.get_mut(idx) { *cursor=t; }
                }
            }
            LaneCursorAction::DragEnd => {
                self.graphics_multi_cursor_drag=None;
            }
            LaneCursorAction::Clear => {
                self.graphics_multi_cursors.remove(lane_id);
                if self.graphics_multi_cursor_armed.as_deref()==Some(lane_id) { self.graphics_multi_cursor_armed=None; }
                if self.graphics_multi_cursor_drag.as_ref().is_some_and(|(id,_)|id==lane_id) { self.graphics_multi_cursor_drag=None; }
            }
        }
    }

    fn ensure_graphics_source_classification(&mut self,d:&Dataset,bus:&str,frame_key:&str,channel:u32){
        let source_key=graphics_source_key(bus,channel,frame_key);
        if self.graphics_source_class_ready.contains(&source_key){return;}
        if let Some(ix)=d.index.by_key.get(frame_key){
            // One indexed pass classifies every payload byte for this exact dataset source.
            // Filter bus as well as channel so identical frame keys on different buses never mix.
            // The per-frame-key index is time ordered. Restrict it with two
            // partition points before bus/channel filtering so a small analysis
            // selection never walks the complete capture.
            let (range_lo,range_hi)=if let Some((a,b))=self.graphics_class_window {
                (ix.partition_point(|i|d.frames[*i].relative_time_s<a),
                 ix.partition_point(|i|d.frames[*i].relative_time_s<=b))
            } else {(0,ix.len())};
            let source_indices=ix[range_lo..range_hi].iter().copied().filter(|&i|{
                d.frames.get(i).map(|f|f.channel_id==channel && f.bus_type.to_string()==bus).unwrap_or(false)
            }).collect::<Vec<_>>();
            // The index slice already applies the window; avoid a second full-range filter.
            let (start_s,end_s)=(None,None);
            for r in byte_activity_indexed(&d.frames,&source_indices,Some(channel),start_s,end_s){
                self.graphics_class_cache.insert(
                    format!("{}|{}|{}|{}",bus,channel,frame_key,r.byte),
                    (r.class,r.confidence)
                );
            }
        }
        self.graphics_source_class_ready.insert(source_key);
    }
    fn graphics_byte_class(&mut self,d:&Dataset,bus:&str,frame_key:&str,channel:u32,byte:usize)->(&'static str,f64){
        let ck=format!("{}|{}|{}|{}",bus,channel,frame_key,byte);
        if let Some(v)=self.graphics_class_cache.get(&ck){return *v;}
        self.ensure_graphics_source_classification(d,bus,frame_key,channel);
        self.graphics_class_cache.get(&ck).copied().unwrap_or(("NO DATA",0.0))
    }
    fn graphics_class_visible(&self,class:&str)->bool{
        self.graphics_activity_filter.matches(class)
    }

    fn graphics_source_activity_matches(&mut self,d:&Dataset,bus:&str,key:&str,ch:u32,dlc:usize)->bool {
        if self.graphics_activity_filter.is_empty(){return false;}
        if self.graphics_activity_filter.is_all(){return true;}
        if bus=="ANALOG"{return self.graphics_activity_filter.matches("ANALOG");}
        if dlc==0{return self.graphics_activity_filter.matches("NO DATA");}
        (0..dlc).any(|b|{let class=self.graphics_byte_class(d,bus,key,ch,b).0;self.graphics_class_visible(class)})
    }
    fn graphics_signal_activity_matches(&mut self,d:&Dataset,sig:&SignalDefinition)->bool {
        if self.graphics_activity_filter.is_empty(){return false;}
        if self.graphics_activity_filter.is_all(){return true;}
        if sig.start_bit>7 || sig.bit_length==0 || sig.bit_length>64{return self.graphics_activity_filter.matches("NO DATA");}
        let offset=if endian_is_little(&sig.endian){sig.start_bit}else{7-sig.start_bit};
        let bytes=(offset+sig.bit_length+7)/8;
        let sources:Vec<_>=self.cached_graphics_sources.iter().filter(|(_,bus,ch,key,_)|
            self.project_channels.contains(&project_channel_key(bus,*ch)) && signal_matches_source(bus,key,*ch,sig)).cloned().collect();
        sources.iter().any(|(_,bus,ch,key,_)| {
            if bus=="ANALOG"{return self.graphics_activity_filter.matches("ANALOG");}
            (sig.start_byte..sig.start_byte.saturating_add(bytes)).any(|b| {
                let class=self.graphics_byte_class(d,bus,key,*ch,b).0;self.graphics_class_visible(class)
            })
        })
    }
    fn graphics_lane_activity_matches(&mut self,d:&Dataset,lane:&str)->bool {
        if self.graphics_activity_filter.is_empty(){return false;}
        if self.graphics_activity_filter.is_all(){return true;}
        if let Some(name)=lane.strip_prefix("SIG|") {
            return self.signals.iter().find(|s|s.name==name).cloned().is_some_and(|sig|self.graphics_signal_activity_matches(d,&sig));
        }
        let Some(source)=graphics_lane_source(lane) else{return false;};
        let Some((bus,ch,key))=parse_graphics_source_key(&source) else{return false;};
        if bus=="ANALOG"{return self.graphics_activity_filter.matches("ANALOG");}
        let byte=if let Some(rest)=lane.strip_prefix("SRC|"){rest.rsplit_once('|').and_then(|(_,b)|b.parse::<usize>().ok())}
            else if let Some(rest)=lane.strip_prefix("BIT|"){rest.rsplit_once('|').and_then(|(prefix,_)|prefix.rsplit_once('|')).and_then(|(_,b)|b.parse::<usize>().ok())}else{None};
        if let Some(b)=byte {let class=self.graphics_byte_class(d,&bus,&key,ch,b).0;self.graphics_class_visible(class)}
        else {self.graphics_source_activity_matches(d,&bus,&key,ch,Self::frame_payload_len(d,&key))}
    }

    fn sync_graphics_lane_order(&mut self) {
        let mut desired=Vec::<String>::new();

        for source in self.graphics_sources.iter() {
            if self.graphics_whole_frames.contains(source) { desired.push(format!("FRAME|{}",source)); }
            if let Some(bytes)=self.selected_frame_bytes.get(source) {
                for &b in bytes { desired.push(format!("SRC|{}|{}",source,b)); }
            }
        }
        for bit_key in self.graphics_bits.iter() {
            desired.push(format!("BIT|{}",bit_key));
        }
        for sig in self.graphics_signals.iter() {
            desired.push(format!("SIG|{}",sig));
        }

        // Preserve user ordering for existing lanes without repeated linear membership scans.
        let desired_set:BTreeSet<String>=desired.iter().cloned().collect();
        self.graphics_lane_order.retain(|k| desired_set.contains(k));
        let mut existing:BTreeSet<String>=self.graphics_lane_order.iter().cloned().collect();
        for k in desired {
            if existing.insert(k.clone()) {
                self.graphics_lane_order.push(k);
            }
        }
    }

    fn move_graphics_lane_to_target(&mut self, moving:&str,target:&str) {
        if moving==target { return; }
        let Some(from)=self.graphics_lane_order.iter().position(|x|x==moving) else{return;};
        let Some(to)=self.graphics_lane_order.iter().position(|x|x==target) else{return;};
        let item=self.graphics_lane_order.remove(from);
        // Moving down lands after the target; moving up lands before it.
        let insert=to;
        self.graphics_lane_order.insert(insert,item);
    }

    fn trace_display_row(&mut self, d: &Dataset, idx: usize, channel_names: &BTreeMap<u32, String>) -> TraceDisplayRow {
        if let Some(r) = self.trace_view_cache.get(&idx) { return r.clone(); }
        let f=&d.frames[idx];
        let r=TraceDisplayRow {
            timestamp: format_timestamp_ns(f.timestamp_ns),
            dt: format_duration_s(d.trace_dt_s.get(idx).copied().unwrap_or(0.0)),
            channel: channel_display(f.channel_id,channel_names.get(&f.channel_id).map(String::as_str),self.show_raw_ids),
            protocol: f.bus_type.to_string(),
            frame_key: frame_key_display(&f.frame_key),
            dlc: f.payload.len().to_string(),
            data: f.payload.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" "),
        };
        // Bounded cache: Trace memory remains independent of capture frame count.
        if self.trace_view_cache.len() > 512 { self.trace_view_cache.clear(); }
        self.trace_view_cache.insert(idx, r.clone());
        r
    }

    fn copy_trace_selection(&mut self, d:&Dataset, selected:&[(usize,usize,f64)], names:&BTreeMap<u32,String>, payload_only:bool)->String {
        let columns:Vec<usize>=if self.trace_columns.selected_columns.is_empty() {(0..8).collect()}
            else {self.trace_columns.columns.iter().filter(|c|c.visible && self.trace_columns.selected_columns.contains(&(c.column as usize))).map(|c|c.column as usize).collect()};
        let labels=["Timestamp","dt","Channel","Protocol","ID/Key","DLC","Count","Data"];
        let mut text=if payload_only {String::new()} else {columns.iter().map(|c|labels[*c]).collect::<Vec<_>>().join("\t")};
        for &(idx,count,dt) in selected {
            let mut row=self.trace_display_row(d,idx,names);row.dt=format_duration_s(dt);
            if !text.is_empty() {text.push('\n');}
            if payload_only {text.push_str(&row.data);}
            else {
                let count=count.to_string();
                let values=[&row.timestamp,&row.dt,&row.channel,&row.protocol,&row.frame_key,&row.dlc,&count,&row.data];
                text.push_str(&columns.iter().map(|c|tsv_field(values[*c])).collect::<Vec<_>>().join("\t"));
            }
        }
        text
    }

    fn trace_copy_controls(&mut self, ui: &mut egui::Ui, d: &Dataset, selected: &[(usize, usize, f64)], channel_names: &BTreeMap<u32, String>) {
        let shortcut=!selected.is_empty() && !ui.ctx().wants_keyboard_input() && ui.input_mut(|i| {
            let copy=i.events.iter().any(|e|matches!(e,egui::Event::Copy));
            if copy {i.events.retain(|e|!matches!(e,egui::Event::Copy));} copy
        });
        if shortcut {ui.ctx().copy_text(self.copy_trace_selection(d,selected,channel_names,false));}
        ui.small(format!("{} rows selected · Cmd/Ctrl+A: select all · Cmd/Ctrl+C: copy · Esc: clear · Cmd/Ctrl-click: add · Shift-click: range",selected.len()));
    }

    fn clear_trace_selection(&mut self) {
        self.selected_trace_rows.clear();self.trace_row_anchor=None;self.selected_trace_row=None;
        self.trace_columns.clear_column_selection();
    }

    fn canoe_trace_table_virtual(&mut self, ui: &mut egui::Ui, d: &Dataset, visible_count: usize, channel_names: &BTreeMap<u32, String>) {
        let trace_render_started=Instant::now();
        const ROW_H:f32=28.0;
        let indices_len=visible_count.min(self.cached_scoped_indices.len());

        // Invalidate formatted rows only when the Trace scope changes.
        if self.trace_view_cache_scope != self.cached_scope_signature {
            self.trace_view_cache.clear();
            self.trace_view_cache_scope=self.cached_scope_signature.clone();
        }
        if let Some(select_all)=trace_selection_command(ui) {
            self.clear_trace_selection();
            if select_all {
                self.selected_trace_rows.extend(self.cached_scoped_indices[..indices_len].iter().copied().filter(|idx| {
                    !self.trace_only_changing || self.cached_changing_trace_sources.contains(&trace_source_key(&d.frames[*idx]))
                }));
                self.trace_row_anchor=self.selected_trace_rows.first().copied();
            }
        }
        let selected:Vec<_> = self.selected_trace_rows.iter().copied().filter(|idx| {
            self.cached_scoped_indices[..indices_len].binary_search(idx).is_ok()
                && (!self.trace_only_changing || self.cached_changing_trace_sources.contains(&trace_source_key(&d.frames[*idx])))
        }).map(|idx| (idx, 1, d.trace_dt_s.get(idx).copied().unwrap_or(0.0))).collect();
        self.trace_copy_controls(ui, d, &selected, channel_names);
        let copy_rows=selected;

        // Both Trace modes use the same persisted column geometry.
        egui::ScrollArea::horizontal()
            .id_salt("canoe_trace_hscroll_v018")
            .auto_shrink([false,false])
            .show(ui, |ui| {
                self.trace_columns.header(ui,ROW_H);
                let expanded=self.trace_columns.expanded_row().and_then(|id| {
                    let position=self.cached_scoped_indices[..indices_len].iter().position(|i|*i==id)?;
                    let data=self.trace_display_row(d,id,channel_names).data;
                    Some((position,data))
                });

                egui::ScrollArea::vertical()
                    .id_salt("canoe_trace_vscroll_v018")
                    .auto_shrink([false,false])
                    .stick_to_bottom(self.trace_follow_replay && self.measurement_running && !self.trace_freeze_view)
                    .show_viewport(ui, |ui,viewport| {
                        let expanded=expanded.as_ref().map(|(position,data)|(*position,self.trace_columns.expanded_height(ui,ROW_H,data)));
                        crate::trace_columns::trace_viewport_rows(ui,viewport,ROW_H,indices_len,expanded,|ui,range| {
                        for row in range {
                            // Copy the index before mutable cache access; no large collection clone.
                            let idx=self.cached_scoped_indices[row];
                            let f=&d.frames[idx];
                            if self.trace_only_changing && !self.cached_changing_trace_sources.contains(&trace_source_key(f)) { continue; }

                            let r=self.trace_display_row(d,idx,channel_names);
                            let selected=self.selected_trace_rows.contains(&idx);
                            let resp=self.trace_columns.row(ui,ROW_H,idx,selected,
                                [&r.timestamp,&r.dt,&r.channel,&r.protocol,&r.frame_key,&r.dlc,"1",&r.data]);
                            if resp.clicked() {
                                crate::trace_columns::select_items(&mut self.selected_trace_rows,&mut self.trace_row_anchor,idx,
                                    &self.cached_scoped_indices[..indices_len],ui.input(|i|i.modifiers));
                                self.selected_trace_row=Some(idx);
                                self.selected_frame_key=f.frame_key.clone();
                                self.selected_channel=Some(f.channel_id);
                                self.cursor_a_s=f.relative_time_s;
                                self.cursor_b_s=f.relative_time_s;
                            }
                            resp.context_menu(|ui| {
                                if !copy_rows.is_empty() && ui.button("Copy selection").clicked() {
                                    ui.ctx().copy_text(self.copy_trace_selection(d,&copy_rows,channel_names,false));ui.close();
                                }
                                r.copy_menu(ui,1);
                            });
                            resp.on_hover_text(format!("Raw: {} ns | Relative: {:.9} s",f.timestamp_ns,f.relative_time_s));
                        }
                    });
                    });
            });
        self.perf_trace_render_ms=self.perf_trace_render_ms*0.85+trace_render_started.elapsed().as_secs_f32()*1000.0*0.15;
    }

    fn canoe_trace_table(&mut self, ui: &mut egui::Ui, d: &Dataset, rows: &[TraceRow], channel_names: &BTreeMap<u32, String>) {
        let row_h = 28.0;
        if let Some(select_all)=trace_selection_command(ui) {
            self.clear_trace_selection();
            if select_all {
                self.selected_trace_rows.extend(rows.iter().map(|r|r.idx));
                self.trace_row_anchor=rows.first().map(|r|r.idx);
            }
        }
        let copy_rows:Vec<_> = rows.iter().filter(|r|self.selected_trace_rows.contains(&r.idx))
            .map(|r| (r.idx, r.count, r.dt_s)).collect();
        self.trace_copy_controls(ui, d, &copy_rows, channel_names);
        egui::ScrollArea::horizontal()
            .id_salt("canoe_trace_hscroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
        self.trace_columns.header(ui,row_h);
                let expanded=self.trace_columns.expanded_row().and_then(|id| {
                    let position=rows.iter().position(|r|r.idx==id)?;
                    let data=self.trace_display_row(d,id,channel_names).data;
                    Some((position,data))
                });
        egui::ScrollArea::vertical()
            .id_salt("canoe_trace_vscroll")
            .auto_shrink([false, false])
            .stick_to_bottom(!self.trace_overwrite && self.trace_follow_replay && self.measurement_running && !self.trace_freeze_view)
            .show_viewport(ui, |ui,viewport| {
                        let expanded=expanded.as_ref().map(|(position,data)|(*position,self.trace_columns.expanded_height(ui,row_h,data)));
                        crate::trace_columns::trace_viewport_rows(ui,viewport,row_h,rows.len(),expanded,|ui,range| {
            for row in range {
                let tr = &rows[row];
                let idx = tr.idx;
                let f = &d.frames[idx];
                let selected = self.selected_trace_rows.contains(&idx);
                let mut r=self.trace_display_row(d,idx,channel_names);
                r.dt=format_duration_s(tr.dt_s);
                let count=tr.count.to_string();
                let resp=self.trace_columns.row(ui,row_h,idx,selected,
                    [&r.timestamp,&r.dt,&r.channel,&r.protocol,&r.frame_key,&r.dlc,&count,&r.data]);
                if resp.clicked() {
                    let order:Vec<_>=rows.iter().map(|r|r.idx).collect();
                    crate::trace_columns::select_items(&mut self.selected_trace_rows,&mut self.trace_row_anchor,idx,&order,ui.input(|i|i.modifiers));
                    self.selected_trace_row=Some(idx);self.selected_frame_key=f.frame_key.clone();
                    self.selected_channel=Some(f.channel_id);self.cursor_a_s=f.relative_time_s;self.cursor_b_s=f.relative_time_s;
                }
                resp.context_menu(|ui| {
                    if !copy_rows.is_empty() && ui.button("Copy selection").clicked() {
                        ui.ctx().copy_text(self.copy_trace_selection(d,&copy_rows,channel_names,false));ui.close();
                    }
                    r.copy_menu(ui, tr.count);
                    ui.separator();
                    if ui.button("Add frame/channel to Graphics").clicked() {
                        self.graphics_sources.insert(graphics_source_key(&f.bus_type.to_string(), f.channel_id, &f.frame_key));
                        if self.graphics_bytes.is_empty() {
                            for a in byte_activity(&d.frames, &f.frame_key, Some(f.channel_id)).into_iter().filter(|a| a.transitions > 0).take(4) {
                                self.graphics_bytes.insert(a.byte);
                            }
                        }
                        self.inspector_pane = InspectorPane::Graphics;
                        ui.close();
                    }
                    if ui.button("Add changing bytes to Graphics").clicked() {
                        self.graphics_sources.insert(graphics_source_key(&f.bus_type.to_string(), f.channel_id, &f.frame_key));
                        self.graphics_bytes.clear();
                        for a in byte_activity(&d.frames, &f.frame_key, Some(f.channel_id)).into_iter().filter(|a| a.transitions > 0).take(8) {
                            self.graphics_bytes.insert(a.byte);
                        }
                        self.inspector_pane = InspectorPane::Graphics;
                        ui.close();
                    }
                    if ui.button("Analyze Bytes / Bits").clicked() {
                        self.selected_frame_key = f.frame_key.clone();
                        self.selected_channel = Some(f.channel_id);
                        self.analysis_page = AnalysisPage::FrameByte;
                        self.workspace = Workspace::Analysis;
                        ui.close();
                    }
                    if ui.button("Filter this ID").clicked() {
                        self.filter.text = f.frame_key.clone();
                        self.cached_filter_signature.clear();
                        ui.close();
                    }
                });
            }
        });
                    });
        });
    }

    fn selected_frame_channels(&self, d: &Dataset) -> Vec<u32> {
        let mut s = BTreeSet::new();
        if let Some(ix) = d.index.by_key.get(&self.selected_frame_key) { for i in ix { s.insert(d.frames[*i].channel_id); } }
        s.into_iter().collect()
    }

    fn export_manager(&mut self, ctx:&egui::Context) {
        if !self.show_export_manager { return; }
        let Some(d)=self.dataset.clone() else {
            self.show_export_manager=false;
            return;
        };

        // Native detached viewport: same architecture used by the Graphics window.
        let viewport_id=egui::ViewportId::from_hash_of("native_export_manager");
        let builder=egui::ViewportBuilder::default()
            .with_title("LogLens - Export")
            .with_inner_size(Vec2::new(760.0, 620.0))
            .with_min_inner_size(Vec2::new(560.0, 420.0))
            .with_resizable(true);

        let mut close_request=false;
        ctx.show_viewport_immediate(viewport_id,builder,|ctx,_class|{
            egui::CentralPanel::default().show(ctx,|ui|{

            ui.heading("Export preview");
            ui.separator();
            ui.strong("Range");
            ui.horizontal(|ui|{
                ui.radio_value(&mut self.export_range,ExportRange::CursorAB,"Cursor A → B");
                ui.radio_value(&mut self.export_range,ExportRange::Visible,"Visible range");
                ui.radio_value(&mut self.export_range,ExportRange::Full,"Full recording");
            });
            let full_end=d.frames.last().map(|f|f.relative_time_s).unwrap_or(0.0);
            let (a,b)=match self.export_range{
                ExportRange::CursorAB=>(self.cursor_a_s.min(self.cursor_b_s),self.cursor_a_s.max(self.cursor_b_s)),
                ExportRange::Visible=>(self.graphics_view_start_s,self.graphics_view_end_s),
                ExportRange::Full=>(0.0,full_end),
            };
            ui.monospace(format!("{:.6} s → {:.6} s   Δ {:.6} s",a,b,(b-a).max(0.0)));
            ui.separator();ui.strong("Content");
            ui.horizontal_wrapped(|ui|{
                ui.radio_value(&mut self.export_content,ExportContent::CurrentSelection,"Current selection");
                ui.radio_value(&mut self.export_content,ExportContent::SelectedChannels,"All selected channels");
                ui.radio_value(&mut self.export_content,ExportContent::AllData,"All data in range");
            });
            ui.checkbox(&mut self.export_skip_constant_frames,"Skip constant frames")
                .on_hover_text("Exclude message/frame sources whose payload stays unchanged for the complete selected export range. Analog sources are retained.");

            // First apply time + content scope. SelectedChannels is driven only by the Project Tree's
            // bus/channel keys and does not require a Graphics message/byte/signal selection.
            let mut rows=d.frames.iter().filter(|f|f.relative_time_s>=a&&f.relative_time_s<=b)
                .filter(|f|match self.export_content {
                    ExportContent::AllData=>true,
                    ExportContent::SelectedChannels=>self.project_channels.contains(&project_channel_key(&f.bus_type.to_string(),f.channel_id)),
                    ExportContent::CurrentSelection=>export_frame_selected(f,&self.graphics_sources,&self.graphics_signals,&self.signals),
                }).collect::<Vec<_>>();

            // Optional frame-source de-noising for CSV/Excel. A source is constant only when every
            // payload in the selected export range is identical. Keep Analog because its packet
            // payload contains sample arrays rather than a conventional message/frame payload.
            let before_constant_skip=rows.len();
            let mut constant_sources=BTreeSet::<String>::new();
            if self.export_skip_constant_frames {
                let mut source_state=BTreeMap::<String,(Vec<u8>,bool)>::new();
                for f in &rows {
                    if f.bus_type.to_string()=="ANALOG" { continue; }
                    let key=format!("{}|{}|{}",f.bus_type,f.channel_id,f.frame_key);
                    match source_state.get_mut(&key) {
                        Some((first,changed))=>{ if !*changed && *first!=f.payload { *changed=true; } },
                        None=>{ source_state.insert(key,(f.payload.clone(),false)); },
                    }
                }
                for (key,(_,changed)) in source_state { if !changed { constant_sources.insert(key); } }
                rows.retain(|f|{
                    if f.bus_type.to_string()=="ANALOG" { return true; }
                    !constant_sources.contains(&format!("{}|{}|{}",f.bus_type,f.channel_id,f.frame_key))
                });
            }
            let skipped_constant_frames=before_constant_skip.saturating_sub(rows.len());

            let selected_signals=self.signals.iter().filter(|sig|match self.export_content {
                ExportContent::CurrentSelection=>self.graphics_signals.contains(&sig.name),
                ExportContent::SelectedChannels|ExportContent::AllData=>true,
            }).cloned().collect::<Vec<_>>();
            let mapped=export::map_selected_signals(&rows,&selected_signals);
            let mapped_names=mapped.iter().map(|r|r.signal.as_str()).collect::<BTreeSet<_>>().len();
            ui.separator();ui.strong("Mapping status");
            ui.horizontal_wrapped(|ui|{
                ui.label(format!("{} frame(s)",rows.len()));ui.separator();
                ui.label(format!("{} configured signal(s)",selected_signals.len()));ui.separator();
                ui.label(format!("{} mapped signal(s)",mapped_names));ui.separator();
                ui.label(format!("{} mapped sample(s)",mapped.len()));
                if self.export_skip_constant_frames { ui.separator(); ui.label(format!("{} constant frame(s) skipped",skipped_constant_frames)); }
            });
            if self.export_content==ExportContent::CurrentSelection && self.graphics_sources.is_empty() && self.graphics_signals.is_empty(){
                ui.colored_label(Color32::YELLOW,"No current selection. Select a message/byte/signal in Graphics, choose All selected channels, or choose All data in range.");
            }
            if self.export_content==ExportContent::SelectedChannels && self.project_channels.is_empty(){
                ui.colored_label(Color32::YELLOW,"No Project Tree channels are enabled. Select one or more channels or choose All data in range.");
            }
            ui.separator();ui.strong("Format");
            ui.horizontal(|ui|{
                ui.radio_value(&mut self.export_format,ExportFormat::Excel,"Excel (.xlsx)");
                ui.radio_value(&mut self.export_format,ExportFormat::Csv,"CSV (.csv)");
                ui.radio_value(&mut self.export_format,ExportFormat::Pcapng,"PCAPNG (.pcapng)");
            });
            let pcapng_scope_supported=self.export_format!=ExportFormat::Pcapng || self.export_content==ExportContent::AllData;
            if self.export_format==ExportFormat::Pcapng && self.export_content!=ExportContent::AllData{
                ui.colored_label(Color32::YELLOW,"PCAPNG preserves original packet containers and currently supports All data in range only. Use CSV/Excel for Current selection or All selected channels.");
            }
            if self.export_format==ExportFormat::Pcapng && self.export_skip_constant_frames {
                ui.small("Skip constant frames applies to CSV/Excel normalized-frame export; it is not applied to PCAPNG packet-container export.");
            }
            ui.separator();
            ui.horizontal(|ui|{
                let export_enabled=pcapng_scope_supported && (!rows.is_empty() || self.export_format==ExportFormat::Pcapng);
                if ui.add_enabled(export_enabled,egui::Button::new("Export")).clicked(){
                    match self.export_format{
                        ExportFormat::Csv=>if let Some(dest)=rfd::FileDialog::new().add_filter("CSV", &["csv"]).set_file_name("loglens_export.csv").save_file(){
                            match export::selected_mapped_csv(&dest,&rows,&selected_signals){Ok((nf,ns))=>self.info=Some(format!("CSV exported: {nf} frames, {ns} mapped samples")),Err(e)=>self.error=Some(format!("CSV export failed: {e}"))}
                        },
                        ExportFormat::Excel=>if let Some(dest)=rfd::FileDialog::new().add_filter("Excel", &["xlsx"]).set_file_name("loglens_export.xlsx").save_file(){
                            match export::selected_mapped_xlsx(&dest,&rows,&selected_signals){Ok((nf,ns))=>self.info=Some(format!("Excel exported: {nf} frames, {ns} mapped samples")),Err(e)=>self.error=Some(format!("Excel export failed: {e}"))}
                        },
                        ExportFormat::Pcapng=>if let Some(source)=self.path.clone(){if let Some(dest)=rfd::FileDialog::new().add_filter("PCAPNG", &["pcapng"]).set_file_name("loglens_segment.pcapng").save_file(){
                            match capture::write_pcapng_segment_with_timeline(&source,&dest,a,b,self.max_packets,d.frames.first().map(|f|f.timestamp_ns),d.source_time){Ok(n)=>self.info=Some(format!("PCAPNG exported: {n} packets")),Err(e)=>self.error=Some(format!("PCAPNG export failed: {e}"))}
                        }}
                    }
                }
            });
            });
            if ctx.input(|i|i.viewport().close_requested()) {
                close_request=true;
            }
        });
        if close_request {
            self.show_export_manager=false;
        }
    }

    fn analysis_workspace(&mut self, ui: &mut egui::Ui) {
        let Some(_d) = self.dataset.clone() else { return; };

        // Clean engineering layout: one compact command bar, one narrow navigator,
        // and one large analysis work area. Measurement controls are intentionally absent.
        egui::Frame::new()
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(egui::Margin::symmetric(6, 3))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_sized([120.0, 32.0], egui::Label::new(egui::RichText::new("Analysis").strong().size(15.0)));
                    ui.separator();
                    let page_name = match self.analysis_page {
                        AnalysisPage::FrameByte => "Frame / Byte",
                        AnalysisPage::SignalDiscovery => "Signal Discovery",
                        AnalysisPage::Timing => "Timing / Jitter",
                        AnalysisPage::CountersCrc => "Counters / CRC",
                        AnalysisPage::Events => "CM Markers",
                        AnalysisPage::AbCompare => "A ↔ B Compare",
                    };
                    ui.add_sized([150.0, 32.0], egui::Label::new(egui::RichText::new(page_name).strong()));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_sized([104.0, 32.0], egui::Button::new("Clear")).on_hover_text("Clear discovered candidate signals").clicked() {
                            self.candidates.clear();
                            self.candidates_ready = false;
                        }
                        if ui.add_sized([126.0, 32.0], egui::Button::new("Detect Signals")).on_hover_text("Run DBC-free signal candidate discovery").clicked() {
                            self.analysis_page = AnalysisPage::SignalDiscovery;
                            self.candidates_ready = false;
                        }
                    });
                });
            });
        ui.add_space(3.0);

        let available_h = ui.available_height();
        ui.horizontal_top(|ui| {
            // Navigator: compact rows, not large card-like buttons.
            ui.allocate_ui_with_layout(
                egui::vec2(176.0, available_h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::Frame::new()
                        .fill(ui.visuals().faint_bg_color)
                        .inner_margin(egui::Margin::symmetric(6, 6))
                        .show(ui, |ui| {
                            ui.set_min_width(164.0);
                            ui.small(egui::RichText::new("ANALYSIS TOOLS").strong());
                            ui.add_space(5.0);
                            if ui.add_sized([164.0, 30.0], egui::SelectableLabel::new(self.analysis_page == AnalysisPage::FrameByte, "Byte Analysis")).clicked() {
                                self.analysis_page = AnalysisPage::FrameByte;
                            }
                            if ui.add_sized([164.0, 30.0], egui::SelectableLabel::new(self.analysis_page == AnalysisPage::SignalDiscovery, "Signal Discovery")).clicked() {
                                self.analysis_page = AnalysisPage::SignalDiscovery;
                            }
                            if ui.add_sized([164.0, 30.0], egui::SelectableLabel::new(self.analysis_page == AnalysisPage::Timing, "Timing / Jitter")).clicked() {
                                self.analysis_page = AnalysisPage::Timing;
                            }
                            if ui.add_sized([164.0, 30.0], egui::SelectableLabel::new(self.analysis_page == AnalysisPage::CountersCrc, "Counters / CRC")).clicked() {
                                self.analysis_page = AnalysisPage::CountersCrc;
                            }
                            if ui.add_sized([164.0, 30.0], egui::SelectableLabel::new(self.analysis_page == AnalysisPage::Events, "CM Markers")).clicked() {
                                self.analysis_page = AnalysisPage::Events;
                            }
                            if ui.add_sized([164.0, 30.0], egui::SelectableLabel::new(self.analysis_page == AnalysisPage::AbCompare, "A ↔ B Compare")).clicked() {
                                self.analysis_page = AnalysisPage::AbCompare;
                            }
                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(5.0);
                            ui.small(egui::RichText::new("STATUS").strong());
                            ui.horizontal(|ui| { ui.weak("Channels"); ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label(self.project_channels.len().to_string()); }); });
                            ui.horizontal(|ui| { ui.weak("Signals"); ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label(self.signals.len().to_string()); }); });
                            ui.horizontal(|ui| { ui.weak("Candidates"); ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label(self.candidates.len().to_string()); }); });
                        });
                },
            );

            ui.add_space(3.0);
            ui.separator();
            ui.add_space(3.0);

            // Work area consumes the remaining width and owns its scrolling.
            ui.vertical(|ui| {
                ui.set_min_width((ui.available_width() - 2.0).max(520.0));
                egui::ScrollArea::both()
                    .id_salt("analysis_content_v122")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        match self.analysis_page {
                            AnalysisPage::FrameByte => self.frame_explorer(ui),
                            AnalysisPage::SignalDiscovery => self.signal_processing(ui),
                            AnalysisPage::Timing => self.timing_intelligence(ui),
                            AnalysisPage::CountersCrc => self.counter_crc_intelligence(ui),
                            AnalysisPage::Events => {
                                if let Some(d) = self.dataset.clone() { self.canoe_events_panel(ui, &d); }
                            }
                            AnalysisPage::AbCompare => self.ab_compare_intelligence(ui),
                        }
                    });
            });
        });
    }

    fn configuration_workspace(&mut self, ui: &mut egui::Ui) {
        let Some(_d) = self.dataset.clone() else { return; };

        let (title, description, import_kind) = match self.config_page {
            ConfigPage::Project => ("Project", "project.csv", "project"),
            ConfigPage::Channels => ("Channels", "channels.csv", "channels"),
            ConfigPage::Messages => ("Messages", "messages.csv", "messages"),
            ConfigPage::Signals => ("Signals", "signals.csv", "signals"),
            ConfigPage::Events => ("CM Markers", "events.csv", "events"),
        };

        // Configuration command bar: page identity on the left, edit actions on the right.
        egui::Frame::new()
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(egui::Margin::symmetric(6, 3))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_sized([120.0, 32.0], egui::Label::new(egui::RichText::new("Configuration").strong().size(15.0)));
                    ui.separator();
                    ui.add_sized([100.0, 32.0], egui::Label::new(egui::RichText::new(title).strong()));
                    ui.weak(description);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.config_page==ConfigPage::Signals && ui.add_sized([132.0, 32.0], egui::Button::new("Migrate v0.47.8 Big")).on_hover_text("Convert legacy Big-endian Bit 0=MSB rows to v0.47.9 physical bit numbering (new_bit = 7 - old_bit).").clicked() {
                            let migrated=migrate_legacy_big_bit_positions(&mut self.signal_table);
                            if migrated>0{
                                self.signal_config_dirty=true;
                                self.apply_signal_configuration();
                                self.persist_session_configuration();
                                let msg=format!("Migrated {migrated} legacy Big-endian signal(s) to physical Motorola bit numbering.");
                                self.info=Some(msg.clone());
                                self.error=None;
                                self.log_tool("INFO",msg);
                            }else{
                                self.info=Some("No legacy v0.47.8 Big-endian rows found. New Motorola rows are unchanged.".into());
                            }
                        }
                        if ui.add_sized([96.0, 32.0], egui::Button::new("Validate")).on_hover_text("Check table shape and required headers").clicked() {
                            let table = match self.config_page {
                                ConfigPage::Project => &self.project_config,
                                ConfigPage::Channels => &self.channel_map,
                                ConfigPage::Messages => &self.vehicle_db,
                                ConfigPage::Signals => &self.signal_table,
                                ConfigPage::Events => &self.event_log,
                            };
                            let validation = if self.config_page==ConfigPage::Signals {
                                validate_signal_configuration_table(table)
                            } else if table.headers.is_empty() {
                                Err(format!("{title}: no table loaded"))
                            } else if table.rows.iter().any(|r| r.len() != table.headers.len()) {
                                Err(format!("{title}: one or more rows do not match the header column count"))
                            } else {
                                Ok(format!("{title}: {} row(s) validated", table.rows.len()))
                            };
                            match validation {
                                Ok(msg) => { self.error = None; self.info = Some(msg.clone()); self.log_tool("INFO", msg); }
                                Err(msg) => { self.error = Some(msg.clone()); self.log_tool("WARN", msg); }
                            }
                        }
                        if ui.add_sized([96.0, 32.0], egui::Button::new("Save CSV")).clicked() {
                            let table = match self.config_page {
                                ConfigPage::Project => &mut self.project_config,
                                ConfigPage::Channels => &mut self.channel_map,
                                ConfigPage::Messages => &mut self.vehicle_db,
                                ConfigPage::Signals => &mut self.signal_table,
                                ConfigPage::Events => &mut self.event_log,
                            };
                            match save_table_csv(table) {
                                Ok(path) => {
                                    let msg = format!("Saved {}", path.file_name().and_then(|x| x.to_str()).unwrap_or("configuration.csv"));
                                    self.info = Some(msg.clone());
                                    self.log_tool("INFO", msg);
                                }
                                Err(e) if e != "Save cancelled" => { self.error = Some(e.clone()); self.log_tool("ERROR", e); }
                                Err(_) => {}
                            }
                        }
                        if ui.add_sized([90.0, 32.0], egui::Button::new("+ Row")).clicked() {
                            let table = match self.config_page {
                                ConfigPage::Project => &mut self.project_config,
                                ConfigPage::Channels => &mut self.channel_map,
                                ConfigPage::Messages => &mut self.vehicle_db,
                                ConfigPage::Signals => &mut self.signal_table,
                                ConfigPage::Events => &mut self.event_log,
                            };
                            ensure_configuration_schema(title,table);
                            if !table.headers.is_empty() { table.rows.push(vec![String::new(); table.headers.len()]); }
                        }
                        if ui.add_sized([96.0, 32.0], egui::Button::new("Import")).clicked() { self.open_context(import_kind); }
                    });
                });
            });
        ui.add_space(3.0);

        let available_h = ui.available_height();
        ui.horizontal_top(|ui| {
            // Flat navigator with compact aligned rows and useful counts.
            ui.allocate_ui_with_layout(
                egui::vec2(176.0, available_h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::Frame::new()
                        .fill(ui.visuals().faint_bg_color)
                        .inner_margin(egui::Margin::symmetric(6, 6))
                        .show(ui, |ui| {
                            ui.set_min_width(164.0);
                            ui.small(egui::RichText::new("PROJECT DATA").strong());
                            ui.add_space(5.0);
                            for (page, label, count) in [
                                (ConfigPage::Project, "Project", self.project_config.rows.len()),
                                (ConfigPage::Channels, "Channels", self.channel_map.rows.len()),
                                (ConfigPage::Messages, "Messages", self.vehicle_db.rows.len()),
                                (ConfigPage::Signals, "Signals", self.signal_table.rows.len()),
                                (ConfigPage::Events, "CM Markers", self.event_log.rows.len()),
                            ] {
                                ui.horizontal(|ui| {
                                    if ui.add_sized([116.0, 30.0], egui::SelectableLabel::new(self.config_page == page, label)).clicked() {
                                        if self.config_page==ConfigPage::Signals && page!=ConfigPage::Signals && self.signal_config_dirty {
                                            self.apply_signal_configuration();
                                        }
                                        self.config_page = page;
                                    }
                                    ui.add_sized([40.0, 30.0], egui::Label::new(egui::RichText::new(count.to_string()).weak()));
                                });
                            }
                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(5.0);
                            ui.small(egui::RichText::new("FORMAT").strong());
                            ui.weak("CSV / Excel");
                            ui.weak("User editable");
                            ui.weak("Capture remains read-only");
                        });
                },
            );

            ui.add_space(3.0);
            ui.separator();
            ui.add_space(3.0);

            ui.vertical(|ui| {
                ui.set_min_width((ui.available_width() - 2.0).max(560.0));
                // Small context row aligned with the editor, then the table fills the rest.
                ui.horizontal(|ui| {
                    ui.strong(title);
                    let table = match self.config_page {
                        ConfigPage::Project => &self.project_config,
                        ConfigPage::Channels => &self.channel_map,
                        ConfigPage::Messages => &self.vehicle_db,
                        ConfigPage::Signals => &self.signal_table,
                        ConfigPage::Events => &self.event_log,
                    };
                    ui.weak(format!("{} row(s)", table.rows.len()));
                    if !table.filename.is_empty() { ui.weak(format!("- {}", table.filename)); }
                });
                ui.separator();

                if self.config_page==ConfigPage::Signals {
                    ensure_signal_position_columns(&mut self.signal_table);
                    ensure_signal_data_type_column(&mut self.signal_table);
                }
                let config_channel_choices=configuration_channel_choices(&self.channel_map);
                let config_channel_names=configuration_channel_name_map(&self.channel_map);
                match self.config_page {
                    ConfigPage::Project => {
                        let edit = editable_configuration_table(ui, "Project", &mut self.project_config, "No project.csv loaded.", 100, &config_channel_choices,&config_channel_names,self.show_raw_ids);
                        if edit.changed || edit.saved_path.is_some() { self.persist_session_configuration(); }
                        apply_config_edit_feedback(edit, &mut self.info, &mut self.error);
                    }
                    ConfigPage::Channels => {
                        let edit = editable_configuration_table(ui, "Channels", &mut self.channel_map, "No channels.csv loaded.", 300, &config_channel_choices,&config_channel_names,self.show_raw_ids);
                        if edit.changed || edit.saved_path.is_some() { self.persist_session_configuration(); }
                        apply_config_edit_feedback(edit, &mut self.info, &mut self.error);
                    }
                    ConfigPage::Messages => {
                        let edit = editable_configuration_table(ui, "Messages", &mut self.vehicle_db, "No messages.csv loaded.", 500, &config_channel_choices,&config_channel_names,self.show_raw_ids);
                        if edit.changed || edit.saved_path.is_some() { self.persist_session_configuration(); }
                        apply_config_edit_feedback(edit, &mut self.info, &mut self.error);
                    }
                    ConfigPage::Signals => {
                        let edit = editable_configuration_table(ui, "Signals", &mut self.signal_table, "No signals.csv loaded.", 1000, &config_channel_choices,&config_channel_names,self.show_raw_ids);
                        if edit.changed { self.signal_config_dirty=true; }
                        if edit.commit_requested && self.signal_config_dirty { self.apply_signal_configuration(); }
                        if edit.changed || edit.commit_requested || edit.saved_path.is_some() { self.persist_session_configuration(); }
                        apply_config_edit_feedback(edit, &mut self.info, &mut self.error);
                    }
                    ConfigPage::Events => {
                        let edit = editable_configuration_table(ui, "CM Markers", &mut self.event_log, "No events.csv loaded.", 500, &config_channel_choices,&config_channel_names,self.show_raw_ids);
                        if edit.changed { self.info=Some("Control Message marker configuration updated.".into()); }
                        if edit.changed || edit.saved_path.is_some() { self.persist_session_configuration(); }
                        apply_config_edit_feedback(edit, &mut self.info, &mut self.error);
                    }
                }
            });
        });
    }


    fn graphics_compact_menu(&mut self, ui:&mut egui::Ui) {
        ui.menu_button("...", |ui| {
            ui.strong("Display");
            ui.radio_value(&mut self.graphics_density, GraphicsDensity::Compact, "Compact");
            ui.radio_value(&mut self.graphics_density, GraphicsDensity::Standard, "Standard");
            ui.radio_value(&mut self.graphics_density, GraphicsDensity::Detailed, "Detailed");
            ui.separator();
            ui.checkbox(&mut self.graphics_show_filters, "Class filters");
            ui.checkbox(&mut self.graphics_show_help, "Mouse controls");
            let before_raw = self.show_raw_ids;
            ui.toggle_value(&mut self.show_raw_ids, "Show Raw IDs")
                .on_hover_text("ON: ConfiguredName (RawID). OFF: ConfiguredName only. No configured name: raw ID remains visible.");
            if before_raw != self.show_raw_ids { self.trace_view_cache.clear(); self.persist_session_configuration(); }
            ui.separator();
            ui.label("Compact is the default for maximum waveform area.");
        });
    }

    fn timing_intelligence(&mut self,ui:&mut egui::Ui){
        let Some(d)=self.dataset.clone() else{return;};
        ui.heading("Communication Timing / Jitter");
        if self.selected_frame_key.is_empty(){ui.label("Select a frame in Byte Analysis or Trace first.");return;}
        ui.horizontal(|ui|{ui.strong("Frame");ui.monospace(frame_key_display(&self.selected_frame_key));if let Some(ch)=self.selected_channel{ui.label(format!("Channel {}",channel_id_hex(ch)));}});
        let t=timing_analysis(&d.frames,&self.selected_frame_key,self.selected_channel);
        egui::Grid::new("timing_summary_v037").striped(true).show(ui,|ui|{
            for (k,v) in [
                ("Samples",t.samples.to_string()),("Median cycle",format!("{:.3} ms",t.median_ms)),
                ("Mean cycle",format!("{:.3} ms",t.mean_ms)),("Minimum",format!("{:.3} ms",t.min_ms)),
                ("Maximum",format!("{:.3} ms",t.max_ms)),("Peak-to-peak jitter",format!("{:.3} ms",t.jitter_ms)),
                ("Late/gap candidates",t.late_frames.to_string())
            ]{ui.strong(k);ui.label(v);ui.end_row();}
        });
        ui.separator(); ui.strong("Gap / late-frame candidates");
        ui.small("Candidate threshold = max(1.5 x median cycle, median + 0.1 ms). These are timing anomalies, not protocol-spec violations unless a configured timing requirement is available.");
        egui::ScrollArea::vertical().max_height(300.0).show(ui,|ui|{
            egui::Grid::new("timing_gaps_v037").striped(true).show(ui,|ui|{
                ui.strong("Time");ui.strong("Interval");ui.strong("Action");ui.end_row();
                for (ts,dt) in t.gap_events.iter().take(500){
                    ui.label(format!("{ts:.6} s"));ui.label(format!("{dt:.3} ms"));
                    if ui.small_button("Jump").clicked(){self.replay_position_s=*ts;self.trace_display_position_s=*ts;self.cursor_a_s=*ts;}
                    ui.end_row();
                }
            });
        });
    }

    fn counter_crc_intelligence(&mut self,ui:&mut egui::Ui){
        let Some(d)=self.dataset.clone() else{return;};
        ui.heading("Counter / CRC / Pattern Intelligence");
        if self.selected_frame_key.is_empty(){ui.label("Select a frame first.");return;}
        let indices=d.index.by_key.get(&self.selected_frame_key).cloned().unwrap_or_default();
        let (start_s,end_s)=self.graphics_class_window.map(|(a,b)|(Some(a),Some(b))).unwrap_or((None,None));
        let rows=byte_activity_indexed(&d.frames,&indices,self.selected_channel,start_s,end_s);
        if let Some((a,b))=self.graphics_class_window { ui.label(format!("Analysis Range {:.6}-{:.6}s",a,b)); }
        else { ui.label("Analysis Range: Full capture"); }
        egui::Grid::new("counter_crc_v037").striped(true).show(ui,|ui|{
            for h in ["Byte","Classification","Confidence","Activity","Unique","Reason"]{ui.strong(h);}ui.end_row();
            for r in &rows{
                ui.label(format!("B{}",r.byte));ui.label(crate::analysis::friendly_byte_class(r.class));
                ui.label(format!("{:.1}%",r.confidence*100.0));ui.label(format!("{:.1}%",r.activity_pct));
                ui.label(r.unique_values.to_string());ui.label(&r.reason);ui.end_row();
            }
        });
        for r in rows.iter().filter(|r|r.class=="COUNTER4"||r.class=="COUNTER8"){
            ui.separator();let width=if r.class=="COUNTER4"{4}else{8};
            let shift=if width==4 && r.reason.starts_with("High-nibble") {4}else{0};
            let c=counter_integrity_range(&d.frames,&self.selected_frame_key,self.selected_channel,r.byte,width,shift,start_s,end_s);
            let field=if width==4 && shift==4{"high nibble"}else if width==4{"low nibble"}else{"full byte"};
            ui.strong(format!("B{} — {}-bit counter integrity ({})",r.byte,width,field));
            ui.label(format!("{} samples • {} valid +1 transitions • {:.1}% exact continuity • {} discontinuities",
                c.samples,c.valid_transitions,c.confidence*100.0,c.discontinuities.len()));
            egui::ScrollArea::vertical().max_height(180.0).show(ui,|ui|{
                for (ts,a,b) in c.discontinuities.iter().take(100){
                    ui.horizontal(|ui|{ui.monospace(format!("{ts:.6}s  {a:02X} → {b:02X}"));if ui.small_button("Jump").clicked(){self.cursor_a_s=*ts;self.replay_position_s=*ts;}});
                }
            });
        }
        ui.small("Counter/pattern detection is protocol-independent on normalized payloads. CRC/checksum results remain candidates until the exact OEM polynomial/Data-ID coverage is validated.");
    }

    fn ab_compare_intelligence(&mut self,ui:&mut egui::Ui){
        let Some(d)=self.dataset.clone() else{return;};
        ui.heading("Cursor A ↔ B Compare");
        if self.selected_frame_key.is_empty(){ui.label("Select a frame first.");return;}
        let a=self.cursor_a_s.min(self.cursor_b_s);let b=self.cursor_a_s.max(self.cursor_b_s);
        ui.label(format!("{}   A {:.6}s   B {:.6}s   Δ {:.3} ms",frame_key_display(&self.selected_frame_key),a,b,(b-a)*1000.0));
        let rows=ab_byte_compare(&d.frames,&self.selected_frame_key,self.selected_channel,a,b);
        egui::Grid::new("ab_bytes_v037").striped(true).show(ui,|ui|{
            for h in ["Byte","At A","At B","Changed","Δ decimal"]{ui.strong(h);}ui.end_row();
            for (byte,av,bv) in rows{
                ui.label(format!("B{byte}"));
                ui.monospace(av.map(|v|format!("{v:02X}")).unwrap_or_else(||"--".into()));
                ui.monospace(bv.map(|v|format!("{v:02X}")).unwrap_or_else(||"--".into()));
                ui.label(if av!=bv{"YES"}else{"—"});
                ui.label(match(av,bv){(Some(x),Some(y))=>(y as i16-x as i16).to_string(),_=>"-".into()});ui.end_row();
            }
        });
        ui.separator();ui.strong("Configured signals");
        egui::Grid::new("ab_signals_v037").striped(true).show(ui,|ui|{
            for h in ["Signal","A","B","Δ","Unit"]{ui.strong(h);}ui.end_row();
            for sig in &self.signals{
                let vals=decoded_series_range(&d.frames,sig,a,b,100_000);
                if vals.is_empty(){continue;}
                let va=vals.first().map(|p|p[1]).unwrap_or(0.0);let vb=vals.last().map(|p|p[1]).unwrap_or(va);
                ui.label(&sig.name);ui.label(format!("{va:.4}"));ui.label(format!("{vb:.4}"));ui.label(format!("{:.4}",vb-va));ui.label(&sig.unit);ui.end_row();
            }
        });
    }

    fn add_candidate_to_signal_configuration(&mut self, c:&SignalCandidate) {
        // Canonical Signals schema is used for generated rows so the row is immediately
        // editable by the existing Configuration workspace and decoder.
        ensure_configuration_schema("Signals",&mut self.signal_table);
        ensure_signal_position_columns(&mut self.signal_table);
        ensure_signal_data_type_column(&mut self.signal_table);

        let parts=c.frame_key.split(" / ").collect::<Vec<_>>();
        let protocol=parts.get(0).copied().unwrap_or("").trim().to_string();
        let channel=parts.get(1).copied().unwrap_or("").trim().trim_start_matches("CH").to_string();
        let frame=if parts.len()>=3 { parts[2..].join(" / ") } else { c.frame_key.clone() };

        let base=format!("PossibleSignal_B{}_b{}_L{}",c.byte,c.bit,c.bit_length);
        let existing=self.signal_table.unique(&["signal_name","SignalName","Signal","Name"]);
        let mut name=base.clone(); let mut suffix=2usize;
        while existing.iter().any(|x|x.eq_ignore_ascii_case(&name)) {
            name=format!("{base}_{suffix}"); suffix+=1;
        }

        let mut row=vec![String::new();self.signal_table.headers.len()];
        let mut put=|aliases:&[&str],value:String| {
            if let Some(i)=self.signal_table.headers.iter().position(|h|aliases.iter().any(|a|h.eq_ignore_ascii_case(a))) {
                if i<row.len(){row[i]=value;}
            }
        };
        put(&["protocol","Protocol","bus","Bus","bus_type","BusType"],protocol);
        put(&["channel_id","Channel","ChannelID","Channel ID"],channel);
        put(&["message_id","frame_id","FrameID","CAN_ID","CAN ID","ID"],frame);
        put(&["signal_name","SignalName","Signal","Name"],name.clone());
        put(&["byte_position","BytePosition","byte_pos","start_byte","StartByte","Byte","ByteIndex"],c.byte.to_string());
        let config_bit=if c.likely_endian.eq_ignore_ascii_case("Big") { 7usize.saturating_sub(c.bit) } else { c.bit };
        put(&["bit_position","BitPosition","bit_pos","start_bit","StartBit","Bit"],config_bit.to_string());
        put(&["length","bit_length","BitLength","Length","SignalLength"],c.bit_length.to_string());
        let endian=if c.likely_endian.eq_ignore_ascii_case("Big"){"Motorola / Big Endian"}else{"Intel / Little Endian"};
        put(&["endian","Endian","ByteOrder"],endian.into());
        let inferred_signed=c.likely_type.eq_ignore_ascii_case("Signed");
        put(&["data_type","DataType","Data Type","type","Type"],if inferred_signed{"Signed Integer".into()}else{"Unsigned Integer".into()});
        put(&["signed","Signed","IsSigned"],if inferred_signed{"true".into()}else{"false".into()});
        put(&["factor","scale","Scale","Factor"],"1".into());
        put(&["offset","Offset"],"0".into());
        put(&["unit","Unit"],String::new());
        put(&["description","Description"],format!("Detected candidate; confidence {:.0}%; observed {:.0}..{:.0}",c.score*100.0,c.min_value,c.max_value));

        self.signal_table.rows.push(row);
        self.signal_config_dirty=true;
        self.apply_signal_configuration();
        self.persist_session_configuration();
        self.config_page=ConfigPage::Signals;
        self.info=Some(format!("{name} added to Signal Configuration"));
        self.log_tool("INFO",format!("Detected signal added to configuration: {name}"));
    }

    fn signal_processing(&mut self, ui: &mut egui::Ui) {
        let Some(d) = self.dataset.clone() else { return; };
        ui.heading("Signal Discovery");
        ui.horizontal_wrapped(|ui| {
            if ui.button("Load Signals CSV/XLSX").clicked() { self.open_context("signals"); }
            ui.separator();
            ui.label("Protocol");
            egui::ComboBox::from_id_salt("signal_discovery_scope")
                .selected_text(self.signal_discovery_scope.label())
                .show_ui(ui,|ui|{
                    for scope in [SignalDiscoveryScope::AllDigital,SignalDiscoveryScope::Can,SignalDiscoveryScope::CanFd,
                        SignalDiscoveryScope::Lin,SignalDiscoveryScope::Ethernet,SignalDiscoveryScope::ReviewPdu,SignalDiscoveryScope::Unknown] {
                        ui.selectable_value(&mut self.signal_discovery_scope,scope,scope.label());
                    }
                });
            if ui.button("Detect Signals").on_hover_text("Protocol-aware DBC-free discovery using the committed Analysis Range; A/B cursors are measurement-only.").clicked() {
                let window=self.graphics_class_window;
                self.candidates=match window {
                    Some((a,b))=>crate::analysis::detect_signals_scoped_range(&d.frames,500,Some(a),Some(b),self.signal_discovery_scope),
                    None=>crate::analysis::detect_signals_scoped_range(&d.frames,500,None,None,self.signal_discovery_scope),
                };
                self.candidates_ready=true;
                self.info=Some(format!("{} discovery completed: {} candidate(s)",self.signal_discovery_scope.label(),self.candidates.len()));
            }
            if self.candidates_ready {
                if let Some((a,b))=self.graphics_class_window { ui.label(format!("{} candidates · window {:.3}-{:.3}s",self.candidates.len(),a,b)); }
                else { ui.label(format!("{} candidates · full capture",self.candidates.len())); }
            }
        });
        if !self.signal_table.is_empty() {
            table_preview(ui, &self.signal_table, 15);
        }
        ui.separator();
        ui.label("Possible signal fields (DBC-free)");
        ui.weak("Protocol-aware discovery suppresses constant/counter/repetitive housekeeping, compares field widths and endian interpretations, infers signedness when evidence is stronger, and suppresses heavily overlapping duplicates.");
        if !self.candidates_ready { ui.label("Candidate detection is now on-demand to avoid freezing large logs."); }
        else {
            egui::ScrollArea::both().id_salt("candidate_signal_scroll").max_height(260.0).auto_shrink([false, false]).show(ui, |ui| {
                egui::Grid::new("candidates").striped(true).show(ui, |ui| {
                    for h in ["Frame", "Byte", "Bit", "Len", "Endian", "Type", "Min", "Max", "Transitions", "Confidence", "Action"] { ui.strong(h); } ui.end_row();
                    let mut add_candidate:Option<SignalCandidate>=None;
                    for c in self.candidates.iter().take(250) {
                        ui.label(frame_key_display(&c.frame_key));
                        ui.label(c.byte.to_string());
                        ui.label(c.bit.to_string());
                        ui.label(c.bit_length.to_string());
                        ui.label(&c.likely_endian);
                        ui.label(&c.likely_type);
                        ui.label(format!("{:.0}",c.min_value));
                        ui.label(format!("{:.0}",c.max_value));
                        ui.label(c.transitions.to_string());
                        ui.label(format!("{:.0}%",c.score*100.0));
                        ui.horizontal(|ui|{
                            if ui.small_button("Plot").on_hover_text("Preview this candidate over the selected analysis range").clicked(){self.candidate_preview=Some(c.clone());}
                            if ui.small_button("+ Config").on_hover_text("Add this detected field directly to Signal Configuration").clicked(){add_candidate=Some(c.clone());}
                        });
                        ui.end_row();
                    }
                    if let Some(c)=add_candidate { self.add_candidate_to_signal_configuration(&c); }
                });
            });
        }
        if let Some(c)=self.candidate_preview.clone() {
            ui.separator();
            ui.horizontal(|ui|{
                ui.strong(format!("Candidate Preview · {} · B{}.b{} · {} bit · {} {}",frame_key_display(&c.frame_key),c.byte,c.bit,c.bit_length,c.likely_endian,c.likely_type));
                if ui.small_button("Close").clicked(){self.candidate_preview=None;}
            });
            let window=self.graphics_class_window;
            let pts=crate::analysis::candidate_series(&d.frames,&c,window.map(|x|x.0),window.map(|x|x.1),50_000);
            if pts.len()>1 {
                let marks=control_triggers(&d.frames,0.0);
                draw_canoe_lane(ui,"Signal candidate",&downsample_minmax(&pts,3000),180.0,self.cursor_a_s,self.cursor_b_s,&marks,None);
            }
            else { ui.weak("Not enough samples to preview this candidate in the selected range."); }
        }
        ui.separator();
        ui.horizontal(|ui| { ui.label("Search definitions"); ui.text_edit_singleline(&mut self.signal_search); });
        let defs = self.signals.iter().filter(|s| self.signal_search.trim().is_empty() || s.name.to_ascii_lowercase().contains(&self.signal_search.to_ascii_lowercase())).collect::<Vec<_>>();
        egui::ScrollArea::both().id_salt("signal_definition_scroll").max_height(330.0).auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("signal_defs").striped(true).show(ui, |ui| {
                for h in ["Signal", "Frame", "Ch", "Byte", "Bit", "Len", "Endian", "Data Type", "Scale", "Offset", "Min", "Max", "Transitions"] { ui.strong(h); } ui.end_row();
                for s in defs.iter().take(250) {
                    // Configured-signal statistics follow the committed Analysis Range just
                    // like signal discovery/classification. This prevents values outside the
                    // selected window from appearing in Min/Max/Transitions.
                    let vals = match self.graphics_class_window {
                        Some((a,b)) => decoded_series_range(&d.frames, s, a, b, 20_000),
                        None => decoded_series(&d.frames, s, 20_000),
                    };
                    let min = vals.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
                    let max = vals.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
                    let transitions = vals.windows(2).filter(|w| w[0][1] != w[1][1]).count();
                    ui.label(&s.name); ui.label(&s.frame_id); ui.label(channel_text_hex(&s.channel)); ui.label(s.start_byte.to_string()); ui.label(s.start_bit.to_string()); ui.label(s.bit_length.to_string()); ui.label(&s.endian); ui.label(&s.data_type); ui.label(format!("{}", s.scale)); ui.label(format!("{}", s.offset)); ui.label(if min.is_finite() { format!("{min:.3}") } else { "-".into() }); ui.label(if max.is_finite() { format!("{max:.3}") } else { "-".into() }); ui.label(transitions.to_string()); ui.end_row();
                }
            });
        });
    }

    fn signal_waveforms(&mut self, ui: &mut egui::Ui) {
        let Some(d) = self.dataset.clone() else { return; };
        ui.heading("Signal Waveforms");
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.waveform_overlap, "Compact plots");
            ui.add(egui::DragValue::new(&mut self.time_start).speed(0.01).prefix("Start "));
            ui.add(egui::DragValue::new(&mut self.time_end).speed(0.01).prefix("End "));
        });
        if self.signals.is_empty() {
            ui.label("Load Signals in Configuration or Analysis. Decoded waveforms are shown in Graphics.");
            return;
        }
        let names = self.signals.iter().map(|s| s.name.clone()).collect::<Vec<_>>();
        egui::CollapsingHeader::new(format!("Signal selection ({})", self.waveform_keys.len())).default_open(true).show(ui, |ui| {
            egui::ScrollArea::vertical().max_height(150.0).auto_shrink([false, false]).show(ui, |ui| {
                for n in names.iter() {
                    let mut on = self.waveform_keys.contains(n);
                    if ui.checkbox(&mut on, n).changed() {
                        if on { self.waveform_keys.insert(n.clone()); } else { self.waveform_keys.remove(n); }
                    }
                }
            });
        });
        let selected_defs = self.signals.iter().filter(|s| self.waveform_keys.contains(&s.name)).collect::<Vec<_>>();
        egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(520.0).show(ui, |ui| {
            for s in selected_defs.iter() {
                let indices=decoded_signal_indices(&d,s,&self.project_channels);
                let raw=decoded_signal_view_points(&d,&indices,s,self.time_start,self.time_end);
                let pts=held_points_in_view(&raw,self.time_start,self.time_end);
                ui.label(format!("{}{}", s.name, if s.unit.is_empty() { String::new() } else { format!(" [{}]", s.unit) }));
                let marks=control_triggers(&d.frames,0.0);
                draw_canoe_lane(ui,&s.name,&pts,if self.waveform_overlap {150.0}else{210.0},self.cursor_a_s,self.cursor_b_s,&marks,None);
                ui.add_space(5.0);
            }
        });
        egui::CollapsingHeader::new("Advanced transition and latency analysis").default_open(false).show(ui, |ui| {
            ui.label("Transition counts are calculated from the selected physical signals. Use two signals to compare their first transition times within the active window.");
            if selected_defs.len() >= 2 {
                let a = decoded_series_range(&d.frames, selected_defs[0], self.time_start, self.time_end, 100_000);
                let b = decoded_series_range(&d.frames, selected_defs[1], self.time_start, self.time_end, 100_000);
                let ta = first_transition_time(&a);
                let tb = first_transition_time(&b);
                match (ta, tb) {
                    (Some(x), Some(y)) => { ui.label(format!("First-transition latency: {:.3} ms", (y - x) * 1000.0)); }
                    _ => { ui.label("No comparable transition found in the selected window."); }
                }
            }
        });
        egui::CollapsingHeader::new("Readable points").default_open(false).show(ui, |ui| {
            for s in selected_defs.iter().take(4) {
                let rows = decoded_series_range(&d.frames, s, self.time_start, self.time_end, 200);
                ui.strong(&s.name);
                egui::Grid::new(format!("pts_{}", s.name)).striped(true).show(ui, |ui| {
                    ui.strong("Time"); ui.strong("Value"); ui.end_row();
                    for p in rows.iter().take(100) { ui.label(format!("{:.6}", p[0])); ui.label(format!("{:.6}", p[1])); ui.end_row(); }
                });
            }
        });
    }
    fn performance_hud(&mut self,ctx:&egui::Context) {
        if !self.show_perf_hud { return; }
        egui::Window::new("Performance")
            .id(egui::Id::new("loglens_perf_hud"))
            .default_pos([12.0,90.0]).resizable(false).collapsible(true)
            .show(ctx,|ui|{
                ui.monospace(format!("Trace query {:>6.1} ms",self.perf_trace_query_ms));
                ui.monospace(format!("Trace draw  {:>6.1} ms",self.perf_trace_render_ms));
                if let Some(d)=self.dataset.as_ref(){ui.monospace(format!("Frames {:>10}",d.frames.len()));}
                if ui.small_button("Hide").clicked(){self.show_perf_hud=false;}
            });
    }

}

/// Explicit diagnostic entry point. Uses generated data and never opens or saves a user session.
pub fn run_native_graph_image_verification(output:PathBuf)->eframe::Result<()> {
    struct Verification {app:LogLensApp,data:Arc<Dataset>,output:PathBuf,queued:bool,started:Instant,_fixture:tempfile::NamedTempFile}
    impl eframe::App for Verification {
        fn update(&mut self,ctx:&egui::Context,_frame:&mut eframe::Frame) {
            let viewport=egui::ViewportId::from_hash_of("native_graph_export_verification");
            egui::CentralPanel::default().show(ctx,|ui|{ui.label("Verifying native graph image export…");});
            ctx.show_viewport_immediate(viewport,egui::ViewportBuilder::default().with_title("LogLens - Export verification").with_inner_size([1380.0,820.0]),|child,_| {
                if !self.queued {self.app.save_graph_image(child,&self.data,self.output.clone());self.queued=true;}
                egui::CentralPanel::default().show(child,|ui|self.app.canoe_graphics_viewer_content(ui,&self.data));
            });
            let complete=self.queued;
            if complete || self.started.elapsed()>Duration::from_secs(20) {
                let ok=complete && self.app.graph_image_export.status.as_ref().is_some_and(|s|s.0) && self.output.is_file();
                let dimensions=image::image_dimensions(&self.output).ok();
                let report=serde_json::json!({"ok":ok,"native_immediate_viewport":true,"rendered_from_data":true,"export_theme":if crate::graph_image::system_dark(ctx){"dark"}else{"light"},"dimensions":dimensions,
                    "cursor_a_s":self.app.cursor_a_s,"cursor_b_s":self.app.cursor_b_s,"status":self.app.graph_image_export.status});
                let _=std::fs::write(self.output.with_extension("check.json"),serde_json::to_vec_pretty(&report).unwrap());
                println!("{report}");ctx.send_viewport_cmd_to(viewport,egui::ViewportCommand::Close);
                ctx.send_viewport_cmd_to(egui::ViewportId::ROOT,egui::ViewportCommand::Close);
            }
            ctx.request_repaint_after(Duration::from_millis(40));
        }
    }
    let options=eframe::NativeOptions{viewport:egui::ViewportBuilder::default().with_title("LogLens export verification").with_inner_size([480.0,160.0]),..Default::default()};
    eframe::run_native("LogLens export verification",options,Box::new(move |_cc| {
        use std::io::Write;
        let mut fixture=tempfile::Builder::new().suffix(".csv").tempfile()?;
        writeln!(fixture,"time,channel,id,payload")?;
        for i in 0..=20 {writeln!(fixture,"{:.1},1,123,{:02X}",i as f64*0.1,i)?;}
        fixture.flush()?;
        let data=Arc::new(crate::engine::load(fixture.path(),100).map_err(|e|std::io::Error::other(e.to_string()))?);
        let mut app=LogLensApp::initial_state();app.dataset=Some(data.clone());
        app.cached_stats=Some(statistics(&data.frames));app.project_channels=data.index.project_channel_keys.clone();
        app.cached_graphics_sources=graphics_available_sources(&data.frames);
        let source=app.cached_graphics_sources[0].0.clone();app.set_graphics_source_byte(&source,0,true);
        app.graphics_stacked=true;app.graphics_view_start_s=0.0;app.graphics_view_end_s=2.0;
        app.cursor_a_s=0.2;app.cursor_b_s=1.4;
        Ok(Box::new(Verification{app,data,output,queued:false,started:Instant::now(),_fixture:fixture}))
    }))
}

impl eframe::App for LogLensApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.log_tool("INFO","LogLens shutdown requested");
        self.persist_session_configuration();
        self.persist_workspace_if_changed(true);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let guarded=panic::catch_unwind(AssertUnwindSafe(|| {
        self.poll_background_load(ctx);
        if self.dataset.is_some() { self.persist_workspace_if_changed(false); ctx.request_repaint_after(Duration::from_millis(900)); }
        if self.signal_config_dirty && self.workspace!=Workspace::Configuration {
            self.apply_signal_configuration();
        }
        if self.measurement_running && !self.measurement_paused {
            let duration = self.cached_stats.as_ref().map(|s| s.duration_s).unwrap_or(0.0).max(0.0);
            let replay_end = if let Some(d) = self.dataset.as_ref() { self.replay_bounds(d).1 } else { duration };
            if let Some(anchor) = self.replay_anchor {
                let elapsed = anchor.elapsed().as_secs_f64() * self.replay_speed as f64;
                self.replay_position_s = (self.replay_position_s + elapsed).min(replay_end);
                self.replay_anchor = Some(Instant::now());
                if !self.trace_freeze_view {
                    self.trace_display_position_s = self.replay_position_s;
                }
                if self.graphics_follow_replay {
                    let span = (self.graphics_view_end_s - self.graphics_view_start_s).max(0.050).min(duration.max(0.050));
                    if self.replay_position_s > self.graphics_view_end_s || self.replay_position_s < self.graphics_view_start_s {
                        self.graphics_view_end_s = self.replay_position_s.min(duration);
                        self.graphics_view_start_s = (self.graphics_view_end_s - span).max(0.0);
                    }
                }
                if self.replay_position_s >= replay_end {
                    self.measurement_running = false;
                    self.measurement_paused = false;
                    self.replay_anchor = None;
                } else {
                    ctx.request_repaint_after(Duration::from_millis(33));
                }
            } else {
                self.replay_anchor = Some(Instant::now());
            }
        }
        if self.dataset.is_none() || self.show_start_page {
            self.settings_panel(ctx); self.tool_log_panel(ctx);
            self.start_page(ctx);
            self.loading_overlay(ctx);
            self.performance_hud(ctx);
            return;
        }

        // The legacy Sessions/New Session sidebar is intentionally not shown once a log is loaded.
        // Session actions live on the Start Page / command toolbar so Project Tree is the leftmost analysis pane.
        self.header(ctx);
        self.settings_panel(ctx); self.tool_log_panel(ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            self.workspace_tabs(ui);
            match self.workspace {
                Workspace::Measurement => self.canoe_workspace(ui),
                Workspace::Analysis => self.analysis_workspace(ui),
                Workspace::Configuration => self.configuration_workspace(ui),
            }
        });
        // Export is a separate non-modal utility window, independent of the active workspace.
        self.export_manager(ctx);
        self.loading_overlay(ctx);
        self.performance_hud(ctx);
        }));
        if let Err(payload)=guarded {
            let detail=panic_payload_text(payload.as_ref());
            self.measurement_running=false;
            self.measurement_paused=true;
            self.replay_anchor=None;
            self.graphics_viewer_open=false;
            self.selected_trace_row=None;
            self.workspace=Workspace::Measurement;
            self.inspector_pane=InspectorPane::None;
            self.show_start_page=true;
            self.invalidate_dataset_graphics_caches();
            self.cached_filter_signature.clear();
            self.filtered_indices.clear();
            let msg=format!("A processing/UI error was contained and LogLens recovered instead of closing: {detail}");
            self.log_tool("ERROR",&msg);
            self.log_tool("ERROR",format!("Recovery resource snapshot: {}",resources::limits().memory_label()));
            self.error=Some(msg);
            ctx.request_repaint();
        }
    }
}


fn panic_payload_text(payload:&(dyn std::any::Any+Send))->String {
    if let Some(s)=payload.downcast_ref::<&str>() { (*s).to_string() }
    else if let Some(s)=payload.downcast_ref::<String>() { s.clone() }
    else { "unexpected Rust panic".into() }
}

fn tool_log_time() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

fn format_session_time(unix: u64) -> String {
    if unix == 0 { return "unknown".into(); }
    chrono::DateTime::<chrono::Utc>::from_timestamp(unix as i64, 0)
        .map(|dt| dt.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "unknown".into())
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.into() } else { format!("{}...", s.chars().take(n).collect::<String>()) }
}
fn card(ui: &mut egui::Ui, title: &str, value: String) {
    egui::Frame::group(ui.style()).inner_margin(egui::Margin::same(10)).show(ui, |ui| { ui.set_min_width(150.0); ui.weak(title); ui.heading(value); });
}
struct ConfigEditResult {
    changed: bool,
    commit_requested: bool,
    saved_path: Option<PathBuf>,
    error: Option<String>,
}

#[derive(Clone, Copy, Default)]
struct ConfigCellEditResult {
    changed: bool,
    commit_requested: bool,
}

fn configuration_headers(title:&str)->Vec<String>{
    let cols:&[&str]=match title {
        "Project" => &["key","value"],
        "Channels" => &["channel_id","name","protocol","bitrate","description"],
        "Messages" => &["protocol","channel_id","message_id","message_name","dlc","cycle_ms","source","destination","description"],
        "Signals" => &["protocol","channel_id","message_id","signal_name","byte_position","bit_position","length","endian","data_type","signed","factor","offset","unit","description"],
        "CM Markers" => &["event_name","start_s","end_s","description"],
        _ => &[],
    };
    cols.iter().map(|x|x.to_string()).collect()
}

fn ensure_configuration_schema(title:&str,table:&mut TableData){
    if table.headers.is_empty(){table.headers=configuration_headers(title);}
}

fn ensure_signal_position_columns(table:&mut TableData){
    if table.headers.is_empty(){return;}
    let byte_idx=table.headers.iter().position(|h|["byte_position","BytePosition","byte_pos","start_byte","StartByte","Byte","ByteIndex"].iter().any(|n|h.eq_ignore_ascii_case(n)));
    let bit_idx=table.headers.iter().position(|h|["bit_position","BitPosition","bit_pos","start_bit","StartBit","Bit"].iter().any(|n|h.eq_ignore_ascii_case(n)));

    match (byte_idx,bit_idx){
        // Legacy absolute start_bit/bit_position without a byte column. Convert it once
        // to the canonical zero-based byte_position + intra-byte bit_position pair.
        (None,Some(bit_i))=>{
            table.headers[bit_i]="byte_position".into();
            table.headers.insert(bit_i+1,"bit_position".into());
            for row in &mut table.rows{
                if row.len()<=bit_i{row.resize(bit_i+1,String::new());}
                let raw=row[bit_i].trim().parse::<usize>().ok();
                match raw{
                    Some(v)=>{row[bit_i]=(v/8).to_string();row.insert(bit_i+1,(v%8).to_string());}
                    None=>{row[bit_i]=String::new();row.insert(bit_i+1,String::new());}
                }
            }
        }
        (Some(byte_i),Some(bit_i))=>{
            table.headers[byte_i]="byte_position".into();
            table.headers[bit_i]="bit_position".into();
        }
        (Some(byte_i),None)=>{
            table.headers[byte_i]="byte_position".into();
            table.headers.insert(byte_i+1,"bit_position".into());
            for row in &mut table.rows{
                if row.len()<=byte_i{row.resize(byte_i+1,String::new());}
                row.insert(byte_i+1,"0".into());
            }
        }
        (None,None)=>{}
    }
}

fn ensure_signal_data_type_column(table:&mut TableData){
    if table.headers.is_empty(){return;}
    if table.headers.iter().any(|h|matches!(h.trim().to_ascii_lowercase().as_str(),"data_type"|"datatype"|"data type")){return;}
    let signed_idx=table.headers.iter().position(|h|matches!(h.trim().to_ascii_lowercase().as_str(),"signed"|"issigned"));
    let insert_at=signed_idx.unwrap_or(table.headers.len());
    table.headers.insert(insert_at,"data_type".into());
    for row in &mut table.rows {
        if row.len()<insert_at { row.resize(insert_at,String::new()); }
        let signed=signed_idx.and_then(|i|row.get(i)).map(|v|matches!(v.trim().to_ascii_lowercase().as_str(),"true"|"1"|"yes"|"signed")).unwrap_or(false);
        row.insert(insert_at,if signed{"Signed Integer".into()}else{"Unsigned Integer".into()});
    }
}

fn configuration_channel_choices(table:&TableData)->Vec<String>{
    let idx=table.headers.iter().position(|h|{
        let n=h.trim().to_ascii_lowercase();
        n=="channel_id" || n=="channel" || n=="channelid" || n=="channel id"
    });
    let Some(idx)=idx else{return Vec::new();};
    let mut out=Vec::<String>::new();
    for row in &table.rows{
        if let Some(v)=row.get(idx){
            let v=v.trim();
            if !v.is_empty() && !out.iter().any(|x|x==v){out.push(v.to_string());}
        }
    }
    out
}

fn configuration_channel_name_map(table:&TableData)->HashMap<String,String>{
    let id_idx=table.headers.iter().position(|h|{
        matches!(h.trim().to_ascii_lowercase().as_str(),"channel_id"|"channel"|"channelid"|"channel id")
    });
    let name_idx=table.headers.iter().position(|h|{
        matches!(h.trim().to_ascii_lowercase().as_str(),"name"|"channel_name"|"channelname"|"display_name"|"displayname")
    });
    let (Some(id_idx),Some(name_idx))=(id_idx,name_idx) else{return HashMap::new();};
    let mut out=HashMap::new();
    for row in &table.rows{
        let id=row.get(id_idx).map(|v|v.trim()).unwrap_or("");
        let name=row.get(name_idx).map(|v|v.trim()).unwrap_or("");
        if !id.is_empty() && !name.is_empty(){out.insert(id.to_string(),name.to_string());}
    }
    out
}

fn config_channel_display(raw:&str,names:&HashMap<String,String>,show_raw_ids:bool)->String{
    let raw=raw.trim();
    let name=names.get(raw).or_else(||parse_channel_id_text(raw).and_then(|id|names.iter().find(|(key,_)|parse_channel_id_text(key)==Some(id)).map(|(_,name)|name)));
    match parse_channel_id_text(raw) {
        Some(id)=>channel_display(id,name.map(String::as_str),show_raw_ids),
        None=>name.cloned().unwrap_or_else(||raw.to_string()),
    }
}

fn config_dropdown_values(title:&str,header:&str)->Option<&'static [&'static str]>{
    let h=header.trim().to_ascii_lowercase();
    match (title,h.as_str()){
        ("Channels","protocol")|("Messages","protocol")|("Signals","protocol") =>
            Some(&["CAN","CAN FD","LIN","Automotive Ethernet","Analog","Unknown"]),
        ("Signals","endian") => Some(&["Intel / Little Endian","Motorola / Big Endian"]),
        ("Signals","data_type") => Some(&["Unsigned Integer","Signed Integer","Float32","Float64"]),
        ("Signals","signed") => Some(&["false","true"]),
        _ => None,
    }
}

fn config_cell_editor(ui:&mut egui::Ui,title:&str,header:&str,value:&mut String,channel_choices:&[String],channel_names:&HashMap<String,String>,show_raw_ids:bool,signal_little_endian:bool)->ConfigCellEditResult{
    let h=header.trim().to_ascii_lowercase();

    // Signal bit addressing is deliberately explicit: byte_position is zero-based and
    // bit_position is always 0..7 inside that byte. Keeping these as bounded numeric
    // editors prevents an accidental Bit 8 from shifting the decoder into the next byte.
    if title=="Signals" && matches!(h.as_str(),"byte_position"|"byteposition"|"byte_pos"|"bit_position"|"bitposition"|"bit_pos"|"length"|"bit_length"|"bitlength"|"signallength") {
        let (default,min,max,prefix)=if matches!(h.as_str(),"byte_position"|"byteposition"|"byte_pos") {
            (0_i64,0_i64,65535_i64,"Byte ")
        } else if matches!(h.as_str(),"bit_position"|"bitposition"|"bit_pos") {
            (if signal_little_endian {0_i64} else {7_i64},0_i64,7_i64,"Bit ")
        } else {
            (8_i64,1_i64,64_i64,"Len ")
        };
        let before=value.clone();
        let mut n=value.trim().parse::<i64>().unwrap_or(default).clamp(min,max);
        let response=ui.add_sized([140.0,28.0],egui::DragValue::new(&mut n).range(min..=max).prefix(prefix));
        if response.changed(){*value=n.to_string();}
        return ConfigCellEditResult{changed:*value!=before,commit_requested:response.lost_focus() || response.changed()};
    }
    if (title=="Messages" || title=="Signals") && (h=="channel_id" || h=="channel" || h=="channelid" || h=="channel id") && !channel_choices.is_empty(){
        let before=value.clone();
        if value.trim().is_empty(){*value=channel_choices[0].clone();}
        egui::ComboBox::from_id_salt(format!("cfg_channel_{}_{}_{}",title,header,ui.next_auto_id().value()))
            .width(176.0)
            .selected_text(config_channel_display(value,channel_names,show_raw_ids))
            .show_ui(ui,|ui|{
                for option in channel_choices{
                    let display=config_channel_display(option,channel_names,show_raw_ids);
                    ui.selectable_value(value,option.clone(),display);
                }
            });
        let changed=*value!=before;
        return ConfigCellEditResult{changed,commit_requested:changed};
    }
    if let Some(values)=config_dropdown_values(title,header){
        let before=value.clone();
        if value.trim().is_empty(){*value=values[0].to_string();}
        egui::ComboBox::from_id_salt(format!("cfg_combo_{}_{}_{}",title,header,ui.next_auto_id().value()))
            .width(136.0)
            .selected_text(value.as_str())
            .show_ui(ui,|ui|{
                for option in values{
                    ui.selectable_value(value,(*option).to_string(),*option);
                }
            });
        let changed=*value!=before;
        return ConfigCellEditResult{changed,commit_requested:changed};
    }
    let before=value.clone();
    let response=ui.add_sized([140.0,28.0],egui::TextEdit::singleline(value));
    if matches!(h.as_str(),"channel_id"|"channel"|"channelid"|"channel id") && !response.has_focus() {
        *value=channel_text_hex(value);
    }
    let normalized=!response.has_focus() && *value!=before;
    ConfigCellEditResult{changed:response.changed() || normalized,commit_requested:response.lost_focus() || normalized}
}

fn migrate_legacy_big_bit_positions(table:&mut TableData)->usize{
    let Some(endian_idx)=table.headers.iter().position(|h|matches!(h.trim().to_ascii_lowercase().as_str(),"endian"|"byteorder")) else{return 0;};
    let Some(bit_idx)=table.headers.iter().position(|h|matches!(h.trim().to_ascii_lowercase().as_str(),"bit_position"|"bitposition"|"bit_pos"|"start_bit"|"startbit"|"bit")) else{return 0;};
    let mut migrated=0usize;
    for row in &mut table.rows{
        if row.len()<=endian_idx || row.len()<=bit_idx{continue;}
        // v0.47.8's dropdown stored exactly "Big" and interpreted Bit 0 as the MSB.
        // v0.47.9 stores physical bit numbers, so preserve the same signal by mapping
        // old logical bit N to physical bit 7-N. New Motorola labels are left untouched.
        if row[endian_idx].trim().eq_ignore_ascii_case("Big"){
            if let Ok(old_bit)=row[bit_idx].trim().parse::<usize>(){
                if old_bit<=7{
                    row[bit_idx]=(7-old_bit).to_string();
                    row[endian_idx]="Motorola / Big Endian".into();
                    migrated+=1;
                }
            }
        }
    }
    migrated
}

fn validate_signal_configuration_table(table:&TableData)->Result<String,String>{
    if table.headers.is_empty(){return Err("Signals: no table loaded".into());}
    if table.rows.iter().any(|r|r.len()!=table.headers.len()){
        return Err("Signals: one or more rows do not match the header column count".into());
    }
    let has_byte_position=table.headers.iter().any(|h|matches!(h.trim().to_ascii_lowercase().as_str(),"byte_position"|"byteposition"|"byte_pos"|"start_byte"|"startbyte"|"byte"|"byteindex"));
    for row in 0..table.rows.len(){
        let name=table.col(row,&["signal_name","SignalName","Signal","Name"]);
        let frame=table.col(row,&["message_id","frame_id","FrameID","CAN_ID","CAN ID","ID"]);
        // Completely empty rows are allowed while the user is building the table.
        if table.rows[row].iter().all(|v|v.trim().is_empty()){continue;}
        if name.trim().is_empty(){return Err(format!("Signals row {}: signal_name is required",row+1));}
        if frame.trim().is_empty(){return Err(format!("Signals row {} ({name}): message_id/frame_id is required",row+1));}

        let byte_text=table.col(row,&["byte_position","BytePosition","byte_pos","start_byte","StartByte","Byte","ByteIndex"]);
        let bit_text=table.col(row,&["bit_position","BitPosition","bit_pos","start_bit","StartBit","Bit"]);
        let length_text=table.col(row,&["length","bit_length","BitLength","Length","SignalLength"]);
        let endian=table.col(row,&["endian","Endian","ByteOrder"]);
        let little=parse_little_endian(&endian).ok_or_else(||format!("Signals row {} ({name}): unsupported endian '{}'. Use Intel/Little/LE or Motorola/Big/BE.",row+1,endian))?;
        let default_bit=if little {0} else {7};
        if has_byte_position && !byte_text.trim().is_empty(){
            if byte_text.trim().parse::<usize>().is_err(){return Err(format!("Signals row {} ({name}): byte_position must be a zero-based integer",row+1));}
            let bit=if bit_text.trim().is_empty(){default_bit}else{bit_text.trim().parse::<usize>().map_err(|_|format!("Signals row {} ({name}): bit_position must be 0..7",row+1))?};
            if bit>7{
                return Err(format!("Signals row {} ({name}): bit_position {bit} is invalid when byte_position is present. Use physical Bit 0..7 only.",row+1));
            }
        }
        let length=if length_text.trim().is_empty(){8}else{length_text.trim().parse::<usize>().map_err(|_|format!("Signals row {} ({name}): length must be 1..64 bits",row+1))?};
        if !(1..=64).contains(&length){return Err(format!("Signals row {} ({name}): length {length} is outside 1..64 bits",row+1));}
        let dt=table.col(row,&["data_type","DataType","Data Type","type","Type"]).trim().to_ascii_lowercase();
        let local_bit=if has_byte_position && !byte_text.trim().is_empty(){bit_text.trim().parse::<usize>().unwrap_or(default_bit)}else{default_bit};
        let aligned_bit=if little { 0 } else { 7 };
        if (dt=="float32"||dt=="float 32"||dt=="f32") && (length!=32||local_bit!=aligned_bit){
            return Err(format!("Signals row {} ({name}): Float32 requires Length 32 and {} Bit {}",row+1,if little{"Intel/Little"}else{"Motorola/Big"},aligned_bit));
        }
        if (dt=="float64"||dt=="float 64"||dt=="f64"||dt=="double") && (length!=64||local_bit!=aligned_bit){
            return Err(format!("Signals row {} ({name}): Float64 requires Length 64 and {} Bit {}",row+1,if little{"Intel/Little"}else{"Motorola/Big"},aligned_bit));
        }
    }
    Ok(format!("Signals: {} row(s) validated; byte positions are zero-based, physical bits are 0..7 (Intel aligned = Bit 0, Motorola aligned = Bit 7)",table.rows.len()))
}

fn editable_configuration_table(ui: &mut egui::Ui, title: &str, table: &mut TableData, empty_text: &str, _max_rows: usize, channel_choices:&[String], channel_names:&HashMap<String,String>, show_raw_ids:bool) -> ConfigEditResult {
    let mut result = ConfigEditResult { changed: false, commit_requested:false, saved_path: None, error: None };
    if table.headers.is_empty() {
        ui.horizontal(|ui|{
            ui.label(empty_text);
            if ui.button("+ Add first row").clicked(){
                ensure_configuration_schema(title,table);
                if !table.headers.is_empty(){
                    table.rows.push(vec![String::new();table.headers.len()]);
                    result.changed=true;
                    result.commit_requested=true;
                }
            }
        });
        return result;
    }

    ui.horizontal(|ui| {
        ui.label(format!("{} row(s)", table.rows.len()));
        if !table.filename.is_empty() { ui.weak(format!("- {}", table.filename)); }
        ui.separator();
        ui.weak("Only visible rows are rendered; edits are applied after a field is committed.");
    });
    ui.add_space(4.0);

    let row_count=table.rows.len();
    let row_h=32.0;
    let total_w=40.0 + table.headers.len() as f32 * 146.0;
    let mut delete_row:Option<usize>=None;
    egui::ScrollArea::horizontal()
        .id_salt(format!("config_h_{}_{}",title,table.filename))
        .auto_shrink([false,false])
        .show(ui,|ui|{
            ui.set_min_width(total_w);
            ui.horizontal(|ui|{
                ui.add_sized([34.0,28.0],egui::Label::new(""));
                for h in &table.headers {
                    ui.add_sized([140.0,28.0],egui::Label::new(egui::RichText::new(h).strong()));
                    ui.add_space(6.0);
                }
            });
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt(format!("config_v_{}_{}",title,table.filename))
                .max_height(460.0)
                .auto_shrink([false,false])
                .show_rows(ui,row_h,row_count,|ui,rows|{
                    for row_idx in rows {
                        ui.push_id((title,row_idx),|ui|{
                            ui.horizontal(|ui|{
                                if ui.add_sized([34.0,28.0],egui::Button::new("X")).on_hover_text("Delete row").clicked(){
                                    delete_row=Some(row_idx);
                                }
                                if table.rows[row_idx].len()<table.headers.len(){
                                    table.rows[row_idx].resize(table.headers.len(),String::new());
                                }
                                let endian_col=table.headers.iter().position(|h|matches!(h.trim().to_ascii_lowercase().as_str(),"endian"|"byteorder"));
                                let signal_little_endian=endian_col
                                    .and_then(|i|table.rows[row_idx].get(i))
                                    .map(|v|endian_is_little(v))
                                    .unwrap_or(true);
                                for col_idx in 0..table.headers.len(){
                                    let header=table.headers[col_idx].clone();
                                    let edit=ui.push_id(col_idx,|ui|config_cell_editor(ui,title,&header,&mut table.rows[row_idx][col_idx],channel_choices,channel_names,show_raw_ids,signal_little_endian)).inner;
                                    result.changed|=edit.changed;
                                    result.commit_requested|=edit.commit_requested;
                                    ui.add_space(6.0);
                                }
                            });
                        });
                    }
                });
        });
    if let Some(row)=delete_row {
        table.rows.remove(row);
        result.changed=true;
        result.commit_requested=true;
    }
    result
}

fn save_table_csv(table: &TableData) -> Result<PathBuf, String> {
    let suggested = if table.filename.to_ascii_lowercase().ends_with(".csv") && !table.filename.is_empty() { table.filename.clone() } else { "configuration.csv".into() };
    let Some(path) = rfd::FileDialog::new().add_filter("CSV", &["csv"]).set_file_name(&suggested).save_file() else { return Err("Save cancelled".into()); };
    let mut writer = csv::Writer::from_path(&path).map_err(|e| e.to_string())?;
    writer.write_record(&table.headers).map_err(|e| e.to_string())?;
    for row in &table.rows {
        let values=row.iter().enumerate().map(|(i,v)| {
            if table.headers.get(i).is_some_and(|h|matches!(h.trim().to_ascii_lowercase().as_str(),"channel_id"|"channel"|"channelid"|"channel id")){channel_text_hex(v)}else{v.clone()}
        }).collect::<Vec<_>>();
        writer.write_record(values).map_err(|e|e.to_string())?;
    }
    writer.flush().map_err(|e| e.to_string())?;
    Ok(path)
}

fn apply_config_edit_feedback(result: ConfigEditResult, info: &mut Option<String>, error: &mut Option<String>) {
    if let Some(path) = result.saved_path {
        *info = Some(format!("Saved {}", path.file_name().and_then(|x| x.to_str()).unwrap_or("configuration.csv")));
    }
    if let Some(e) = result.error {
        if e != "Save cancelled" { *error = Some(e); }
    }
}

fn table_preview(ui: &mut egui::Ui, t: &TableData, max: usize) {
    egui::ScrollArea::horizontal().show(ui, |ui| {
        egui::Grid::new(format!("preview_{}", t.filename)).striped(true).show(ui, |ui| {
            for h in &t.headers { ui.strong(h); } ui.end_row();
            for r in t.rows.iter().take(max) { for (i,v) in r.iter().enumerate() {
                let is_channel=t.headers.get(i).is_some_and(|h|matches!(h.trim().to_ascii_lowercase().as_str(),"channel_id"|"channel"|"channelid"|"channel id"));
                ui.label(if is_channel{channel_text_hex(v)}else{v.clone()});
            } ui.end_row(); }
        });
    });
}
fn virtual_frame_table(ui: &mut egui::Ui, frames: &[crate::model::NormalizedFrame], indices: &[usize], channel_names: &BTreeMap<u32, String>, show_raw_ids: bool) {
    let row_h = 22.0;
    egui::ScrollArea::both().auto_shrink([false, false]).max_height(460.0).show(ui, |ui| {
        ui.horizontal(|ui| {
            for (h, w) in [("#", 55.0), ("Timestamp", 188.0), ("Bus", 70.0), ("Channel", 155.0), ("Dir", 45.0), ("ID/Key", 105.0), ("Len", 40.0), ("Payload", 420.0)] { ui.add_sized([w, row_h], egui::Label::new(egui::RichText::new(h).strong())); }
        });
        egui::ScrollArea::vertical().show_rows(ui, row_h, indices.len(), |ui, range| {
            for row in range {
                let f = &frames[indices[row]];
                ui.horizontal(|ui| {
                    ui.add_sized([55.0, row_h], egui::Label::new(f.sequence.to_string()));
                    ui.add_sized([188.0, row_h], egui::Label::new(format_timestamp_ns(f.timestamp_ns))).on_hover_text(format!("Raw: {} ns | Relative: {:.9} s", f.timestamp_ns, f.relative_time_s));
                    ui.add_sized([70.0, row_h], egui::Label::new(f.bus_type.to_string()));
                    let ch = channel_display(f.channel_id,channel_names.get(&f.channel_id).map(String::as_str),show_raw_ids);
                    ui.add_sized([155.0, row_h], egui::Label::new(ch));
                    ui.add_sized([45.0, row_h], egui::Label::new(f.direction.to_string()));
                    ui.add_sized([105.0, row_h], egui::Label::new(frame_key_display(&f.frame_key)));
                    ui.add_sized([40.0, row_h], egui::Label::new(f.payload.len().to_string()));
                    ui.add_sized([420.0, row_h], egui::Label::new(egui::RichText::new(f.payload_hex()).monospace()));
                });
            }
        });
    });
}
fn parse_channel_id_text(s: &str) -> Option<u32> {
    crate::model::parse_channel_id(s)
}


#[derive(Clone, Debug)]
struct TraceRow { idx: usize, count: usize, dt_s: f64 }

fn trace_source_key(f: &crate::model::NormalizedFrame) -> String { format!("{}|{}|{}", f.bus_type, f.channel_id, f.frame_key) }
fn project_channel_key(bus: &str, channel_id: u32) -> String { format!("{}|{}", bus, channel_id) }
fn nearest_overlay_trace(traces:&[(String,Vec<[f64;2]>)],time:f64,value:f64)->Option<usize> {
    traces.iter().enumerate().filter_map(|(index,(_,points))|sample_hold_value(points,time).map(|v|(index,(v-value).abs())))
        .min_by(|a,b|a.1.total_cmp(&b.1)).map(|(index,_)|index)
}

fn graphics_lane_source(lane:&str)->Option<String> {
    if let Some(source)=lane.strip_prefix("FRAME|"){return Some(source.to_string());}
    let (rest,parts)=if let Some(rest)=lane.strip_prefix("SRC|"){(rest,1)}else{(lane.strip_prefix("BIT|")?,2)};
    let mut source=rest;
    for _ in 0..parts {source=source.rsplit_once('|')?.0;}
    parse_graphics_source_key(source)?;Some(source.to_string())
}

fn graphics_source_key(bus: &str, channel_id: u32, frame_key: &str) -> String { format!("{}|{}|{}", bus, channel_id, frame_key) }
fn graphics_eth_stream_key(frame_key:&str)->Option<String> {
    if frame_key.starts_with("ETH:UDP:") {
        return Some(frame_key.to_string());
    }
    if let Some((_pdu,parent))=frame_key.split_once("@UDP:") {
        // Review keys use src:port->dst:port internally; raw ETH keys use >.
        // Canonicalize to the raw ETH stream label so both land under one parent.
        return Some(format!("ETH:UDP:{}",parent.replace("->",">")));
    }
    None
}
fn graphics_frame_display_label(frame_key:&str)->String {
    if frame_key.starts_with("ETH:UDP:") {
        return "ETH/UDP stream".to_string();
    }
    if let Some((pdu,_parent))=frame_key.split_once("@UDP:") {
        return pdu.to_string();
    }
    frame_key_display(frame_key)
}
fn load_completion_summary(warnings:&[String],duration:f64)->String {
    if let Some(warning)=warnings.iter().find(|w|w.starts_with("Safe load limit reached")) {
        format!("Partial log loaded ({duration:.3} s): {warning}")
    }else{format!("Decoded ({duration:.3} s). {}",warnings.first().map(String::as_str).unwrap_or(""))}
}

fn restored_graph_view(start:f64,end:f64,duration:f64,same_clock:bool)->Option<(f64,f64)>{
    if same_clock && start.is_finite() && end.is_finite() && start>=0.0 && start<duration && end>start && end<=duration+0.001 {
        Some((start,end.min(duration)))
    } else {None}
}

fn digital_value_at(d:&Dataset,bus:&str,key:&str,channel:u32,byte:Option<usize>,bit:Option<usize>,time:f64)->Option<f64>{
    let indices=d.index.by_key.get(key)?;
    let end=indices.partition_point(|i|d.frames[*i].relative_time_s<=time);
    indices[..end].iter().rev().find_map(|i|{
        let f=&d.frames[*i];
        if f.channel_id!=channel || f.bus_type.to_string()!=bus{return None;}
        digital_frame_value(f,byte,bit)
    })
}

fn digital_frame_value(f:&crate::model::NormalizedFrame,byte:Option<usize>,bit:Option<usize>)->Option<f64>{
    if f.bus_type==BusType::Analog{return None;}
    if let Some(byte)=byte {
        let v=*f.payload.get(byte)?;
        if let Some(bit)=bit {if bit>=8{return None;}Some(((v>>bit)&1) as f64)}else{Some(v as f64)}
    }else{
        let mut h=0xcbf29ce484222325u64;
        for b in &f.payload{h^=*b as u64;h=h.wrapping_mul(0x100000001b3);}
        Some((h&((1u64<<53)-1)) as f64)
    }
}

fn digital_view_points(d:&Dataset,bus:&str,key:&str,channel:u32,byte:Option<usize>,bit:Option<usize>,start:f64,end:f64)->Vec<[f64;2]>{
    let Some(indices)=d.index.by_key.get(key)else{return Vec::new();};
    // Filter by bus/channel before choosing boundary points: identical frame IDs
    // on another channel must not supply the sample held at the viewport edge.
    let selected=indices.iter().copied().filter(|i|{
        let f=&d.frames[*i];f.channel_id==channel && f.bus_type.to_string()==bus
    }).collect::<Vec<_>>();
    let lo=selected.partition_point(|i|d.frames[*i].relative_time_s<start).saturating_sub(1);
    let hi=(selected.partition_point(|i|d.frames[*i].relative_time_s<=end)+1).min(selected.len());
    if lo>=hi{return Vec::new();}
    let points=selected[lo..hi].iter().filter_map(|i|{
        let f=&d.frames[*i];
        let value=digital_frame_value(f,byte,bit)?;
        Some([f.relative_time_s,value])
    }).collect::<Vec<_>>();
    downsample_minmax(&points,12_000)
}

fn export_axis_range(points:&[[f64;2]])->(f64,f64) {
    let lo=points.iter().map(|p|p[1]).filter(|v|v.is_finite()).fold(f64::INFINITY,f64::min);
    let hi=points.iter().map(|p|p[1]).filter(|v|v.is_finite()).fold(f64::NEG_INFINITY,f64::max);
    if !lo.is_finite(){return (0.0,1.0);}
    let pad=if (hi-lo).abs()<1e-12{(lo.abs()*0.05).max(0.5)}else{(hi-lo)*0.05};(lo-pad,hi+pad)
}

fn held_points_in_view(points:&[[f64;2]],start:f64,end:f64)->Vec<[f64;2]>{
    if end<=start{return Vec::new();}
    let mut visible=Vec::new();
    let lo=points.partition_point(|p|p[0]<start);
    let hi=points.partition_point(|p|p[0]<=end);
    if lo>0{visible.push([start,points[lo-1][1]]);}
    visible.extend_from_slice(&points[lo..hi]);
    if let Some(last)=visible.last().copied(){if last[0]<end {visible.push([end,last[1]]);}}
    visible
}

fn analog_sample_at(d:&Dataset,key:&str,channel:u32,time:f64)->Option<[f64;2]> {
    if !time.is_finite(){return None;}
    let indices=d.index.by_key.get(key)?;
    let end=indices.partition_point(|i|d.frames[*i].relative_time_s<=time);
    let mut best:Option<[f64;2]>=None;
    for &i in &indices[..end] {
        let f=&d.frames[i];
        if f.bus_type!=BusType::Analog || f.channel_id!=channel{continue;}
        let Some(meta)=f.analog.as_ref() else{continue;};
        let count=meta.sample_count.min(f.payload.len()/2);
        if count==0 || meta.sample_period_ns==0{continue;}
        let step=meta.sample_period_ns as f64*1e-9;
        let mut n=(((time-f.relative_time_s)/step+1e-7).floor().max(0.0) as usize).min(count-1);
        let mut t=f.relative_time_s+n as f64*step;
        if t>time+1e-12 {if n==0{continue;} n-=1;t=f.relative_time_s+n as f64*step;}
        let value=i16::from_be_bytes([f.payload[2*n],f.payload[2*n+1]]) as f64*meta.factor;
        if best.is_none_or(|p|t>=p[0]){best=Some([t,value]);}
    }
    best
}

fn original_analog_view_points(d:&Dataset,key:&str,channel:u32,start:f64,end:f64)->Vec<[f64;2]> {
    if !start.is_finite() || !end.is_finite() || end<=start{return Vec::new();}
    let Some(indices)=d.index.by_key.get(key) else{return Vec::new();};
    // Bucket by visible time, directly from original samples. This bounds display
    // memory while allowing a zoom to recover samples omitted by the full-log cache.
    let mut buckets=vec![None::<([f64;2],[f64;2])>;6_000];
    let mut first:Option<[f64;2]>=None;let mut last:Option<[f64;2]>=None;
    let mut after:Option<[f64;2]>=None;
    for &i in indices {
        let f=&d.frames[i];
        if f.bus_type!=BusType::Analog || f.channel_id!=channel{continue;}
        let Some(meta)=f.analog.as_ref() else{continue;};
        let count=meta.sample_count.min(f.payload.len()/2);
        if count==0 || meta.sample_period_ns==0{continue;}
        let step=meta.sample_period_ns as f64*1e-9;
        let point=|n:usize|[f.relative_time_s+n as f64*step,
            i16::from_be_bytes([f.payload[2*n],f.payload[2*n+1]]) as f64*meta.factor];
        let lo=(((start-f.relative_time_s)/step-1e-7).ceil().max(0.0).min(count as f64)) as usize;
        let hi=(((end-f.relative_time_s)/step+1e-7).floor()+1.0).max(0.0).min(count as f64) as usize;
        for n in lo..hi {
            let p=point(n);
            if p[0]<start-1e-12 || p[0]>end+1e-12{continue;}
            if first.is_none_or(|q|p[0]<q[0]){first=Some(p);}
            if last.is_none_or(|q|p[0]>=q[0]){last=Some(p);}
            let bucket=(((p[0]-start)/(end-start)*6_000.0).floor().max(0.0) as usize).min(5_999);
            match &mut buckets[bucket] {
                Some((min,max))=>{if p[1]<min[1]{*min=p;}if p[1]>max[1]{*max=p;}},
                slot@None=>*slot=Some((p,p)),
            }
        }
        if hi<count {
            let p=point(hi);
            if p[0]>end && after.is_none_or(|q|p[0]<q[0]){after=Some(p);}
        }
    }
    let mut out=Vec::with_capacity(12_004);
    if let Some(p)=analog_sample_at(d,key,channel,start){out.push(p);}
    out.extend(first);out.extend(last);out.extend(after);
    for (min,max) in buckets.into_iter().flatten(){out.push(min);if max!=min{out.push(max);}}
    out.sort_by(|a,b|a[0].total_cmp(&b[0]));out.dedup();out
}

fn sample_hold_value(points:&[[f64;2]], t:f64)->Option<f64>{
    if points.is_empty(){return None;}
    let idx=points.partition_point(|p|p[0] <= t);
    if idx==0 { None } else { Some(points[idx-1][1]) }
}

fn parse_graphics_source_key(key: &str) -> Option<(String, u32, String)> {
    let mut parts = key.splitn(3, '|');
    let bus = parts.next()?.to_string();
    let channel = parts.next()?.parse().ok()?;
    let frame = parts.next()?.to_string();
    Some((bus, channel, frame))
}
fn graphics_available_sources(frames: &[crate::model::NormalizedFrame]) -> Vec<(String,String,u32,String,usize)> {
    let mut map: BTreeMap<String,(String,u32,String,usize)> = BTreeMap::new();
    for f in frames {
        let bus = f.bus_type.to_string();
        let key = graphics_source_key(&bus, f.channel_id, &f.frame_key);
        map.entry(key).and_modify(|v| v.3 = v.3.max(f.payload.len())).or_insert((bus, f.channel_id, f.frame_key.clone(), f.payload.len()));
    }
    map.into_iter().map(|(k,(bus,ch,f,dlc))|(k,bus,ch,f,dlc)).collect()
}

fn scoped_payload_points(d:&Dataset,key:&str,channel:Option<u32>,scope:&BTreeSet<String>,byte:usize,bit:Option<usize>)->Vec<[f64;2]>{
    let Some(indices)=d.index.by_key.get(key)else{return Vec::new();};
    let raw=indices.iter().filter_map(|i|{
        let f=&d.frames[*i];
        if channel.is_some_and(|ch|ch!=f.channel_id) || !scope.contains(&project_channel_key(&f.bus_type.to_string(),f.channel_id)){return None;}
        digital_frame_value(f,Some(byte),bit).map(|v|[f.relative_time_s,v])
    }).collect::<Vec<_>>();
    downsample_minmax(&raw,50_000)
}

fn draw_wave_fixed_range(ui:&mut egui::Ui,pts:&[[f64;2]],height:f32,ymin:f64,ymax:f64){
    draw_canoe_lane(ui,"Raw byte",pts,height,f64::NEG_INFINITY,f64::NEG_INFINITY,&[],Some((ymin,ymax)));
}

fn draw_multi_byte_wave(ui: &mut egui::Ui, traces: &[(String, Vec<[f64; 2]>)], height: f32) {
    let desired = Vec2::new(ui.available_width().max(300.0), height);
    let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
    let p = ui.painter_at(rect);
    let dark=ui.visuals().dark_mode;
    p.rect_filled(rect,0.0,if dark{Color32::from_gray(20)}else{Color32::from_gray(248)});
    p.rect_stroke(rect,0.0,Stroke::new(1.0,if dark{Color32::from_gray(62)}else{Color32::from_gray(175)}),egui::StrokeKind::Inside);
    let nonempty = traces.iter().filter(|(_, pts)| pts.len() >= 2).collect::<Vec<_>>();
    if nonempty.is_empty() { return; }
    let xmin = nonempty.iter().filter_map(|(_,p)| p.first().map(|x| x[0])).fold(f64::INFINITY, f64::min);
    let xmax = nonempty.iter().filter_map(|(_,p)| p.last().map(|x| x[0])).fold(f64::NEG_INFINITY, f64::max).max(xmin + 1e-9);
    for (idx, (name, pts)) in nonempty.iter().enumerate() {
        let cv = |q: [f64; 2]| Pos2::new(
            rect.left() + (((q[0] - xmin) / (xmax - xmin)) as f32) * rect.width(),
            rect.bottom() - ((q[1] / 255.0) as f32) * rect.height(),
        );
        let mut prev = cv(pts[0]);
        let color = waveform_color(name,dark);
        for &q in &pts[1..] { let cur = cv(q); p.line_segment([prev, cur], Stroke::new(1.1_f32, color)); prev = cur; }
        p.text(rect.left_top() + Vec2::new(6.0, 6.0 + idx as f32 * 14.0), egui::Align2::LEFT_TOP, name, egui::FontId::monospace(11.0), color);
    }
    p.text(rect.right_top() + Vec2::new(-6.0, 6.0), egui::Align2::RIGHT_TOP, "Raw value 0..255", egui::FontId::monospace(11.0), Color32::GRAY);
}

fn draw_multi_byte_wave_with_cursors(ui: &mut egui::Ui, traces: &[(String, Vec<[f64; 2]>)], height: f32, cursor_a: f64, cursor_b: f64) {
    let desired = Vec2::new(ui.available_width().max(300.0), height);
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click_and_drag());
    let p = ui.painter_at(rect);
    let dark=ui.visuals().dark_mode;
    p.rect_stroke(rect, 0.0, Stroke::new(1.0_f32, Color32::GRAY), egui::StrokeKind::Inside);
    let nonempty = traces.iter().filter(|(_, pts)| pts.len() >= 2).collect::<Vec<_>>();
    if nonempty.is_empty() { return; }
    let xmin = nonempty.iter().filter_map(|(_, pts)| pts.first().map(|x| x[0])).fold(f64::INFINITY, f64::min);
    let xmax = nonempty.iter().filter_map(|(_, pts)| pts.last().map(|x| x[0])).fold(f64::NEG_INFINITY, f64::max).max(xmin + 1e-9);
    for (idx, (name, pts)) in nonempty.iter().enumerate() {
        let cv = |q: [f64; 2]| Pos2::new(
            rect.left() + (((q[0] - xmin) / (xmax - xmin)) as f32) * rect.width(),
            rect.bottom() - ((q[1] / 255.0) as f32) * rect.height(),
        );
        let mut prev = cv(pts[0]);
        let color = waveform_color(name,dark);
        for &q in &pts[1..] { let cur = cv(q); p.line_segment([prev, cur], Stroke::new(1.1_f32, color)); prev = cur; }
        p.text(rect.left_top() + Vec2::new(6.0, 6.0 + idx as f32 * 14.0), egui::Align2::LEFT_TOP, name, egui::FontId::monospace(11.0), color);
    }
    let (a_color,b_color)=graphics_cursor_colors(ui);
    for (label, t, color) in [("A", cursor_a, a_color), ("B", cursor_b, b_color)] {
        if t >= xmin && t <= xmax {
            let x = rect.left() + (((t - xmin) / (xmax - xmin)) as f32) * rect.width();
            p.line_segment([Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())], Stroke::new(1.2_f32, color));
            p.text(Pos2::new(x + 3.0, rect.top() + 3.0), egui::Align2::LEFT_TOP, label, egui::FontId::monospace(11.0), color);
        }
    }
    p.text(rect.right_top() + Vec2::new(-6.0, 6.0), egui::Align2::RIGHT_TOP, "Raw byte 0..255", egui::FontId::monospace(11.0), Color32::GRAY);
    let _ = response;
}

fn parse_f64_any(t: &TableData, row: usize, names: &[&str]) -> Option<f64> {
    let s = t.col(row, names);
    s.trim().parse::<f64>().ok()
}
fn parse_numeric_id(v:&str)->Option<u32>{
    let t=v.trim();
    if let Some(h)=t.strip_prefix("0x").or_else(||t.strip_prefix("0X")){u32::from_str_radix(h,16).ok()}
    else {t.parse::<u32>().ok().or_else(||u32::from_str_radix(t,16).ok())}
}
fn frame_match(key:&str,id:&str)->bool{
    let a=key.trim().to_ascii_lowercase(); let b=id.trim().to_ascii_lowercase();
    if a==b || a.trim_start_matches("0x")==b.trim_start_matches("0x"){return true;}
    if a.starts_with("ext:") || b.starts_with("ext:") {
        let av=a.strip_prefix("ext:").unwrap_or(&a);let bv=b.strip_prefix("ext:").unwrap_or(&b);
        let extended_a=a.starts_with("ext:") || parse_numeric_id(av).is_some_and(|id|id>0x7FF);
        let extended_b=b.starts_with("ext:") || parse_numeric_id(bv).is_some_and(|id|id>0x7FF);
        return extended_a && extended_b && parse_numeric_id(av).zip(parse_numeric_id(bv)).is_some_and(|(a,b)|a==b);
    }
    // REVIEW PDU keys carry @UDP:<parent> for stream isolation. Match configured signals
    // against the PDU portion while retaining the full key internally for indexing/caches.
    let logical=a.split('@').next().unwrap_or(&a);
    if logical==b {return true;}
    let tail=logical.rsplit(|c|c==':'||c=='/'||c=='|').next().unwrap_or(logical);
    if tail==b || tail.trim_start_matches("0x")==b.trim_start_matches("0x"){return true;}
    matches!((parse_numeric_id(tail),parse_numeric_id(&b)),(Some(x),Some(y)) if x==y)
}
fn protocol_token(v:&str)->String{
    let t=v.trim().to_ascii_lowercase().chars().filter(|c|!matches!(*c,' '|'_'|'-')).collect::<String>();
    match t.as_str(){
        "canfd"=>"canfd".into(),
        "automotiveethernet"|"ethernet"|"eth"=>"eth".into(),
        "can"=>"can".into(),
        "lin"=>"lin".into(),
        "analog"=>"analog".into(),
        "tecmp"=>"tecmp".into(),
        "unknown"|"none"=>"unknown".into(),
        _=>t,
    }
}
fn signal_protocol_matches_bus_name(protocol:&str,bus:&str)->bool{
    let p=protocol.trim();
    if p.is_empty() || p=="*" || p.eq_ignore_ascii_case("all"){return true;}
    protocol_token(p)==protocol_token(bus)
}
fn signal_matches_frame(f:&crate::model::NormalizedFrame,s:&SignalDefinition)->bool{
    if !signal_protocol_matches_bus_name(&s.protocol,&f.bus_type.to_string()){return false;}
    if f.frame_key.contains(":ERROR:") || f.frame_key.contains(":DECODE_ISSUE:") || f.frame_key.contains(":RTR:"){return false;}
    if matches!(f.bus_type,BusType::Can|BusType::CanFd) {
        let actual_extended=f.frame_key.starts_with("EXT:") || f.identifier.is_some_and(|id|id>0x7FF);
        let requested_extended=s.frame_id.trim().to_ascii_lowercase().starts_with("ext:") || parse_numeric_id(&s.frame_id).is_some_and(|id|id>0x7FF);
        if actual_extended!=requested_extended{return false;}
    }
    let id_ok=frame_match(&f.frame_key,&s.frame_id)
        || parse_numeric_id(&s.frame_id).zip(f.identifier).map(|(x,y)|x==y).unwrap_or(false);
    if !id_ok{return false;}
    let ch=s.channel.trim();
    if ch.is_empty() || ch=="*" || ch.eq_ignore_ascii_case("all"){return true;}
    parse_numeric_id(ch).map(|x|x==f.channel_id).unwrap_or(false)
}
fn signal_matches_source(bus:&str,frame_key:&str,channel:u32,s:&SignalDefinition)->bool{
    if !signal_protocol_matches_bus_name(&s.protocol,bus){return false;}
    if !frame_match(frame_key,&s.frame_id){return false;}
    let ch=s.channel.trim();
    if ch.is_empty() || ch=="*" || ch.eq_ignore_ascii_case("all"){return true;}
    parse_numeric_id(ch).map(|x|x==channel).unwrap_or(false)
}

fn decoded_series(frames: &[crate::model::NormalizedFrame], s: &SignalDefinition, limit: usize) -> Vec<[f64; 2]> {
    decoded_series_range(frames, s, f64::NEG_INFINITY, f64::INFINITY, limit)
}

fn export_frame_selected(
    f:&crate::model::NormalizedFrame,
    graphics_sources:&BTreeSet<String>,
    graphics_signals:&BTreeSet<String>,
    signals:&[SignalDefinition]
)->bool{
    // Raw source selection: source key is Bus|Channel|FrameKey.
    let raw_selected=graphics_sources.iter().any(|key|{
        parse_graphics_source_key(key).map(|(bus,ch,frame)|bus==f.bus_type.to_string()&&ch==f.channel_id&&frame==f.frame_key).unwrap_or(false)
    });
    if raw_selected{return true;}
    // A selected configured signal also selects its parent frames.
    graphics_signals.iter().any(|name|{
        signals.iter().find(|sig|sig.name==*name)
            .map(|sig|signal_matches_frame(f,sig)).unwrap_or(false)
    })
}

fn decoded_signal_indices(d:&Dataset,s:&SignalDefinition,scope:&BTreeSet<String>)->Vec<usize> {
    let mut selected=Vec::new();
    if scope.is_empty(){return selected;}
    for (key,indices) in &d.index.by_key {
        let candidate=frame_match(key,&s.frame_id) || indices.first().is_some_and(|i|
            parse_numeric_id(&s.frame_id).zip(d.frames[*i].identifier).is_some_and(|(a,b)|a==b));
        if !candidate{continue;}
        selected.extend(indices.iter().copied().filter(|i|{
            let f=&d.frames[*i];scope.contains(&project_channel_key(&f.bus_type.to_string(),f.channel_id)) && signal_matches_frame(f,s)
        }));
    }
    selected.sort_unstable();selected
}
fn decoded_frame_value(f:&crate::model::NormalizedFrame,s:&SignalDefinition)->Option<f64>{
    decode_signal_typed(&f.payload,s.start_byte,s.start_bit,s.bit_length,endian_is_little(&s.endian),s.signed,&s.data_type,s.scale,s.offset).filter(|v|v.is_finite())
}
fn decoded_signal_sample_at(d:&Dataset,indices:&[usize],s:&SignalDefinition,time:f64)->Option<[f64;2]>{
    let end=indices.partition_point(|i|d.frames[*i].relative_time_s<=time);
    indices[..end].iter().rev().find_map(|i|{
        let f=&d.frames[*i];decoded_frame_value(f,s).map(|value|[f.relative_time_s,value])
    })
}
fn decoded_signal_view_points(d:&Dataset,indices:&[usize],s:&SignalDefinition,start:f64,end:f64)->Vec<[f64;2]>{
    let lo=indices.partition_point(|i|d.frames[*i].relative_time_s<start);
    let hi=indices.partition_point(|i|d.frames[*i].relative_time_s<=end);
    if lo>hi{return Vec::new();}
    let point=|i:&usize|{let f=&d.frames[*i];decoded_frame_value(f,s).map(|value|[f.relative_time_s,value])};
    let mut points=Vec::new();
    if let Some(p)=indices[..lo].iter().rev().find_map(point){points.push(p);}
    points.extend(indices[lo..hi].iter().filter_map(point));
    if let Some(p)=indices[hi..].iter().find_map(point){points.push(p);}
    downsample_minmax(&points,12_000)
}

fn decoded_series_range(frames: &[crate::model::NormalizedFrame], s: &SignalDefinition, start: f64, end: f64, limit: usize) -> Vec<[f64; 2]> {
    let little = endian_is_little(&s.endian);
    frames.iter()
        .filter(|f| f.relative_time_s >= start && f.relative_time_s <= end)
        .filter(|f| signal_matches_frame(f,s))
        .filter_map(|f| decode_signal_typed(&f.payload, s.start_byte, s.start_bit, s.bit_length, little, s.signed, &s.data_type, s.scale, s.offset).map(|v| [f.relative_time_s, v]))
        .take(limit)
        .collect()
}
fn first_transition_time(points: &[[f64; 2]]) -> Option<f64> {
    for w in points.windows(2) { if w[0][1] != w[1][1] { return Some(w[1][0]); } }
    None
}
fn draw_canoe_lane(
    ui: &mut egui::Ui,
    label: &str,
    pts: &[[f64; 2]],
    height: f32,
    cursor_a: f64,
    cursor_b: f64,
    events: &[ControlTrigger],
    fixed_range: Option<(f64, f64)>,
) {
    let desired=Vec2::new(ui.available_width().max(420.0),height);
    let (rect,_)=ui.allocate_exact_size(desired,Sense::hover());
    let p=ui.painter_at(rect);
    let dark=ui.visuals().dark_mode;
    let canvas=if dark{Color32::from_gray(20)}else{Color32::from_gray(248)};
    let gutter_bg=if dark{Color32::from_gray(25)}else{Color32::from_gray(240)};
    let border=if dark{Color32::from_gray(62)}else{Color32::from_gray(175)};
    let grid=if dark{Color32::from_gray(42)}else{Color32::from_gray(218)};
    let secondary=if dark{Color32::from_gray(125)}else{Color32::from_gray(90)};
    let marker_color=if dark{Color32::LIGHT_BLUE}else{Color32::from_rgb(0,90,160)};
    let wave_color=waveform_color(label,dark);
    let (a_color,b_color)=graphics_cursor_colors(ui);
    p.rect_filled(rect,0.0,canvas);
    p.rect_stroke(rect,0.0,Stroke::new(1.0,border),egui::StrokeKind::Inside);

    let gutter_w=142.0_f32.min((rect.width()*0.28).max(90.0));
    let gutter=egui::Rect::from_min_max(rect.min,Pos2::new(rect.left()+gutter_w,rect.bottom()));
    let graph=egui::Rect::from_min_max(Pos2::new(gutter.right()+1.0,rect.top()),rect.max);
    p.rect_filled(gutter,0.0,gutter_bg);
    p.line_segment([Pos2::new(gutter.right(),rect.top()),Pos2::new(gutter.right(),rect.bottom())],Stroke::new(1.0,border));
    p.rect_filled(egui::Rect::from_min_size(gutter.left_top()+Vec2::new(8.0,8.0),Vec2::new(8.0,8.0)),1.5,wave_color);
    p.text(gutter.left_top()+Vec2::new(21.0,7.0),egui::Align2::LEFT_TOP,label,egui::FontId::monospace(10.0),ui.visuals().text_color());

    if pts.len()<2 {
        p.text(graph.center(),egui::Align2::CENTER_CENTER,"No samples",egui::FontId::monospace(11.0),secondary);
        return;
    }
    let xmin=pts.first().map(|x|x[0]).unwrap_or(0.0);
    let xmax=pts.last().map(|x|x[0]).unwrap_or(xmin+1.0).max(xmin+1e-9);
    let (ymin,ymax)=fixed_range.unwrap_or_else(||{
        let lo=pts.iter().map(|x|x[1]).fold(f64::INFINITY,f64::min);
        let hi=pts.iter().map(|x|x[1]).fold(f64::NEG_INFINITY,f64::max);
        if (hi-lo).abs()<1e-12{(lo-0.5,hi+0.5)}else{(lo,hi)}
    });
    for i in 1..4 {
        let y=graph.top()+graph.height()*(i as f32/4.0);
        p.line_segment([Pos2::new(graph.left(),y),Pos2::new(graph.right(),y)],Stroke::new(0.7,grid));
    }
    for i in 1..6 {
        let x=graph.left()+graph.width()*(i as f32/6.0);
        p.line_segment([Pos2::new(x,graph.top()),Pos2::new(x,graph.bottom())],Stroke::new(0.7,grid));
    }
    let cv=|q:[f64;2]|Pos2::new(
        graph.left()+(((q[0]-xmin)/(xmax-xmin)) as f32)*graph.width(),
        graph.bottom()-(((q[1]-ymin)/(ymax-ymin).max(1e-12)) as f32)*graph.height()
    );
    for marker in events {
        if marker.time_s>=xmin&&marker.time_s<=xmax {
            let x=graph.left()+(((marker.time_s-xmin)/(xmax-xmin)) as f32)*graph.width();
            p.line_segment([Pos2::new(x,graph.top()),Pos2::new(x,graph.bottom())],Stroke::new(1.0,marker_color));
        }
    }
    let mut prev=cv(pts[0]);
    for &q in &pts[1..] {
        let cur=cv(q);
        let corner=Pos2::new(cur.x,prev.y);
        p.line_segment([prev,corner],Stroke::new(1.7,wave_color));
        p.line_segment([corner,cur],Stroke::new(1.7,wave_color));
        prev=cur;
    }
    for (name,t,color) in [("A",cursor_a,a_color),("B",cursor_b,b_color)] {
        if t>=xmin&&t<=xmax {
            let x=graph.left()+(((t-xmin)/(xmax-xmin)) as f32)*graph.width();
            p.line_segment([Pos2::new(x,graph.top()),Pos2::new(x,graph.bottom())],Stroke::new(2.2,color));
            p.text(Pos2::new(x+3.0,graph.top()+3.0),egui::Align2::LEFT_TOP,name,egui::FontId::monospace(10.0),color);
        }
    }
    p.text(gutter.left_bottom()+Vec2::new(8.0,-7.0),egui::Align2::LEFT_BOTTOM,
        format!("{:.3}..{:.3}",ymin,ymax),egui::FontId::monospace(9.0),secondary);
    p.text(graph.right_bottom()+Vec2::new(-6.0,-4.0),egui::Align2::RIGHT_BOTTOM,
        format!("{:.3}..{:.3}s",xmin,xmax),egui::FontId::monospace(9.0),secondary);
}


fn timestamp_at_relative(capture_start_ns: u64, relative_s: f64) -> u64 {
    capture_start_ns.saturating_add((relative_s.max(0.0) * 1_000_000_000.0).round() as u64)
}

fn format_timestamp_ns(timestamp_ns: u64) -> String {
    // Prefer an absolute UTC date/time when the source looks like Unix epoch nanoseconds.
    // For non-epoch device clocks, fall back to a readable HH:MM:SS.mmm value.
    let secs = (timestamp_ns / 1_000_000_000) as i64;
    let nanos = (timestamp_ns % 1_000_000_000) as u32;
    if secs >= 946_684_800 { // 2000-01-01 UTC: avoids misinterpreting small device-relative counters.
        if let Some(dt) = chrono::DateTime::<chrono::Utc>::from_timestamp(secs, nanos) {
            return dt.format("%Y-%m-%d %H:%M:%S%.3f UTC").to_string();
        }
    }
    let total_s = timestamp_ns as f64 / 1_000_000_000.0;
    let hours = (total_s / 3600.0).floor() as u64;
    let minutes = ((total_s % 3600.0) / 60.0).floor() as u64;
    let seconds = total_s % 60.0;
    format!("{:02}:{:02}:{:06.3}", hours, minutes, seconds)
}

fn format_duration_s(seconds: f64) -> String {
    if seconds.abs() < 0.001 {
        format!("{:.3} us", seconds * 1_000_000.0)
    } else if seconds.abs() < 1.0 {
        format!("{:.3} ms", seconds * 1_000.0)
    } else {
        format!("{:.6} s", seconds)
    }
}

fn clamp_time_view(start: &mut f64, end: &mut f64, full_start: f64, full_end: f64) {
    let full_span = (full_end - full_start).max(0.001);
    let min_span = (full_span / 100_000.0).max(0.000_001);
    let mut span = (*end - *start).abs().clamp(min_span, full_span);
    if *start < full_start { *start = full_start; *end = *start + span; }
    if *end > full_end { *end = full_end; *start = *end - span; }
    if *start < full_start { *start = full_start; }
    if *end <= *start { span = min_span; *end = (*start + span).min(full_end); }
}

fn zoom_time_view(start: &mut f64, end: &mut f64, factor: f64, center: f64, full_start: f64, full_end: f64) {
    let old_span = (*end - *start).max(0.000_001);
    let new_span = (old_span * factor).max(0.000_001).min((full_end - full_start).max(0.001));
    let ratio = ((center - *start) / old_span).clamp(0.0, 1.0);
    *start = center - ratio * new_span;
    *end = *start + new_span;
    clamp_time_view(start, end, full_start, full_end);
}

fn pan_time_view(start: &mut f64, end: &mut f64, fraction: f64, full_start: f64, full_end: f64) {
    let span = (*end - *start).max(0.000_001);
    let shift = span * fraction;
    *start += shift;
    *end += shift;
    clamp_time_view(start, end, full_start, full_end);
}


fn range_changed_source_bytes(frames:&[crate::model::NormalizedFrame], start:f64, end:f64)->BTreeMap<String,BTreeSet<usize>>{
    let mut previous:BTreeMap<String,Vec<u8>>=BTreeMap::new();
    let mut changed:BTreeMap<String,BTreeSet<usize>>=BTreeMap::new();
    // Frames are timestamp sorted by engine::load(). Binary-search the selected
    // analysis interval instead of testing every frame in the capture.
    let lo=frames.partition_point(|f|f.relative_time_s < start);
    let hi=frames.partition_point(|f|f.relative_time_s <= end);
    for f in &frames[lo..hi] {
        let key=graphics_source_key(&f.bus_type.to_string(),f.channel_id,&f.frame_key);
        if f.bus_type==BusType::Analog {
            let mut base:Option<i16>=None; let mut has_change=false;
            for b in f.payload.chunks_exact(2) {
                let v=i16::from_be_bytes([b[0],b[1]]);
                match base {None=>base=Some(v),Some(x) if x!=v=>{has_change=true;break},_=>{}}
            }
            if has_change {changed.entry(key).or_default().insert(0);}
            continue;
        }
        if let Some(prev)=previous.get(&key) {
            let n=prev.len().min(f.payload.len());
            for i in 0..n {
                if prev[i]!=f.payload[i]{changed.entry(key.clone()).or_default().insert(i);}
            }
            if f.payload.len()!=prev.len() {
                for i in n..f.payload.len().max(prev.len()){
                    changed.entry(key.clone()).or_default().insert(i);
                }
            }
        }
        // Compare consecutive samples so a byte that changes away and later returns
        // to the first value is still correctly classified as changing.
        previous.insert(key,f.payload.clone());
    }
    changed
}

fn graphics_cursor_colors(ui:&egui::Ui)->(Color32,Color32){
    if ui.visuals().dark_mode {
        (Color32::from_rgb(255,220,55),Color32::from_rgb(255,105,105))
    } else {
        // High-contrast light-theme cursor colors: amber A and red B.
        (Color32::from_rgb(185,105,0),Color32::from_rgb(190,35,45))
    }
}

fn draw_xviewer_time_ruler(ui:&mut egui::Ui,start:f64,end:f64,first_ns:u64,a:f64,b:f64,events:&[ControlTrigger]){
    let (rect,_)=ui.allocate_exact_size(Vec2::new((ui.available_width()-ui.spacing().scroll.allocated_width()).max(80.0),54.0),Sense::hover());
    let p=ui.painter_at(rect);
    let dark=ui.visuals().dark_mode;
    let bg=if dark{Color32::from_gray(24)}else{Color32::from_gray(248)};
    let secondary=if dark{Color32::from_gray(145)}else{Color32::from_gray(80)};
    let marker_color=if dark{Color32::LIGHT_BLUE}else{Color32::from_rgb(0,90,160)};
    let (cursor_a_color,cursor_b_color)=graphics_cursor_colors(ui);
    p.rect_filled(rect,0.0,bg);
    // v0.52.2: use the exact same horizontal engineering gutter and right inset as
    // draw_canoe_lane_interactive(). The CM ruler must share the waveform graph
    // rectangle, otherwise cursor/event X positions drift by the lane gutter width.
    let gutter_w=58.0_f32;
    let graph=egui::Rect::from_min_max(
        Pos2::new(rect.left()+gutter_w,rect.top()),
        Pos2::new(rect.right()-4.0,rect.bottom())
    );
    let xof=|t:f64|graph.left()+(((t-start)/(end-start).max(1e-12)) as f32)*graph.width();
    p.line_segment([Pos2::new(graph.left(),rect.top()),Pos2::new(graph.left(),rect.bottom())],Stroke::new(0.7,secondary));
    for m in events.iter().filter(|m|m.time_s>=start&&m.time_s<=end){
        let x=xof(m.time_s);
        p.line_segment([Pos2::new(x,rect.top()+16.0),Pos2::new(x,rect.bottom())],Stroke::new(1.0,marker_color));
        let label=p.layout_no_wrap(m.label.replace("CM 0x","CM "),egui::FontId::monospace(9.0),marker_color);
        let label_x=(x+2.0).min(graph.right()-label.size().x).max(graph.left());
        p.galley(Pos2::new(label_x,rect.top()+2.0),label,marker_color);
    }
    for (name,t,color) in [("A",a,cursor_a_color),("B",b,cursor_b_color)] {
        if t>=start&&t<=end {let x=xof(t);p.line_segment([Pos2::new(x,rect.top()),Pos2::new(x,rect.bottom())],Stroke::new(2.0,color));p.text(Pos2::new(x+3.0,rect.top()+17.0),egui::Align2::LEFT_TOP,name,egui::FontId::monospace(10.0),color);}
    }
    let intervals=(graph.width()/100.0).floor().clamp(1.0,5.0) as usize;
    for i in 0..=intervals {
        let f=i as f64/intervals as f64;let t=start+(end-start)*f;let x=graph.left()+graph.width()*(f as f32);
        let anchor=if i==0 {egui::Align2::LEFT_BOTTOM} else if i==intervals {egui::Align2::RIGHT_BOTTOM} else {egui::Align2::CENTER_BOTTOM};
        p.text(Pos2::new(x,rect.bottom()-17.0),anchor,format!("{:.3}s",t),egui::FontId::monospace(8.0),secondary);
        let actual=format_timestamp_ns(timestamp_at_relative(first_ns,t));let clock=actual.split_whitespace().nth(1).unwrap_or(&actual).trim_end_matches(" UTC");
        p.text(Pos2::new(x,rect.bottom()-2.0),anchor,clock,egui::FontId::monospace(8.0),secondary);
    }
}
// Keep descriptive text out of the numeric columns. Every lane has the same
// compact two-line identity and three equal-width measurement columns.
fn draw_cursor_value_card(ui:&mut egui::Ui,lane_id:&str,name:&str,ab:Option<[&str;3]>,multi:&[(f64,String)],height:f32) {
    ui.set_min_width(0.0);
    ui.set_max_width(ui.available_width());
    ui.spacing_mut().item_spacing=Vec2::new(8.0,3.0);
    let parts=name.split(" · ").collect::<Vec<_>>();
    let (title,source)=if parts.len()>=4 {(parts[3..].join(" · "),parts[..3].join(" · "))}else{(name.to_string(),String::new())};
    ui.add(egui::Label::new(egui::RichText::new(title).strong().color(waveform_color(lane_id,ui.visuals().dark_mode))).truncate()).on_hover_text(name);
    if !source.is_empty() {
        ui.add(egui::Label::new(egui::RichText::new(source).small().weak()).truncate()).on_hover_text(name);
    }
    if let Some(values)=ab {
        ui.columns(3,|columns| {
            for ((column,label),value) in columns.iter_mut().zip(["A","B","Δ"]).zip(values) {
                column.add(egui::Label::new(egui::RichText::new(label).small().weak()).truncate());
                column.add(egui::Label::new(egui::RichText::new(value).monospace()).truncate()).on_hover_text(format!("{label}: {value}"));
            }
        });
    } else {
        egui::ScrollArea::vertical().id_salt(("cursor_card",lane_id))
            .max_height((height-44.0).max(24.0)).auto_shrink([false,true])
            .show(ui,|ui| {
                for (idx,(time,value)) in multi.iter().enumerate() {
                    ui.columns(3,|columns| {
                        columns[0].add(egui::Label::new(egui::RichText::new(format!("C{}",idx+1)).strong()).truncate());
                        columns[1].add(egui::Label::new(egui::RichText::new(format!("{time:.6} s")).monospace().small()).truncate()).on_hover_text(format!("{time:.9} s"));
                        columns[2].add(egui::Label::new(egui::RichText::new(value).monospace()).truncate()).on_hover_text(value);
                    });
                }
            });
    }
}

fn graphics_lane_header(bus:&str, channel:&str, frame:&str, detail:&str)->String {
    format!("{} · {} · {} · {}",bus,channel,graphics_frame_display_label(frame),detail)
}

fn graphics_signal_header(signal:&SignalDefinition, channel_label:&impl Fn(u32)->String)->String {
    let channel=if let Some(id)=parse_channel_id_text(&signal.channel) { channel_label(id) }
        else if signal.channel.trim().is_empty() || signal.channel.trim()=="*" || signal.channel.eq_ignore_ascii_case("all") { "All channels".into() }
        else { signal.channel.clone() };
    let bus=if signal.protocol.trim().is_empty() || signal.protocol=="*" { "All protocols" } else { &signal.protocol };
    let detail=if signal.unit.trim().is_empty() {signal.name.clone()} else {format!("{} [{}]",signal.name,signal.unit)};
    graphics_lane_header(bus,&channel,&signal.frame_id,&detail)
}

fn graphics_lane_scroll_area(offset:f32,height:f32)->egui::ScrollArea {
    egui::ScrollArea::vertical()
        .id_salt("graphics_waveform_lane_scroll")
        .vertical_scroll_offset(offset)
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
        .max_height(height)
        .auto_shrink([false,false])
}

fn waveform_interaction(ui: &egui::Ui, response: &egui::Response, rect: egui::Rect, start: &mut f64, end: &mut f64, full_end: f64) {
    // Simple, conflict-free controls:
    // wheel = vertical lane scrolling; Shift+wheel = time pan;
    // Ctrl/Cmd+wheel = pointer-centred X zoom; middle drag = pan.
    if response.dragged_by(egui::PointerButton::Middle) {
        let dx = response.drag_delta().x as f64;
        let span = (*end - *start).max(0.000_001);
        let shift = -dx / rect.width().max(1.0) as f64 * span;
        *start += shift;
        *end += shift;
        clamp_time_view(start, end, 0.0, full_end);
    }
    if response.hovered() {
        let (delta, mods)=ui.ctx().input(|i|(i.raw_scroll_delta,i.modifiers));
        // v0.52.3: plain vertical wheel is deliberately not handled here; it must
        // propagate to the shared parent lane ScrollArea. Only horizontal/modified
        // gestures are consumed as time navigation.
        // Axis separation: native horizontal trackpad motion pans time; native
        // vertical motion is left to the parent lane ScrollArea. Shift+wheel is
        // the mouse fallback for horizontal time navigation. This prevents a
        // left/right gesture from also moving the stacked graphs up/down.
        let horizontal = if delta.x.abs() > delta.y.abs() { delta.x }
            else if mods.shift { delta.y } else { 0.0 };
        let zoom_delta = if mods.ctrl || mods.command {
            if delta.y.abs() >= delta.x.abs() { delta.y } else { delta.x }
        } else { 0.0 };
        if zoom_delta.abs()>0.0 || horizontal.abs()>0.0 {
            ui.ctx().input_mut(|i| { i.smooth_scroll_delta=Vec2::ZERO; });
        }
        if zoom_delta.abs()>0.0 {
            if let Some(pos)=response.hover_pos() {
                let frac=((pos.x-rect.left())/rect.width().max(1.0)).clamp(0.0,1.0) as f64;
                let anchor=*start+frac*(*end-*start);
                let factor=if zoom_delta>0.0{0.82}else{1.22};
                zoom_time_view(start,end,factor,anchor,0.0,full_end);
            }
        } else if horizontal.abs()>0.0 {
            pan_time_view(start,end,-(horizontal as f64)*0.10/120.0,0.0,full_end);
        }
    }
}

/// Stable engineering waveform color derived from the lane identity.
/// The same raw byte/signal keeps the same color across repaint, pan, zoom and session reload.
fn waveform_color(key:&str,dark:bool)->Color32 {
    const DARK_PALETTE:[Color32;12]=[
        Color32::from_rgb(86,180,233), Color32::from_rgb(0,158,115),
        Color32::from_rgb(240,228,66), Color32::from_rgb(230,159,0),
        Color32::from_rgb(204,121,167), Color32::from_rgb(0,114,178),
        Color32::from_rgb(213,94,0), Color32::from_rgb(120,190,110),
        Color32::from_rgb(190,140,240), Color32::from_rgb(90,210,200),
        Color32::from_rgb(245,150,180), Color32::from_rgb(210,210,210),
    ];
    const LIGHT_PALETTE:[Color32;12]=[
        Color32::from_rgb(0,95,160), Color32::from_rgb(0,112,78),
        Color32::from_rgb(150,112,0), Color32::from_rgb(170,90,0),
        Color32::from_rgb(145,70,120), Color32::from_rgb(0,80,145),
        Color32::from_rgb(170,62,0), Color32::from_rgb(55,120,45),
        Color32::from_rgb(112,72,170), Color32::from_rgb(0,125,125),
        Color32::from_rgb(165,65,105), Color32::from_rgb(80,80,80),
    ];
    // FNV-1a makes assignment deterministic and independent of render/order changes.
    let mut h:u64=0xcbf29ce484222325;
    for b in key.as_bytes(){h^=*b as u64;h=h.wrapping_mul(0x100000001b3);}
    let index=(h as usize)%DARK_PALETTE.len();
    if dark { DARK_PALETTE[index] } else { LIGHT_PALETTE[index] }
}


/// Network-type filter used by the channel picker. Mixed CAN/CAN-FD channels appear
/// under both protocol filters but are a single physical channel in the checklist.
fn picker_filter_matches(protocol:&str, filter:&str) -> bool {
    let p=protocol.to_ascii_lowercase();
    if filter=="All" {return true;}
    if start_capture_group(protocol)==StartCaptureGroup::TecmpSystem {return filter=="TECMP";}
    match filter {
        "CAN" => p.contains("can"),
        "CAN FD" => p.contains("can-fd") || p.contains("canfd") || p.contains("can fd") || p.contains("fd"),
        "LIN" => p.contains("lin"),
        "Ethernet" => p.contains("eth"),
        "Analog" => p.contains("analog"),
        "TECMP" => false,
        "Other" => start_capture_group(protocol)==StartCaptureGroup::Other,
        _ => false,
    }
}

#[cfg(test)]
mod channel_picker_ui_tests {
    use super::*;

    #[test]
    fn picker_protocol_filters_keep_mixed_can_and_control_channels_distinct() {
        assert!(picker_filter_matches("CAN/CAN-FD", "CAN"));
        assert!(picker_filter_matches("CAN/CAN-FD", "CAN FD"));
        assert!(picker_filter_matches("CAN-FD", "CAN FD"));
        assert!(!picker_filter_matches("LIN", "CAN"));
        assert!(picker_filter_matches("ETH", "Ethernet"));
        assert!(picker_filter_matches("Analog", "Analog"));
        assert!(picker_filter_matches("TECMP Control Messages", "TECMP"));
        assert!(!picker_filter_matches("TECMP Control Messages", "CAN"));
        assert!(picker_filter_matches("Unknown", "Other"));
        assert!(!picker_filter_matches("Unknown", "Ethernet"));
    }
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum StartCaptureGroup { BusData, TecmpSystem, Other }
impl StartCaptureGroup {
    fn label(self)->&'static str { match self {
        Self::BusData=>"BUS DATA", Self::TecmpSystem=>"TECMP SYSTEM DATA", Self::Other=>"OTHER",
    }}
    fn description(self)->&'static str { match self {
        Self::BusData=>"vehicle-network payload channels",
        Self::TecmpSystem=>"control and status interfaces",
        Self::Other=>"unknown or unsupported capture content",
    }}
}
fn start_capture_group(protocol:&str)->StartCaptureGroup {
    let p=protocol.to_ascii_lowercase();
    if p.contains("control messages") || p.contains("tecmp status") { return StartCaptureGroup::TecmpSystem; }
    if ["can","lin","eth","analog","flexray","gpio","uart"].iter().any(|k|p.contains(k)) { return StartCaptureGroup::BusData; }
    StartCaptureGroup::Other
}

#[derive(Clone, Copy, Debug)]
enum LaneCursorAction { Select, Remove, Arm, Add(f64), DragStart(usize), Move(usize,f64), DragEnd, Clear }

fn draw_graph_lane_header(ui:&mut egui::Ui,lane:&str,label:&str,canonical:&str,active:&mut Option<String>) -> (bool,bool) {
    let mut rename=false;let mut close=false;
    ui.push_id(("graph_header",lane),|ui| {
        ui.horizontal(|ui| {
            graphics_lane_drag_handle(ui,lane,active);ui.separator();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui| {
                close=ui.add_sized([24.0,24.0],egui::Button::new(egui::RichText::new("×").size(18.0))).on_hover_text("Close graph").clicked();
                let response=ui.add(egui::Label::new(egui::RichText::new(label).strong()).truncate().sense(Sense::click())).on_hover_text(format!("{canonical}\nRight-click to rename"));
                if response.double_clicked(){rename=true;}
                response.context_menu(|ui|{if ui.button("Rename signal / graph…").clicked(){rename=true;ui.close();}});
            });
        });
    });
    (rename,close)
}

// Drag begins only on the handle; the entire visible lane is a drop target.
fn graphics_lane_drag_handle(ui:&mut egui::Ui,lane_id:&str,active_lane:&mut Option<String>) -> egui::Response {
    let response=ui.push_id(("lane_drag",lane_id),|ui| {
        ui.add_sized([52.0,24.0],egui::Button::new("Drag").sense(egui::Sense::drag()))
    }).inner.on_hover_text("Hold and drop anywhere on another graph to move above or below it");
    if response.drag_started() { *active_lane=Some(lane_id.to_owned()); }
    if response.contains_pointer() || response.dragged() {
        ui.ctx().set_cursor_icon(if active_lane.is_some() {egui::CursorIcon::Grabbing} else {egui::CursorIcon::Grab});
    }
    response
}

fn graphics_lane_drop_target(ui:&egui::Ui,active:Option<&str>,order:&[String],lanes:&[(String,f32,f32)],viewport:egui::Rect) -> Option<(String,String)> {
    let active=active?;
    let from=order.iter().position(|id|id==active)?;
    let pointer=ui.input(|i|i.pointer.interact_pos())?;
    let visible=viewport.intersect(ui.clip_rect());
    if !visible.contains(pointer) { return None; }
    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    for (target,top,bottom) in lanes {
        let rect=egui::Rect::from_min_max(Pos2::new(viewport.left(),*top),Pos2::new(viewport.right(),*bottom));
        if target==active || !rect.contains(pointer) {continue;}
        let to=order.iter().position(|id|id==target)?;
        let y=if from<to {*bottom} else {*top};
        ui.painter().with_clip_rect(visible).hline(rect.x_range(),y,egui::Stroke::new(3.0,ui.visuals().selection.stroke.color));
        if ui.input(|i|i.pointer.any_released()) {return Some((active.to_owned(),target.clone()));}
        break;
    }
    None
}

fn draw_canoe_lane_interactive(
    ui: &mut egui::Ui,
    label: &str,
    color_key: &str,
    pts: &[[f64; 2]],
    height: f32,
    cursor_a: &mut f64,
    cursor_b: &mut f64,
    drag_cursor: &mut Option<char>,
    active_cursor: &mut char,
    unit: &str,
    playhead: f64,
    events: &[ControlTrigger],
    fixed_range: Option<(f64, f64)>,
    auto_y: bool,
    view_start: &mut f64,
    view_end: &mut f64,
    full_end: f64,
    y_scale: &mut f64,
    y_offset: &mut f64,
    density: GraphicsDensity,
    range_start: &mut Option<f64>,
    range_end: &mut Option<f64>,
    range_mode: &mut bool,
    range_interaction: &mut u8,
    range_last_pointer_s: &mut Option<f64>,
    use_colors: bool,
    show_grid: bool,
    show_markers: bool,
    show_cursor_labels: bool,
    graph_line_width: f32,
    multi_cursor_mode: bool,
    multi_cursor_armed: bool,
    multi_cursors: &[f64],
    multi_drag_index: Option<usize>,
) -> Option<LaneCursorAction> {
    clamp_time_view(view_start, view_end, 0.0, full_end);
    let wave_color=if use_colors { waveform_color(color_key,ui.visuals().dark_mode) } else if ui.visuals().dark_mode { Color32::LIGHT_GREEN } else { Color32::from_rgb(0,105,65) };
    let (cursor_a_color,cursor_b_color)=graphics_cursor_colors(ui);
    let mut multi_action=None;
    if multi_cursor_mode {
        ui.horizontal(|ui| {
            let count=multi_cursors.len();
            if count < 8 {
                let text=if multi_cursor_armed { format!("Click graph to place C{}",count+1) } else { "+ Cursor".to_string() };
                if ui.small_button(text).clicked() { multi_action=Some(LaneCursorAction::Arm); }
            } else { ui.weak("8 cursors"); }
            if count>0 && ui.small_button("Clear").clicked() { multi_action=Some(LaneCursorAction::Clear); }
            ui.weak(format!("{} cursor{}",count,if count==1{""}else{"s"}));
        });
    }
    let desired = Vec2::new(ui.available_width().max(80.0), height);
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click_and_drag());
    // Fixed engineering readout gutter + waveform canvas.
    // Pointer-to-time conversion must use this same graph rectangle.
    let gutter_w=58.0_f32; // XViewer-style scale gutter; metadata is outside the plot.
    let graph=egui::Rect::from_min_max(
        Pos2::new(rect.left()+gutter_w,rect.top()+4.0),
        Pos2::new(rect.right()-4.0,rect.bottom()-4.0)
    );
    waveform_interaction(ui, &response, graph, view_start, view_end, full_end);

    // Persistent XViewer-style analysis window.
    // 1=create, 2=resize left, 3=resize right, 4=move complete window.
    let time_at_x=|x:f32| -> f64 {
        *view_start
            + ((x-graph.left())/graph.width().max(1.0)).clamp(0.0,1.0) as f64
                * (*view_end-*view_start)
    };
    let x_at_time=|t:f64| -> f32 {
        graph.left()+(((t-*view_start)/(*view_end-*view_start).max(1e-12)) as f32)*graph.width()
    };

    if response.drag_started_by(egui::PointerButton::Primary) {
        if let Some(pos)=response.interact_pointer_pos() {
            if graph.contains(pos) {
                let t=time_at_x(pos.x);
                // Analysis-window editing is EXPLICITLY gated by Select Range mode.
                // A/B cursor interaction must never implicitly enter, resize, or move
                // the analysis range merely because the pointer is over that range.
                if *range_mode {
                    if let (Some(a),Some(b))=(*range_start,*range_end) {
                        let lo=a.min(b); let hi=a.max(b);
                        let xl=x_at_time(lo); let xr=x_at_time(hi);
                        let handle_px=14.0_f32;
                        if (pos.x-xl).abs()<=handle_px {
                            *range_interaction=2;
                        } else if (pos.x-xr).abs()<=handle_px {
                            *range_interaction=3;
                        } else if pos.x>xl && pos.x<xr {
                            *range_interaction=4;
                        } else {
                            // In Select Range mode, dragging outside the existing
                            // selection starts a new independent analysis region.
                            *range_start=Some(t); *range_end=Some(t); *range_interaction=1;
                        }
                    } else {
                        *range_start=Some(t); *range_end=Some(t); *range_interaction=1;
                    }
                    if *range_interaction!=0 {
                        *range_last_pointer_s=Some(t);
                    }
                }
            }
        }
    }

    if *range_interaction!=0 && response.dragged_by(egui::PointerButton::Primary) {
        if let Some(pos)=response.interact_pointer_pos() {
            let t=time_at_x(pos.x).clamp(0.0,full_end);
            match *range_interaction {
                1 => { *range_end=Some(t); }
                2 => {
                    if let Some(b)=*range_end { let min_span=((*view_end-*view_start)*0.002).max(0.000_001); *range_start=Some(t.min(b-min_span).max(0.0)); }
                }
                3 => {
                    if let Some(a)=*range_start { let min_span=((*view_end-*view_start)*0.002).max(0.000_001); *range_end=Some(t.max(a+min_span).min(full_end)); }
                }
                4 => {
                    if let (Some(a),Some(b),Some(prev))=(*range_start,*range_end,*range_last_pointer_s) {
                        let lo=a.min(b); let hi=a.max(b); let span=hi-lo;
                        let mut nlo=lo+(t-prev);
                        nlo=nlo.clamp(0.0,(full_end-span).max(0.0));
                        *range_start=Some(nlo); *range_end=Some((nlo+span).min(full_end));
                    }
                }
                _ => {}
            }
            *range_last_pointer_s=Some(t);
        }
    }

    if *range_interaction!=0 && response.drag_stopped_by(egui::PointerButton::Primary) {
        if let (Some(a),Some(b))=(*range_start,*range_end) {
            let lo=a.min(b); let hi=a.max(b);
            let min_span=((*view_end-*view_start)*0.002).max(0.000_001).min(full_end.max(0.000_001));
            if (hi-lo)<min_span {
                let center=(lo+hi)*0.5;
                let mut nlo=(center-min_span*0.5).max(0.0);
                if nlo+min_span>full_end { nlo=(full_end-min_span).max(0.0); }
                *range_start=Some(nlo);
                *range_end=Some((nlo+min_span).min(full_end));
            } else {
                *range_start=Some(lo);
                *range_end=Some(hi);
            }
        }
        *range_interaction=0;
        *range_last_pointer_s=None;
        *range_mode=false;
    }
    if response.double_clicked() {
        *view_start=0.0; *view_end=full_end;
    }
    if let (Some(a),Some(b))=(*range_start,*range_end) {
        let lo=a.min(b).max(*view_start); let hi=a.max(b).min(*view_end);
        if hi>lo {
            let x0=graph.left()+(((lo-*view_start)/(*view_end-*view_start).max(1e-12)) as f32)*graph.width();
            let x1=graph.left()+(((hi-*view_start)/(*view_end-*view_start).max(1e-12)) as f32)*graph.width();
            ui.painter().rect_filled(egui::Rect::from_min_max(Pos2::new(x0,graph.top()),Pos2::new(x1,graph.bottom())),0.0,Color32::from_rgba_unmultiplied(90,120,170,28));
            let rp=ui.painter();
            // Strong persistent [ ... ] analysis-window visualization.
            let hc=Color32::from_rgb(80,190,220);
            let fill=Color32::from_rgba_unmultiplied(80,190,220,42);
            let sel=egui::Rect::from_min_max(Pos2::new(x0,graph.top()),Pos2::new(x1,graph.bottom()));
            rp.rect_filled(sel,2.0,fill);
            rp.line_segment([Pos2::new(x0,graph.top()),Pos2::new(x0,graph.bottom())],Stroke::new(3.0,hc));
            rp.line_segment([Pos2::new(x1,graph.top()),Pos2::new(x1,graph.bottom())],Stroke::new(3.0,hc));
            let hw=6.0;
            rp.rect_filled(egui::Rect::from_center_size(Pos2::new(x0,graph.center().y),Vec2::new(hw,30.0)),2.0,hc);
            rp.rect_filled(egui::Rect::from_center_size(Pos2::new(x1,graph.center().y),Vec2::new(hw,30.0)),2.0,hc);
            if x1-x0>42.0 {
                let cx=(x0+x1)*0.5;
                rp.line_segment([Pos2::new(cx-8.0,graph.center().y),Pos2::new(cx+8.0,graph.center().y)],Stroke::new(3.0,hc));
                rp.line_segment([Pos2::new(cx-8.0,graph.center().y),Pos2::new(cx-3.0,graph.center().y-4.0)],Stroke::new(2.0,hc));
                rp.line_segment([Pos2::new(cx-8.0,graph.center().y),Pos2::new(cx-3.0,graph.center().y+4.0)],Stroke::new(2.0,hc));
                rp.line_segment([Pos2::new(cx+8.0,graph.center().y),Pos2::new(cx+3.0,graph.center().y-4.0)],Stroke::new(2.0,hc));
                rp.line_segment([Pos2::new(cx+8.0,graph.center().y),Pos2::new(cx+3.0,graph.center().y+4.0)],Stroke::new(2.0,hc));
            }
        }
    }
    let time_from_x=|x:f32| -> f64 {*view_start+((x-graph.left())/graph.width().max(1.0)).clamp(0.0,1.0) as f64*(*view_end-*view_start)};
    let x_from_time=|t:f64| -> f32 {graph.left()+(((t-*view_start)/(*view_end-*view_start).max(1e-12)) as f32)*graph.width()};
    // v0.53.4: per-waveform cursors are draggable. The hit target is the cursor line/handle,
    // and only the cursor belonging to this stable lane id is updated.
    if multi_cursor_mode && *range_interaction==0 && !*range_mode {
        if response.hovered() && ui.ctx().input(|i|i.pointer.primary_pressed()) {
            if let Some(pos)=response.interact_pointer_pos().or_else(||response.hover_pos()) {
                if graph.contains(pos) {
                    let mut nearest:Option<(usize,f32)>=None;
                    for (idx,t) in multi_cursors.iter().enumerate() {
                        let dx=(pos.x-x_from_time(*t)).abs();
                        if dx<=20.0 && nearest.map(|(_,best)|dx<best).unwrap_or(true) { nearest=Some((idx,dx)); }
                    }
                    if let Some((idx,_))=nearest { multi_action=Some(LaneCursorAction::DragStart(idx)); }
                }
            }
        }
        if response.dragged_by(egui::PointerButton::Primary) {
            if let (Some(idx),Some(pos))=(multi_drag_index,response.interact_pointer_pos()) {
                multi_action=Some(LaneCursorAction::Move(idx,time_from_x(pos.x).clamp(0.0,full_end)));
            }
        }
        if response.drag_stopped_by(egui::PointerButton::Primary) && multi_drag_index.is_some() {
            multi_action=Some(LaneCursorAction::DragEnd);
        }
        if let Some(pos)=response.hover_pos() {
            if multi_cursors.iter().any(|t|(pos.x-x_from_time(*t)).abs()<=20.0) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
        }
    }

    if multi_cursor_mode && multi_cursor_armed && *range_interaction==0 && !*range_mode && response.clicked_by(egui::PointerButton::Primary) {
        if let Some(pos)=response.interact_pointer_pos() {
            if graph.contains(pos) && multi_cursors.len()<8 {
                multi_action=Some(LaneCursorAction::Add(time_from_x(pos.x).clamp(0.0,full_end)));
            }
        }
    }
    if !multi_cursor_mode && *range_interaction==0 && !*range_mode && response.hovered() && ui.ctx().input(|i|i.pointer.primary_pressed()) {
        if let Some(pos)=response.hover_pos() {
            let da=(pos.x-x_from_time(*cursor_a)).abs(); let db=(pos.x-x_from_time(*cursor_b)).abs();
            let (a_key,b_key)=ui.ctx().input(|i|(i.key_down(egui::Key::A),i.key_down(egui::Key::B)));
            if a_key {
                *active_cursor='A'; *cursor_a=time_from_x(pos.x).clamp(0.0,full_end); *drag_cursor=Some('A');
            } else if b_key {
                *active_cursor='B'; *cursor_b=time_from_x(pos.x).clamp(0.0,full_end); *drag_cursor=Some('B');
            } else if da<=12.0||db<=12.0 {
                let which=if da<=db{'A'}else{'B'}; *active_cursor=which; *drag_cursor=Some(which);
            }
        }
    }
    if !multi_cursor_mode && *range_interaction==0 && !*range_mode && response.hovered() && ui.ctx().input(|i|i.pointer.primary_down()) {
        if let(Some(which),Some(pos))=(*drag_cursor,response.hover_pos()){let t=time_from_x(pos.x).clamp(0.0,full_end);if which=='A'{*cursor_a=t}else{*cursor_b=t}}
    }
    if ui.ctx().input(|i|i.pointer.primary_released()){
        // Keep the exact cursor timestamp selected by the user. Signal values are
        // resolved sample-and-hold at that time; cursor position itself never snaps.
        *drag_cursor=None;
    }
    let p = ui.painter_at(rect);
    let dark=ui.visuals().dark_mode;
    let canvas_bg=if dark{Color32::from_gray(20)}else{Color32::from_gray(248)};
    let gutter_bg=if dark{Color32::from_gray(25)}else{Color32::from_gray(240)};
    let border=if dark{Color32::from_gray(62)}else{Color32::from_gray(175)};
    let grid=if dark{Color32::from_gray(42)}else{Color32::from_gray(218)};
    let secondary=if dark{Color32::from_gray(125)}else{Color32::from_gray(90)};
    p.rect_filled(rect,0.0,canvas_bg);
    let gutter=egui::Rect::from_min_max(rect.min,Pos2::new(graph.left()-1.0,rect.bottom()));
    p.rect_filled(gutter,0.0,gutter_bg);
    p.line_segment([Pos2::new(graph.left()-1.0,rect.top()),Pos2::new(graph.left()-1.0,rect.bottom())],Stroke::new(1.0,border));
    p.rect_stroke(rect,0.0,Stroke::new(1.0_f32,border),egui::StrokeKind::Inside);
    let visible = held_points_in_view(pts,*view_start,*view_end);
    if visible.len() < 2 {
        p.text(rect.center(), egui::Align2::CENTER_CENTER, "No samples in visible range", egui::FontId::monospace(11.0), Color32::GRAY);
        return None;
    }
    let (ymin, ymax) = fixed_range.unwrap_or_else(|| {
        // Visible-window Auto Y: never rescan/redecode the capture. Both `pts` and
        // `visible` are derived from the already cached waveform. With Auto Y off,
        // keep a stable full-series engineering range while the user pans in X.
        let y_source: &[[f64;2]] = if auto_y { visible.as_slice() } else { pts };
        let lo = y_source.iter().map(|x| x[1]).fold(f64::INFINITY, f64::min);
        let hi = y_source.iter().map(|x| x[1]).fold(f64::NEG_INFINITY, f64::max);
        if (hi - lo).abs() < 1e-12 {
            // Constant traces still need a useful engineering-scale lane.
            let pad=(lo.abs()*0.05).max(0.5);
            (lo-pad, hi+pad)
        } else {
            let pad=(hi-lo)*0.05;
            (lo-pad, hi+pad)
        }
    });
    let base_mid=(ymin+ymax)*0.5;
    let base_span=(ymax-ymin).max(1e-12);
    let shown_span=base_span / (*y_scale).max(0.05);
    let shown_mid=base_mid + *y_offset * base_span;
    let ymin=shown_mid-shown_span*0.5;
    let ymax=shown_mid+shown_span*0.5;
    // Shared CANoe-like grid. All lanes use the same X view, while Y remains lane-local.
    if show_grid {
        for i in 0..=10 {
            let frac=i as f32/10.0; let x=graph.left()+graph.width()*frac;
            p.line_segment([Pos2::new(x,graph.top()),Pos2::new(x,graph.bottom())],Stroke::new(0.5,grid));
            if i%2==0 { let t=*view_start+(*view_end-*view_start)*(frac as f64);
                p.text(Pos2::new(x,graph.bottom()-2.0),egui::Align2::CENTER_BOTTOM,format!("{:.3}",t),egui::FontId::monospace(8.0),secondary); }
        }
        for i in 0..=4 {
            let y=graph.top()+graph.height()*(i as f32/4.0);
            p.line_segment([Pos2::new(graph.left(),y),Pos2::new(graph.right(),y)],Stroke::new(0.5,grid));
        }
    }
    p.text(Pos2::new(graph.left()-7.0,graph.top()+2.0),egui::Align2::RIGHT_TOP,format!("{:.3}",ymax),egui::FontId::monospace(9.0),Color32::GRAY);
    p.text(Pos2::new(graph.left()-7.0,graph.bottom()-2.0),egui::Align2::RIGHT_BOTTOM,format!("{:.3}",ymin),egui::FontId::monospace(9.0),Color32::GRAY);
    let cv = |q: [f64; 2]| Pos2::new(
        graph.left() + (((q[0] - *view_start) / (*view_end - *view_start).max(1e-12)) as f32) * graph.width(),
        graph.bottom() - (((q[1] - ymin) / (ymax - ymin).max(1e-12)) as f32) * graph.height(),
    );
    if show_markers {
        for marker in events {
            let t=marker.time_s;
            if t >= *view_start && t <= *view_end {
                let x = graph.left() + (((t - *view_start) / (*view_end - *view_start)) as f32) * graph.width();
                p.line_segment([Pos2::new(x, graph.top()), Pos2::new(x, graph.bottom())], Stroke::new(1.0, Color32::from_gray(105)));
            }
        }
    }
    // CANoe/XViewer-style sample-and-hold rendering: automotive bus values stay
    // constant until the next received sample rather than being diagonally interpolated.
    let mut prev = cv(visible[0]);
    for &q in &visible[1..] {
        let cur = cv(q);
        let corner=Pos2::new(cur.x,prev.y);
        p.line_segment([prev,corner],Stroke::new(graph_line_width,wave_color));
        p.line_segment([corner,cur],Stroke::new(graph_line_width,wave_color));
        prev=cur;
    }
    let global_cursor_marks:Vec<(&str,f64,Color32)>=if multi_cursor_mode {
        vec![(">",playhead,if ui.visuals().dark_mode{Color32::LIGHT_BLUE}else{Color32::from_rgb(0,90,160)})]
    } else {
        vec![("A",*cursor_a,cursor_a_color),("B",*cursor_b,cursor_b_color),(">",playhead,if ui.visuals().dark_mode{Color32::LIGHT_BLUE}else{Color32::from_rgb(0,90,160)})]
    };
    for (name,t,color) in global_cursor_marks {
        if t>=*view_start && t<=*view_end {
            let x=graph.left()+(((t-*view_start)/(*view_end-*view_start)) as f32)*graph.width();
            let active=(name=="A"&&*active_cursor=='A')||(name=="B"&&*active_cursor=='B');
            p.line_segment([Pos2::new(x,graph.top()),Pos2::new(x,graph.bottom())],Stroke::new(if active{3.0}else if name==">"{1.5}else{2.0},color));
            if name!=">" {
                let handle=vec![Pos2::new(x-6.0,graph.top()),Pos2::new(x+6.0,graph.top()),Pos2::new(x,graph.top()+9.0)];
                p.add(egui::Shape::convex_polygon(handle,color,Stroke::NONE));
                if show_cursor_labels {
                    p.text(Pos2::new(x+4.0,graph.top()+10.0),egui::Align2::LEFT_TOP,name,egui::FontId::monospace(10.0),color);
                }
            }

        }
    }
    if multi_cursor_mode {
        let colors=[Color32::YELLOW,Color32::LIGHT_RED,Color32::LIGHT_BLUE,Color32::LIGHT_GREEN,Color32::GOLD,Color32::from_rgb(204,121,167),Color32::from_rgb(90,210,200),Color32::from_rgb(230,159,0)];
        for (idx,t) in multi_cursors.iter().enumerate() {
            if *t>=*view_start && *t<=*view_end {
                let x=graph.left()+(((*t-*view_start)/(*view_end-*view_start).max(1e-12)) as f32)*graph.width();
                let color=colors[idx%colors.len()];
                p.line_segment([Pos2::new(x,graph.top()),Pos2::new(x,graph.bottom())],Stroke::new(2.0,color));
                let handle=vec![Pos2::new(x-6.0,graph.top()),Pos2::new(x+6.0,graph.top()),Pos2::new(x,graph.top()+9.0)];
                p.add(egui::Shape::convex_polygon(handle,color,Stroke::NONE));
                p.text(Pos2::new(x+4.0,graph.top()+10.0),egui::Align2::LEFT_TOP,format!("C{}",idx+1),egui::FontId::monospace(10.0),color);
            }
        }
    }
    // XViewer-style plot canvas: cursor/statistics/decoded metadata are kept in the
    // persistent Measurement pane rather than painted over the waveform.
    let _ = (unit, density);
    if multi_action.is_none() && response.clicked(){multi_action=Some(LaneCursorAction::Select);}
    multi_action

}

fn draw_graphics_overview(ui:&mut egui::Ui,full_end:f64,view_start:&mut f64,view_end:&mut f64,playhead:f64){
    // Explicit horizontal time scrollbar. The thumb width represents the visible
    // fraction of the capture; dragging it pans all stacked graphs together.
    clamp_time_view(view_start,view_end,0.0,full_end);
    ui.horizontal(|ui|{
        if ui.small_button("◀").on_hover_text("Pan time left").clicked(){pan_time_view(view_start,view_end,-0.10,0.0,full_end);}
        let desired=Vec2::new((ui.available_width()-30.0).max(260.0),24.0);
        let (rect,response)=ui.allocate_exact_size(desired,Sense::click_and_drag());
        let p=ui.painter_at(rect);
        let dark=ui.visuals().dark_mode;
        let bg=if dark{Color32::from_gray(25)}else{Color32::from_gray(245)};
        let border=if dark{Color32::from_gray(75)}else{Color32::from_gray(170)};
        let thumb_color=if dark{Color32::from_gray(72)}else{Color32::from_gray(205)};
        p.rect_filled(rect,3.0,bg);
        p.rect_stroke(rect,3.0,Stroke::new(1.0,border),egui::StrokeKind::Inside);
        let span=(*view_end-*view_start).max(0.000_001).min(full_end);
        let frac=(span/full_end.max(1e-9)).clamp(0.0,1.0);
        let thumb_w=(rect.width()*frac as f32).max(28.0).min(rect.width());
        let travel=(rect.width()-thumb_w).max(0.0);
        let max_start=(full_end-span).max(0.0);
        let start_frac=if max_start>0.0{(*view_start/max_start).clamp(0.0,1.0)}else{0.0};
        let x0=rect.left()+travel*start_frac as f32;
        let thumb=egui::Rect::from_min_size(Pos2::new(x0,rect.top()+3.0),Vec2::new(thumb_w,rect.height()-6.0));
        p.rect_filled(thumb,3.0,thumb_color);
        p.rect_stroke(thumb,3.0,Stroke::new(1.2,Color32::LIGHT_BLUE),egui::StrokeKind::Inside);
        let px=rect.left()+((playhead/full_end.max(1e-9)).clamp(0.0,1.0) as f32)*rect.width();
        p.line_segment([Pos2::new(px,rect.top()+2.0),Pos2::new(px,rect.bottom()-2.0)],Stroke::new(1.0,Color32::LIGHT_BLUE));
        if (response.dragged()||response.clicked()) && response.interact_pointer_pos().is_some() && max_start>0.0 {
            let pos=response.interact_pointer_pos().unwrap();
            let f=((pos.x-rect.left()-thumb_w*0.5)/travel.max(1.0)).clamp(0.0,1.0) as f64;
            *view_start=f*max_start; *view_end=(*view_start+span).min(full_end);
        }
        if ui.small_button("▶").on_hover_text("Pan time right").clicked(){pan_time_view(view_start,view_end,0.10,0.0,full_end);}
    });
    ui.horizontal(|ui|{
        ui.small(format!("Time {:.3} s",*view_start));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{ui.small(format!("{:.3} s",*view_end));});
    });
}

fn draw_multi_byte_wave_interactive(
    ui: &mut egui::Ui,
    traces: &[(String, Vec<[f64; 2]>)],
    height: f32,
    cursor_a: f64,
    cursor_b: f64,
    playhead: f64,
    view_start: &mut f64,
    view_end: &mut f64,
    full_end: f64,
    use_colors: bool,
    show_grid: bool,
    show_cursor_labels: bool,
    graph_line_width: f32,
) -> Option<(usize,bool)> {
    clamp_time_view(view_start,view_end,0.0,full_end);
    let desired=Vec2::new(ui.available_width().max(420.0),height);
    let (rect,response)=ui.allocate_exact_size(desired,Sense::click_and_drag());
    let gutter_w=(rect.width()*0.18).clamp(125.0,185.0);
    let graph=egui::Rect::from_min_max(Pos2::new(rect.left()+gutter_w,rect.top()+4.0),Pos2::new(rect.right()-4.0,rect.bottom()-4.0));
    waveform_interaction(ui,&response,graph,view_start,view_end,full_end);
    let p=ui.painter_at(rect);
    let dark=ui.visuals().dark_mode;
    let canvas=if dark{Color32::from_gray(20)}else{Color32::from_gray(248)};
    let gutter_bg=if dark{Color32::from_gray(25)}else{Color32::from_gray(240)};
    let border=if dark{Color32::from_gray(62)}else{Color32::from_gray(175)};
    let grid=if dark{Color32::from_gray(42)}else{Color32::from_gray(218)};
    let secondary=if dark{Color32::from_gray(125)}else{Color32::from_gray(90)};
    let (a_color,b_color)=graphics_cursor_colors(ui);
    p.rect_filled(rect,0.0,canvas);
    let gutter=egui::Rect::from_min_max(rect.min,Pos2::new(graph.left()-1.0,rect.bottom()));
    p.rect_filled(gutter,0.0,gutter_bg);
    p.line_segment([Pos2::new(graph.left()-1.0,rect.top()),Pos2::new(graph.left()-1.0,rect.bottom())],Stroke::new(1.0,border));
    p.rect_stroke(rect,0.0,Stroke::new(1.0,border),egui::StrokeKind::Inside);

    if show_grid {
        for i in 0..=10 {
            let frac=i as f32/10.0; let x=graph.left()+graph.width()*frac;
            p.line_segment([Pos2::new(x,graph.top()),Pos2::new(x,graph.bottom())],Stroke::new(0.5,grid));
            if i%2==0 {
                let t=*view_start+(*view_end-*view_start)*(frac as f64);
                p.text(Pos2::new(x,graph.bottom()-2.0),egui::Align2::CENTER_BOTTOM,format!("{:.3}",t),egui::FontId::monospace(8.0),secondary);
            }
        }
        for i in 0..=4 {
            let y=graph.top()+graph.height()*(i as f32/4.0);
            p.line_segment([Pos2::new(graph.left(),y),Pos2::new(graph.right(),y)],Stroke::new(0.5,grid));
        }
    }
    p.text(gutter.left_top()+Vec2::new(8.0,7.0),egui::Align2::LEFT_TOP,"Overlay",egui::FontId::monospace(10.0),ui.visuals().text_color());
    p.text(gutter.left_bottom()+Vec2::new(8.0,-7.0),egui::Align2::LEFT_BOTTOM,"0 .. 255",egui::FontId::monospace(9.0),secondary);

    for (idx,(name,pts)) in traces.iter().enumerate() {
        let visible=held_points_in_view(pts,*view_start,*view_end);
        if visible.len()<2{continue;}
        let cv=|q:[f64;2]|Pos2::new(
            graph.left()+(((q[0]-*view_start)/(*view_end-*view_start).max(1e-12)) as f32)*graph.width(),
            graph.bottom()-((q[1]/255.0).clamp(0.0,1.0) as f32)*graph.height()
        );
        let color=if use_colors{waveform_color(name,dark)}else if dark{Color32::LIGHT_GREEN}else{Color32::from_rgb(0,105,65)};
        let mut prev=cv(visible[0]);
        for &q in &visible[1..] {
            let cur=cv(q); let corner=Pos2::new(cur.x,prev.y);
            p.line_segment([prev,corner],Stroke::new(graph_line_width,color));
            p.line_segment([corner,cur],Stroke::new(graph_line_width,color));
            prev=cur;
        }
        p.rect_filled(egui::Rect::from_min_size(gutter.left_top()+Vec2::new(8.0,25.0+idx as f32*15.0),Vec2::new(7.0,7.0)),1.0,color);
        p.text(gutter.left_top()+Vec2::new(20.0,22.0+idx as f32*15.0),egui::Align2::LEFT_TOP,name,egui::FontId::monospace(9.0),color);
    }

    for (name,t,color) in [("A",cursor_a,a_color),("B",cursor_b,b_color),(">",playhead,if dark{Color32::LIGHT_BLUE}else{Color32::from_rgb(0,90,160)})] {
        if t>=*view_start&&t<=*view_end {
            let x=graph.left()+(((t-*view_start)/(*view_end-*view_start).max(1e-12)) as f32)*graph.width();
            p.line_segment([Pos2::new(x,graph.top()),Pos2::new(x,graph.bottom())],Stroke::new(if name==">"{1.5}else{2.0},color));
            if name!=">" && show_cursor_labels {
                p.text(Pos2::new(x+4.0,graph.top()+4.0),egui::Align2::LEFT_TOP,name,egui::FontId::monospace(10.0),color);
            }
        }
    }
    let mut action=None;
    if response.clicked() {
        if let Some(pos)=response.interact_pointer_pos() {
            let index=if graph.contains(pos) {
                let time=*view_start+((pos.x-graph.left())/graph.width()) as f64*(*view_end-*view_start);
                let value=((graph.bottom()-pos.y)/graph.height()) as f64*255.0;
                nearest_overlay_trace(traces,time,value)
            } else {
                let index=((pos.y-gutter.top()-22.0)/15.0).floor() as isize;
                (index>=0 && (index as usize)<traces.len()).then_some(index as usize)
            };
            action=index.map(|i|(i,false));
        }
    }
    response.context_menu(|ui| {
        for (index,(name,_)) in traces.iter().enumerate() {
            if ui.button(format!("Close graph: {}",name)).clicked(){action=Some((index,true));ui.close();}
        }
    });
    action

}

fn draw_wave(ui: &mut egui::Ui, pts: &[[f64; 2]], height: f32) {
    let desired = Vec2::new(ui.available_width().max(300.0), height);
    let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
    let p = ui.painter_at(rect);
    let dark=ui.visuals().dark_mode;
    p.rect_filled(rect,0.0,if dark{Color32::from_gray(20)}else{Color32::from_gray(248)});
    p.rect_stroke(rect,0.0,Stroke::new(1.0_f32,if dark{Color32::from_gray(70)}else{Color32::from_gray(175)}),egui::StrokeKind::Inside);
    if pts.len() < 2 { return; }
    let xmin = pts.first().unwrap()[0];
    let xmax = pts.last().unwrap()[0].max(xmin + 1e-9);
    let ymin = pts.iter().map(|x| x[1]).fold(f64::INFINITY, f64::min);
    let ymax = pts.iter().map(|x| x[1]).fold(f64::NEG_INFINITY, f64::max).max(ymin + 1e-9);
    let cv = |q: [f64; 2]| Pos2::new(
        rect.left() + (((q[0] - xmin) / (xmax - xmin)) as f32) * rect.width(),
        rect.bottom() - (((q[1] - ymin) / (ymax - ymin)) as f32) * rect.height(),
    );
    let mut prev = cv(pts[0]);
    for &q in &pts[1..] {
        let cur = cv(q);
        p.line_segment([prev, cur], Stroke::new(1.2_f32, if dark{Color32::from_rgb(86,180,233)}else{Color32::from_rgb(0,95,160)}));
        prev = cur;
    }
    p.text(rect.left_top() + Vec2::new(5.0, 5.0), egui::Align2::LEFT_TOP, format!("{:.3}..{:.3}s | {:.3}..{:.3}", xmin, xmax, ymin, ymax), egui::FontId::monospace(11.0), Color32::GRAY);
}

#[cfg(test)]
mod session_lifecycle_tests {
    use super::*;
    fn late_channel_capture(dir:&std::path::Path)->PathBuf {
        let path=dir.join("late-selected-channel.pcap");
        let mut raw=vec![0u8;12];raw.extend(0x99feu16.to_be_bytes());raw.extend([0,1,0,1,3,3,0,0x20,0,0,0,0]);
        for (channel,time) in [(1u32,0u64),(1,1_000_000_000),(2,5_000_000_000)] {
            raw.extend(channel.to_be_bytes());raw.extend((1_760_000_000_000_000_000u64+time).to_be_bytes());
            raw.extend(2u16.to_be_bytes());raw.extend(0x5900u16.to_be_bytes());raw.extend(100i16.to_be_bytes());
        }
        let mut bytes=vec![0x4d,0x3c,0xb2,0xa1];bytes.extend(2u16.to_le_bytes());bytes.extend(4u16.to_le_bytes());bytes.extend([0u8;8]);
        bytes.extend(65535u32.to_le_bytes());bytes.extend(1u32.to_le_bytes());bytes.extend(1_760_000_000u32.to_le_bytes());bytes.extend(0u32.to_le_bytes());
        bytes.extend((raw.len() as u32).to_le_bytes());bytes.extend((raw.len() as u32).to_le_bytes());bytes.extend(raw);std::fs::write(&path,bytes).unwrap();path
    }
    #[test]
    fn analog_capture_warning_switches_to_source_time_without_losing_channels() {
        session::with_test_storage(|dir| {
            assert!(LogLensApp::initial_state().tecmp_source_time);
            let path=dir.join("buffered-analog.pcap");
            let epoch=1_760_000_000_000_000_000u64;
            let raw_packet=|mt:u8,dt:u16,channels:&[(u32,Vec<u8>)]| {
                let mut raw=vec![0u8;12];raw.extend(0x99feu16.to_be_bytes());
                raw.extend([0,1,0,1,3,mt]);raw.extend(dt.to_be_bytes());raw.extend([0,0,0,0]);
                for (ch,payload) in channels {
                    raw.extend(ch.to_be_bytes());raw.extend((epoch+if mt==0{533_000_000}else{0}).to_be_bytes());
                    raw.extend((payload.len() as u16).to_be_bytes());raw.extend((if mt==0{0u16}else{0x5900u16}).to_be_bytes());raw.extend(payload);
                }
                raw
            };
            let mut bytes=vec![0x4d,0x3c,0xb2,0xa1];bytes.extend(2u16.to_le_bytes());bytes.extend(4u16.to_le_bytes());bytes.extend([0u8;8]);
            bytes.extend(65535u32.to_le_bytes());bytes.extend(1u32.to_le_bytes());
            let payload=(0..700).flat_map(|n|(if n<533{0i16}else{5000i16}).to_be_bytes()).collect::<Vec<_>>();
            for (capture_ns,raw) in [(epoch+533_000_000,raw_packet(0,0,&[(99,vec![0,1,0,7])])),
                (epoch+700_000_000,raw_packet(3,0x20,&[(1,payload.clone()),(2,payload)]))] {
                bytes.extend(((capture_ns/1_000_000_000)as u32).to_le_bytes());bytes.extend(((capture_ns%1_000_000_000)as u32).to_le_bytes());
                bytes.extend((raw.len()as u32).to_le_bytes());bytes.extend((raw.len()as u32).to_le_bytes());bytes.extend(raw);
            }
            std::fs::write(&path,bytes).unwrap();
            let channels=BTreeSet::from([1,2]);
            let d=crate::engine::load_selected_with_timeline(&path,100,Some(&channels),false).unwrap();
            assert!(d.warnings.iter().any(|w|w.contains("700.000 ms")));
            let mut app=LogLensApp::initial_state();app.apply_loaded_dataset(path,false,d,None,Some(channels.clone()));
            app.graphics_sources=BTreeSet::from(["ANALOG|1|ANALOG:1".into()]);
            app.project_channels.remove("ANALOG|2");app.graph_channel_names.insert(1,"Hand".into());
            app.graphics_frame_id_filter.selected=Some(BTreeSet::from(["ANALOG:1".into()]));
            let scope=app.project_channels.clone();let sources=app.graphics_sources.clone();
            let ctx=egui::Context::default();
            let draw=|app:&mut LogLensApp,events|ctx.run(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(Pos2::ZERO,Vec2::new(1800.0,500.0))),events,..Default::default()},|ctx|{
                let d=app.dataset.clone().unwrap();egui::CentralPanel::default().show(ctx,|ui|app.analog_timeline_notice(ui,&d));
            });
            let out=draw(&mut app,vec![]);
            let pos=out.shapes.iter().find_map(|shape|if let egui::epaint::Shape::Text(text)=&shape.shape {
                (text.galley.job.text=="Use TECMP source time").then_some(text.pos+Vec2::new(4.0,5.0))
            }else{None}).expect("capture-time warning must offer a source-time fix");
            for pressed in [true,false] {draw(&mut app,vec![egui::Event::PointerMoved(pos),egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed,modifiers:egui::Modifiers::NONE}]);}
            assert!(app.background_load.is_some());finish(&mut app);
            let d=app.dataset.clone().unwrap();assert!(d.source_time);
            assert_eq!(app.active_preload_channels,channels);assert_eq!(app.project_channels,scope);assert_eq!(app.graphics_sources,sources);
            assert_eq!(app.graph_channel_names[&1],"Hand");assert!(app.graphics_frame_id_filter.matches("ANALOG:1"));assert!(!app.graphics_frame_id_filter.matches("ANALOG:2"));
            let marker=control_triggers(&d.frames,0.0)[0].time_s;
            assert!((marker-0.533).abs()<1e-12);
            assert_eq!(analog_sample_at(&d,"ANALOG:1",1,marker),Some([0.533,5.0]));
            assert_eq!(analog_sample_at(&d,"ANALOG:1",1,marker-0.001).unwrap()[1],0.0);
            assert!(app.analog_view_points(&d,"ANALOG:1",1,0.532,0.534).iter().any(|p|p[0]==0.533 && p[1]==5.0));
            assert!(!d.warnings.iter().any(|w|w.starts_with("Analog capture-time timing:")));
            assert_eq!(app.workspace_snapshot().analysis_settings["tecmp_source_time"],serde_json::json!(true));
        });
    }

    #[test]
    fn full_loaded_range_reveals_tail_rows_without_changing_channel_or_graph_scope() {
        session::with_test_storage(|dir| {
            let mut app=LogLensApp::initial_state();open(&mut app,capture(dir,"full-span.csv"));
            let d=app.dataset.clone().unwrap();
            assert_eq!((app.graphics_view_start_s,app.graphics_view_end_s),(0.0,10.0));
            assert!(app.graphics_range_start_s.is_none(),"opening must not silently crop to activity");
            app.graphics_sources=BTreeSet::from(["CAN|1|0x123".into()]);app.graph_channel_names.insert(1,"Bus".into());
            let scope=app.project_channels.clone();let sources=app.graphics_sources.clone();
            app.use_manual_window=true;app.time_start=2.0;app.time_end=3.0;
            app.graphics_view_start_s=2.0;app.graphics_view_end_s=3.0;app.graphics_range_start_s=Some(2.0);app.graphics_range_end_s=Some(3.0);
            app.ensure_scope_cache();assert!(app.cached_scoped_indices.is_empty());
            app.show_full_loaded_range(&d);app.ensure_scope_cache();
            assert_eq!(app.cached_scoped_indices,vec![0,1]);assert_eq!((app.graphics_view_start_s,app.graphics_view_end_s),(0.0,10.0));
            assert_eq!(app.project_channels,scope);assert_eq!(app.graphics_sources,sources);assert_eq!(app.graph_channel_names[&1],"Bus");
            assert!(!app.use_manual_window);assert!(app.graphics_range_start_s.is_none());
        });
    }

    #[test]
    fn old_partial_session_channel_group_resolves_without_losing_enabled_choice() {
        session::with_test_storage(|dir| {
            let mut app=LogLensApp::initial_state();open(&mut app,capture(dir,"legacy-scope.csv"));
            let mut saved=app.workspace_snapshot();saved.project_channels=BTreeSet::from(["TECMP|1".into()]);
            app.apply_workspace_snapshot(saved);assert_eq!(app.project_channels,BTreeSet::from(["CAN|1".into()]));
            let mut disabled=app.workspace_snapshot();disabled.project_channels.clear();
            app.apply_workspace_snapshot(disabled);assert!(app.project_channels.is_empty(),"disabled channels must stay disabled");
        });
    }
    #[test]
    fn saved_partial_capture_reopens_with_late_selected_channels_still_checked() {
        session::with_test_storage(|dir| {
            let path=late_channel_capture(dir);let channels=BTreeSet::from([1,2]);
            let d=crate::engine::load_selected_with_timeline(&path,1,Some(&channels),true).unwrap();
            let mut app=LogLensApp::initial_state();app.apply_loaded_dataset(path,false,d,None,Some(channels.clone()));
            let meta=app.session.clone().unwrap();
            let mut reopened=LogLensApp::initial_state();reopened.max_packets=1;reopened.load_saved_session(meta);finish(&mut reopened);
            assert_eq!(reopened.active_preload_channels,channels);
            assert!(reopened.project_channels.contains("ANALOG|1"));assert!(reopened.project_channels.contains("ANALOG|2"));
            assert!(reopened.dataset.as_ref().unwrap().index.by_project_channel["ANALOG|2"].is_empty());
            assert!(reopened.change_capture_channels());assert_eq!(reopened.pending_channel_selection,channels);
        });
    }
    #[test]
    fn partial_retry_keeps_unplotted_late_channel_and_disabled_project_scope() {
        session::with_test_storage(|dir| {
            let path=late_channel_capture(dir);let channels=BTreeSet::from([1,2]);
            let d=crate::engine::load_selected_with_timeline(&path,1,Some(&channels),true).unwrap();
            let mut app=LogLensApp::initial_state();app.apply_loaded_dataset(path.clone(),false,d,None,Some(channels.clone()));
            assert_eq!(app.active_preload_channels,channels);assert!(app.project_channels.contains("ANALOG|2"));
            assert!(app.change_capture_channels());assert_eq!(app.pending_channel_selection,channels);app.return_to_workspace();
            // Keep only channel 1 plotted, and disable channel 2's project checkbox.
            app.graphics_sources=BTreeSet::from(["ANALOG|1|ANALOG:1".into()]);app.project_channels.remove("ANALOG|2");
            app.graph_channel_names.insert(2,"Late analog".into());app.cursor_a_s=0.0001;app.cursor_b_s=0.0005;
            app.max_packets=1;assert!(app.reload_selected_channels());
            assert_eq!(app.background_load.as_ref().unwrap().channels.as_ref(),Some(&channels));finish(&mut app);
            assert_eq!(app.active_preload_channels,channels);assert!(!app.project_channels.contains("ANALOG|2"));
            assert_eq!(app.cursor_a_s,0.0001);assert_eq!(app.cursor_b_s,0.0005);assert_eq!(app.graph_channel_names[&2],"Late analog");
            let saved=session::load_configuration(&app.session.as_ref().unwrap().id).unwrap().unwrap();assert_eq!(saved.preload_channels,channels);
            assert!(app.change_capture_channels());assert_eq!(app.pending_channel_selection,channels);app.return_to_workspace();
            app.max_packets=100;assert!(app.reload_selected_channels());finish(&mut app);
            assert_eq!(app.dataset.as_ref().unwrap().frames.len(),3);assert_eq!(app.active_preload_channels,channels);
            assert!(!app.project_channels.contains("ANALOG|2"),"retry must not re-enable a manually disabled project channel");
            assert!(!app.dataset.as_ref().unwrap().warnings.iter().any(|w|w.starts_with("Safe load limit reached")));
            assert_eq!((app.cursor_a_s,app.cursor_b_s),(0.0001,0.0005));
        });
    }
    #[test]
    fn smaller_partial_retry_does_not_replace_loaded_data_or_channel_choices() {
        session::with_test_storage(|dir| {
            let path=late_channel_capture(dir);let channels=BTreeSet::from([1,2]);
            let complete=crate::engine::load_selected_with_timeline(&path,2,Some(&channels),true).unwrap();
            let mut app=LogLensApp::initial_state();app.apply_loaded_dataset(path.clone(),false,complete,None,Some(channels.clone()));
            let old=app.dataset.clone().unwrap();app.cursor_a_s=0.2;app.cursor_b_s=0.8;app.max_packets=1;
            assert!(app.reload_selected_channels());finish(&mut app);
            assert!(Arc::ptr_eq(&old,app.dataset.as_ref().unwrap()));assert_eq!(app.active_preload_channels,channels);
            assert_eq!((app.cursor_a_s,app.cursor_b_s),(0.2,0.8));assert!(app.info.as_ref().unwrap().contains("Kept the current data"));
            // A clock change must not bypass the bounded-reload coverage guard.
            app.tecmp_source_time=false;assert!(app.reload_selected_channels());finish(&mut app);
            assert!(Arc::ptr_eq(&old,app.dataset.as_ref().unwrap()));assert!(app.tecmp_source_time);
            assert_eq!(app.active_preload_channels,channels);assert_eq!((app.cursor_a_s,app.cursor_b_s),(0.2,0.8));
        });
    }
    #[test]
    fn clean_graph_export_uses_original_analog_samples_and_units() {
        session::with_test_storage(|dir| {
            let path=capture(dir,"analog-image.csv");let mut app=LogLensApp::initial_state();open(&mut app,path.clone());
            let mut d=crate::engine::load(&path,100).unwrap();d.frames.truncate(1);
            let frame=&mut d.frames[0];frame.bus_type=BusType::Analog;frame.channel_id=32785;frame.frame_key="ANALOG:32785".into();
            frame.payload=[1000i16,2000,3000].into_iter().flat_map(i16::to_be_bytes).collect();
            frame.analog=Some(crate::model::AnalogMeta{sample_count:3,sample_width_bytes:2,sample_period_ns:1_000_000,factor:0.001,unit:"V".into()});
            d.index=crate::index::FrameIndex::build(&d.frames);app.project_channels=d.index.project_channel_keys.clone();app.cached_graphics_sources=graphics_available_sources(&d.frames);
            app.graphics_sources.clear();app.selected_frame_bytes.clear();app.graphics_lane_order.clear();
            let source=app.cached_graphics_sources[0].0.clone();app.set_graphics_source_byte(&source,0,true);
            app.graphics_view_start_s=0.0;app.graphics_view_end_s=0.003;app.cursor_a_s=0.001;app.cursor_b_s=0.002;
            let report=app.graph_image_report(&d,false).unwrap();assert_eq!(report.lanes.len(),1);assert_eq!(report.lanes[0].unit,"V");
            assert_eq!(report.lanes[0].cursors[0].value,"2.000 V");assert_eq!(report.lanes[0].cursors[1].value,"3.000 V");assert_eq!(report.lanes[0].delta.as_deref(),Some("+1.000 V"));
        });
    }
    #[test]
    fn graph_settings_open_in_viewer_change_shared_controls_and_persist() {
        session::with_test_storage(|dir| {
            let mut app=LogLensApp::initial_state();open(&mut app,capture(dir,"graph-settings.csv"));
            let d=app.dataset.clone().unwrap();let ctx=egui::Context::default();let mut time=0.0;
            let mut draw=|app:&mut LogLensApp,events| {
                time+=0.2;ctx.run(egui::RawInput{time:Some(time),screen_rect:Some(egui::Rect::from_min_size(Pos2::ZERO,Vec2::new(2200.0,1300.0))),events,..Default::default()},|ctx| {
                    egui::CentralPanel::default().show(ctx,|ui|app.canoe_graphics_viewer_content(ui,&d));
                })
            };
            let position=|out:&egui::FullOutput,label:&str|out.shapes.iter().find_map(|s|match &s.shape {
                egui::epaint::Shape::Text(t) if t.galley.job.text==label=>Some(t.pos+Vec2::new(4.0,5.0)),_=>None
            }).unwrap_or_else(||panic!("missing {label}"));
            let click=|p,pressed|egui::Event::PointerButton{pos:p,button:egui::PointerButton::Primary,pressed,modifiers:egui::Modifiers::NONE};
            draw(&mut app,vec![]);let out=draw(&mut app,vec![]);let p=position(&out,"Settings");
            draw(&mut app,vec![egui::Event::PointerMoved(p),click(p,true)]);draw(&mut app,vec![click(p,false)]);
            let out=draw(&mut app,vec![]);position(&out,"Graph Settings");
            let p=position(&out,"Grid");assert!(app.graph_show_grid);
            draw(&mut app,vec![egui::Event::PointerMoved(p),click(p,true)]);draw(&mut app,vec![click(p,false)]);
            assert!(!app.graph_show_grid);assert!(!app.show_settings,"root Settings should remain closed");
            app.graph_line_width=2.5;
            let settings=app.analysis_settings();let mut restored=LogLensApp::initial_state();restored.restore_analysis_settings(&settings);
            assert!(!restored.graph_show_grid);assert_eq!(restored.graph_line_width,2.5);
        });
    }
    #[test]
    fn graph_image_export_uses_all_plotted_data_and_exact_cursor_values() {
        session::with_test_storage(|dir| {
            let mut app=LogLensApp::initial_state();open(&mut app,capture(dir,"clean-export.csv"));
            let d=app.dataset.clone().unwrap();let source=app.cached_graphics_sources[0].0.clone();
            app.set_graphics_source_byte(&source,0,true);app.graphics_stacked=true;app.graphics_view_start_s=0.0;app.graphics_view_end_s=10.0;
            app.cursor_a_s=0.0;app.cursor_b_s=10.0;app.graphics_lane_scroll_y=10_000.0;
            let report=app.graph_image_report(&d,false).unwrap();assert_eq!(report.lanes.len(),1);
            let dark_report=app.graph_image_report(&d,true).unwrap();assert!(dark_report.dark);assert!(!report.dark);
            assert_ne!(dark_report.lanes[0].color,report.lanes[0].color);
            assert_eq!(dark_report.lanes[0].points,report.lanes[0].points);
            assert_eq!(dark_report.lanes[0].cursors[0].value,report.lanes[0].cursors[0].value);
            assert_eq!(report.lanes[0].cursors[0].value,"0x00 / 0");assert_eq!(report.lanes[0].cursors[1].value,"0x01 / 1");
            assert_eq!(report.lanes[0].delta.as_deref(),Some("+1"));assert!(!report.lanes[0].points.is_empty());
            let path=dir.join("data-export.png");app.save_graph_image(&egui::Context::default(),&d,path.clone());
            let dimensions=image::image_dimensions(path).unwrap();assert_eq!(dimensions.0,2000);assert!(dimensions.1<900);
            app.graphics_per_waveform_cursors=true;app.graphics_multi_cursors.insert(format!("SRC|{}|0",source),vec![0.0,10.0]);
            let report=app.graph_image_report(&d,false).unwrap();assert_eq!(report.lanes[0].cursors[1].label,"C2");assert_eq!(report.lanes[0].cursors[1].value,"0x01 / 1");
        });
    }
    #[test]
    fn activity_filter_uses_only_bytes_covered_by_decoded_signals() {
        use crate::activity_filter::Activity;
        session::with_test_storage(|dir| {
            let path=dir.join("signal-activity.csv");let mut csv="time,channel,id,payload\n".to_owned();
            for i in 0..64{csv.push_str(&format!("{},1,123,{:02X}2A\n",i,if i%2==0{1}else{7}));}
            std::fs::write(&path,csv).unwrap();let mut app=LogLensApp::initial_state();open(&mut app,path);
            let d=app.dataset.clone().unwrap();
            app.graphics_activity_filter.selected=Some(BTreeSet::from([Activity::Pattern]));
            let mut sig=SignalDefinition{name:"constant".into(),protocol:"CAN".into(),channel:"1".into(),frame_id:"0x123".into(),start_byte:1,start_bit:0,bit_length:8,endian:"Intel".into(),..Default::default()};
            assert!(!app.graphics_signal_activity_matches(&d,&sig),"unrelated Pattern byte must not classify a Constant signal");
            sig.start_byte=0;assert!(app.graphics_signal_activity_matches(&d,&sig));
            sig.endian="Motorola".into();sig.start_bit=7;assert!(app.graphics_signal_activity_matches(&d,&sig));
            app.graphics_activity_filter.selected=Some(BTreeSet::from([Activity::Constant]));
            sig.start_byte=1;assert!(app.graphics_signal_activity_matches(&d,&sig));
            assert!(!app.graphics_lane_activity_matches(&d,"SRC|ANALOG|1|ANALOG:1|0"));
            app.graphics_activity_filter.selected=Some(BTreeSet::from([Activity::Analog]));
            assert!(app.graphics_lane_activity_matches(&d,"SRC|ANALOG|1|ANALOG:1|0"));
        });
    }
    #[test]
    fn activity_checklist_filters_messages_and_graphs_and_combines_with_ids() {
        use crate::activity_filter::Activity;
        session::with_test_storage(|dir| {
            let path=dir.join("activity-checklist.csv");let mut csv="time,channel,id,payload\n".to_owned();
            for i in 0..64 {csv.push_str(&format!("{},1,123,{:02X}\n{},1,456,2A\n",i,if i%2==0{1}else{7},i));}
            std::fs::write(&path,csv).unwrap();let mut app=LogLensApp::initial_state();open(&mut app,path);
            let d=app.dataset.clone().unwrap();app.graphics_sources.clear();app.selected_frame_bytes.clear();
            for source in app.cached_graphics_sources.clone(){app.set_graphics_source_byte(&source.0,0,true);}
            app.graphics_stacked=true;app.graphics_range_start_s=None;app.graphics_range_end_s=None;app.graphics_class_window=None;
            assert_eq!(app.graphics_byte_class(&d,"CAN","0x123",1,0).0,"PATTERN");
            assert_eq!(app.graphics_byte_class(&d,"CAN","0x456",1,0).0,"STATIC");
            let ctx=egui::Context::default();let mut time=0.0;
            let mut draw=|app:&mut LogLensApp,events| {
                time+=0.2;
                ctx.run(egui::RawInput{time:Some(time),screen_rect:Some(egui::Rect::from_min_size(Pos2::ZERO,Vec2::new(2200.0,1300.0))),events,..Default::default()},|ctx| {
                    egui::CentralPanel::default().show(ctx,|ui|app.canoe_graphics_viewer_content(ui,&d));
                })
            };
            let pos=|out:&egui::FullOutput,label:&str|out.shapes.iter().find_map(|s|match &s.shape {
                egui::epaint::Shape::Text(t) if t.galley.job.text==label=>Some(t.pos+Vec2::new(4.0,5.0)),_=>None
            }).unwrap_or_else(||panic!("missing {label}"));
            let click=|p,pressed|egui::Event::PointerButton{pos:p,button:egui::PointerButton::Primary,pressed,modifiers:egui::Modifiers::NONE};
            draw(&mut app,vec![]);let out=draw(&mut app,vec![]);let p=pos(&out,"Activity: All");
            draw(&mut app,vec![egui::Event::PointerMoved(p),click(p,true)]);draw(&mut app,vec![click(p,false)]);
            let out=draw(&mut app,vec![]);let p=pos(&out,"Select none");
            draw(&mut app,vec![egui::Event::PointerMoved(p),click(p,true)]);draw(&mut app,vec![click(p,false)]);
            let out=draw(&mut app,vec![]);let p=pos(&out,"Constant");
            draw(&mut app,vec![egui::Event::PointerMoved(p),click(p,true)]);draw(&mut app,vec![click(p,false)]);
            let p=Pos2::new(2100.0,1200.0);draw(&mut app,vec![egui::Event::PointerMoved(p),click(p,true)]);draw(&mut app,vec![click(p,false)]);
            let out=draw(&mut app,vec![]);pos(&out,"Activity: Constant");pos(&out,"1 messages · 0 signals");
            assert!(app.graphics_lane_activity_matches(&d,"SRC|CAN|1|0x456|0"));
            assert!(!app.graphics_lane_activity_matches(&d,"SRC|CAN|1|0x123|0"));
            assert!(!app.graphics_lane_activity_matches(&d,"BIT|CAN|1|0x123|0|2"));
            assert!(!app.graphics_lane_activity_matches(&d,"FRAME|CAN|1|0x123"));
            assert_eq!(app.graphics_sources.len(),2,"filtering must retain both plotted selections");
            app.graphics_activity_filter.selected=Some(BTreeSet::from([Activity::Pattern,Activity::Constant]));
            let out=draw(&mut app,vec![]);pos(&out,"2 messages · 0 signals");
            app.graphics_frame_id_filter.selected=Some(BTreeSet::from(["0x123".into()]));
            let out=draw(&mut app,vec![]);pos(&out,"1 messages · 0 signals");
            let settings=app.analysis_settings();let mut restored=LogLensApp::initial_state();restored.restore_analysis_settings(&settings);
            assert_eq!(restored.graphics_activity_filter,app.graphics_activity_filter);
            app.graphics_activity_filter.selected=Some(BTreeSet::new());let out=draw(&mut app,vec![]);pos(&out,"0 messages · 0 signals");
            app.graphics_activity_filter.selected=None;app.graphics_frame_id_filter.selected=None;
            let out=draw(&mut app,vec![]);pos(&out,"2 messages · 0 signals");
        });
    }
    #[test]
    fn data_expands_in_the_trace_for_both_rendering_modes() {
        session::with_test_storage(|dir| {
            let path=dir.join("inline-data.csv");
            std::fs::write(&path,format!("time,channel,id,payload\n0,1,123,{}\n1,1,456,01\n2,1,789,02\n","AA".repeat(80))).unwrap();
            for virtual_mode in [false,true] {
                let mut app=LogLensApp::initial_state();open(&mut app,path.clone());
                let d=app.dataset.clone().unwrap();app.cached_scoped_indices=(0..d.frames.len()).collect();
                app.trace_only_changing=false;
                for c in &mut app.trace_columns.columns {
                    c.visible=matches!(c.column,crate::trace_columns::TraceColumn::FrameKey|crate::trace_columns::TraceColumn::Data);c.width=150.0;
                }
                let rows:Vec<_>=(0..d.frames.len()).map(|idx|TraceRow{idx,count:1,dt_s:0.0}).collect();
                let data=app.trace_display_row(&d,0,&BTreeMap::new()).data;
                let ctx=egui::Context::default();let mut time=0.0;
                let mut draw=|app:&mut LogLensApp,events| {
                    time+=0.2;
                    ctx.run(egui::RawInput{time:Some(time),screen_rect:Some(egui::Rect::from_min_size(Pos2::ZERO,Vec2::new(600.0,800.0))),events,..Default::default()},|ctx| {
                        egui::CentralPanel::default().show(ctx,|ui| {
                            if virtual_mode{app.canoe_trace_table_virtual(ui,&d,d.frames.len(),&BTreeMap::new());}
                            else{app.canoe_trace_table(ui,&d,&rows,&BTreeMap::new());}
                        });
                    })
                };
                let text=|output:&egui::FullOutput,label:&str|output.shapes.iter().find_map(|s|match &s.shape {
                    egui::epaint::Shape::Text(t) if t.galley.job.text==label=>Some((t.pos,t.galley.size())),_=>None
                }).unwrap_or_else(||panic!("missing {label}"));
                let click=|pos,pressed|egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed,modifiers:egui::Modifiers::NONE};
                draw(&mut app,vec![]);let output=draw(&mut app,vec![]);let p=text(&output,"… [expand]").0+Vec2::new(3.0,5.0);
                draw(&mut app,vec![egui::Event::PointerMoved(p),click(p,true)]);draw(&mut app,vec![click(p,false)]);
                let output=draw(&mut app,vec![]);let (top,size)=text(&output,&data);assert!(size.y>21.0);
                let next=text(&output,"0x456").0;
                assert!(next.y>=top.y+size.y,"following trace row must start below expanded payload in mode {virtual_mode}");
                let p=text(&output,"[collapse]").0+Vec2::new(3.0,5.0);
                draw(&mut app,vec![egui::Event::PointerMoved(p),click(p,true)]);draw(&mut app,vec![click(p,false)]);draw(&mut app,vec![]);
                assert_eq!(app.trace_columns.expanded_row(),None);
            }
        });
    }
    #[test]
    fn same_capture_reload_preserves_live_names_filters_layout_and_cursors() {
        session::with_test_storage(|dir| {
            let path=dir.join("preserve-edits.csv");
            std::fs::write(&path,"time,channel,id,payload\n0,1,123,00\n1,1,123,01\n10,1,123,02\n").unwrap();
            let mut app=LogLensApp::initial_state();open(&mut app,path.clone());
            let source=app.cached_graphics_sources[0].0.clone();let lane=format!("SRC|{}|0",source);
            app.set_graphics_source_byte(&source,0,true);app.graphics_lane_order=vec![lane.clone()];
            app.graph_channel_names.insert(1,"Supply".into());app.graph_lane_names.insert(lane.clone(),"Voltage".into());
            app.graphics_cursor_panel_width=520.0;app.cursor_a_s=0.3;app.cursor_b_s=0.7;
            app.graphics_view_start_s=0.2;app.graphics_view_end_s=0.8;
            app.graphics_range_start_s=Some(0.25);app.graphics_range_end_s=Some(0.75);
            app.graphics_multi_cursors.insert(lane.clone(),vec![0.35,0.65]);
            app.graphics_frame_id_filter.selected=Some(BTreeSet::from([app.cached_graphics_sources[0].3.clone()]));
            app.graphics_eth_filters.insert(7,crate::eth_filter::EndpointFilter{source:Some(BTreeSet::from(["10.0.0.1".into(),"10.0.0.2".into()])),destination:None});
            let before=app.workspace_snapshot();let bytes=serde_json::to_value(&before).unwrap();
            let d=crate::engine::load(&path,100).unwrap();app.apply_loaded_dataset(path,true,d,None,Some(BTreeSet::from([1])));
            assert_eq!(app.channel_name_map().get(&1).unwrap(),"Supply");
            assert_eq!(app.graph_lane_names[&lane],"Voltage");
            assert_eq!((app.cursor_a_s,app.cursor_b_s),(0.3,0.7));
            assert_eq!((app.graphics_view_start_s,app.graphics_view_end_s),(0.2,0.8));
            assert_eq!((app.graphics_range_start_s,app.graphics_range_end_s),(Some(0.25),Some(0.75)));
            assert_eq!(app.graphics_multi_cursors[&lane],vec![0.35,0.65]);
            assert_eq!(app.graphics_cursor_panel_width,520.0);
            assert_eq!(app.graphics_frame_id_filter.selected,Some(BTreeSet::from([app.cached_graphics_sources[0].3.clone()])));
            assert_eq!(app.graphics_eth_filters[&7].source.as_ref().unwrap().len(),2);
            assert_eq!(app.graphics_lane_order,vec![lane]);
            // Aliases are presentation only: source keys and frame bytes remain original.
            assert_eq!(app.dataset.as_ref().unwrap().frames[0].channel_id,1);
            let saved=session::load_workspace(&app.session.as_ref().unwrap().id).unwrap().unwrap();
            assert_eq!(saved.analysis_settings["graph_lane_names"],bytes["analysis_settings"]["graph_lane_names"]);
        });
    }
    #[test]
    fn decoded_browser_collapse_survives_filter_count_changes() {
        session::with_test_storage(|dir| {
            let path=dir.join("decoded-collapse.csv");
            std::fs::write(&path,"time,channel,id,payload\n0,1,123,00\n0,1,456,00\n1,1,123,01\n1,1,456,02\n").unwrap();
            let mut app=LogLensApp::initial_state();open(&mut app,path);
            let d=app.dataset.clone().unwrap();
            app.signals=["Speed","Temperature"].iter().enumerate().map(|(i,name)|SignalDefinition{name:name.to_string(),protocol:"CAN".into(),channel:"1".into(),frame_id:app.cached_graphics_sources[i].3.clone(),..Default::default()}).collect();
            let ctx=egui::Context::default();let mut time=0.0;
            let mut draw=|app:&mut LogLensApp,events| {
                time+=0.2;
                ctx.run(egui::RawInput{time:Some(time),screen_rect:Some(egui::Rect::from_min_size(Pos2::ZERO,Vec2::new(2200.0,1300.0))),events,..Default::default()},|ctx| {
                    egui::CentralPanel::default().show(ctx,|ui|{app.canoe_graphics_viewer_content(ui,&d);});
                })
            };
            draw(&mut app,vec![]);let output=draw(&mut app,vec![]);
            let pos=output.shapes.iter().find_map(|s|if let egui::epaint::Shape::Text(t)=&s.shape {
                (t.galley.job.text=="Decoded (2)").then_some(t.pos+Vec2::new(3.0,5.0))
            }else{None}).unwrap();
            let click=|pressed|egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed,modifiers:egui::Modifiers::NONE};
            draw(&mut app,vec![egui::Event::PointerMoved(pos),click(true)]);draw(&mut app,vec![click(false)]);draw(&mut app,vec![]);let output=draw(&mut app,vec![]);
            assert!(!output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.job.text=="Speed" && t.pos.x<280.0)),"Decoded group must first collapse");
            app.graphics_frame_id_filter.selected=Some([app.cached_graphics_sources[0].3.clone()].into_iter().collect());draw(&mut app,vec![]);let output=draw(&mut app,vec![]);
            assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.job.text=="Decoded (1)")));
            let header_y=output.shapes.iter().find_map(|s|if let egui::epaint::Shape::Text(t)=&s.shape {
                (t.galley.job.text=="Decoded (1)").then_some(t.pos.y)
            }else{None}).unwrap();
            assert!(!output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.job.text=="Speed" && t.pos.x<280.0 && t.pos.y>header_y)),"filtering must not reopen the collapsed Decoded group");
        });
    }
    #[test]
    fn graphics_headers_collapse_repeatedly_after_graph_reveal() {
        session::with_test_storage(|dir| {
            let mut app=LogLensApp::initial_state();open(&mut app,capture(dir,"collapse.csv"));
            let d=app.dataset.clone().unwrap();let source=app.cached_graphics_sources[0].0.clone();
            app.graphics_only_changing=false;app.graphics_hide_constant=false;app.graphics_hide_repetitive=false;
            app.select_graphics_lane(&format!("SRC|{}|0",source));
            let ctx=egui::Context::default();let mut time=0.0;
            let mut draw=|app:&mut LogLensApp,events| {
                time+=0.2;
                ctx.run(egui::RawInput{time:Some(time),screen_rect:Some(egui::Rect::from_min_size(Pos2::ZERO,Vec2::new(2200.0,1300.0))),events,..Default::default()},|ctx| {
                    egui::CentralPanel::default().show(ctx,|ui|{app.canoe_graphics_viewer_content(ui,&d);});
                })
            };
            let has_text=|output:&egui::FullOutput,text:&str|output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.job.text==text && t.pos.x<280.0));
            let text_pos=|output:&egui::FullOutput,text:&str|output.shapes.iter().find_map(|s|if let egui::epaint::Shape::Text(t)=&s.shape {
                (t.galley.job.text==text && t.pos.x<280.0).then_some(t.pos+Vec2::new(3.0,5.0))
            }else{None}).unwrap_or_else(||panic!("missing header: {text}"));
            let click=|pos,pressed|egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed,modifiers:egui::Modifiers::NONE};
            draw(&mut app,vec![]);draw(&mut app,vec![]);let mut output=draw(&mut app,vec![]);
            assert!(has_text(&output,"Whole Frame"));
            let frame=graphics_frame_display_label(&d.frames[0].frame_key);
            for _ in 0..2 {
                let pos=text_pos(&output,&frame);
                draw(&mut app,vec![egui::Event::PointerMoved(pos),click(pos,true)]);
                draw(&mut app,vec![click(pos,false)]);draw(&mut app,vec![]);output=draw(&mut app,vec![]);
                assert!(!has_text(&output,"Whole Frame"),"message collapse must survive subsequent repaints");
                assert!(app.graphics_reveal_source.is_none(),"a header click must not trigger graph reveal");
                let pos=text_pos(&output,&frame);
                draw(&mut app,vec![egui::Event::PointerMoved(pos),click(pos,true)]);
                draw(&mut app,vec![click(pos,false)]);draw(&mut app,vec![]);output=draw(&mut app,vec![]);
                assert!(has_text(&output,"Whole Frame"));
            }
            for header in ["0x1","CAN","GRAPHICS SOURCES"] {
                let pos=text_pos(&output,header);
                draw(&mut app,vec![egui::Event::PointerMoved(pos),click(pos,true)]);
                draw(&mut app,vec![click(pos,false)]);draw(&mut app,vec![]);output=draw(&mut app,vec![]);
                assert!(!has_text(&output,"Whole Frame"),"{header} collapse must remain closed");
                let pos=text_pos(&output,header);
                draw(&mut app,vec![egui::Event::PointerMoved(pos),click(pos,true)]);
                draw(&mut app,vec![click(pos,false)]);draw(&mut app,vec![]);output=draw(&mut app,vec![]);
                assert!(has_text(&output,"Whole Frame"),"{header} should expand again with remembered child state");
            }
            // Abort a pending reveal whose message is hidden by a new ID selection.
            app.select_graphics_lane(&format!("SRC|{}|0",source));app.graphics_frame_id_filter.selected=Some(BTreeSet::new());
            output=draw(&mut app,vec![]);let pos=text_pos(&output,"GRAPHICS SOURCES");
            draw(&mut app,vec![egui::Event::PointerMoved(pos),click(pos,true)]);
            draw(&mut app,vec![click(pos,false)]);draw(&mut app,vec![]);output=draw(&mut app,vec![]);
            assert!(!has_text(&output,"Protocol > Channel > Frame > Byte > Bit"),"manual collapse must override a pending auto-reveal");
            assert!(app.graphics_reveal_source.is_none());
        });
    }
    #[test]
    fn ethernet_channel_filter_limits_browser_and_graph_click_reveals_other_endpoint() {
        session::with_test_storage(|dir| {
            let path=capture(dir,"endpoints.csv");let mut d=crate::engine::load(&path,100).unwrap();
            let base=d.frames[0].clone();
            d.frames=["10.0.0.1","10.0.0.3"].iter().enumerate().map(|(i,src)|{
                let mut f=base.clone();f.bus_type=BusType::Ethernet;f.channel_id=7;
                f.frame_key=format!("ETH:UDP:{}:1000>10.0.0.2:2000",src);f.relative_time_s=i as f64;
                Arc::make_mut(&mut f.network).ip_src=Some(src.to_string());Arc::make_mut(&mut f.network).ip_dst=Some("10.0.0.2".into());f
            }).collect();d.index=crate::index::FrameIndex::build(&d.frames);
            let mut app=LogLensApp::initial_state();app.apply_loaded_dataset(path,false,d,None,None);
            let d=app.dataset.clone().unwrap();let sources=app.cached_graphics_sources.clone();
            app.graphics_sources.clear();app.selected_frame_bytes.clear();
            for (source,_,_,_,_) in &sources {app.set_graphics_source_byte(source,0,true);}
            app.graphics_only_changing=false;app.graphics_hide_constant=false;app.graphics_hide_repetitive=false;
            app.graphics_eth_filters.insert(7,crate::eth_filter::EndpointFilter{source:Some(BTreeSet::from(["10.0.0.1".into()])),destination:Some(BTreeSet::from(["10.0.0.2".into()]))});
            app.select_graphics_lane(&format!("SRC|{}|0",sources[0].0));
            assert!(app.graphics_eth_filters.contains_key(&7),"a matching graph keeps endpoint selection");
            let ctx=egui::Context::default();let mut time=0.0;
            let mut draw=|app:&mut LogLensApp| {
                time+=0.2;
                ctx.run(egui::RawInput{time:Some(time),screen_rect:Some(egui::Rect::from_min_size(Pos2::ZERO,Vec2::new(2200.0,1300.0))),..Default::default()},|ctx| {
                    egui::CentralPanel::default().show(ctx,|ui|{app.canoe_graphics_viewer_content(ui,&d);});
                })
            };
            draw(&mut app);draw(&mut app);let output=draw(&mut app);
            assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.job.text=="1 matching messages / streams")));
            assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.job.text.starts_with("Source IP:"))));
            assert!(!output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.job.text==sources[1].3)));
            assert_eq!(app.graphics_sources.len(),2,"browser filtering must preserve existing plotted selections");
            app.select_graphics_lane(&format!("SRC|{}|0",sources[1].0));
            assert!(!app.graphics_eth_filters.contains_key(&7),"a conflicting IP filter must not hide the clicked graph's message");
            draw(&mut app);draw(&mut app);let output=draw(&mut app);
            assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.job.text=="2 matching messages / streams")));
            assert!(app.graphics_reveal_source.is_none());
        });
    }
    #[test]
    fn compact_viewer_controls_keep_replay_range_reset_and_graph_clear_working() {
        session::with_test_storage(|dir| {
            let mut app=LogLensApp::initial_state();open(&mut app,capture(dir,"compact-buttons.csv"));
            let d=app.dataset.clone().unwrap();
            let source=app.cached_graphics_sources[0].0.clone();
            app.set_graphics_source_byte(&source,0,true);
            app.graphics_stacked=true;app.graphics_hide_constant=false;app.graphics_hide_repetitive=false;
            app.graphics_range_start_s=Some(1.0);app.graphics_range_end_s=Some(2.0);
            let ctx=egui::Context::default();let mut time=0.0;
            let mut draw=|app:&mut LogLensApp,events| {
                time+=0.2;
                ctx.run(egui::RawInput{time:Some(time),screen_rect:Some(egui::Rect::from_min_size(Pos2::ZERO,Vec2::new(2200.0,1300.0))),events,..Default::default()},|ctx| {
                    egui::CentralPanel::default().show(ctx,|ui|app.canoe_graphics_viewer_content(ui,&d));
                })
            };
            let pos=|output:&egui::FullOutput,text:&str|output.shapes.iter().rev().find_map(|s|match &s.shape {
                egui::epaint::Shape::Text(t) if t.galley.job.text==text=>Some(t.pos+Vec2::new(5.0,5.0)),_=>None
            }).unwrap_or_else(||panic!("Missing {text}"));
            let pointer=|pos,pressed|vec![egui::Event::PointerMoved(pos),egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed,modifiers:egui::Modifiers::NONE}];
            let output=draw(&mut app,vec![]);
            for label in ["Up","Down","Run","Pause","Stop","Clear Range","Clear analysis range"] {
                assert!(!output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.job.text==label)),"redundant {label} should not occupy the toolbar");
            }
            for action in ["Run","Pause","Resume","Stop"] {
                let output=draw(&mut app,vec![]);let p=pos(&output,"Replay");draw(&mut app,pointer(p,true));draw(&mut app,pointer(p,false));
                let output=draw(&mut app,vec![]);let p=pos(&output,action);draw(&mut app,pointer(p,true));draw(&mut app,pointer(p,false));
                match action {
                    "Run"=>assert!(app.measurement_running && !app.measurement_paused),
                    "Pause"=>assert!(app.measurement_running && app.measurement_paused),
                    "Resume"=>assert!(app.measurement_running && !app.measurement_paused),
                    "Stop"=>assert!(!app.measurement_running && !app.measurement_paused && app.replay_position_s==0.0),_=>unreachable!()
                }
            }
            let output=draw(&mut app,vec![]);let p=pos(&output,"Full Range");draw(&mut app,pointer(p,true));draw(&mut app,pointer(p,false));
            assert!(app.graphics_range_start_s.is_none() && app.graphics_range_end_s.is_none());
            assert_eq!(app.graphics_view_start_s,0.0);assert_eq!(app.graphics_view_end_s,app.cached_stats.as_ref().unwrap().duration_s.max(0.001));
            let output=draw(&mut app,vec![]);let p=pos(&output,"Options");draw(&mut app,pointer(p,true));draw(&mut app,pointer(p,false));
            let output=draw(&mut app,vec![]);let p=pos(&output,"Clear graphs");draw(&mut app,pointer(p,true));draw(&mut app,pointer(p,false));
            assert!(app.graphics_sources.is_empty() && app.graphics_lane_order.is_empty());
        });
    }

    #[test]
    fn searched_id_checkboxes_update_full_viewer_results() {
        session::with_test_storage(|dir| {
            let path=dir.join("checkbox-results.csv");
            std::fs::write(&path,"time,channel,id,payload\n0,1,123,00\n0,1,456,00\n0,1,789,00\n1,1,123,01\n1,1,456,00\n1,1,789,02\n").unwrap();
            let mut app=LogLensApp::initial_state();open(&mut app,path);
            let d=app.dataset.clone().unwrap();
            app.graphics_only_changing=true;app.graphics_hide_constant=true;
            let ctx=egui::Context::default();let mut time=0.0;
            let mut draw=|app:&mut LogLensApp,events| {
                time+=0.2;
                ctx.run(egui::RawInput{time:Some(time),screen_rect:Some(egui::Rect::from_min_size(Pos2::ZERO,Vec2::new(2200.0,1300.0))),events,..Default::default()},|ctx|{
                    egui::CentralPanel::default().show(ctx,|ui|app.canoe_graphics_viewer_content(ui,&d));
                })
            };
            let pos=|output:&egui::FullOutput,text:&str|output.shapes.iter().rev().find_map(|s|match &s.shape {
                egui::epaint::Shape::Text(t) if t.galley.job.text==text=>Some(t.pos+Vec2::new(6.0,6.0)),_=>None
            }).unwrap_or_else(||panic!("Missing {text}"));
            let pointer=|pos,pressed|vec![egui::Event::PointerMoved(pos),egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed,modifiers:egui::Modifiers::NONE}];
            draw(&mut app,vec![]);draw(&mut app,vec![]);let output=draw(&mut app,vec![]);
            let p=pos(&output,"Frame IDs: All");draw(&mut app,pointer(p,true));draw(&mut app,pointer(p,false));
            let output=draw(&mut app,vec![]);let p=pos(&output,"Select none");draw(&mut app,pointer(p,true));draw(&mut app,pointer(p,false));
            assert_eq!(app.graphics_frame_id_filter.selected,Some(BTreeSet::new()));
            let output=draw(&mut app,vec![]);let p=pos(&output,"Search Frame IDs");draw(&mut app,pointer(p,true));draw(&mut app,pointer(p,false));
            draw(&mut app,vec![egui::Event::Text("0x123".into())]);
            let output=draw(&mut app,vec![]);let p=pos(&output,"0x123");draw(&mut app,pointer(p,true));draw(&mut app,pointer(p,false));
            let output=draw(&mut app,vec![]);let p=pos(&output,"0x123");
            // Focus the search input rather than the ID row (the input is above Select all matching).
            let header_y=pos(&output,"Select all matching").y;
            let p=output.shapes.iter().find_map(|s|match &s.shape {egui::epaint::Shape::Text(t) if t.galley.job.text=="0x123" && t.pos.y<header_y && t.pos.y>p.y-200.0=>Some(t.pos+Vec2::new(6.0,6.0)),_=>None}).unwrap();
            draw(&mut app,pointer(p,true));draw(&mut app,pointer(p,false));
            let modifiers=egui::Modifiers{command:true,mac_cmd:true,..Default::default()};
            draw(&mut app,vec![egui::Event::Key{key:egui::Key::A,physical_key:None,pressed:true,repeat:false,modifiers},egui::Event::Text("0x456".into())]);
            let output=draw(&mut app,vec![]);let p=pos(&output,"0x456");draw(&mut app,pointer(p,true));draw(&mut app,pointer(p,false));
            assert_eq!(app.graphics_frame_id_filter.selected,Some(["0x123".into(),"0x456".into()].into_iter().collect()));
            let p=Pos2::new(1000.0,80.0);draw(&mut app,pointer(p,true));draw(&mut app,pointer(p,false));
            draw(&mut app,vec![]);let output=draw(&mut app,vec![]);
            assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.job.text=="2 messages · 0 signals")));
            for id in ["0x123","0x456"] {assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.job.text==id && t.pos.x<280.0)),"selected {id} must be visible in the browser");}
            assert!(!output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.job.text=="0x789" && t.pos.x<280.0)));
            let p=pos(&output,"CAN");draw(&mut app,pointer(p,true));draw(&mut app,pointer(p,false));
            draw(&mut app,vec![]);let output=draw(&mut app,vec![]);
            assert!(!output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.job.text=="0x123" && t.pos.x<280.0)),"manual collapse must remain closed after filter reveal");
        });
    }

    #[test]
    fn frame_id_filter_shows_static_ids_and_graph_reveal_preserves_other_selections() {
        session::with_test_storage(|dir| {
            let path=dir.join("all-ids.csv");
            std::fs::write(&path,"time,channel,id,payload\n0,1,123,00\n0,1,456,00\n0,2,789,00\n1,1,123,01\n1,1,456,00\n1,2,789,01\n").unwrap();
            let mut app=LogLensApp::initial_state();open(&mut app,path.clone());
            let d=app.dataset.clone().unwrap();
            app.project_channels=[project_channel_key("CAN",1)].into_iter().collect();
            let sources=app.cached_graphics_sources.clone();
            let static_source=sources.iter().find(|(_,_,ch,id,_)|*ch==1 && id.contains("456")).unwrap();
            app.graphics_only_changing=true;app.graphics_hide_constant=true;
            app.graphics_frame_id_filter.selected=Some([static_source.3.clone()].into_iter().collect());
            let ctx=egui::Context::default();
            let output=ctx.run(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(Pos2::ZERO,Vec2::new(2200.0,1300.0))),..Default::default()},|ctx|{
                egui::CentralPanel::default().show(ctx,|ui|app.canoe_graphics_viewer_content(ui,&d));
            });
            let has=|text:&str|output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.job.text==text));
            assert!(has("Frame IDs: 1 selected"));
            // Detailed totals are intentionally shown only inside the dropdown.
            assert!(!output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.job.text.contains("1/2"))));
            assert!(has("1 messages · 0 signals"),"explicit static ID must be available");
            let other=sources.iter().find(|(_,_,ch,id,_)|*ch==1 && id.contains("123")).unwrap();
            app.select_graphics_lane(&format!("SRC|{}|0",other.0));
            assert!(app.graphics_frame_id_filter.matches(&other.3));
            assert!(app.graphics_frame_id_filter.matches(&static_source.3));
            assert_eq!(app.selected_channel,Some(1));
            let expected=app.graphics_frame_id_filter.selected.clone();
            open(&mut app,path);
            assert_eq!(app.graphics_frame_id_filter.selected,expected,"reopening the same capture retains saved ID choices");
        });
    }

    #[test]
    fn viewer_filter_reveal_and_close_controls_execute_in_the_full_viewer() {
        session::with_test_storage(|dir| {
            let path=dir.join("viewer.csv");
            std::fs::write(&path,"time,channel,id,payload\n0,1,123,00\n0,2,456,00\n0,3,789,00\n1,1,123,01\n1,2,456,02\n1,3,789,00\n").unwrap();
            let mut app=LogLensApp::initial_state();open(&mut app,path);
            let d=app.dataset.clone().unwrap();
            app.project_channels=d.frames.iter().map(|f|project_channel_key(&f.bus_type.to_string(),f.channel_id)).collect();
            app.graphics_sources.clear();app.selected_frame_bytes.clear();
            let sources=app.cached_graphics_sources.clone();
            for (key,_,_,_,_) in &sources {app.set_graphics_source_byte(key,0,true);}
            app.graphics_stacked=true;app.graphics_viewer_open=true;app.graphics_hide_constant=false;app.graphics_hide_repetitive=false;
            app.graphics_frame_id_filter.selected=Some([sources[0].3.clone()].into_iter().collect());
            let ctx=egui::Context::default();let mut time=0.0;
            let mut draw=|app:&mut LogLensApp,events| {
                time+=0.2;
                ctx.run(egui::RawInput{time:Some(time),screen_rect:Some(egui::Rect::from_min_size(Pos2::ZERO,Vec2::new(2200.0,1300.0))),events,..Default::default()},|ctx| {
                    egui::CentralPanel::default().show(ctx,|ui|{app.canoe_graphics_viewer_content(ui,&d);});
                })
            };
            let output=draw(&mut app,vec![]);
            assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.job.text=="1 messages · 0 signals")));
            let second=&sources[1];app.select_graphics_lane(&format!("SRC|{}|0",second.0));
            draw(&mut app,vec![]);draw(&mut app,vec![]);let output=draw(&mut app,vec![]);
            assert!(app.graphics_frame_id_filter.matches(&second.3));assert_eq!(app.selected_channel,Some(second.2));
            assert!(app.graphics_reveal_source.is_none(),"selected message must be reached in the expanded browser");
            let label=graphics_frame_display_label(&second.3);
            assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.job.text==label)));
            let static_source=&sources[2];
            app.graphics_only_changing=true;
            app.select_graphics_lane(&format!("SRC|{}|0",static_source.0));
            draw(&mut app,vec![]);draw(&mut app,vec![]);let output=draw(&mut app,vec![]);
            assert!(app.graphics_reveal_source.is_none(),"graph selection must reveal even a message hidden by activity filters");
            let label=graphics_frame_display_label(&static_source.3);
            assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.job.text==label)));
            let close=output.shapes.iter().find_map(|s|if let egui::epaint::Shape::Text(t)=&s.shape {
                (t.galley.job.text=="Close viewer").then_some(t.pos+Vec2::new(4.0,5.0))
            }else{None}).unwrap();
            draw(&mut app,vec![egui::Event::PointerMoved(close),egui::Event::PointerButton{pos:close,button:egui::PointerButton::Primary,pressed:true,modifiers:egui::Modifiers::NONE}]);
            draw(&mut app,vec![egui::Event::PointerButton{pos:close,button:egui::PointerButton::Primary,pressed:false,modifiers:egui::Modifiers::NONE}]);
            assert!(!app.graphics_viewer_open);
        });
    }
    fn key_event(key:egui::Key,modifiers:egui::Modifiers)->egui::Event {
        egui::Event::Key{key,physical_key:None,pressed:true,repeat:false,modifiers}
    }
    #[test]
    fn trace_select_all_and_escape_work_in_both_modes() {
        session::with_test_storage(|dir| {
            let mut app=LogLensApp::initial_state();open(&mut app,capture(dir,"shortcuts.csv"));
            let d=app.dataset.clone().unwrap();app.cached_scoped_indices=vec![0,1];
            for overwrite in [false,true] {
                let ctx=egui::Context::default();
                let modifiers=if overwrite {egui::Modifiers::COMMAND}else{egui::Modifiers::CTRL};
                app.selected_trace_rows=BTreeSet::from([99]);
                app.trace_columns.selected_columns=BTreeSet::from([7]);
                let output=ctx.run(egui::RawInput{modifiers,events:vec![key_event(egui::Key::A,modifiers),egui::Event::Copy],..Default::default()},|ctx| {
                    egui::CentralPanel::default().show(ctx,|ui| {
                        if overwrite {app.canoe_trace_table(ui,&d,&[TraceRow{idx:1,count:2,dt_s:1.0},TraceRow{idx:0,count:3,dt_s:2.0}],&BTreeMap::new());}
                        else {app.canoe_trace_table_virtual(ui,&d,1,&BTreeMap::new());}
                    });
                });
                assert_eq!(app.selected_trace_rows,if overwrite {BTreeSet::from([0,1])}else{BTreeSet::from([0])});
                assert!(app.trace_columns.selected_columns.is_empty());
                let copied=output.platform_output.commands.iter().find_map(|cmd|if let egui::OutputCommand::CopyText(t)=cmd{Some(t)}else{None}).unwrap();
                assert!(copied.starts_with("Timestamp\tdt\tChannel\tProtocol\tID/Key\tDLC\tCount\tData\n"));
                assert_eq!(copied.lines().count(),if overwrite {3}else{2});
                app.trace_columns.selected_columns=BTreeSet::from([7]);
                let output=ctx.run(egui::RawInput{events:vec![key_event(egui::Key::Escape,egui::Modifiers::NONE),egui::Event::Copy],..Default::default()},|ctx| {
                    egui::CentralPanel::default().show(ctx,|ui| {
                        if overwrite {app.canoe_trace_table(ui,&d,&[TraceRow{idx:0,count:3,dt_s:2.0}],&BTreeMap::new());}
                        else {app.canoe_trace_table_virtual(ui,&d,1,&BTreeMap::new());}
                    });
                });
                assert!(app.selected_trace_rows.is_empty());assert!(app.trace_columns.selected_columns.is_empty());
                assert!(app.trace_row_anchor.is_none());
                assert!(!output.platform_output.commands.iter().any(|c|matches!(c,egui::OutputCommand::CopyText(_))));
            }
        });
    }
    #[test]
    fn trace_selection_shortcuts_do_not_intercept_focused_text_fields() {
        for (key,modifiers) in [(egui::Key::A,egui::Modifiers::CTRL),(egui::Key::A,egui::Modifiers::COMMAND)] {
            let ctx=egui::Context::default();let mut text="filter text".to_string();
            let _=ctx.run(Default::default(),|ctx| {
                egui::CentralPanel::default().show(ctx,|ui| {ui.text_edit_singleline(&mut text).request_focus();});
            });
            let _=ctx.run(egui::RawInput{modifiers,events:vec![key_event(key,modifiers)],..Default::default()},|ctx| {
                egui::CentralPanel::default().show(ctx,|ui| {
                    assert!(ctx.wants_keyboard_input());
                    assert_eq!(trace_selection_command(ui),None);
                    assert!(ui.input(|i|i.events.iter().any(|e|matches!(e,egui::Event::Key{key:k,..} if *k==key))));
                    ui.text_edit_singleline(&mut text);
                });
            });
        }
    }
    #[test]
    fn trace_copy_shortcut_uses_full_rows_and_current_mode_count() {
        session::with_test_storage(|dir| {
            let mut app=LogLensApp::initial_state(); open(&mut app,capture(dir,"copy.csv"));
            let d=app.dataset.clone().unwrap();
            app.cached_scoped_indices=vec![0,1];
            app.selected_trace_row=Some(0);app.selected_trace_rows=BTreeSet::from([0]);
            app.show_raw_ids=false;
            app.trace_columns.columns[7].visible=false;
            let names=BTreeMap::from([(1,"Channel\t\"one\"".to_string())]);
            for overwrite in [false,true] {
                let ctx=egui::Context::default();
                let output=ctx.run(egui::RawInput{events:vec![egui::Event::Copy],..Default::default()},|ctx| {
                    egui::CentralPanel::default().show(ctx,|ui| {
                        if overwrite {
                            app.canoe_trace_table(ui,&d,&[TraceRow{idx:0,count:42,dt_s:2.3}],&names);
                        } else { app.canoe_trace_table_virtual(ui,&d,2,&names); }
                    });
                });
                let copied=output.platform_output.commands.iter().find_map(|cmd| {
                    if let egui::OutputCommand::CopyText(text)=cmd {Some(text)} else {None}
                }).expect("Trace must emit a native clipboard command");
                assert!(copied.starts_with("Timestamp\tdt\tChannel\tProtocol\tID/Key\tDLC\tCount\tData\n"));
                assert!(copied.contains("\"Channel\t\"\"one\"\"\""));
                assert!(copied.ends_with(if overwrite {"\t42\t00"}else{"\t1\t00"}));
                if overwrite { assert!(copied.contains(&format!("\t{}\t",format_duration_s(2.3)))); }
            }
            // A selection removed from the scope must never copy an unrelated old row.
            app.selected_trace_row=Some(1);app.selected_trace_rows=BTreeSet::from([1]);
            let ctx=egui::Context::default();
            let output=ctx.run(egui::RawInput{events:vec![egui::Event::Copy],..Default::default()},|ctx| {
                egui::CentralPanel::default().show(ctx,|ui| {app.canoe_trace_table_virtual(ui,&d,1,&names);});
            });
            assert!(!output.platform_output.commands.iter().any(|c|matches!(c,egui::OutputCommand::CopyText(_))));
        });
    }
    #[test]
    fn trace_multirow_multicolumn_copy_preserves_table_order() {
        session::with_test_storage(|dir| {
            let mut app=LogLensApp::initial_state();open(&mut app,capture(dir,"multi.csv"));
            let d=app.dataset.clone().unwrap();
            app.selected_trace_rows=BTreeSet::from([0,1]);
            app.trace_columns.selected_columns=BTreeSet::from([6,7]);
            app.cached_scoped_indices=vec![0,1];
            for overwrite in [false,true] {
                let ctx=egui::Context::default();
                let output=ctx.run(egui::RawInput{events:vec![egui::Event::Copy],..Default::default()},|ctx| {
                    egui::CentralPanel::default().show(ctx,|ui| {
                        if overwrite {app.canoe_trace_table(ui,&d,&[TraceRow{idx:1,count:4,dt_s:1.0},TraceRow{idx:0,count:3,dt_s:2.0}],&BTreeMap::new());}
                        else {app.canoe_trace_table_virtual(ui,&d,2,&BTreeMap::new());}
                    });
                });
                let text=output.platform_output.commands.iter().find_map(|cmd|if let egui::OutputCommand::CopyText(t)=cmd{Some(t.as_str())}else{None}).unwrap();
                assert_eq!(text,if overwrite {"Count\tData\n4\t01\n3\t00"}else{"Count\tData\n1\t00\n1\t01"});
            }
            app.apply_loaded_dataset(capture(dir,"replacement.csv"),false,crate::engine::load(&capture(dir,"replacement.csv"),100).unwrap(),None,None);
            assert!(app.selected_trace_rows.is_empty());assert!(app.trace_row_anchor.is_none());
        });
    }
    #[test]
    fn trace_copy_retains_large_payload_without_display_clipping() {
        let payload=(0..1400).map(|n|format!("{:02X}",n%256)).collect::<Vec<_>>().join(" ");
        let row=TraceDisplayRow{timestamp:"time".into(),dt:"dt".into(),channel:"1".into(),
            protocol:"ANALOG".into(),frame_key:"ANALOG:1".into(),dlc:"1400".into(),data:payload.clone()};
        assert!(row.clipboard_row(1).ends_with(&payload));
    }
    fn capture(dir:&std::path::Path,name:&str)->PathBuf {
        let path=dir.join(name);
        std::fs::write(&path,"time,channel,id,payload\n0,1,123,00\n10,1,123,01\n").unwrap(); path
    }
    fn open(app:&mut LogLensApp,path:PathBuf) {
        let data=crate::engine::load(&path,100).unwrap();
        app.apply_loaded_dataset(path,false,data,None,Some(BTreeSet::from([1])));
    }
    fn finish(app:&mut LogLensApp) {
        let ctx=egui::Context::default(); let deadline=Instant::now()+Duration::from_secs(10);
        while app.background_load.is_some() {
            assert!(Instant::now()<deadline,"loader did not finish");
            app.poll_background_load(&ctx); std::thread::sleep(Duration::from_millis(5));
        }
    }
    #[test]
    fn loaded_capture_channel_selection_can_be_changed_and_reopened() {
        session::with_test_storage(|dir| {
            let path=dir.join("channels.pcap");
            let mut raw=vec![0u8;12];raw.extend(0x99feu16.to_be_bytes());
            raw.extend([0,1,0,1,3,3,0,0x20,0,0,0,0]);
            for channel in [1u32,2] {
                raw.extend(channel.to_be_bytes());raw.extend(1_760_000_000_000_000_000u64.to_be_bytes());
                raw.extend(2u16.to_be_bytes());raw.extend(0x5900u16.to_be_bytes());raw.extend(100i16.to_be_bytes());
            }
            let mut bytes=vec![0x4d,0x3c,0xb2,0xa1];
            bytes.extend(2u16.to_le_bytes());bytes.extend(4u16.to_le_bytes());bytes.extend([0u8;8]);
            bytes.extend(65535u32.to_le_bytes());bytes.extend(1u32.to_le_bytes());
            bytes.extend(1_760_000_000u32.to_le_bytes());bytes.extend(0u32.to_le_bytes());
            bytes.extend((raw.len() as u32).to_le_bytes());bytes.extend((raw.len() as u32).to_le_bytes());bytes.extend(raw);
            std::fs::write(&path,bytes).unwrap();
            let first=BTreeSet::from([1]);
            let d=crate::engine::load_selected_with_timeline(&path,100,Some(&first),true).unwrap();
            let mut app=LogLensApp::initial_state();app.apply_loaded_dataset(path.clone(),false,d,None,Some(first.clone()));
            let session_id=app.session.as_ref().unwrap().id.clone();
            app.project_config=TableData{filename:"retained-config".into(),headers:vec!["name".into()],rows:vec![vec!["A".into()]]};
            let before=Arc::clone(app.dataset.as_ref().unwrap());
            assert_eq!(app.trace_display_row(&before,0,&BTreeMap::new()).frame_key,"ANALOG:0x1");
            assert!(app.change_capture_channels());assert!(app.show_start_page);
            assert_eq!(app.pending_channels.iter().map(|(id,_)|*id).collect::<Vec<_>>(),vec![1,2]);
            assert_eq!(app.pending_channel_selection,first);
            assert!(Arc::ptr_eq(&before,app.dataset.as_ref().unwrap()),"opening selection keeps loaded data");
            app.tecmp_source_time=false; // Draft clock selection must also be discarded.
            app.return_to_workspace();
            assert!(app.tecmp_source_time);assert!(app.pending_log_path.is_none());
            assert_eq!(app.active_preload_channels,first);
            assert!(app.change_capture_channels());
            app.pending_channel_selection=BTreeSet::from([2]);
            assert!(app.load_path(path,None));finish(&mut app);
            assert!(app.error.is_none());assert_eq!(app.session.as_ref().unwrap().id,session_id);
            assert_eq!(app.project_config.filename,"retained-config");
            assert_eq!(app.active_preload_channels,BTreeSet::from([2]));
            assert_eq!(app.dataset.as_ref().unwrap().frames[0].channel_id,2);
            let changed=Arc::clone(app.dataset.as_ref().unwrap());
            assert_eq!(app.trace_display_row(&changed,0,&BTreeMap::new()).frame_key,"ANALOG:0x2");
            assert!(app.graphics_sources.contains("ANALOG|2|ANALOG:2"));
            assert!(app.change_capture_channels());
            assert_eq!(app.pending_channels.len(),2);
            assert_eq!(app.pending_channel_selection,BTreeSet::from([2]));
            app.return_to_workspace();
            assert!(app.prepare_capture_selection(app.path.clone().unwrap(),None));
            assert!(app.show_start_page,"Open from the workspace must show the checklist");
            assert_eq!(app.pending_channel_selection,BTreeSet::from([1,2]));
        });
    }

    #[test]
    fn chronological_trace_preserves_timestamp_order_when_columns_are_hidden() {
        session::with_test_storage(|dir| {
            let path=dir.join("unordered.csv");
            std::fs::write(&path,"time,channel,id,payload\n3,2,123,AA\n1,1,123,BB\n1,2,123,CC\n2,1,123,DD\n").unwrap();
            let d=crate::engine::load(&path,100).unwrap();
            let mut app=LogLensApp::initial_state();assert!(!app.trace_overwrite);
            app.apply_loaded_dataset(path,false,d,None,None);
            app.trace_overwrite=false;app.trace_columns.columns[0].visible=false;
            app.ensure_filter_cache();app.ensure_scope_cache();
            let d=Arc::clone(app.dataset.as_ref().unwrap());
            let indices=app.cached_scoped_indices.clone();
            assert_eq!(indices.len(),4);
            assert!(indices.windows(2).all(|p|d.frames[p[0]].timestamp_ns<=d.frames[p[1]].timestamp_ns));
            assert_eq!(indices.iter().map(|i|d.frames[*i].sequence).collect::<Vec<_>>(),vec![1,2,3,0]);
            let ctx=egui::Context::default();
            let _=ctx.run(Default::default(),|ctx| {egui::CentralPanel::default().show(ctx,|ui| {
                app.canoe_trace_table_virtual(ui,&d,indices.len(),&BTreeMap::new());
            });});
            assert_eq!(app.cached_scoped_indices,indices,"changing visible columns cannot reorder rows");
            let count=indices.partition_point(|i|d.timeline_s[*i]<=0.5);
            assert_eq!(count,2,"replay includes the complete equal-time group in chronological order");
        });
    }

    #[test]
    fn switching_a_b_a_restores_analysis_and_configuration() {
        session::with_test_storage(|dir| {
            let mut app=LogLensApp::initial_state();
            let a=capture(dir,"a.csv"); let b=capture(dir,"b.csv"); open(&mut app,a.clone());
            let meta_a=app.session.clone().unwrap();
            app.workspace=Workspace::Analysis; app.analysis_page=AnalysisPage::AbCompare;
            app.cursor_a_s=2.0; app.cursor_b_s=7.0;
            app.graphics_view_start_s=1.0; app.graphics_view_end_s=8.0;
            app.graphics_range_start_s=Some(2.0); app.graphics_range_end_s=Some(7.0);
            app.graphics_multi_cursors.insert("lane".into(),vec![2.0,4.0]);
            app.graphics_y_scale.insert("lane".into(),2.5);
            app.selected_frame_bytes.insert("CAN|1|0x123".into(),BTreeSet::from([0]));
            app.graphics_lane_order=vec!["lane".into()]; app.filter.text="123".into();
            app.project_channels.clear(); // An intentionally empty scope must stay empty.
            app.project_config=TableData{filename:"a-config".into(),headers:vec!["name".into()],rows:vec![vec!["A".into()]]};
            let expected=serde_json::to_value(app.workspace_snapshot()).unwrap();
            open(&mut app,b);
            let meta_b=app.session.clone().unwrap();
            assert!(app.filter.text.is_empty()); assert!(app.project_config.is_empty());
            app.cursor_a_s=5.0;
            app.load_saved_session(meta_a.clone());
            assert_eq!(app.session.as_ref().unwrap().id,meta_b.id); // Still AWAITING A, B remains active.
            finish(&mut app);
            assert_eq!(app.session.as_ref().unwrap().id,meta_a.id);
            assert_eq!(serde_json::to_value(app.workspace_snapshot()).unwrap(),expected);
            assert_eq!(app.project_config.filename,"a-config");
            assert_eq!(session::load_workspace(&meta_b.id).unwrap().unwrap().cursor_a_s,5.0);
            assert_eq!(std::fs::read(&a).unwrap(),std::fs::read(&meta_a.source_path).unwrap());
        });
    }
    #[test]
    fn failed_background_load_keeps_current_session_and_analysis() {
        session::with_test_storage(|dir| {
            let mut app=LogLensApp::initial_state(); open(&mut app,capture(dir,"a.csv"));
            app.cursor_a_s=6.0; let id=app.session.as_ref().unwrap().id.clone();
            let bad=dir.join("bad.pcap"); std::fs::write(&bad,b"bad").unwrap();
            let target=session::create(&bad).unwrap(); app.load_saved_session(target.clone()); finish(&mut app);
            assert!(app.error.is_some()); assert_eq!(app.session.as_ref().unwrap().id,id);
            assert_eq!(app.cursor_a_s,6.0); assert!(session::load_workspace(&target.id).unwrap().is_none());
        });
    }
    #[test]
    fn exit_flush_restores_latest_changes_in_fresh_app() {
        session::with_test_storage(|dir| {
            let mut app=LogLensApp::initial_state(); open(&mut app,capture(dir,"a.csv"));
            app.cursor_b_s=9.0; app.graphics_viewer_open=true;
            app.trace_columns.columns[2].visible=false;
            app.trace_columns.columns[7].width=620.0;
            let meta=app.session.clone().unwrap();
            eframe::App::on_exit(&mut app,None);
            let mut restarted=LogLensApp::initial_state(); restarted.load_saved_session(meta); finish(&mut restarted);
            assert_eq!(restarted.cursor_b_s,9.0); assert!(restarted.graphics_viewer_open);
            assert!(!restarted.measurement_running);
            assert!(!restarted.trace_columns.columns[2].visible);
            assert_eq!(restarted.trace_columns.columns[7].width,620.0);
        });
    }
}

#[cfg(test)]
mod viewer_upgrade_tests {
    use super::*;
    #[test]
    fn graph_header_icons_align_and_rename_editor_saves_display_alias() {
        session::with_test_storage(|_| {
            let ctx=egui::Context::default();let mut app=LogLensApp::initial_state();let lane="SIG|Original";
            app.graph_rename=Some((None,lane.into(),"Old".into()));let mut time=0.0;
            let mut draw=|app:&mut LogLensApp,events| {
                time+=0.2;
                ctx.run(egui::RawInput{time:Some(time),screen_rect:Some(egui::Rect::from_min_size(Pos2::ZERO,Vec2::new(700.0,500.0))),events,..Default::default()},|ctx| {
                    app.graph_rename_window(ctx);
                    egui::CentralPanel::default().show(ctx,|ui| {
                        app.graph_lane_header_ui(ui,lane,"Original");
                        app.graph_lane_header_ui(ui,"BIT|CAN|1|0x123|0|3",&"Long graph identity ".repeat(20));
                    });
                })
            };
            draw(&mut app,vec![]);let output=draw(&mut app,vec![]);
            let text_pos=|output:&egui::FullOutput,text:&str|output.shapes.iter().find_map(|s|match &s.shape {
                egui::epaint::Shape::Text(t) if t.galley.job.text==text=>Some(t.pos+Vec2::new(3.0,5.0)),_=>None
            }).unwrap_or_else(||panic!("missing {text}"));
            let click=|p,pressed|egui::Event::PointerButton{pos:p,button:egui::PointerButton::Primary,pressed,modifiers:egui::Modifiers::NONE};
            let p=text_pos(&output,"Old");draw(&mut app,vec![egui::Event::PointerMoved(p),click(p,true)]);draw(&mut app,vec![click(p,false)]);
            let modifiers=egui::Modifiers{ctrl:true,command:true,..Default::default()};
            draw(&mut app,vec![egui::Event::Key{key:egui::Key::A,physical_key:None,pressed:true,repeat:false,modifiers},egui::Event::Text("Voltage".into())]);
            let output=draw(&mut app,vec![]);let p=text_pos(&output,"Save");draw(&mut app,vec![egui::Event::PointerMoved(p),click(p,true)]);draw(&mut app,vec![click(p,false)]);
            assert!(app.graph_rename.is_none());assert_eq!(app.graph_lane_names[lane],"Voltage");
            let output=draw(&mut app,vec![]);text_pos(&output,"Voltage");
            let centers=|text:&str|output.shapes.iter().filter_map(|s|match &s.shape {
                egui::epaint::Shape::Text(t) if t.galley.job.text==text=>Some(t.pos+t.galley.size()/2.0),_=>None
            }).collect::<Vec<_>>();
            let close=centers("×");let drag=centers("Drag");assert_eq!(close.len(),2);assert_eq!(drag.len(),2);
            for (x,handle) in close.iter().zip(&drag) {assert!(x.x>660.0);assert!((x.y-handle.y).abs()<1.0,"close icon and handle must share one row");}
        });
    }
    #[test]
    fn overlay_selection_finds_nearest_trace() {
        let traces=vec![("one".into(),vec![[0.0,10.0],[1.0,20.0]]),("two".into(),vec![[0.0,100.0],[1.0,80.0]])];
        assert_eq!(nearest_overlay_trace(&traces,0.5,98.0),Some(1));
        assert_eq!(nearest_overlay_trace(&traces,1.0,22.0),Some(0));
    }
    #[test]
    fn graph_selection_reveals_exact_channel_and_decoded_message() {
        let mut app=LogLensApp::initial_state();
        app.project_channels=BTreeSet::from(["CAN|1".into(),"CAN|2".into()]);
        app.cached_graphics_sources=vec![("CAN|1|0x123".into(),"CAN".into(),1,"0x123".into(),2),("CAN|2|0x123".into(),"CAN".into(),2,"0x123".into(),2)];
        app.graphics_frame_id_filter.selected=Some(BTreeSet::new());app.graphics_show_signal_browser=false;
        for lane in ["SRC|CAN|2|0x123|1","BIT|CAN|2|0x123|1|3","FRAME|CAN|2|0x123"] {
            app.apply_lane_cursor_action(lane,Some(LaneCursorAction::Select));
            assert_eq!(app.selected_channel,Some(2));assert_eq!(app.selected_frame_key,"0x123");
            assert_eq!(app.graphics_selected_source.as_deref(),Some("CAN|2|0x123"));
            assert_eq!(app.graphics_reveal_source,app.graphics_selected_source);
            assert!(app.graphics_frame_id_filter.matches("0x123"));assert!(app.graphics_show_signal_browser);
        }
        app.signals=vec![SignalDefinition{name:"Speed".into(),protocol:"CAN".into(),channel:"1".into(),frame_id:"0x123".into(),..Default::default()}];
        app.select_graphics_lane("SIG|Speed");assert_eq!(app.selected_channel,Some(1));
        assert_eq!(app.graphics_selected_signal.as_deref(),Some("Speed"));
        assert_eq!(graphics_lane_source("SRC|ETH|3|ETH:UDP:a:1>b:2|0"),Some("ETH|3|ETH:UDP:a:1>b:2".into()));
    }
    #[test]
    fn closing_lanes_does_not_remove_other_waveforms_or_resurrect_closed_ones() {
        let mut app=LogLensApp::initial_state();let source="CAN|1|0x123";
        app.graphics_sources.insert(source.into());app.selected_frame_bytes.insert(source.into(),BTreeSet::from([0,1]));
        app.graphics_whole_frames.insert(source.into());app.graphics_bits.insert(format!("{}|0|3",source));
        app.graphics_signals.insert("Speed".into());app.sync_graphics_lane_order();
        app.graphics_multi_cursors.insert(format!("SRC|{}|0",source),vec![1.0]);
        app.remove_graphics_lane(&format!("SRC|{}|0",source));app.sync_graphics_lane_order();
        assert!(!app.graphics_lane_order.contains(&format!("SRC|{}|0",source)));
        assert!(app.graphics_lane_order.contains(&format!("SRC|{}|1",source)));
        assert!(app.graphics_lane_order.contains(&format!("FRAME|{}",source)));
        assert!(app.graphics_multi_cursors.is_empty());
        app.remove_graphics_lane(&format!("SRC|{}|1",source));app.sync_graphics_lane_order();
        assert!(app.graphics_sources.contains(source));assert!(app.graphics_lane_order.contains(&format!("FRAME|{}",source)));
        app.remove_graphics_lane(&format!("FRAME|{}",source));app.remove_graphics_lane(&format!("BIT|{}|0|3",source));
        app.sync_graphics_lane_order();assert!(!app.graphics_sources.contains(source));
        assert_eq!(app.graphics_lane_order,vec!["SIG|Speed".to_string()]);
        app.remove_graphics_lane("SIG|Speed");app.sync_graphics_lane_order();assert!(app.graphics_lane_order.is_empty());
    }
    #[test]
    fn narrow_cm_ruler_keeps_edge_labels_inside_its_bounds() {
        let ctx=egui::Context::default();
        let event=ControlTrigger{frame_index:0,time_s:1.0,control_message_id:7,tecmp_cm_id:7,occurrence:1,label:"CM 0x0007".into()};
        for width in [260.0,600.0] {
            let output=ctx.run(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(Pos2::ZERO,Vec2::new(width,200.0))),..Default::default()},|ctx| {
                egui::CentralPanel::default().show(ctx,|ui|draw_xviewer_time_ruler(ui,0.0,1.0,0,0.0,0.5,std::slice::from_ref(&event)));
            });
            let rect=output.shapes.iter().find_map(|s|match &s.shape {
                egui::epaint::Shape::Rect(r) if r.fill==Color32::from_gray(24)=>Some(r.rect),_=>None
            }).unwrap();
            let mut saw_cm=false;
            for shape in &output.shapes {
                if let egui::epaint::Shape::Text(t)=&shape.shape {
                    if t.galley.job.text=="CM 0007" {saw_cm=true;}
                    assert!(t.pos.x>=rect.left()-0.5 && t.pos.x+t.galley.size().x<=rect.right()+0.5,"label {} extends outside ruler",t.galley.job.text);
                }
            }
            assert!(saw_cm);
        }
    }
    #[test]
    fn cm_ruler_and_waveforms_stay_aligned_when_cursor_pane_is_resized() {
        session::with_test_storage(|dir| {
            let path=dir.join("ruler-layout.csv");
            std::fs::write(&path,"time,channel,id,payload\n0,1,123,00\n1,1,123,01\n").unwrap();
            let mut app=LogLensApp::initial_state();let data=crate::engine::load(&path,100).unwrap();
            app.apply_loaded_dataset(path,false,data,None,Some(BTreeSet::from([1])));
            let d=app.dataset.clone().unwrap();
            let name="Very long configured engineering signal name ".repeat(15);
            app.signals=vec![SignalDefinition{name:name.clone(),protocol:"CAN".into(),channel:"1".into(),frame_id:app.cached_graphics_sources[0].3.clone(),bit_length:8,..Default::default()}];
            app.graphics_sources.clear();app.graphics_whole_frames.clear();app.graphics_bits.clear();
            app.graphics_signals=BTreeSet::from([name]);app.graphics_show_signal_browser=false;
            app.graphics_stacked=true;app.graphics_lane_height=140.0;
            app.graphics_view_start_s=0.0;app.graphics_view_end_s=1.0;
            let ctx=egui::Context::default();let mut time=0.0;
            for screen_width in [1200.0,1800.0] {
                for cursor_width in [280.0,520.0,760.0] {
                    app.graphics_cursor_panel_width=cursor_width;
                    let mut output=None;
                    for _ in 0..3 {
                        time+=0.2;
                        output=Some(ctx.run(egui::RawInput{time:Some(time),screen_rect:Some(egui::Rect::from_min_size(Pos2::ZERO,Vec2::new(screen_width,900.0))),..Default::default()},|ctx| {
                            egui::CentralPanel::default().show(ctx,|ui|app.canoe_graphics_viewer_content(ui,&d));
                        }));
                    }
                    let output=output.unwrap();
                    let rectangle=|gray:u8,height:f32|output.shapes.iter().find_map(|s|match &s.shape {
                        egui::epaint::Shape::Rect(r) if r.fill==Color32::from_gray(gray) && (r.rect.height()-height).abs()<0.5=>Some(r.rect),_=>None
                    }).unwrap_or_else(||panic!("missing rectangle {gray}/{height}"));
                    let ruler=rectangle(24,54.0);let plot=rectangle(20,140.0);
                    assert!((ruler.left()-plot.left()).abs()<1.0);
                    assert!((ruler.right()-plot.right()).abs()<1.0,"ruler {:?} vs plot {:?}, screen {screen_width}, values {cursor_width}",ruler,plot);
                    let values_x=output.shapes.iter().find_map(|s|match &s.shape {
                        egui::epaint::Shape::Text(t) if t.galley.job.text=="CURSOR VALUES"=>Some(t.pos.x),_=>None
                    }).unwrap();
                    assert!(plot.right()<values_x,"waveform must stay left of Cursor Values");
                    assert!(values_x+cursor_width<=screen_width+1.0,"cursor pane must fit window");
                }
            }
        });
    }
    #[test]
    fn full_viewer_accepts_plot_area_drops_and_swaps_adjacent_lanes() {
        session::with_test_storage(|dir| {
            let path=dir.join("lane-drops.csv");
            std::fs::write(&path,"time,channel,id,payload\n0,1,123,00\n0,1,456,00\n0,1,789,00\n1,1,123,01\n1,1,456,02\n1,1,789,03\n").unwrap();
            let mut app=LogLensApp::initial_state();
            let data=crate::engine::load(&path,100).unwrap();
            app.apply_loaded_dataset(path,false,data,None,Some(BTreeSet::from([1])));
            let d=app.dataset.clone().unwrap();
            app.graphics_sources.clear();app.selected_frame_bytes.clear();app.graphics_lane_order.clear();
            app.graphics_signals.clear();app.graphics_bits.clear();app.graphics_whole_frames.clear();
            for source in app.cached_graphics_sources.clone() {app.set_graphics_source_byte(&source.0,0,true);}
            app.graphics_stacked=true;app.graphics_hide_constant=false;app.graphics_hide_repetitive=false;
            app.graphics_only_changing=false;app.graphics_lane_height=140.0;
            let ctx=egui::Context::default();let mut time=0.0;
            let mut draw=|app:&mut LogLensApp,events| {
                time+=0.2;
                ctx.run(egui::RawInput{time:Some(time),screen_rect:Some(egui::Rect::from_min_size(Pos2::ZERO,Vec2::new(2200.0,1500.0))),events,..Default::default()},|ctx| {
                    egui::CentralPanel::default().show(ctx,|ui|app.canoe_graphics_viewer_content(ui,&d));
                })
            };
            let handles=|output:&egui::FullOutput| {
                let mut points:Vec<Pos2>=output.shapes.iter().filter_map(|s|match &s.shape {
                    egui::epaint::Shape::Text(t) if t.galley.job.text=="Drag"=>Some(t.pos+Vec2::new(5.0,5.0)),_=>None
                }).collect();points.sort_by(|a,b|a.y.total_cmp(&b.y));assert_eq!(points.len(),3);points
            };
            let button=|pos,pressed|egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed,modifiers:egui::Modifiers::NONE};
            draw(&mut app,vec![]);let output=draw(&mut app,vec![]);
            let original=app.graphics_lane_order.clone();let points=handles(&output);
            // First lane onto the next plot body: this previously left the order unchanged.
            let start=points[0];let target=points[1]+Vec2::new(250.0,80.0);
            draw(&mut app,vec![egui::Event::PointerMoved(start),button(start,true)]);
            draw(&mut app,vec![egui::Event::PointerMoved(target)]);
            assert_eq!(app.graphics_drag_lane.as_deref(),Some(original[0].as_str()));
            draw(&mut app,vec![button(target,false)]);
            assert_eq!(app.graphics_lane_order,vec![original[1].clone(),original[0].clone(),original[2].clone()]);
            let output=draw(&mut app,vec![]);let points=handles(&output);
            let start=points[1];let target=points[0]+Vec2::new(250.0,80.0);
            draw(&mut app,vec![egui::Event::PointerMoved(start),button(start,true)]);
            draw(&mut app,vec![egui::Event::PointerMoved(target)]);
            draw(&mut app,vec![button(target,false)]);
            assert_eq!(app.graphics_lane_order,original);
            assert!(app.graphics_drag_lane.is_none());
        });
    }
    #[test]
    fn drag_handles_use_hand_cursor_and_reorder_in_both_directions() {
        let ctx=egui::Context::default();
        let mut app=LogLensApp::initial_state();
        app.graphics_lane_order=vec!["A".into(),"B".into(),"C".into()];
        let mut time=0.0;
        let mut draw=|app:&mut LogLensApp,events| {
            time+=0.1;
            let mut request=None;
            let mut rects=BTreeMap::new();
            let mut lanes=Vec::new();
            let output=ctx.run(egui::RawInput{time:Some(time),screen_rect:Some(egui::Rect::from_min_size(Pos2::ZERO,Vec2::new(700.0,400.0))),events,..Default::default()},|ctx| {
                egui::CentralPanel::default().show(ctx,|ui| {
                    for lane in app.graphics_lane_order.clone() {
                        let r=graphics_lane_drag_handle(ui,&lane,&mut app.graphics_drag_lane);
                        rects.insert(lane.clone(),r.rect);ui.add_space(50.0);
                        lanes.push((lane,r.rect.top(),ui.cursor().top()));
                    }
                    request=graphics_lane_drop_target(ui,app.graphics_drag_lane.as_deref(),&app.graphics_lane_order,&lanes,ui.max_rect());
                });
            });
            if let Some((moving,target))=request {app.move_graphics_lane_to_target(&moving,&target);}
            if ctx.input(|i|i.pointer.any_released()) {app.graphics_drag_lane=None;}
            (output,rects)
        };
        draw(&mut app,vec![]);
        let (_,rects)=draw(&mut app,vec![]);
        let a=rects["A"].center()+Vec2::new(300.0,30.0);let c=rects["C"].center();
        let (output,_)=draw(&mut app,vec![egui::Event::PointerMoved(c)]);
        assert_eq!(output.platform_output.cursor_icon,egui::CursorIcon::Grab);
        let click=|pos,pressed|egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed,modifiers:egui::Modifiers::NONE};
        draw(&mut app,vec![click(c,true)]);
        let (output,_)=draw(&mut app,vec![egui::Event::PointerMoved(c+Vec2::new(0.0,-20.0))]);
        assert_eq!(app.graphics_drag_lane.as_deref(),Some("C"));
        assert_eq!(output.platform_output.cursor_icon,egui::CursorIcon::Grabbing);
        draw(&mut app,vec![egui::Event::PointerMoved(a)]);
        draw(&mut app,vec![click(a,false)]);
        assert_eq!(app.graphics_lane_order,vec!["C","A","B"]);
        assert!(app.graphics_drag_lane.is_none());
        let (_,rects)=draw(&mut app,vec![]);
        let c=rects["C"].center();let b=rects["B"].center()+Vec2::new(300.0,30.0);
        draw(&mut app,vec![egui::Event::PointerMoved(c),click(c,true)]);
        draw(&mut app,vec![egui::Event::PointerMoved(b)]);
        draw(&mut app,vec![click(b,false)]);
        assert_eq!(app.graphics_lane_order,vec!["A","B","C"]);
        let (_,rects)=draw(&mut app,vec![]);let a=rects["A"].center();let outside=Pos2::new(500.0,300.0);
        draw(&mut app,vec![egui::Event::PointerMoved(a),click(a,true)]);
        draw(&mut app,vec![egui::Event::PointerMoved(outside)]);
        draw(&mut app,vec![click(outside,false)]);
        assert_eq!(app.graphics_lane_order,vec!["A","B","C"]);
        assert!(app.graphics_drag_lane.is_none());
    }
    fn lane_frame(ctx:&egui::Context,events:Vec<egui::Event>)->(egui::FullOutput,Option<LaneCursorAction>) {
        let mut action=None;let(mut a,mut b,mut start,mut end,mut ys,mut yo)=(0.0,1.0,0.0,2.0,1.0,0.0);
        let(mut drag,mut active,mut rs,mut re,mut mode,mut interaction,mut last)=(None,'A',None,None,false,0,None);
        let output=ctx.run(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(Pos2::ZERO,Vec2::new(700.0,400.0))),events,..Default::default()},|ctx| {
            egui::CentralPanel::default().show(ctx,|ui| {
                let (_,close)=draw_graph_lane_header(ui,"SRC|CAN|1|0x123|0","CAN message","CAN message",&mut None);
                action=draw_canoe_lane_interactive(ui,"CAN message","SRC|CAN|1|0x123|0",&[[0.0,1.0],[2.0,3.0]],160.0,
                    &mut a,&mut b,&mut drag,&mut active,"",0.0,&[],None,true,&mut start,&mut end,2.0,&mut ys,&mut yo,
                    GraphicsDensity::Standard,&mut rs,&mut re,&mut mode,&mut interaction,&mut last,true,true,true,true,1.0,false,false,&[],None);
                if close{action=Some(LaneCursorAction::Remove);}
            });
        });(output,action)
    }
    #[test]
    fn graph_pointer_click_and_close_button_emit_actions() {
        let ctx=egui::Context::default();let(output,_)=lane_frame(&ctx,vec![]);
        let close=output.shapes.iter().find_map(|s|if let egui::epaint::Shape::Text(t)=&s.shape {
            (t.galley.job.text=="×").then_some(t.pos+Vec2::new(4.0,5.0))
        }else{None}).unwrap();
        assert!(close.x>650.0,"close icon must be aligned to the right edge of the graph");
        assert!(!output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.job.text=="Close graph")));
        let click=|pos,pressed|egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed,modifiers:egui::Modifiers::NONE};
        let pos=Pos2::new(360.0,100.0);
        lane_frame(&ctx,vec![egui::Event::PointerMoved(pos),click(pos,true)]);
        let(_,action)=lane_frame(&ctx,vec![click(pos,false)]);assert!(matches!(action,Some(LaneCursorAction::Select)));
        lane_frame(&ctx,vec![egui::Event::PointerMoved(close),click(close,true)]);
        let(_,action)=lane_frame(&ctx,vec![click(close,false)]);assert!(matches!(action,Some(LaneCursorAction::Remove)));
    }
}

#[cfg(test)]
mod graph_navigation_tests {
    use super::*;

    #[test]
    fn byte_bit_and_signal_headers_keep_channel_identity() {
        let byte=graphics_lane_header("CAN","Powertrain (7)","0x123","Byte 2");
        let bit=graphics_lane_header("CAN","Powertrain (7)","0x123","Byte 2 · Bit 3");
        let signal=SignalDefinition{protocol:"CAN".into(),channel:"7".into(),frame_id:"0x123".into(),name:"Speed".into(),unit:"km/h".into(),..Default::default()};
        for title in [byte,bit,graphics_signal_header(&signal,&|_|"Powertrain (7)".into())] {
            assert!(title.starts_with("CAN · Powertrain (7) · 0x123 ·"),"{title}");
        }
        let wildcard=SignalDefinition{channel:"*".into(),..Default::default()};
        assert!(graphics_signal_header(&wildcard,&|_|panic!("wildcard is not a channel ID")).contains("All channels"));
    }

    #[test]
    fn scrollbar_outside_plot_and_mouse_wheel_both_move_lanes() {
        let ctx=egui::Context::default();
        ctx.style_mut(|s|s.spacing.scroll=egui::style::ScrollStyle::solid());
        let mut offset=0.0;
        let mut frame=|events:Vec<egui::Event>| {
            let mut rect=egui::Rect::NOTHING;
            let input=egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(Pos2::ZERO,Vec2::new(400.0,300.0))),events,..Default::default()};
            let _=ctx.run(input,|ctx| { egui::CentralPanel::default().show(ctx,|ui| {
                let result=graphics_lane_scroll_area(offset,200.0).show(ui,|ui| {
                    for n in 0..20 { ui.allocate_ui(Vec2::new(200.0,60.0),|ui|{ui.label(format!("Lane {n}"));}); }
                });
                offset=result.state.offset.y;
                rect=result.inner_rect;
            }); });
            (offset,rect)
        };
        frame(vec![]);
        let (_,rect)=frame(vec![]);
        let bar=Pos2::new(rect.right()+7.0,rect.top()+130.0);
        assert!(!rect.contains(bar));
        frame(vec![egui::Event::PointerMoved(bar)]);
        let (clicked,_)=frame(vec![egui::Event::PointerButton{pos:bar,button:egui::PointerButton::Primary,pressed:true,modifiers:egui::Modifiers::NONE}]);
        assert!(clicked>0.0,"scrollbar must move while pointer is outside plot");
        let (released,_)=frame(vec![egui::Event::PointerButton{pos:bar,button:egui::PointerButton::Primary,pressed:false,modifiers:egui::Modifiers::NONE}]);
        assert!(released>0.0,"scroll position must survive release");
        let (up,_)=frame(vec![egui::Event::PointerMoved(rect.center()),egui::Event::MouseWheel{unit:egui::MouseWheelUnit::Point,delta:Vec2::new(0.0,80.0),modifiers:egui::Modifiers::NONE}]);
        assert!(up<released,"wheel must scroll upwards");
    }
}

#[cfg(test)]
mod cursor_layout_tests {
    use super::*;
    #[test]
    fn long_names_and_eight_cursors_fit_narrow_lane_cards() {
        let ctx=egui::Context::default();
        for multi_mode in [false,true] {
            let input=egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(Pos2::ZERO,Vec2::new(400.0,300.0))),..Default::default()};
            let _=ctx.run(input,|ctx| {egui::CentralPanel::default().show(ctx,|ui| {
                let rect=egui::Rect::from_min_size(ui.cursor().min,Vec2::new(280.0,104.0));
                #[allow(deprecated)]
                ui.allocate_ui_at_rect(rect,|ui| {
                    let values=(0..8).map(|n|(n as f64, "12345678901234567890 engineering units".into())).collect::<Vec<_>>();
                    draw_cursor_value_card(ui,"lane","CAN · Very long configured Powertrain channel name (12345) · 0x12345678 · Byte 20 · Bit 7",
                        if multi_mode {None}else{Some(["12345678901234567890","12345678901234567890","+12345678901234567890"])},&values,rect.height());
                    assert!(ui.min_rect().right()<=rect.right()+0.5,"card overflowed horizontally: {:?}",ui.min_rect());
                    assert!(ui.min_rect().bottom()<=rect.bottom()+0.5,"card overflowed vertically: {:?}",ui.min_rect());
                });
            });});
        }
    }
}

#[cfg(test)]
mod tecmp_graph_regressions {
    use super::*;
    fn data()->Dataset {
        let file=tempfile::Builder::new().suffix(".csv").tempfile().unwrap();
        let mut text="time,channel,id,payload\n".to_string();
        for i in 0..10_000 {text.push_str(&format!("{},1,123,{:02X}\n{},2,123,FF\n",i as f64*0.001,if i==5001{128}else{0},i as f64*0.001));}
        std::fs::write(file.path(),text).unwrap();crate::engine::load(file.path(),30_000).unwrap()
    }
    fn signal(protocol:&str)->SignalDefinition {
        SignalDefinition{protocol:protocol.into(),name:"Test".into(),channel:"1".into(),frame_id:"0x123".into(),start_byte:0,start_bit:0,bit_length:8,endian:"Intel".into(),signed:false,data_type:String::new(),scale:1.0,offset:0.0,unit:String::new()}
    }
    #[test]
    fn configured_signal_cursor_uses_original_sample_and_zoom_recomputes(){
        let d=data();let s=signal("CAN");let scope=BTreeSet::from(["CAN|1".into()]);let mut app=LogLensApp::initial_state();
        let indices=app.cached_signal_indices(&d,&s,&scope);
        assert_eq!(decoded_signal_sample_at(&d,&indices,&s,5.001).unwrap()[1],128.0);
        assert_eq!(decoded_signal_sample_at(&d,&indices,&s,5.002).unwrap()[1],0.0);
        app.graphics_view_start_s=5.0;app.graphics_view_end_s=5.003;
        let points=app.cached_decoded_signal_waveform(&d,&s,&scope);assert!(points.len()<12);assert!(points.iter().any(|p|p[1]==128.0));
        app.graphics_view_start_s=9.0;app.graphics_view_end_s=9.003;
        let other=app.cached_decoded_signal_waveform(&d,&s,&scope);assert!(!other.iter().any(|p|p[1]==128.0));
        assert!(decoded_signal_indices(&d,&s,&BTreeSet::new()).is_empty());
    }
    #[test]
    fn large_analog_zoom_keeps_samples_discarded_by_the_display_cache(){
        let mut d=data();d.frames.truncate(1);
        let f=&mut d.frames[0];f.bus_type=BusType::Analog;f.channel_id=1;f.frame_key="ANALOG:1".into();f.relative_time_s=0.0;
        let count=resources::limits().analog_point_limit+100;
        f.payload=(0..count).flat_map(|i|([0i16,123,1000][i%3]).to_be_bytes()).collect();
        f.analog=Some(crate::model::AnalogMeta{sample_count:count,sample_width_bytes:2,sample_period_ns:1_000_000,factor:1.0,unit:"V".into()});
        d.index=crate::index::FrameIndex::build(&d.frames);
        let mut app=LogLensApp::initial_state();
        let points=app.analog_view_points(&d,"ANALOG:1",1,0.0005,0.0015);
        assert!(points.iter().any(|p|p[0]==0.001 && p[1]==123.0),"zoom must decode the original middle sample");
        assert_eq!(analog_sample_at(&d,"ANALOG:1",1,0.0015),Some([0.001,123.0]));
        assert!(app.analog_view_points(&d,"ANALOG:1",1,0.0,count as f64*0.001).len()<=12_004);
    }

    #[test]
    fn configured_signal_fixed_y_range_remains_stable_while_zooming(){
        let d=data();let s=signal("CAN");let scope=BTreeSet::from(["CAN|1".into()]);let mut app=LogLensApp::initial_state();
        app.graphics_view_start_s=5.0;app.graphics_view_end_s=5.003;
        let range=app.cached_decoded_signal_range(&d,&s,&scope).unwrap();assert!(range.0<0.0 && range.1>128.0);
        app.graphics_view_start_s=9.0;app.graphics_view_end_s=9.003;
        assert_eq!(app.cached_decoded_signal_range(&d,&s,&scope),Some(range));
        assert!(app.cached_decoded_signal_waveform(&d,&s,&scope).iter().all(|p|p[1]==0.0));
    }
    #[test]
    fn configured_signal_includes_frames_after_old_200000_sample_cutoff(){
        let mut d=data();let base=d.frames[0].clone();d.frames=vec![base;200_005];
        for (i,f) in d.frames.iter_mut().enumerate(){f.relative_time_s=i as f64*0.001;f.timestamp_ns=i as u64*1_000_000;f.payload=vec![if i==200_002{123}else{0}];}
        d.index=crate::index::FrameIndex::build(&d.frames);
        let s=signal("CAN");let indices=decoded_signal_indices(&d,&s,&BTreeSet::from(["CAN|1".into()]));
        let points=decoded_signal_view_points(&d,&indices,&s,200.001,200.004);
        assert!(points.iter().any(|p|p[1]==123.0));assert!(points.len()<8);
        assert_eq!(decoded_signal_sample_at(&d,&indices,&s,200.002).unwrap()[1],123.0);
    }
    #[test]
    fn configured_can_ids_and_lin_error_records_do_not_mix(){
        let mut f=data().frames[0].clone();let mut s=signal("CAN");f.identifier=Some(0x123);
        assert!(signal_matches_frame(&f,&s));f.frame_key="EXT:0x123".into();assert!(!signal_matches_frame(&f,&s));
        s.frame_id="EXT:0x123".into();assert!(signal_matches_frame(&f,&s));f.frame_key="0x123".into();assert!(!signal_matches_frame(&f,&s));
        f.bus_type=BusType::Lin;f.frame_key="LIN:ERROR:CHECKSUM:0x12".into();f.identifier=Some(0x12);s.protocol="LIN".into();s.frame_id="0x12".into();assert!(!signal_matches_frame(&f,&s));
        f.frame_key="LIN:0x12".into();assert!(signal_matches_frame(&f,&s));
        f.bus_type=BusType::Can;f.frame_key="EXT:0x18DAF121".into();f.identifier=Some(0x18DAF121);s.protocol="CAN".into();s.frame_id="0x18DAF121".into();
        assert!(signal_matches_frame(&f,&s));assert!(signal_matches_source("CAN",&f.frame_key,1,&s));
        assert!(frame_match("EXT:0x123","EXT:291"));
    }
    #[test]
    fn remaining_protocol_byte_bit_and_cursor_values_agree(){
        for (bus,name) in [(BusType::Can,"CAN"),(BusType::CanFd,"CAN-FD"),(BusType::Lin,"LIN"),(BusType::Ethernet,"ETH"),(BusType::Tecmp,"TECMP")] {
            let mut d=data();for f in &mut d.frames{f.bus_type=bus;}
            let points=digital_view_points(&d,name,"0x123",1,Some(0),None,5.0,5.003);
            assert!(points.iter().any(|p|p[1]==128.0));assert!(!points.iter().any(|p|p[1]==255.0));
            assert_eq!(digital_value_at(&d,name,"0x123",1,Some(0),None,5.001),Some(128.0));
            assert_eq!(digital_value_at(&d,name,"0x123",1,Some(0),Some(7),5.001),Some(1.0));
        }
    }
    #[test]
    fn capture_explorer_responsive_panels_keep_trace_space() {
        assert_eq!(explorer_panel_width(1440.0, 360.0), 360.0);
        assert_eq!(explorer_panel_width(1920.0, 180.0), 280.0);
        assert!(explorer_panel_width(800.0, 2000.0) <= 384.0);
        let narrow = explorer_panel_width(420.0, 360.0);
        assert!(narrow <= 420.0 * 0.48);
        let height = docked_pane_extent(450.0, 330.0, 220.0, 160.0);
        assert!(height <= 225.0);
        assert!(450.0 - height - 5.0 >= 220.0);
        let width = docked_pane_extent(1100.0, 800.0, 300.0, 240.0);
        assert!(1100.0 - width - 5.0 >= 300.0);
        assert_eq!(docked_pane_extent(300.0, 200.0, 300.0, 240.0), 0.0);
    }

    #[test]
    fn capture_explorer_search_filters_names_hex_and_protocol_without_mutating_scope() {
        let mut enabled = BTreeSet::from(["CAN|1".to_string(), "ETH|10".to_string()]);
        assert!(explorer_matches_channel("CAN", 1, "Body CAN", "body", "All"));
        assert!(explorer_matches_channel("CAN", 1, "Body CAN", "0x1", "CAN"));
        assert!(explorer_matches_channel("ETH", 10, "UDP stream", "10", "Ethernet"));
        assert!(!explorer_matches_channel("ETH", 10, "UDP stream", "0xA", "CAN"));
        assert!(!explorer_matches_channel("CAN", 1, "Body CAN", "missing", "All"));
        assert!(explorer_matches_channel("CAN-FD", 3, "Chassis FD", "fd", "CAN FD"));
        assert!(explorer_matches_channel("UNKNOWN", 77, "Unknown", "", "Other"));
        let selected_before = enabled.clone();
        let _display_only = explorer_matches_channel("CAN", 1, "Body CAN", "missing", "All");
        assert_eq!(enabled, selected_before, "tree filtering must not mutate enabled scope");
        enabled.clear();
        assert!(!enabled.contains("CAN|1"));
    }

    #[test]
    fn analysis_bit_and_byte_previews_respect_channel_and_project_scope(){
        let d=data();let scope=BTreeSet::from(["CAN|1".into()]);
        let bytes=scoped_payload_points(&d,"0x123",Some(1),&scope,0,None);
        assert!(bytes.iter().any(|p|p[1]==128.0));assert!(!bytes.iter().any(|p|p[1]==255.0));
        let bits=scoped_payload_points(&d,"0x123",Some(1),&scope,0,Some(7));
        assert_eq!(bits.iter().filter(|p|p[1]==1.0).count(),1);
        assert!(scoped_payload_points(&d,"0x123",None,&BTreeSet::new(),0,None).is_empty());
    }
    #[test]
    fn viewport_between_frames_retains_the_held_value(){
        assert_eq!(held_points_in_view(&[[1.0,42.0],[5.0,99.0]],2.0,3.0),vec![[2.0,42.0],[3.0,42.0]]);
        let points=(0..100).map(|i|[i as f64,if i==0 || i==99{42.0}else if i%2==0{0.0}else{255.0}]).collect::<Vec<_>>();
        let reduced=downsample_minmax(&points,4);assert_eq!(reduced.first(),points.first());assert_eq!(reduced.last(),points.last());
        let native=native_engine::downsample(&points,4);assert_eq!(native.first(),points.first());assert_eq!(native.last(),points.last());
    }

    #[test]
    #[ignore = "Requires LOGLENS_PROTOCOL_TEST_CAPTURE"]
    fn supplied_capture_remaining_protocol_graph_values_match_frames(){
        let capture=std::env::var("LOGLENS_PROTOCOL_TEST_CAPTURE").expect("capture path");
        let mut groups=BTreeMap::<(String,u32,String),Vec<crate::model::NormalizedFrame>>::new();
        let mut sequence=0;
        crate::capture::for_each_capture_packet(std::path::Path::new(&capture),usize::MAX,|p|{
            if p.linktype==1 {for r in crate::tecmp::parse_ethernet_tecmp(&p.data){
                let mut f=crate::protocols::normalize_tecmp(&r,sequence);sequence+=1;
                if f.bus_type==BusType::Analog{continue;}
                f.timestamp_ns=p.timestamp_ns;
                let children=crate::protocols::detect_container_review(&f);
                for f in std::iter::once(f).chain(children) {
                    let key=(f.bus_type.to_string(),f.channel_id,f.frame_key.clone());let samples=groups.entry(key).or_default();
                    if samples.len()<9{samples.push(f);}else{samples[8]=f;}
                }
            }}Ok(true)
        }).unwrap();
        let mut d=data();d.frames=groups.into_values().flatten().collect();d.frames.sort_by_key(|f|f.timestamp_ns);
        let origin=d.frames[0].timestamp_ns;for f in &mut d.frames{f.relative_time_s=f.timestamp_ns.saturating_sub(origin) as f64/1e9;}
        d.index=crate::index::FrameIndex::build(&d.frames);
        let end=d.frames.last().unwrap().relative_time_s+0.001;let mut buses=BTreeSet::new();let mut checked=0;
        for f in &d.frames {
            let bus=f.bus_type.to_string();buses.insert(bus.clone());
            let bytes=(0..f.payload.len().min(8)).chain(f.payload.len().checked_sub(1)).collect::<BTreeSet<_>>();
            for byte in bytes {
                let points=digital_view_points(&d,&bus,&f.frame_key,f.channel_id,Some(byte),None,0.0,end);
                assert!(points.iter().any(|p|p[0]==f.relative_time_s && p[1]==f.payload[byte] as f64));
                let expected=d.index.by_key[&f.frame_key].iter().rev().find_map(|i|{
                    let v=&d.frames[*i];if v.channel_id==f.channel_id && v.bus_type==f.bus_type && v.relative_time_s<=f.relative_time_s {v.payload.get(byte).copied()}else{None}
                }).unwrap();
                assert_eq!(digital_value_at(&d,&bus,&f.frame_key,f.channel_id,Some(byte),None,f.relative_time_s),Some(expected as f64));
                for bit in 0..8 {
                    assert_eq!(digital_value_at(&d,&bus,&f.frame_key,f.channel_id,Some(byte),Some(bit),f.relative_time_s),Some(((expected>>bit)&1) as f64));
                }
                checked+=1;
            }
        }
        for bus in ["CAN","CAN-FD","LIN","ETH","TECMP"]{assert!(buses.contains(bus));}
        println!("representative_frames={} payload_positions_checked={} buses={buses:?}",d.frames.len(),checked);
    }

    #[test]
    #[ignore = "Requires the supplied capture and a Wireshark analog reference TSV"]
    fn supplied_capture_analog_graph_matches_wireshark_samples(){
        let capture=std::env::var("LOGLENS_ANALOG_TEST_CAPTURE").expect("capture path");
        let reference=std::env::var("LOGLENS_ANALOG_TEST_REFERENCE").expect("reference TSV path");
        let text=std::fs::read_to_string(reference).unwrap();
        let mut expected=BTreeMap::<u32,Vec<f64>>::new();
        for line in text.lines(){
            let fields=line.split('\t').collect::<Vec<_>>();
            let ch=u32::from_str_radix(fields[4].trim_start_matches("0x"),16).unwrap();
            expected.entry(ch).or_default().extend(fields[12].split(',').map(|v|v.parse::<f64>().unwrap()));
        }
        let channels=expected.keys().copied().collect::<BTreeSet<_>>();
        for source_time in [false,true] {
            let d=crate::engine::load_selected_with_timeline(std::path::Path::new(&capture),usize::MAX,Some(&channels),source_time).unwrap();
            assert_eq!(d.frames.iter().filter(|f|f.bus_type==BusType::Analog).count(),234);
            let duration=crate::analysis::statistics(&d.frames).duration_s;
            let mut app=LogLensApp::initial_state();let mut total=0;
            for (ch,values) in &expected {
                let key=format!("ANALOG:{ch}");
                let (points,unit,_,period)=app.analog_waveform(&d,&key,*ch);
                assert_eq!(unit,"V");assert_eq!(period,1_000_000);assert_eq!(points.len(),values.len());
                for (point,value) in points.iter().zip(values){assert!((point[1]-value).abs()<1e-9,"channel {ch}: {} != {value}",point[1]);}
                assert!(points.last().unwrap()[0]<=duration+1e-9,"Fit must include the final sample");
                let visible=app.analog_view_points(&d,&key,*ch,0.0,duration);
                let maximum=|points:&[[f64;2]]|points.iter().map(|p|p[1]).fold(f64::NEG_INFINITY,f64::max);
                assert_eq!(maximum(&visible),maximum(&points),"display must retain voltage peaks");
                total+=points.len();
            }
            assert_eq!(total,163_800);
            println!("clock={} samples={} graph_duration={duration:.9} all_values_match=true",if source_time{"source"}else{"capture"},total);
        }
    }
    #[test]
    #[ignore = "Requires TC_OCM_CD_01.01.pcapng"]
    fn full_ocm_capture_background_load_keeps_every_channel_and_final_record() {
        let capture=PathBuf::from(std::env::var("LOGLENS_FULL_LOG_CAPTURE").expect("capture path"));
        session::with_test_storage(|_| {
            let channels=discover_capture_channels(&capture,usize::MAX).unwrap().into_iter().map(|(ch,_)|ch).collect::<BTreeSet<_>>();
            let mut app=LogLensApp::initial_state();app.pending_log_path=Some(capture.clone());app.pending_channel_selection=channels.clone();
            assert!(app.load_path(capture,None));
            let ctx=egui::Context::default();let deadline=Instant::now()+Duration::from_secs(180);
            while app.background_load.is_some(){assert!(Instant::now()<deadline,"full capture load timeout");app.poll_background_load(&ctx);std::thread::sleep(Duration::from_millis(20));}
            assert!(app.error.is_none(),"{:?}",app.error);
            let d=app.dataset.clone().unwrap();assert_eq!(d.frames.len(),2_922_727);
            assert!(!d.warnings.iter().any(|w|w.starts_with("Safe load limit reached")));
            assert_eq!(d.frames.last().unwrap().timestamp_ns,1_790_011_941_661_093_412);
            let duration=app.cached_stats.as_ref().unwrap().duration_s;
            assert!((duration-31.675734277).abs()<1e-9);
            assert_eq!(app.active_preload_channels,channels);assert_eq!(app.project_channels,d.index.project_channel_keys);
            assert_eq!(app.graphics_view_start_s,0.0);assert_eq!(app.graphics_view_end_s,duration);assert!(app.graphics_range_start_s.is_none());
            println!("full_ocm_loaded_records={} duration={duration:.9} selected_channels={} complete=true",d.frames.len(),channels.len());
        });
    }

    #[test]
    #[ignore = "Requires the supplied capture"]
    fn supplied_capture_analog_cm_alignment_uses_first_sample_source_time() {
        let capture=PathBuf::from(std::env::var("LOGLENS_ANALOG_TEST_CAPTURE").expect("capture path"));
        let selected=BTreeSet::from([32785]);
        for source_time in [false,true] {
            let d=crate::engine::load_selected_with_timeline(&capture,usize::MAX,Some(&selected),source_time).unwrap();
            let marker=control_triggers(&d.frames,0.0).into_iter().find(|m|m.control_message_id==7).unwrap();
            let edge=d.frames.iter().filter(|f|f.bus_type==BusType::Analog && f.channel_id==32785).find_map(|f| {
                let meta=f.analog.as_ref()?;
                f.payload.chunks_exact(2).enumerate().find(|(_,b)|i16::from_be_bytes([b[0],b[1]])as f64*meta.factor>1.0)
                    .map(|(n,_)|f.relative_time_s+n as f64*meta.sample_period_ns as f64*1e-9)
            }).unwrap();
            let delta=edge-marker.time_s;
            if source_time {
                assert!((delta-(-0.000_000_385)).abs()<1e-9);
                assert!(analog_sample_at(&d,"ANALOG:32785",32785,marker.time_s).unwrap()[1]>1.0);
            }else{
                assert!((delta-0.700_152_294).abs()<1e-9);
                assert!(d.warnings.iter().any(|w|w.contains("700.000 ms")));
            }
            println!("clock={} channel=0x8011 voltage_change_minus_CM_ms={:.9}",if source_time{"source"}else{"capture"},delta*1000.0);
        }
    }

    #[test]
    #[ignore = "Requires the supplied capture"]
    fn supplied_capture_partial_graph_reload_reaches_later_voltage() {
        let capture=PathBuf::from(std::env::var("LOGLENS_ANALOG_TEST_CAPTURE").expect("capture path"));
        session::with_test_storage(|_| {
            let channels=BTreeSet::from([32785,32786,36881,36882,36883,36884]);
            let partial=crate::engine::load_selected_with_timeline(&capture,12,Some(&channels),true).unwrap();
            assert!(partial.warnings.iter().any(|w|w.starts_with("Safe load limit reached")));
            let mut app=LogLensApp::initial_state();
            app.apply_loaded_dataset(capture,false,partial,None,Some(channels.clone()));
            // Only one channel is plotted; all six preload choices must survive retry.
            let plotted="ANALOG|32785|ANALOG:32785".to_string();
            app.graphics_sources=BTreeSet::from([plotted.clone()]);
            app.selected_frame_bytes=std::collections::HashMap::from([(plotted.clone(),BTreeSet::from([0]))]);
            app.graphics_lane_order=vec![format!("SRC|{plotted}|0")];
            app.graphics_view_end_s=0.7; app.time_end=0.7;
            app.graphics_range_start_s=Some(0.0); app.graphics_range_end_s=Some(0.7);
            assert!(app.reload_selected_channels());
            let ctx=egui::Context::default(); let deadline=Instant::now()+Duration::from_secs(30);
            while app.background_load.is_some() {
                assert!(Instant::now()<deadline,"scoped reload timed out");
                app.poll_background_load(&ctx); std::thread::sleep(Duration::from_millis(5));
            }
            assert!(app.error.is_none(),"{:?}",app.error);
            let d=Arc::clone(app.dataset.as_ref().unwrap());
            assert!(d.source_time); assert_eq!(d.frames.len(),235);
            let markers=control_triggers(&d.frames,0.0);
            assert_eq!(markers.len(),1); assert_eq!(markers[0].control_message_id,7);
            assert!((markers[0].time_s-11.192042304).abs()<1e-8);
            assert!(app.graphics_show_event_markers);
            assert!(!d.warnings.iter().any(|w|w.starts_with("Safe load limit reached")));
            assert_eq!(app.active_preload_channels,channels);
            assert_eq!(app.graphics_sources,BTreeSet::from([plotted]),"retry must preserve which channels were plotted");
            assert!(app.cached_stats.as_ref().unwrap().duration_s>27.0);
            assert_eq!(app.graphics_view_end_s,0.7);assert_eq!(app.time_end,0.7);
            assert_eq!(app.graphics_range_start_s,Some(0.0)); assert_eq!(app.graphics_range_end_s,Some(0.7));
            let points=app.analog_view_points(&d,"ANALOG:32785",32785,0.0,app.cached_stats.as_ref().unwrap().duration_s);
            let peak=points.iter().map(|p|p[1]).fold(f64::NEG_INFINITY,f64::max);
            assert_eq!(peak,11.909);
            println!("scoped_reload_records={} loaded_duration={:.9} retained_view_end={:.9} channel32785_peak={peak}",d.frames.len(),app.cached_stats.as_ref().unwrap().duration_s,app.graphics_view_end_s);
            let session_id=app.session.as_ref().unwrap().id.clone();
            assert!(app.change_capture_channels());
            assert_eq!(app.pending_channel_selection,channels);
            assert!(app.pending_channels.iter().any(|(_,kind)|kind.contains("CAN")));
            app.pending_channel_selection=BTreeSet::from([32786]);
            assert!(app.load_path(app.pending_log_path.clone().unwrap(),None));
            let deadline=Instant::now()+Duration::from_secs(30);
            while app.background_load.is_some() {
                assert!(Instant::now()<deadline,"changed-channel reload timed out");
                app.poll_background_load(&ctx);std::thread::sleep(Duration::from_millis(5));
            }
            assert!(app.error.is_none());assert_eq!(app.session.as_ref().unwrap().id,session_id);
            assert_eq!(app.active_preload_channels,BTreeSet::from([32786]));
            assert_eq!(app.dataset.as_ref().unwrap().frames.len(),40);
            assert!(app.graphics_sources.contains("ANALOG|32786|ANALOG:32786"));
            assert!(app.change_capture_channels());
            assert!(app.pending_channels.iter().any(|(ch,_)|*ch==32785));
            assert_eq!(app.pending_channel_selection,BTreeSet::from([32786]));
            println!("changed_channel=32786 records=40 session_retained=true omitted_channels_reselectable=true");
        });
    }

    #[test]
    fn analog_selection_uses_engineering_lanes_even_after_overlay_restore(){
        let mut app=LogLensApp::initial_state();
        app.graphics_stacked=false;
        app.graphics_sources.insert("ANALOG|1|ANALOG:1".into());
        app.enforce_analog_graph_mode(&BTreeSet::from(["CAN|1".into()]));assert!(!app.graphics_stacked);
        app.enforce_analog_graph_mode(&BTreeSet::from(["ANALOG|1".into()]));assert!(app.graphics_stacked);
        let mut d=data();d.frames[0].bus_type=BusType::Analog;
        assert_eq!(digital_frame_value(&d.frames[0],Some(0),None),None);
    }
    #[test]
    fn partial_load_warning_precedes_clock_information(){
        let warnings=vec!["TECMP timeline: Wireshark capture time".into(),"Safe load limit reached at approximately 750000 decoded records.".into()];
        let text=load_completion_summary(&warnings,8.018);
        assert!(text.starts_with("Partial log loaded (8.018 s)"));assert!(text.contains("750000"));
    }
    #[test]
    fn obsolete_saved_graph_ranges_reset_on_clock_change(){
        assert_eq!(restored_graph_view(1.0,3.0,8.0,true),Some((1.0,3.0)));
        assert_eq!(restored_graph_view(1.0,3.0,8.0,false),None);
        assert_eq!(restored_graph_view(1_788_958_312.0,1_788_958_320.0,8.0,true),None);
        assert_eq!(restored_graph_view(8.0,8.0,8.0,true),None);
    }
    #[test]
    fn zoomed_can_points_retain_selected_channel_transitions(){
        let d=data();
        assert_eq!(digital_value_at(&d,"CAN","0x123",1,Some(0),None,5.001),Some(128.0));
        assert_eq!(digital_value_at(&d,"CAN","0x123",1,Some(0),Some(7),5.001),Some(1.0));
        let points=digital_view_points(&d,"CAN","0x123",1,Some(0),None,5.0,5.003);
        assert!(points.iter().any(|p|p[1]==128.0));assert!(!points.iter().any(|p|p[1]==255.0));assert!(points.len()<12);
        let bits=digital_view_points(&d,"CAN","0x123",1,Some(0),Some(7),5.0,5.003);
        assert!(bits.iter().any(|p|p[1]==1.0));assert!(bits.iter().any(|p|p[1]==0.0));
    }
    #[test]
    fn analog_overlapping_blocks_are_sorted_before_viewport_search(){
        let mut d=data();d.frames.truncate(2);
        for (i,f) in d.frames.iter_mut().enumerate(){
            f.bus_type=BusType::Analog;f.channel_id=1;f.frame_key="ANALOG:1".into();f.relative_time_s=i as f64;f.timestamp_ns=i as u64*1_000_000_000;
            f.payload=vec![0,1,0,2,0,3,0,4];f.analog=Some(crate::model::AnalogMeta{sample_count:4,sample_width_bytes:2,sample_period_ns:1_000_000_000,factor:1.0,unit:"V".into()});
        }
        let mut app=LogLensApp::initial_state();let (points,_,_,_)=app.analog_waveform(&d,"ANALOG:1",1);
        assert_eq!(crate::analysis::statistics(&d.frames).duration_s,4.0);
        assert_eq!(points.len(),8);assert!(points.windows(2).all(|p|p[0][0]<=p[1][0]));
        d.index=crate::index::FrameIndex::build(&d.frames);
        let view=app.analog_view_points(&d,"ANALOG:1",1,2.0,2.5);
        assert!(view.iter().filter(|p|p[0]==2.0).count()>=2);
        assert_eq!(analog_sample_at(&d,"ANALOG:1",1,2.5),Some([2.0,2.0]));
        d.frames[1].analog.as_mut().unwrap().sample_period_ns=0;
        let mut app=LogLensApp::initial_state();
        let (_,unit,_,period)=app.analog_waveform(&d,"ANALOG:1",1);
        assert_eq!(unit,"V");assert_eq!(period,1_000_000_000,"a reserved interval must not hide earlier valid data");
    }
}
