# LogLens — Capture Explorer / Workspace UI Revision 5

This update builds directly on the Revision 4.1 source (which corrected a Result/Option mismatch in the channel-selection page).

## Native Rust changes

- Retains Home UI (R3) and the dedicated capture channel selection page (R4.1).
- Improves the native application header and workspace navigation. The former **Measurement** tab is labeled **Capture Explorer** while keeping the original workspace enum and behaviors.
- Adds a full-width Step 03 capture overview with frame, channel, ID/key and duration counts from the existing load-time index/statistics (no scan of millions of frames per repaint).
- Adds a searchable, protocol-filtered channel explorer with independent search/protocol controls. Filtering changes only which tree rows are visible, not which channels are enabled or loaded.
- Retains per-channel scope checkbox actions and Enable all/Disable all; previously loaded and not-loaded channel metadata remain distinguishable.
- Resolves configured channel names once per tree render rather than constructing a map for every displayed row. Raw-ID visibility still updates saved settings and invalidates trace display strings.
- Adds direct native navigation to the existing Waveform Viewer and Export Manager.
- Makes the Frame Inspector the default dock for a newly opened capture; a graphics-preserving reload keeps the current pane. Trace, detached viewer, session restore and workspace persistence remain available.
- Labels the existing virtualized trace **Frame Trace**. Replay, follow/freeze, timestamp values and existing filters are not reimplemented or overridden.
- Adjusts the explorer's default resizable width to 300 points.
- Adds a pure helper and unit test for display-only channel filtering.

## Preview

The included `LogLens_Capture_Explorer_Revision5_Preview.html` contains sample channels and frames. Its toolbar/search/filter/replay controls simulate the design so the layout can be reviewed. It is **not** the application, and HTML uses placeholders for features that require capture data or native OS windows.

## Validation & limitations

- Source and ZIP integrity/static regression checks performed in this environment.
- HTML loaded in headless Chromium with no JavaScript page errors; channel/protocol/trace filters and mock viewer open were exercised.
- Rust/Cargo toolchain and a macOS/Windows desktop runtime were **not available** here. No compiler-pass or native-screen visual-parity claim is made. Before distributing, run `cargo check --locked`, `cargo test --locked` and the platform-specific build scripts.
- Original v0.54.42 release defects (PCAPNG segment export and older bundled macOS binary) are not corrected in this UI-only revision.

## Build on macOS

```sh
cargo check --locked
cargo test --locked
bash build_macos_app.sh
open dist/LogLens.app
```

Launch the newly built `.app` rather than the previous installed one; the ZIP contains source code, not the rebuilt desktop executable.
