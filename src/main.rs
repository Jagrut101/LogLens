#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

mod analysis;

mod performance;mod app;
mod capture;
mod context;
mod diagnostics;
mod engine;
mod events;
mod export;
mod index;
mod model;
mod native_engine;
mod protocols;
mod resources;
mod session;
mod tecmp;
mod trace_columns;
mod eth_filter;
mod frame_id_filter;
mod activity_filter;
mod graph_image;
#[cfg(test)] mod tecmp_regression_tests;

use app::LogLensApp;

fn main() -> eframe::Result<()> {
    let mut arguments=std::env::args_os().skip(1);
    if arguments.next().is_some_and(|arg|arg=="--verify-graph-image") {
        let output=arguments.next().map(std::path::PathBuf::from).expect("--verify-graph-image requires a PNG output path");
        return app::run_native_graph_image_verification(output);
    }
    let diagnostics_dir = diagnostics::init();
    diagnostics::set_context("Main UI/analysis event loop");
    let resource_profile=resources::limits();
    diagnostics::log("INFO", format!(
        "LogLens v{} process started - OS={} arch={} pid={} - {} - row_cap={} capture_budget={} MiB analog_point_cap={} cache_entries={} - diagnostics={}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::process::id(),
        resource_profile.memory_label(),
        resource_profile.packet_limit,
        resource_profile.capture_memory_budget()/(1024*1024),
        resource_profile.analog_point_limit,
        resource_profile.waveform_cache_entries,
        diagnostics_dir.display()
    ));
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([1600.0, 960.0])
        .with_maximized(true)
        .with_min_inner_size([960.0, 620.0])
        .with_title("LogLens");
    if let Ok(icon)=eframe::icon_data::from_png_bytes(include_bytes!("../assets/LogLens_Dark.png")) {
        viewport=viewport.with_icon(std::sync::Arc::new(icon));
    }
    let opts = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native("LogLens", opts, Box::new(|cc| Ok(Box::new(LogLensApp::new(cc)))))
}
