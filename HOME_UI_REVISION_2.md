# LogLens Home UI — Revision 2: fullscreen-width / adaptive layout

This update builds on HOME_UI_REVISION_1.md and keeps Cargo package version 0.54.42.

## Changes
- Removed the fixed 1060 px home-content width cap and the centered, narrow layout in the native egui start screen. The scroll content now uses the actual CentralPanel available size and has a viewport-height minimum.
- On desktop widths >= 980 logical pixels: Configuration Library and Recent Captures appear in two adaptive columns underneath the full-width capture hero. On narrower windows they stack vertically.
- Recent-session search spans its card width. Recent list height and channel-selection list height adapt to available viewport height; there is still scrolling for long lists.
- The native OS window starts maximized (not exclusive fullscreen), remains resizable/restorable, and uses a smaller minimum inner size of 960 x 620 logical pixels for smaller displays.
- Independent responsive HTML preview uses the full browser viewport and illustrates the two-column layout.
- Retained Open Capture, Resume Analysis, Change Channels, timestamp source, select/clear channel selection, configuration imports, recent capture Open/Remove/Refresh, and status handlers.

## Validation scope
- Source-only UI revision; Rust compilation and native macOS/Windows GUI smoke tests have not been performed in this environment (no cargo/rustc installation).
- The HTML preview is illustrative only and does not decode captures.
- All pre-existing release validation findings remain open, including the Mac bundle source/binary mismatch and PCAPNG segment export concerns.

## Recommended smoke checks
1. Open application on 1366x768, 1600x900, 1920x1080, and high-DPI displays; verify maximized startup, no horizontal clipping, and ability to restore a smaller window.
2. Confirm content responds to resizing and two columns collapse to one around 980 logical px.
3. Open a PCAPNG, verify channel picker and timestamp source controls, then load and resume analysis.
4. Test blank and populated Recent Captures, search/filter/open/remove/refresh, long paths, and configuration imports.
5. Verify dark/light themes and large text scaling.

Build on Rust host: `cargo check --locked && cargo test --locked`.
