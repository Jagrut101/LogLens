# LogLens v0.54.42

> **Home UI revision 2 (source-only).** For a full-width native home page, rebuild this source. Opening the old bundled/installed `.app` or `.exe` will NOT display these changes. See `BUILD_TO_SEE_HOME_UI.md`. The standalone HTML is a preview only.

Automotive log analysis desktop application built with Rust and egui.

## Run on macOS

Run `cargo run --locked` to test the latest source on macOS. The bundled `dist/LogLens.app` predates the full-capture and analog-timeline fixes. Windows v0.54.42 includes these fixes. The additional metadata-sharing memory optimization is currently source-only and is not in that Windows ZIP.

## Build from source

Install Rust, then run `bash build_macos_app.sh` on an Apple silicon Mac.
Run `cargo test` for the test suite. Windows build instructions are in `build_windows_mingw.bat`.

## Included files

- `src/` and `native/`: application source.
- `assets/`, `templates/`, `examples/`: application resources and examples.
- `verification/`: audit reports, reference sources, fixtures, and latest build/test evidence.
- `VERSION_HISTORY.md`: change history.

Build caches and historical generated audit tables/logs were removed for packaging. The detailed audit backup is stored separately beside this project; some historical reports and verification scripts refer to outputs in that backup. Rebuilding regenerates `target/`.

## Graph and trace controls

- The latest source shares repeated immutable network and parent/PDU metadata to reduce memory per decoded record. The network pool is bounded; payloads and acquisition times remain intact. Matched full-capture testing measured 38% lower peak process footprint on TC_OCM_CD_01.01.pcapng. This is a single-capture result, not a guaranteed file-size limit.

- Capture files stream directly into normalized data with an estimated memory budget based on RAM, including space for payloads and indexes. The previous fixed capture-record cutoff no longer restricts files that fit this budget. A genuine memory limit still produces a partial-load warning; text/spreadsheet row limits remain enforced.
- Opening a new capture shows its full loaded time span. Auto Activity cropping is an explicit action. The graph distinguishes loaded coverage from the current view. Full Range restores the whole loaded view and clears time restrictions while retaining channel/frame/graph selections; reopening a saved workspace retains its zoom until Full Range is used.

- New captures default to TECMP source time for acquisition timing. Saved sessions retain their chosen clock. The graph shows the active timeline. In capture-time mode, analog graphs explain the packet buffering delay and offer Use TECMP source time without changing channel/graph selections, names or filters. Switching clocks resets the view and time cursors because the timeline has changed. Raw capture and source timestamps remain available in Details.

- Double-click a graph title, or right-click it and choose Rename signal / graph. Right-click a channel in the Graphics message list to rename that channel. Clear a display name to restore its original label. Names are saved with the workspace and do not alter decoding IDs.
- In ETH channels, use Source IP and Destination IP checkbox menus to select multiple addresses. Uncheck All before choosing only particular addresses. Source and destination selections must match an actual directional endpoint pair.
- Click … [expand] on a clipped Data cell to wrap its complete payload in the trace row; click [collapse] to restore its height.
- Reload selected channels retries the full capture checklist, including unplotted channels, and retains project enable/disable states, names, filters, lane order, cursors and ranges. Use Fit separately when you want to view all newly loaded data.
- Channel IDs use hexadecimal (0x8011) throughout the channel checklist, trace, graphs and exports. Frame IDs retain their protocol-specific formatting.

- In Graphics, use Activity next to Frame IDs. Choose Select none, then check Pattern, Constant, Counter, Signal, CRC / Checksum candidate, Analog, or Other. Multiple types combine, and Clear filter restores all. Activity and Frame ID filters apply together and are saved with the workspace.

- To export an image, open the Waveform Viewer and choose Options > Save graph image…. The PNG is rendered directly from waveform data on a system-themed engineering-plot background, with axes, units, trace labels and a cursor measurement table. All selected, filtered-in traces are exported for the current time window, including traces below the scroll position. Output is 2000 pixels wide and sized to the plot content; viewer buttons, panes and scrollbars are excluded.

- The Waveform Viewer has its own Settings button; graph controls are shared with main Settings and saved with the workspace.
- The local eframe dependency in vendor/ retains native clipboard/screenshot action delivery for separate graph windows. The clean image exporter itself does not use screenshots. Keep vendor/ when building from source.
- Native export verification: run the executable with --verify-graph-image <output.png>. It opens generated sample data in an isolated graph window, saves the image, writes <output>.check.json, then exits without accessing saved sessions.

- Exported plot images automatically use the current OS light/dark theme, including contrast-adjusted text, axes, grid, traces, cursors and measurement table. If system appearance is unavailable, the effective app appearance is used.

- Partial loads keep selected late-channel topology in the project tree and saved session. Channels without loaded records remain checked and are marked accordingly. A genuine record/memory limit can still leave later data unloaded; retries never silently narrow the channel selection or replace data with a shorter partial prefix.

- If an older version already saved a reduced channel selection, use Change Channels once to reselect the desired channels. Subsequent partial loads and retries retain that selection.


## Home UI Revision 3 (source-only)

See `HOME_UI_REVISION_3.md` for the native egui visual-parity changes and verification limitations. After rebuilding on macOS, the status bar must show `Home UI revision 3`.


### UI update — Capture Channel Selection Revision 4
The capture-channel picker is now a full-width second workflow screen. See `CHANNEL_UI_REVISION_4.md`. Rebuild to see this in the native app.
