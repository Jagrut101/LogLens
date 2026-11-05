# LogLens Capture Channel Selection — Native UI Revision 4

This update builds on Home UI revision 3 and changes the *native egui/Rust* channel-selection workflow, not merely an HTML mock-up.

## Changes
- Dedicated second workflow page for PCAP/PCAPNG channel selection; the home dashboard is no longer rendered behind it.
- Full-width, responsive two-panel layout: searchable checklist on the left and import/timestamp summary on the right; panels stack on narrow windows.
- Protocol filters for CAN, CAN FD, LIN, Ethernet, Analog, TECMP system messages, and Other. Mixed CAN/CAN FD channels intentionally appear under both protocol filters while retaining one channel ID.
- Search by hex/decimal channel ID, configured name, or protocol.
- Explicit Select visible, Clear visible, Select all, Clear all actions. Filter changes never implicitly change the selection.
- Selection count, progress, source filename/size, timestamp-source dropdown and per-network selection summary.
- Load selected channels stays disabled when selection is empty. Existing load/reload/session handlers are unchanged.
- Existing Home page (Revision 3), analysis, graph, configuration, exports, and capture decoders stay intact.

## How to try
1. Build the Rust source (`cargo check --locked` then `bash build_macos_app.sh` on macOS).
2. Start the **newly built** app, open a .pcap/.pcapng log from the Home page.
3. Confirm the entire window switches to "Choose channels for analysis".
4. Search/filter; select individual channels; use Select visible / Clear visible; change timestamp mode.
5. Load. To revisit the picker from a loaded capture, choose Change Channels.

## Validation scope
Archive/source integrity and source-level workflow checks have been performed. Rust toolchain and macOS/Windows runtime were not available in this execution environment, so compilation and UI interaction testing remain required. This is source code, NOT a prebuilt application.

## Previous release issues
Packaging mismatches and PCAPNG segment-export defects documented for v0.54.42 are not addressed by this UI-only revision.
