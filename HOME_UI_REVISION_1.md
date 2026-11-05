# LogLens Home UI — Revision 1 (source-only)

Based on the supplied **LogLens v0.54.42** archive, with a focused replacement of the existing `start_page` UI in `src/app.rs`.

## What changed

- New branded home toolbar and capture-first hero with `Open Capture`, conditional `Resume Analysis`, and `Change Channels` actions.
- Active-capture summary with filename and, when available, frame count, channel count, and duration. Loading/selection state is visible in the footer.
- A prominent, bordered channel checklist shown immediately after choosing a PCAP/PCAPNG, retaining channel-group separation, exact IDs, configured display names, multi-selection, and TECMP/capture timestamp mode.
- Fixed Select All handling to also call session configuration persistence; added explicit `Cancel` for the capture checklist.
- Easier-to-find configuration imports for Project, Channels, Messages, Signals, and CM Markers using the existing handlers.
- Searchable recent sessions with Open, Refresh, and removable entries; no changes to session storage format.
- UI colors follow the existing dark/light visual state. The home screen does not scan frames or rebuild decoding caches.

## Not changed

- Capture reader, indexing, signal decode, graphics viewer, trace table, exports, background loader, analysis windows, project/session files, and signal configuration back-end.
- Application Cargo package version remains `0.54.42`; **this is a source revision, not a new verified release**.
- The issues recorded in the earlier independent v0.54.42 validation report (packaging mismatch, PCAPNG segment export risks, etc.) are not fixed by this home-page-only change.

## Preview

`LogLens_Home_Page_Preview.html` is an illustrative, self-contained HTML preview with synthetic example sessions and channels. The HTML page does not load or decode files; actual app handlers remain in the Rust source.

## Build and validate on a Rust-equipped host

```sh
cargo check --locked
cargo test --locked
cargo run --locked
```

Test the home UI at startup and after loading a capture, returning Home, changing channels, resuming Analysis, opening a recent capture, switching dark/light modes, and with an empty or >12-item Recent Sessions list.

**Execution status in preparation environment:** The ZIP source was inspected and modified; basic delimiter/field checks and HTML/JS syntax checks passed. `cargo check`, `cargo test`, native GUI runs and platform packaging were **not executed** (Rust compiler/toolchain unavailable). Old bundled Mac and Windows binaries are intentionally not included in this source-only package.
