## Unreleased — lower capture memory use (2026-11-05)

- Share immutable network contexts across matching records and share TECMP metadata between an Ethernet parent and its extracted PDUs. Keep per-record payloads, IDs, timestamps and graph indexes intact.
- Bound the network metadata pool and account for shared allocations once in the capture memory budget. Metadata edits use copy-on-write to isolate records.
- Preserve serialized frame contents and saved workspace compatibility. No change to the RAM budget itself.
- Verified identical protocol output and full 2,922,727-record/44-channel loading. A matched debug audit reduced peak process footprint from 2.58 GB to 1.60 GB; larger-file limits remain data-dependent.

## v0.54.42 — Full capture loading and analog timing (2026-10-28)

- Stream capture decoding with a RAM-based estimated allocation budget rather than a fixed record cap; avoid retaining a second whole-file raw packet copy.
- Show complete loaded coverage on fresh opens, expose loaded/view spans, and make Full Range restore the entire loaded time span while retaining channel and graph selections.
- Default new captures to TECMP source time; explain analog capture-time buffering and provide a selection-preserving source-time reload.
- Preserve original timestamps, saved clock choices and workspace selections. Memory-constrained captures still report partial coverage accurately.

## v0.54.41 — Compact hexadecimal channel IDs (2026-10-28)

- Consistent compact hexadecimal channel IDs across channel selection, trace/copy, graphics, configuration, analysis and CSV/Excel exports.
- Preserve existing numeric IDs and workspace selections; accept decimal and hexadecimal input.
- Validate export/reimport, hexadecimal signal mapping and graph-header interactions.

## v0.54.40 — Retain channel choices during partial loads and retries (2026-10-28)
- Reload selected channels retries the original preload selection, including channels without plotted graphs; it no longer narrows selection to plotted channels.
- Scan the complete capture for lightweight selected-channel topology even when record storage reaches its limit. Keep late channels in the project tree and saved workspace without fabricating data.
- Preserve enabled/disabled channel scope during a retry and reject a shorter partial replacement when the memory cap shrinks.
- Probe one extra normalized record so a complete capture ending exactly at the load cap is not labeled partial.

## v0.54.39 — Export images follow system theme (2026-10-28)
- Choose the clean plot export palette from the operating system's current light/dark appearance when saving.
- Adapt background, text, axes, grid, cursor colors, trace colors and table headers together for contrast.
- If native system appearance is unavailable, use the effective application theme. Preserve the data-rendered layout and exact cursor measurements.

## v0.54.38 — Data-rendered graph image export (2026-10-28)
- Replace window screenshots with a dedicated white-background engineering plot rendered directly from waveform data.
- Export axes, units, trace labels, cursor lines and a full cursor measurement table without app controls, panels, scrollbars or unused window space.
- Include all selected, filtered-in traces for the current time window, even below the viewer scroll position; support stacked/overlay layouts and global or per-waveform cursors.
- Preserve exact cursor readouts from original samples, independent of display downsampling.

## v0.54.37 — Native graph export fix and graph settings (2026-10-28)
- Fix screenshot requests never returning from native immediate graph windows by patching eframe's Glow action handler; retain tagged viewport routing and clean-frame capture.
- Add a Settings button inside the graph window for theme, colors, grid, markers, cursor modes, line width, scale, layout, detail and lane height.
- Share controls with main Settings and save graph appearance preferences in the workspace.
- Add an isolated native graph export verification command that renders a real secondary window, saves a PNG and reports completion without accessing user sessions.

## v0.54.36 — Save graph images with cursor values (2026-10-28)
- Add Options > Save graph image… to export the visible graph area, time ruler and Cursor Values pane as a PNG.
- Capture the current view at native display resolution, including A/B/Δ or independent waveform cursor values.
- Exclude the source browser and toolbar; wait for the save dialog and Options popup to close before capture.
- Match screenshot responses to the correct graph viewport and save atomically, with errors reported in the viewer.

## v0.54.35 — Activity checklist filter (2026-10-28)
- Add Activity beside Frame IDs: select Signal, Constant, Counter, CRC / Checksum candidate, Pattern, Analog, and Other / No data.
- Support multiple selections, Select none, Select all, and Clear filter, with immediate updates to matching messages, bytes and graphs.
- Persist the filter with the workspace and retain plotted selections while filtered out.
- Replace the duplicate class checkboxes in Options with the shared checklist.

## v0.54.34 — Expand Data inside the trace (2026-10-17)
- Restrict expansion controls to the Data column.
- Wrap the complete payload inside the existing trace row and provide [collapse]; remove the separate full-value window.
- Support variable-height expanded rows in both trace modes while retaining viewport virtualization.

## v0.54.33 — Graph editing, trace expansion and reload preservation (2026-10-17)
- Rename graph/signal display names by double-clicking or right-clicking graph headers; rename channels through their graph-browser context menu. Persist display aliases without changing decoding identities.
- Show … [expand] on clipped trace cells, with a full-value viewer and copy action.
- Add multi-select checkbox menus for Ethernet source and destination IPs, preserving exact directional pair matching.
- Align each graph close icon with its Drag handle and title in one header row.
- Retain the current workspace, configuration, filters, names, lane order, cursors and analysis range when reloading plotted channels from a partial capture. Fit remains an explicit action.
- Display channel IDs in decimal in the preload checklist, trace and graph views; frame and protocol IDs retain their own formats.

## v0.54.32 — Keep control-message ruler aligned while resizing (2026-10-12)
- Constrain long graph headers to the graph pane and keep full identities in tooltips.
- Reserve divider spacing and use one consistent Cursor Values width per frame.
- Align the frozen ruler with the waveform viewport; fit edge timestamps and reduce tick density on narrow panes.

## v0.54.31 — Drop graphs anywhere on a lane (2026-10-12)
- Accept drops across the whole visible graph lane instead of only its Drag handle.
- Place downward moves after the target and upward moves before it, including adjacent lanes.
- Show an insertion line and add the missing whole-frame drag handle.

## v0.54.30 — Fix graph drag handles (2026-10-12)
- Replace selectable Drag labels with larger drag buttons and hand cursors.
- Recognize drop targets during an active drag and highlight the target handle.
- Share drag behavior across analog, byte, bit, and decoded signal lanes.
- Verify pointer dragging in both directions and cancellation outside a handle.

## v0.54.29 — Remove redundant viewer controls (2026-10-12)
- Keep Full Range as the single analysis-range reset; remove Clear Range and the duplicate overflow reset.
- Remove per-lane Up/Down buttons; drag handles retain graph reordering.
- Keep Clear graphs in Options; remove duplicate Clear Traces from the launcher.
- Group the viewer's Run/Pause/Resume/Stop actions under Replay and name its overflow menu Options.
- Remove the inactive Help / Workflow Guide button; mouse guidance remains in the viewer's Options menu.

## v0.54.28 — Right-aligned graph close icon (2026-10-12)
- Replace the Close graph text button above each stacked waveform with a compact × icon aligned to the right.
- Retain a Close graph tooltip and existing removal behavior for analog, byte, bit, whole-frame, and decoded signal waveforms.
- Update the existing pointer regression to verify right-edge placement and graph removal.

## v0.54.27 — Cleaner Frame ID filter presentation (2026-10-12)
- Replace the selected/available fraction with All, None, 1 selected, or N selected.
- Show detailed selected and available totals inside the dropdown.
- Remove the unsupported dropdown glyph that appeared as a square; retain the existing menu and filtering behavior.

## v0.54.26 — Show selected Frame ID filter results (2026-10-12)
- Open matching protocol/channel/stream groups once when checklist selection changes, so filtered IDs are visible even if their groups were collapsed.
- Add Select none for choosing only specific IDs and show small selections beside the message count.
- Preserve selections when searching another ID and keep subsequent manual collapse working.
- Add a full-viewer pointer/keyboard regression selecting 0x123 and static 0x456 through ID search and checking displayed results.

## v0.54.25 — Filter by Frame IDs only (2026-10-12)
- Retain the Excel-style Frame ID checklist, search within IDs, and multiple selections.
- Remove the separate message/signal search input and its protocol, channel-name, and signal-name matching.
- Keep exact Frame ID filtering, matching decoded signals, graph-linked reveal, static IDs, and stable expand/collapse behavior.

## v0.54.24 — Excel-style Graphics Frame ID filter (2026-10-12)
- Add a searchable multi-select Frame ID checklist with Select all, Select all matching, and Clear filter.
- List all unique raw IDs in enabled project channels independently of activity, IP, or text filters; explicit ID selections also expose static messages.
- Apply exact ID selection to messages and matching decoded signals, preserving channel-specific graphs and Ethernet endpoint filters.
- Graph clicks include their ID in the selection to keep linked message reveal working; new captures reset the checklist.

## v0.54.23 — Fix Graphics browser expand/collapse (2026-10-12)
- Message header clicks select the message without triggering graph auto-reveal, so collapse remains closed on subsequent repaints.
- Manual browser clicks cancel pending graph-triggered expansion before header construction; graph clicks still reveal their messages.
- Give Decoded and Changed Data roots stable IDs so changing result counts does not reset their collapse state.
- Add a reproduced-before/fixed-after full-viewer regression and protocol/channel/root/filter-count collapse coverage.

## v0.54.22 — Ethernet source and destination IP selection (2026-10-11)
- Add Source IP and Destination IP dropdowns under each Ethernet channel in the Graphics message browser.
- Populate options from decoded network metadata, preserving direction and channel isolation; destination choices follow the chosen source.
- Show matching message/stream counts and All endpoints reset; endpoint filters combine with the browser text filter.
- Clicking a plotted Ethernet graph clears conflicting endpoint filters to reveal its message.

## v0.54.21 — Graph close actions, message filtering, and linked selection (2026-10-11)
- Add Close viewer inside the waveform window and Close graph on each stacked waveform; overlay graphs have right-click close actions.
- Add a local message/signal browser filter for protocol, channel, configured channel name, message ID, and decoded signal name.
- Clicking a graph selects its channel-specific message, reveals/expands the browser tree, and scrolls to the message. Overlay clicks select the closest plotted trace.
- Closing one waveform retains other bytes, bits, whole-frame graphs, and decoded signals and clears its cursor/scale state.

## v0.54.20 — Trace keyboard selection and simpler toolbar (2026-10-11)
- Remove Copy selection, Copy data, All columns, and Clear selection toolbar buttons; retain right-click copying and the Columns menu.
- Ctrl+A / Cmd+A selects all rows in the current filtered/replayed Trace view, including off-screen rows, and resets the column restriction.
- Esc clears row/column selection; Ctrl+C / Cmd+C copies selected cells. Text-field keyboard behavior remains respected.

## v0.54.19 — Select multiple Trace rows and columns (2026-10-11)
- Cmd/Ctrl-click adds or removes rows and column headers; Shift-click selects a range in displayed order.
- Highlight selected cells and copy selected rows/columns as a tab-separated table with headers.
- Add All columns and Clear selection controls; retain single-row/payload copying via right-click.
- Multirow copying respects filtering, replay scope, and Overwrite count/delta; loading another dataset clears row selection.

## v0.54.18 — Copy Trace rows and payload data (2026-10-11)
- Add Copy row and Copy data buttons and right-click actions in Chronological and Overwrite modes.
- Copy complete rows with headers as tab-separated text, including hidden columns and untruncated payloads.
- Cmd+C / Ctrl+C copies the selected visible row while leaving text-field copy behavior intact.

## v0.54.17 — Adjustable Trace columns in chronological and overwrite modes (2026-10-11)
- Replace fixed-width/padded Trace text with shared aligned pixel-width columns in both modes.
- Add Columns for showing/hiding fields, setting widths, and restoring defaults; drag header edges to resize.
- Save Trace column visibility and widths in the session workspace. Clip long content to its own column and show full cell text on hover.
- New sessions default to Chronological; saved mode choices remain respected. Chronological rows retain selected-timeline timestamp order.

## v0.54.16 — Remaining graph and capture-navigation bugs (2026-10-11)
- Opening a PCAP/PCAPNG from the active workspace now displays its channel checklist immediately.
- Return to Workspace discards pending channels and clock changes, retaining the loaded dataset's timeline.
- Analog A/B and multi-cursor readings use original indexed samples rather than a reduced display cache.
- Analog zooms decode original visible samples into bounded time buckets, recovering samples discarded by whole-log resource reduction while preserving display extrema and boundary samples.
- A final reserved sample interval no longer hides preceding valid analog blocks.
- Unfiltered capture loads count decoded records rather than consuming their limit on unsupported outer packets.
- Added a failing-before/fixed-after large analog reproduction and channel-navigation/cancel coverage; repeated original-capture comparisons.

## v0.54.15 — Change capture channels after loading (2026-10-11)
- Add Change Channels to the workspace toolbar, Graphics toolbar and Start page for a loaded PCAP/PCAPNG.
- Reopen the complete capture's channel checklist with the active load selection checked; keep the existing dataset until Load Selected Channels succeeds.
- Stream channel discovery to the end of the original file so omitted or later channels remain selectable after a partial/subset load.
- Select newly loaded analog waveforms after changing channels, retain the session/configuration, and fit the new sample duration.

## v0.54.14 — Preserve TECMP control markers when loading selected channels (2026-10-11)
- Retain TECMP Control Messages as capture-wide marker context alongside a nonempty selected-channel load, including Reload plotted channels; unrelated status and bus channels remain filtered.
- Show the retained Control Message count next to the CM Markers toggle in Graphics.
- Verify CM 0x0007 from packet 167458 appears at 11.192042304 s on the supplied capture's analog source-time timeline after scoped reload, while preserving all 163,800 analog sample values.

## v0.54.13 — Recover full plotted-channel coverage after a partial load (2026-10-11)
- Show a persistent warning above Graphics when a memory/record cap truncated the capture; Fit explicitly covers the loaded prefix.
- Add Reload plotted channels to stream the original capture using only the plotted source/configured-signal channels, retaining the clock selection and graph selections.
- Apply the selected-record budget after channel filtering; skipped outer packets no longer prevent reaching later sparse channels.
- Fit an explicit channel reload to the new duration and clear the obsolete short analysis window and measurement cursors.
- Verify the partial-load/reload workflow on TC_UC_WIN_01.01.pcapng: all 234 analog entries, full sample extent, and channel 32785 peak of 11.909 V.

## v0.54.12 — Remaining protocol graph/value verification (2026-10-11)
- Verified the complete TC_UC_WIN_01.01.pcapng against Technica libtecmp entry extraction and Wireshark CAN/CAN-FD/LIN metadata; checked all 816 TCP payloads with Wireshark.
- Remove retained Ethernet FCS/padding from IPv4 TCP application payloads using IP total length; bound UDP payloads to the captured IP packet.
- Replace configured-signal full-log sampling with indexed viewport decoding. Cursor values use original decoded samples, including data after the former 200,000-sample cutoff.
- Separate standard and extended CAN identifiers in signal matching; exclude diagnostic error records from normal configured signals.
- Apply channel/project scope to analysis bit/byte and custom previews; extend full signal plots past the prior 100,000-sample cutoff.
- Preserve display boundary samples and draw held values between received frames in Stacked and Overlay views.
- Added native/Rust endpoint checks and actual-capture graph coverage for five remaining protocol groups. See verification/PROTOCOL_VALUES_REPORT.md.

## v0.54.11 — Analog capture validation and graph limits (2026-10-11)
- Validated all 163,800 analog samples in TC_UC_WIN_01.01.pcapng against Wireshark: no value, scaling or unit mismatches.
- Extend graph/measurement duration through the final sample of each analog block, including tails after the last entry timestamp.
- Keep selected analog channels in engineering-value stacked lanes and exclude analog payloads from digital byte/signature plotting.
- Show partial-load warnings ahead of timeline information. Explicit Start channel selections fit the newly loaded data.
- Added a real-capture graph comparison under both clocks and an analog-only capture for complete inspection; see verification/WIN_ANALOG_REPORT.md.

## v0.54.10 — TECMP source-time graph fixes (2026-10-11)
- Align missing/uptime source clocks to a capture-to-source reference so early system records cannot stretch CAN/analog graphs across decades. Original timestamps remain in Details, with projected records marked explicitly.
- Sort overlapping analog sample blocks before viewport searches.
- Select the requested CAN bus/channel and visible time window before reducing whole-frame, byte and bit plots; derive bits before reduction. Cursor values use original indexed samples.
- Reset saved graph ranges when incompatible with the loaded clock or duration. A new explicit Start channel selection takes precedence over saved channel scope.
- Rebuild the macOS app as 0.54.10; see verification/GRAPH_FIXES.md for validation.

## v0.54.9 — TECMP and Wireshark verification fixes (2026-10-11)
- Retain TECMP source time and outer capture time separately. Start-page Timeline selection chooses source or Wireshark capture time; capture time remains the UI default and the chosen basis is saved per session.
- Classify TECMP CAN/CAN-FD reception errors and CAN RTR separately; exclude them and malformed CAN records from automatic payload discovery.
- Validate the permitted CAN-FD byte lengths and classify replay CAN/Ethernet as TX.
- Preserve standard/extended and data/remote/error CAN identity in native statistics.
- Apply signed PCAPNG interface timestamp offsets in both readers, with overflow-safe clamping.
- Keep raw entry bytes, device flags and CAN CRC; show them in Details and append diagnostic metadata to detailed CSV/Excel exports. Store nanosecond Excel timestamps as exact text.
- Select PCAPNG export segments using the active timeline while preserving original packet bytes and capture clocks.
- Remove old application executables before editing; rebuild the macOS app as 0.54.9. Added regression coverage; see verification/FIXES.md.

## Unreleased — Bug-fix review
- CAN: TECMP extended-ID flag (bit 31) honoured; extended IDs get an `EXT:0x..` frame key so they no longer merge with standard IDs. Bad/truncated CAN lengths become `CAN:DECODE_ISSUE:*` records instead of leaking the 5-byte header as payload.
- SocketCAN: ID word read big-endian per the link-type spec with little-endian fallback; error and RTR frames isolated; FD detected from frame size/flags.
- TECMP: a zero-length entry no longer drops the remaining entries in the packet.
- Timestamps: exact integer pcapng tick-to-ns conversion (Rust and C++), removing ~±256 ns f64 error.
- Loaders: `timestamp_s` header recognised; `byteN` columns are decimal (or 0x hex); SavvyCan `D1..Dn`/`LEN` supported; hex parsing stops at the first invalid byte instead of shifting data.
- Container REVIEW detection tolerates up to 4 trailing FCS/padding bytes (fixes a failing unit test).
- A/B byte compare works when cursor A is after B; native-reader fallback is now logged.
- Analog: sample-time codes 0xC..0xF (0.5 / 0.25 / 0.1 / 0.05 ms) were missing, so those packets decoded with period 0 and vanished from the waveform; added. Analog Alt (0x0028) is now an explicit `TECMP:ANALOG_ALT` record instead of Unknown (its payload layout is not decoded). Packets with a reserved sample time are now reported in the tool log.
- Analog graph flat-line fix: (1) the cache limit used 1-of-N decimation, which can skip every sample where the value changes (a toggling signal became a straight line); it now keeps min and max per bucket. (2) Zoom no longer shows a single global 6000-point downsample: the visible window is re-reduced from the decoded samples. (3) With Auto Y off the Y range is the full-capture min/max.
- Known and not changed: decimal-vs-hex ambiguity of unprefixed CSV IDs, session reuse when a file at the same path changes, `app.rs` size.

## v0.54.8 — TECMP Start Classification
- Message Type 0x00 is shown as Control Messages instead of Unknown during capture scan.
- Start selection grouped into Bus Data / TECMP System Data / Other.
- Control/status records excluded from automatic payload analysis.
- LIN normal payload compatibility expanded to 1..=8 bytes based on validated sample capture behavior.

# v0.54.7 - TECMP LIN Decoder Correction

- Decode TECMP LIN Rx Data Flags before interpreting zeroed Data-section bytes.
- Keep valid LIN ID `0x00` as a normal `LIN:0x00` frame when payload length is valid.
- Replace ambiguous Raw LIN fallback for special conditions with explicit LIN Event/Error/Decode Issue keys.
- Decode WUP, Short WUP, SLEEP, No Slave Response, Collision, Parity and Checksum conditions.
- Handle LIN Replay Data WUP/Short-WUP and zero-length remove-slave-response semantics separately from LIN Rx.
- Show decoded LIN Data Flags in the Details inspector.
- Exclude LIN event/error/decode-operation records from automatic signal discovery and automatic activity-range analysis.
- Added regression tests for ID `0x00`, zero-data WUP/No-Slave-Response, checksum-error isolation, invalid zero length and Replay zero-length handling.

# v0.54.6 - Theme Settings

- Added an **Appearance > Theme** selector in Settings with **Dark** and **Light** options.
- **Dark is the default** for fresh installs and for users upgrading without an existing appearance preference.
- Theme changes apply immediately without restarting LogLens.
- The selected theme is stored globally in `%LOCALAPPDATA%\LogLens\settings.json` on Windows (platform-equivalent LogLens storage on macOS/Linux), so it persists across application restarts and all log sessions.
- Theme preference is intentionally separate from per-log `workspace.json` and `configuration.json`.
- Waveform and cursor colors use a higher-contrast palette in Light theme while retaining the existing Dark-theme palette.
- Existing analysis, session, diagnostics, and resource-safety behavior is retained.

# v0.54.5 - Persistent Diagnostic Logging

- Every Tool Log entry is also written to a persistent diagnostic log so evidence survives restart/crash.
- Windows diagnostics are stored under `%LOCALAPPDATA%\LogLens\logs` (with a temp-directory fallback if the preferred location is unavailable).
- Added automatic normal-log rotation: maximum 5 regular log files, 5 MiB per file.
- Added bounded crash-report retention: maximum 5 crash report files.
- Installed a panic hook that writes timestamp, LogLens version, OS/architecture, process/thread, operation context, panic source location and backtrace.
- Loader worker now records start/completion/failure breadcrumbs and load-time RAM/resource limits.
- Contained UI/processing panics record the post-recovery RAM snapshot in the persistent log.
- Added **Open Diagnostics Folder** to the Tool Log panel for support collection.
- UI **Clear** only clears the in-memory Tool Log; persistent diagnostics remain available for diagnosis.
- Diagnostic messages are bounded to 16 KiB to prevent accidental runaway log growth.
- The diagnostics subsystem does not intentionally write CAN/LIN/ETH payload bytes; existing operational messages, filenames/paths, counts and errors are retained.

# v0.54.4 - Resource Safety / Crash Recovery

- Changed release panic strategy from `abort` to `unwind` so guarded worker/UI panics can be recovered instead of terminating LogLens.
- Added resource-aware limits based on installed/available RAM (Windows `GlobalMemoryStatusEx`; Linux `/proc/meminfo` fallback).
- 8 GiB-class systems use conservative packet, native-reader, spreadsheet and Analog-cache limits; 16 GiB-class systems use higher limits.
- Large PCAP/PCAPNG files bypass the whole-file native reader and use a streaming Rust reader to reduce peak RAM.
- Streaming PCAP/PCAPNG parser validates block/packet lengths and uses fallible allocation before retaining packet data.
- Text/log import now streams line-by-line instead of reading the entire file into a `String`.
- Oversized XLS/XLSX input is rejected gracefully with a user-visible recommendation instead of risking memory exhaustion.
- Loader-worker disconnects now terminate the loading state with a recoverable error.
- eframe update work is guarded with `catch_unwind`; on a contained UI/processing panic LogLens stops replay, clears derived graphics caches and reports the error while retaining the loaded dataset.
- Very large Analog channels are uniformly decimated before being retained in the cache, keeping cache RAM bounded.
- Graphics waveform caches are capped/evicted according to the current resource profile.
- Frame-index lowercase keys are interned per unique frame key instead of storing a separately allocated lowercase `String` for every frame.
- Load warnings are surfaced in Tool Log / status text when resource protection reduces the requested workload.

# v0.54.3 - Compact Cursor Values Layout

- Separate selection titles and source details from A/B/delta measurements.
- Use consistent equal-width numeric columns and truncated labels with full-value tooltips.
- Keep readouts anchored to their waveform during partial scrolling.
- Bound per-waveform cursor lists to each lane, with local scrolling for C1-C8.
- Verify long names and eight cursors fit a 280-pixel-wide, 104-pixel-high lane.

# v0.54.2 - Consistent Graph Headers and Scrolling

- Use protocol, channel name/ID, frame and byte/bit/signal in graph headers and cursor readouts.
- Preserve scrollbar drag and momentum offsets even when the pointer is outside the graph body.
- Keep the lane scrollbar visible and fit plots to the available pane width.
- Keep plain vertical scrolling separate from modified time-pan/zoom gestures.
- Add headless header and scrollbar/wheel interaction tests.
- Retain v0.54.1 session restoration.

# v0.54.1 - Reliable Session Restore

- Save the outgoing analysis before switching captures and on app exit.
- Keep session identity isolated during background loading; failed loads preserve the current workspace.
- Reopen Recent Sessions directly with saved channel scope, configuration, and expanded analysis settings.
- Add idle autosave and atomic JSON replacement.
- Prevent same-file capture truncation when refreshing Recent Sessions.
- Add five isolated persistence/lifecycle regression tests.

# v0.54.0 - Persistent Per-Log Workspace Cache

- Added a persistent `workspace.json` per Recent Session/log.
- Automatically remembers Project Tree protocol/channel scope, selected frames, selected raw bytes and bits, configured signal lanes, whole-frame lanes and lane order.
- Remembers CM/event marker visibility, Hide Constant/Hide Repetitive, stacked/auto-Y settings, analysis range and waveform viewport.
- Remembers Global A/B positions and per-waveform C1-C8 cursors.
- Workspace changes are saved with a ~900 ms debounce so normal waveform interaction is not blocked by disk writes on every repaint.
- Reopening the same Recent Session restores the workspace after the capture is decoded; unavailable channels are safely ignored.
- Existing configuration persistence remains separate from workspace/view state.
- Retains v0.53.9 all-protocol export, All Selected Channels and Skip Constant Frames.

# LogLens v0.53.9 — Selected-Channels + Constant-Skip Export

- Added **All selected channels** export content mode. It exports every normalized frame from bus/channel sources currently enabled in the Project Tree, without requiring any Graphics message/byte/signal selection.
- The selected time range still applies independently: Cursor A→B, Visible range, or Full recording.
- Added **Skip constant frames** checkbox for CSV/Excel. A frame source is skipped when its payload remains identical throughout the selected export range.
- Analog sources are retained by the constant-frame filter because Analog packets contain sample arrays rather than conventional frame payloads.
- All selected-channel export covers CAN, CAN-FD, LIN, ETH, ANALOG, TECMP and UNKNOWN through the normalized frame model.
- Configured signal mapping remains available for selected-channel export; unmapped raw frames are still exported.
- PCAPNG is restricted to **All data in range** because packet-container export cannot safely apply normalized message/channel filtering. CSV/Excel should be used for Current selection or All selected channels.
- Retains v0.53.8 all-protocol export and v0.53.7 viewer interaction/performance changes.

# LogLens v0.53.8 — All-Protocol Export Fix

- Export now preserves every selected normalized protocol: CAN, CAN-FD, LIN, ETH, ANALOG, TECMP and UNKNOWN.
- CSV export now emits every selected frame even when no configured signal maps to it; mapped signal fields are appended when available.
- Excel selected export now adds protocol-specific frame sheets dynamically for every protocol present in the selected range, including Unknown.
- Excel Summary now reports per-protocol frame counts.
- Existing range/current-selection/full-data and PCAPNG behavior is retained.

# v0.53.6 - Graphics Sources Default Tree

- GRAPHICS SOURCES is expanded by default.
- Protocol groups (ANALOG, CAN, CAN FD, LIN, ETH, UNKNOWN) start collapsed.
- Channel nodes and Ethernet UDP stream nodes also start collapsed, preventing recursive expansion when GRAPHICS SOURCES is opened.
- Retains v0.53.5 Analog Drag / Up / Down lane controls.

# v0.53.2 — Per-Waveform Multi-Cursors

## v0.53.5 - Graphics Sources + Analog Lane Controls
- Graphics Sources is now collapsed by default in the Waveform Viewer.
- Analog waveform lanes now expose the same Drag / Up / Down ordering controls as other stacked lanes.
- Analog lane movement uses the existing shared graphics lane order; waveform decoding, cursors, colors, and measurement behavior are unchanged.


## v0.53.3 - Multi-Cursor Compile Fix
- Fixed `Option<LaneCursorAction>` early return to return `None`.
- Fixed E0502 whole-frame multi-cursor borrow by cloning cursor timestamps before mutable waveform-cache access.
- No intended UI or cursor-behavior changes from v0.53.2.

- Added Graphics Settings cursor mode: existing Global A/B cursors or independent Per-Waveform Multi-Cursors.
- Multi-cursor mode disables/hides global A/B interaction and keeps A/B state intact for switching back.
- Every stacked waveform can own up to 8 independent C1..C8 measurement cursors.
- Use `+ Cursor` on a lane, then click that waveform to place the next cursor; `Clear` removes that lane's measurement cursors.
- Per-waveform cursor lines are rendered only on their owning lane and use the stable lane identity, so Raw ID display changes do not reassign measurements.
- Measurement panel shows each cursor's exact time and sample-and-hold value for raw bytes, bits, Analog, configured signals, and Whole Frame signature.
- Per-waveform cursor mode uses stacked layout so lane-local measurements remain unambiguous.
- New-log cache invalidation clears per-waveform cursors; switching cursor modes does not delete them.
- Existing color behavior is retained: Color ON uses stable lane colors; Color OFF uses historical `Color32::LIGHT_GREEN`.

# v0.53.1 — Historical Non-Color Green

- Color graph OFF now restores the pre-waveform-color LogLens graph color: `Color32::LIGHT_GREEN`.
- Color graph ON keeps the existing stable per-lane multi-color palette unchanged.
- Raw IDs / Row ID display remains decoupled from waveform color identity.
- No changes to lane alignment, CM markers, cursor values, filtering, classification, or waveform caches.

# v0.53.0 — Stable Waveform Color Key

- Raw IDs / Row ID display toggles no longer change waveform colors.
- Waveform palette selection now hashes the stable internal lane identity instead of the display label.
- Lane Y-scale/Y-offset state also uses the stable lane identity, so display-name changes do not reset graph state.
- Color graph ON keeps the existing deterministic multi-color palette.
- Color graph OFF remains the restored `Color32::LIGHT_BLUE` behavior.
- No changes to decoding, classification, lane ordering, cursor values, CM markers, or filtering.

# v0.52.9 — Rendered Lane / Cursor Alignment

- Cursor Values no longer use a second lane-height formula or an independent vertical ScrollArea.
- The waveform side records the actual rendered screen rectangle for every visible lane.
- Cursor A/B/Δ values are positioned from those exact lane rectangles, so the Lane Height setting is authoritative.
- Analog, byte, bit, configured-signal and whole-frame lanes can use different real heights without cumulative drift.
- Up/Down ordering, hidden lanes and waveform vertical scrolling are reflected directly in Cursor Values geometry.
- CM/time and Cursor Values headers remain frozen.

# v0.52.8 — Exact Lane / Cursor Value Rows

- Fixed Cursor Values rows collapsing to their text content height after Lane height adjustment.
- Cursor Values now reserve an exact rectangle for every visible lane, so the right pane advances by the same lane block height as Graphics.
- Replaced the guessed 24 px lane-control height with the active egui style control height.
- Lane height changes now propagate directly to Cursor Values while preserving the existing lane order, scrolling, CM ruler and A/B/Δ header.
- Color, filtering, classification, decoding and waveform caches are unchanged.

# v0.52.7 — Lane Geometry Borrow Fix

- Fixed Rust E0502 borrow errors introduced by the v0.52.6 lane/value alignment helper.
- Lane height is copied to a local scalar before building the row-geometry closure, so the closure no longer immutably borrows `self`.
- Preserves v0.52.6 graph/Cursor Values geometry alignment and adjustable lane-height behavior.

# v0.52.6 — Lane / Cursor Value Geometry Alignment

- Cursor Values and waveform bodies now start below matching 54 px frozen headers.
- Cursor Values rows use the same complete lane geometry as Graphics: lane-control/title bar + adjustable plot height + lane gap.
- Lane Height changes are reflected identically on both sides.
- Removed the per-value-row separator height that accumulated vertical drift down the Cursor Values list.
- Existing color/non-color behavior, CM alignment, filtering, classification, and lane reordering controls are unchanged.

# v0.52.5 — Restore Previous Non-Color Waveform

- Color graph ON is unchanged and continues to use the stable per-waveform color palette.
- Color graph OFF now uses the previous LogLens default waveform color, `Color32::LIGHT_BLUE`, instead of the newer gray/white-style line.
- No changes to CM markers, cursors, grid, classification, filtering, scrolling, or waveform data.

# v0.52.4 — Graph / Cursor Value Scroll Alignment

- Replaced the ineffective shared ScrollArea ID workaround with one authoritative vertical lane scroll offset.
- Mouse-wheel/scrollbar movement over the waveform pane updates the shared lane offset.
- Mouse-wheel/scrollbar movement over Cursor Values updates the same offset.
- Both panes are forced to the same vertical offset on repaint so scrolling either side keeps the lane/value list synchronized.
- CM/time ruler and A/B/Delta measurement header remain frozen.
- Existing decode, filtering, classification, waveform caching, Hide Constant and Hide Repetitive behavior is unchanged.

# v0.52.3 — Shared Graph / Cursor-Value Scroll

- Restored vertical mouse-wheel scrolling for stacked waveform lanes.
- Waveform body and Cursor Values now use one persistent shared vertical scroll state.
- CM Marker/time ruler remains frozen above the waveform body.
- Cursor A/B/Δ time header remains frozen above measurement rows.
- Cursor-value rows use lane-matched heights so each value stays horizontally aligned with its waveform.
- Bit lanes use their reduced graph height; regular raw/signal/analog lanes use the configured lane height.
- No decoding, filter, classification, or waveform-cache behavior changed.

# v0.52.2 — CM Marker Graph Alignment

- Aligned the CM/event time ruler to the exact waveform plot rectangle.
- CM markers, A/B cursors and time ticks now use the same 58 px engineering gutter and 4 px right inset as stacked waveform lanes.
- Resizing Signals or Cursor Values therefore keeps the CM ruler and waveform X positions synchronized.
- No filtering, decoding, classification or viewer-layout behavior changed.

# v0.52.1 — Filter Performance

- Filter typing remains debounced: no full capture scan while the user is still typing.
- Lowercase frame-key search text is prepared once in `FrameIndex` at log load.
- A committed filter now prepares lowercase query and structured fields once per query instead of once per frame.
- Payload hex formatting is only used as a fallback when the frame key does not match the text query.
- Ordinary filters invalidate only visibility/scope/Trace display caches; decoded frames, classification and waveform caches remain intact.
- Existing Graphics viewer layout and analysis controls are unchanged.

# v0.52.0 — Graphics Performance Optimization

- Performance-only update; v0.51.9 viewer layout and controls are retained.
- Channel display-name mappings are resolved once per Waveform Viewer repaint instead of being rebuilt for every graph/tree/value label.
- TECMP Analog waveforms are decoded once and cached with unit/sample metadata; graph rendering and cursor-value lookup reuse the same Arc-backed samples.
- Dataset invalidation now clears the analog and cursor-derived caches together with existing waveform caches.
- Splitter resizing remains layout-only and does not intentionally invalidate decoded waveform/classification caches.
- No analysis features were removed.

# v0.51.9 — Project-Style Viewer Splitters

- Replaced the detached viewer's `egui::Resize` wrappers with the same explicit draggable vertical-splitter interaction used by the main Project/Trace workspace.
- SIGNALS width is directly adjustable with the left divider.
- CURSOR VALUES width is directly adjustable with the right divider; dragging left widens measurements so A/B/Delta remain visible.
- Graphics remains the flexible center pane and consumes the remaining width.
- Changed Data remains collapsed by default.
- Existing analysis controls and source hierarchy are retained.

# v0.51.8 — Correct Three-Pane Waveform Viewer

- Corrected the detached viewer layout to SIGNALS | WAVEFORM | CURSOR VALUES.
- SIGNALS now forces a vertical browser layout; GRAPHICS SOURCES stays inside SIGNALS instead of expanding horizontally beside it.
- CHANGED DATA remains collapsed by default.
- SIGNALS and CURSOR VALUES remain user-resizable.
- Reserved right-side measurement width before sizing the flexible waveform canvas so B and Delta are not pushed off-screen.
- Cursor Values retains the frozen Trace/A/B/Delta + Time header and scrollable measurement rows.
- Existing Hide Constant, Hide Repetitive, range, CM marker, raw-ID, and signal controls are retained.

# v0.51.7 — Resizable Viewer Side Panels

- SIGNALS panel is user-resizable (180–520 px, 260 px default).
- CURSOR VALUES panel is user-resizable (300–760 px, 430 px default) so A, B and Δ remain visible.
- CHANGED DATA is collapsed/minimized by default and can still be expanded when needed.
- Cursor header remains frozen while measurement rows scroll.
- Required v0.51.4 analysis controls remain retained.

# v0.51.6 – Cursor Value Alignment

- Rebased on v0.51.4 so required Graphics controls such as Hide Constant and Hide Repetitive are retained.
- Cursor Values now uses a frozen header with A, B and Δ time values.
- Increased measurement-panel width so B and Δ remain visible.
- Cursor rows follow Graphics lane order and respect hidden Constant/Repetitive/class filters.
- Added Analog A/B/Δ sampled values instead of treating Analog as a byte.
- Standardized raw bit labels as `Byte N / Bit N`.
- Only the cursor-value rows scroll; the cursor-time header remains visible.

# v0.51.4 — Native Waveform Viewer + Clean Runtime

- Replaced the embedded `egui::Window` Waveform Viewer with an `egui` immediate native viewport. On normal desktop eframe backends this is a separate operating-system window that can be moved, resized, minimized/maximized, or placed on another monitor.
- The native viewer shares the existing LogLens app state, loaded dataset, waveform caches, trace selections, A/B cursors and Analysis Range; opening it does not reload/redecode the capture.
- Closing the Waveform Viewer only closes that viewport and leaves the main LogLens application running. Reopening it retains the current trace/range/cursor state.
- Performance HUD is now hidden by default so normal users do not see the diagnostic Performance window.
- Main Graphics remains the lightweight launcher/selection summary; waveform rendering remains only in the dedicated viewer to avoid duplicate information.

# LogLens v0.51.3 — Detached Waveform Viewer + Duplicate Information Cleanup

- Graphics tab is now a lightweight launcher/summary instead of duplicating the full measurement workspace.
- Added a dedicated resizable **Waveform Viewer** window that reuses the already loaded dataset and caches; opening it does not reload or re-decode the capture.
- Removed duplicate **VIEW**, **SELECTION**, and **DISPLAY** blocks from the Measurement panel.
- Measurement panel now focuses on A/B cursor timing and Cursor Values.
- Corrected raw trace naming to `Frame / Byte N` and `Frame / Byte N / Bit N`; no `B6.Bit3` shorthand.
- Existing stacked/overlay rendering, trace explorer, classification, event markers, analysis range, configuration, and graphics caches are retained.

---


## v0.51.2 – Cursor Values + Clean Measurement Panel
- Removed the visible `XViewer` name from the Graphics Measurement panel.
- Added A/B cursor values for selected raw bytes using hexadecimal and decimal display.
- Added A/B cursor values for selected bit traces.
- Added A/B engineering values and units for configured/decoded signals.
- Added per-trace delta values.
- Cursor lookup uses sample-and-hold semantics: latest sample at or before cursor time.
- Added a compact scrollable Cursor Values table in the right Measurement panel.

# LogLens v0.51.1 — XViewer Graphics Workspace

- Reworked Graphics into Trace Explorer + waveform canvas + persistent Measurement pane.
- Added shared A/B cursor and viewport readout on the right.
- Removed duplicate signal title and per-lane A/B/Delta/Min/Max/current-value text from stacked plot canvases.
- Reduced the waveform gutter to engineering scale only, keeping the plot clean.
- Preserved shared time viewport, analysis-range shading, event markers, filtering, classification, configuration, and waveform caches.

# LogLens Version History

## v0.51.0 — Settings Cleanup + Overlay Graph Unification

- Removed duplicate Graphics toolbar controls that are now owned by Settings: Auto Y and Stacked/Overlay layout.
- Moved Fit Y into Settings beside Auto Y.
- Settings now provides an explicit Stacked/Overlay waveform-layout choice.
- Rebuilt Overlay waveform rendering to match the current CANoe/XViewer graph UI: theme-aware canvas/gutter/grid, shared time viewport, sample-and-hold traces, semantic A/B cursors, stable waveform colors, and configurable line width.
- Overlay honors Color Graph, Grid, A/B Cursor Labels, and Line Width settings.
- Raw-byte Overlay retains the common 0..255 engineering scale; Analog remains stacked because it uses an engineering-value scale.
- Retains all v0.50.9 fixes and existing analysis/config/session behavior.

## Previous history
## v0.50.9 — Settings Compile Fix

- Fixed the malformed brace introduced around the Graphics grid/marker rendering in v0.50.8.
- Grid setting now correctly gates grid rendering.
- CM/Event Markers setting now correctly gates marker rendering.
- A/B Cursor Labels setting now renders labels only when enabled.
- Retains Color Graph, line-width, Auto Y, stacked lanes, raw-ID settings, Recent Sessions de-duplication, and Config channel-name display.

## Previous history
## v0.50.8 — Graphics Settings

- Added a Settings button at the top-right beside Tool Log, including the Start page.
- Added Graphics settings: Color graph, Grid, CM/event markers, A/B cursor labels, and waveform line width.
- Added Display settings: Show Raw IDs, Auto Y scale, and Stacked waveform lanes.
- Added Reset graphics defaults.
- Color graph ON keeps stable identity-based waveform colors; OFF uses a theme-safe monochrome waveform.
- Retains v0.50.7 Recent Sessions source de-duplication and v0.50.6 configuration channel-name display.

## Previous history
## v0.50.7 — Recent Sessions Source De-duplication

- Fixed the remaining Recent Sessions duplication case where a LogLens-managed `uploads/<uuid>/...` copy was treated as a new original capture.
- Session identity now resolves managed upload copies back to their original source metadata.
- Creating/reopening a session no longer promotes an internal upload path into a new source identity.
- Existing legacy duplicates whose original path points inside LogLens uploads are collapsed on the Start page.
- The newest session remains visible; existing stored session folders are not destructively deleted.
- Retains v0.50.6 Channel Config name display and all Graphics/Analysis behavior.

## Previous history
## v0.50.6 — Config Channel Name Display

- Signal Config and Message Config Channel ID dropdowns now resolve names from the loaded Channel Config.
- The stored configuration value remains the raw Channel ID; configured names are display-only.
- With Show Raw IDs ON: `ConfiguredName (ChannelID)`.
- With Show Raw IDs OFF: `ConfiguredName`.
- Channels without a configured name continue to display their raw Channel ID.
- Channel Config changes are reflected dynamically when the configuration UI is rendered.
- Retains v0.50.5 Recent Sessions de-duplication and existing Graphics/Analysis behavior.

## Previous history
## v0.50.5 — Recent Sessions Dedup Fix

- Start-page Recent Sessions now shows one entry per source capture.
- Reopening the same capture reuses/refreshes its existing session instead of creating another card.
- Windows path identity is normalized for slash direction and case.
- Existing duplicate session metadata from older versions is collapsed at list/read time; the most recently opened entry is shown.
- No capture, configuration, Graphics, or Analysis behavior removed.

## Previous history
## v0.50.4 — Analysis Graph Compile Fix

- Fixed self-referential `secondary` and `marker_color` theme initializers in the unified Analysis graph renderer.
- Removed the now-unused `detect_signals` import.
- Removed the reported unnecessary parentheses in the UNKNOWN/TECMP projection condition.
- Retains the v0.50.3 Analysis/Graphics visual unification and all v0.50.x Signal Discovery functionality.

## Previous history
## v0.50.3 — Analysis Graph UI Unification

- Analysis waveform previews now use the same CANoe/XViewer visual language as Graphics.
- Unified light/dark canvas, gutter, grid, borders, stable waveform colors, CM markers and A/B cursor styling.
- Byte/bit waveform, custom-signal preview, Signal Discovery candidate Plot and configured-signal waveform views now use the unified lane renderer.
- Raw-byte stacked graphs use the same theme-aware lane styling.
- Analysis Range remains independent from A/B measurement cursors.
- No analysis algorithms or existing v0.50.2 Signal Discovery behavior removed.

## Previous history
## v0.50.2 — Light Theme CM / Cursor Fix

- Control Message marker time ruler now follows the active light/dark theme instead of forcing a dark background.
- CM marker labels/lines use a light-theme-safe blue.
- Cursor A uses high-contrast amber in light mode; Cursor B uses high-contrast red.
- Increased A/B line width, with additional emphasis for the active cursor.
- Graphics horizontal overview/scrollbar now follows the active theme.
- Existing v0.50.1 unified Signal Discovery and waveform colors are retained.

## Previous history
## v0.50.1 — Theme Compile Fix

- Fixed self-referential `grid` and `secondary` theme color initializers introduced in v0.50.0.
- Dark theme uses gray 42 grid / gray 125 secondary labels; light theme uses gray 218 / gray 90.
- No Signal Discovery or Graphics behavior removed.

## Previous history
## v0.50.0 — Unified Protocol-Aware Signal Discovery

- Replaced separate generic/LIN detection buttons with one **Detect Signals** action.
- Added protocol scope: All Digital, CAN, CAN-FD, LIN, Ethernet, Review PDU, Unknown.
- Detection remains tied to the committed Analysis Range; A/B cursors remain measurement-only.
- Added multi-offset/multi-width field competition, stronger LE/BE comparison and signed-vs-unsigned inference.
- Added pre-candidate suppression for constant, counter-like and repetitive-pattern bytes.
- Added non-maximum overlap suppression for heavily overlapping duplicate candidates.
- `+ Config` now carries inferred Signed/Unsigned type into Signal Configuration.
- Added **Plot** candidate preview before configuration.
- Retained existing Graphics, channel selection, Recent Sessions, Auto Activity, ETH hierarchy, config import/export and raw byte classification.

### Theme-aware Graphics
- Main stacked waveform canvas, gutter, border, grid and time labels now follow egui light/dark theme.
- Candidate preview graph also follows the active theme.
- Stable deterministic waveform colors from v0.49.4 are retained.

### Notes
- CRC/checksum classification remains available in the existing byte classifier. Conservative CRC suppression inside signal discovery is intentionally not generalized without protocol-specific checksum evidence, to avoid hiding real application signals.
- Float32 inference remains conservative/deferred; v0.50.0 infers integer signedness only.

## Previous history
## v0.49.4 — Stable Waveform Colors

### Graphics
- Each stacked Graphics lane now receives a deterministic waveform color from a 12-color engineering palette.
- Color is derived from the lane identity, so pan, zoom, repaint and lane reordering do not change it.
- Raw bytes, bits, whole-frame traces, configured signals and Analog lanes all use the same stable-color mechanism.
- Added a matching color swatch beside each lane label.
- Cursor A/B, playhead, event markers and Analysis Range retain their dedicated colors so waveform identity is not confused with measurement controls.
- Sample-and-hold rendering, Auto Y, vertical scaling, horizontal navigation and waveform caches are unchanged.

### Compatibility
- No protocol decoder or Signal Discovery behavior was removed.
- v0.49.3 LIN Signal Discovery is retained.
- Existing CAN, CAN-FD, LIN, ETH, Review PDU, Analog and Unknown processing is retained.

## Previous history
## v0.49.3 — LIN Signal Discovery

Validated against the user-provided `Test.pcapng` before implementation.

### Capture observations
- Capture size inspected: ~127 MB.
- TECMP EtherType 0x99FE is present.
- TECMP Data Type 0x0004 (LIN) is present.
- 13,605 valid LIN records were observed with the current TECMP/LIN record interpretation.
- LIN traffic spans 11 actual channel IDs in this capture.
- Observed LIN response lengths are 1, 2, 4 and 8 bytes.
- LIN IDs/PIDs span multiple identifiers; signal discovery therefore remains isolated by actual channel + LIN frame ID.

### LIN-specific discovery
- Added `Detect LIN signals` in Signal Discovery.
- LIN-only mode analyzes every normalized occurrence in the committed Analysis Range (or full loaded capture when no range is committed).
- TECMP record channel ID and LIN frame ID remain separate grouping dimensions.
- PID, LIN response length and checksum are excluded from signal-field inference; only normalized LIN response data bytes are analyzed.
- Existing bit-transition correlation and candidate-boundary inference are retained.
- Little-endian receives only a small LIN prior when evidence is ambiguous; clearly stronger Big-endian behavior can still win.
- Candidate results retain direct `+ Config` workflow.

### Configuration correction
- Big-endian candidate transfer now converts the detector's physical candidate bit to the Configuration workspace's Motorola physical start-bit convention.

### Retained
- Generic CAN/CAN-FD/ETH/Review-PDU discovery remains unchanged.
- Counter, CRC, repetitive-pattern, Auto Activity, Recent Sessions and Graphics behavior are retained.

## Previous history
## v0.49.2 — Detected Signal → Configuration

Base: v0.49.1 Possible Signal Detection.

### Direct configuration workflow
- Every detected possible signal now has a `+ Config` action.
- The action creates a real row in the existing Signals configuration table.
- Generated row contains protocol, actual channel ID, message/frame/PDU ID, byte position, bit position, detected bit length, inferred endian and integer type.
- Safe defaults are factor=1, offset=0, signed=false and blank unit.
- Generated signal names use `PossibleSignal_B<byte>_b<bit>_L<length>` and are made unique automatically.
- Detection confidence and observed min/max are retained in the description when that column exists.
- The new row is immediately passed through the normal signal configuration parser, persisted in the Recent Session, and becomes available to the existing decoded-signal/Graphics workflow.
- User can then open Configuration > Signals and edit the generated name, datatype, endian, scale, offset, unit, etc.
- Existing imported Signals tables are respected: values are inserted by recognized column aliases instead of assuming fixed column positions.

### Retained
- v0.49.1 DBC-free field-level possible-signal detection.
- v0.49.0 UI polish.
- Auto Activity Range, channel/session restore, ETH hierarchy, counter/CRC/pattern analysis and existing Graphics behavior.

## Previous history
## v0.49.1 — Possible Signal Detection

Base: v0.49.0 UI Polish.

### DBC-free possible-signal inference
- Candidate detection now operates on contiguous bit fields rather than reporting only individual changing bits.
- Each bus + actual channel + frame/PDU remains isolated.
- Uses the committed Analysis Range when one is active.
- Builds per-bit transition signatures and groups adjacent changing bits with correlated behavior.
- Tests practical candidate widths up to 32 bits, including non-byte-aligned fields.
- Compares Little- and Big-endian numeric interpretations and reports the stronger plausible interpretation.
- Reports start byte, start bit, bit length, likely endian, likely type, observed min/max, transition count and confidence.
- Candidate names are intentionally not invented from payload behavior alone.
- Existing counter/CRC/pattern tools remain separate and unchanged.
- Full Ethernet payload width remains supported; there is no 64-byte candidate cap.

### UI
- Signal Discovery table is updated for field-level candidates and confidence percentages.
- v0.49.0 UI polish and all v0.48.x workflows are retained.

## Previous history
## v0.49.0 — UI Polish

Base: v0.48.7 Auto Activity Range.

### UI beautification
- Start page is now a clear numbered workflow: 1 Capture, 2 Configuration, 3 Channel Selection.
- Configuration is intentionally placed before channel selection so configured channel names are visible before loading.
- Channel IDs use aligned monospace cells while configured names remain visually emphasized.
- Global spacing, margins and corner radii were normalized for a cleaner desktop engineering-tool appearance.
- Summary statistic cards use consistent internal spacing and hierarchy.
- Start header/status wording and displayed version were refreshed.
- Workflow footer now matches the actual capture/configure/select/analyze sequence.
- Existing theme colors are retained; this pass avoids decorative color changes.

### Behavior retained
- No decoding, protocol, signal, CRC, counter, pattern or Auto Activity algorithm was intentionally changed.
- v0.48.7 Auto Activity Range remains intact.
- v0.48.6 Recent Session channel restore remains intact.
- Scrollable channel selection, configured names, Windows GUI launch and ETH hierarchy remain intact.

## Previous history
## v0.48.7 — Auto Activity Range

Base: v0.48.6 Recent Session Channel Restore.

### Automatic testcase focus
- After the user loads the selected channels, LogLens automatically calculates a useful activity window.
- Analysis is grouped by bus + actual channel + frame/PDU identity.
- CONSTANT bytes do not extend the window.
- Verified alive-counter bytes, CRC/checksum candidates and stable repetitive housekeeping patterns do not extend the window.
- The earliest and latest transition of remaining meaningful ACTIVE bytes define the testcase interval.
- A 1.0 second guard is retained before the first activity and after the last activity.
- The resulting interval becomes the committed Analysis Range and the Graphics viewport is focused to it.
- Raw selected-channel data is NOT deleted or physically trimmed.
- `Auto Activity` recalculates the range at any time.
- `Full Range` returns analysis to the complete selected-channel capture.
- If no meaningful activity is found, LogLens safely keeps the full capture.

### Retained
- v0.48.6 Recent Session channel restore.
- v0.48.5 scrollable pre-load channel selector and configured channel names.
- v0.48.4 early selected-channel loading.
- Windows GUI launch, ETH hierarchy and existing signal/pattern/CRC/counter analysis.

## Previous history
## v0.48.6 — Recent Session Channel Restore

Base: v0.48.5 Channel Selector Fix.

### Recent Session persistence
- Saves the pre-load Channel Selection with the Recent Session.
- Closing and reopening LogLens retains the selected actual channel IDs.
- Opening a PCAP/PCAPNG Recent Session restores the Start page first; it does not immediately decode the full capture.
- The capture is rescanned for currently available channels and the saved selection is reapplied.
- Saved channel IDs that no longer exist are not silently remapped; they are reported in the Tool Log.
- Sessions created by older LogLens versions remain compatible. If no saved pre-load selection exists, all discovered channels are selected.
- Channel configuration tables/names continue to restore with the session.
- Checkbox, Select All and Clear All changes are persisted immediately when a Recent Session exists.

### Retained
- v0.48.5 independent selector scrolling and configured channel-name display.
- v0.48.4 early selected-channel loading.
- v0.48.3 Windows GUI launch behavior.
- ETH hierarchy and v0.48.1 analysis behavior remain unchanged.

## Previous history
## v0.48.5 — Channel Selector Scroll + Configured Names

Base: v0.48.4 Pre-Load Channel Selection.

### Fixes
- The Start-page Channel Selection list now has its own 360 px scroll viewport and an always-visible vertical scrollbar.
- Large channel sets (for example 44+ channels) can be scrolled independently of the outer Start page.
- Channel configuration names are resolved on the Start page immediately after a Channels configuration is loaded.
- A configured channel is displayed as: `0x4002  <Configured Name>  CAN-FD`.
- The raw channel ID remains hexadecimal and visible even when a configured name exists.
- Unknown channels remain visible/selectable and may also receive configured names.
- The selector shows how many configured channel-name mappings are active.
- Pre-load filtering behavior from v0.48.4 is unchanged.
- Windows GUI launch, ETH hierarchy and the v0.48.1 analysis engine remain unchanged.

## Previous history
## v0.48.4 — Pre-Load Channel Selection

Base: v0.48.3 Windows GUI Launch.

### Start-window workflow
- Selecting a PCAP/PCAPNG capture first performs channel discovery.
- A `CHANNEL SELECTION` section appears on the Start page before the full decode.
- Channel IDs shown here are the actual capture/TECMP channel IDs, not TECMP data-type IDs.
- Channel IDs are displayed in hexadecimal (`0x0001`, `0x000A`, etc.).
- Each discovered channel shows its detected protocol/type.
- Unrecognized TECMP data types are retained and shown as `Unknown`.
- `Select All` and `Clear All` controls are provided.
- `Load Selected Channels` starts the full processing stage.

### Performance behavior
- For TECMP captures, non-selected channel records are rejected before protocol normalization,
  Review-PDU generation, indexing, classification and waveform/analysis preparation.
- Ordinary Ethernet PCAPNG interface channels are also gated before normalization.
- Existing runtime Project Tree filtering remains separate from this pre-load filter.
- Non-PCAP formats retain their existing direct-load behavior.

### Retained behavior
- v0.48.3 Windows release GUI subsystem behavior is retained.
- v0.48.2 ETH UDP/Review hierarchy is retained.
- v0.48.1 pattern, CRC/counter, signal and graphics analysis behavior is retained.
- `VERSION_HISTORY.md` remains the single root-level version/update document.

## Previous history
## v0.48.3 — Windows GUI Launch

Base: v0.48.2 Rebased ETH Stream Hierarchy.

- Windows release builds now use the Windows GUI subsystem.
- Double-clicking the release `LogLens.exe` starts the LogLens GUI without opening a Command Prompt window first.
- Debug builds retain the console for development diagnostics.
- The change is Windows-only; macOS/Linux behavior is unchanged.
- ETH stream hierarchy and the v0.48.1 analysis engine are unchanged.
- This remains the single root-level version/update file.


## v0.48.2 — Rebased ETH Stream Hierarchy

Base: v0.48.1 Full Pattern Detection + ETH Review Hierarchy.

### Upgrade
- Groups ETH Graphics entries by the exact UDP 4-tuple.
- Shows the original parent as `ETH/UDP stream`.
- Keeps child entries as `REVIEW:PDU:0x...`.
- A Review PDU is nested only under the UDP stream that generated it.
- Same PDU ID on different UDP streams remains isolated.
- v0.48.1 decoding, indexing, pattern detection, CRC/counter analysis,
  Signal Config and waveform behavior are retained.
- Non-UDP ETH sources retain their existing presentation.
- All root-level version/update MD/TXT files are consolidated here.

### Intended hierarchy

    ETH
    └── Channel
        ├── ETH:UDP:172.28.1.4:20000>172.28.1.7:20000
        │   ├── ETH/UDP stream
        │   ├── REVIEW:PDU:0x109
        │   ├── REVIEW:PDU:0x10B
        │   └── REVIEW:PDU:0x150
        └── ETH:UDP:172.28.1.5:20000>172.28.1.7:20000
            ├── ETH/UDP stream
            └── REVIEW:PDU:0xABC


---

## Imported history: MACOS_BUILD.md

# LogLens v0.3 macOS build

Target: Apple Silicon (`aarch64-apple-darwin`).

## Build a normal macOS application

```bash
source "$HOME/.cargo/env"
cd LogLens_Rust_Replica_v0.3_macOS
./build_macos_app.sh
```

Output:

- `target/aarch64-apple-darwin/release/loglens-replica` — native executable
- `dist/LogLens.app` — Finder-launchable application bundle

You can also double-click `build_macos_app.command` in Finder.

## Launch

```bash
open dist/LogLens.app
```

## Install into Applications

```bash
cp -R dist/LogLens.app /Applications/
```

## Optional DMG

```bash
./create_macos_dmg.sh
```

Output: `dist/LogLens-macOS-arm64.dmg`.

The script performs ad-hoc signing for local use. Public distribution without Gatekeeper warnings requires an Apple Developer ID certificate and notarization.


---

## Imported history: PARITY_MATRIX.md

# LogLens Rust Replica v0.3 parity target

This build treats the Python/Streamlit LogLens v1.2 project as the behavioral reference.

## Implemented in v0.3
- Start/open-log flow, sessions and recycle bin
- Summary cards
- Frames & Filters: text/bus/channel filters, VehicleDB, EventLog, ChannelMap, manual/event time window, CSV export
- Virtualized frame table (renders visible rows instead of all matching frames)
- Cached filtering (recomputed only when filter changes)
- Frame Explorer: VehicleDB/Event context, frame/channel selection, byte activity, bit activity, value counts, reference comparison, payload-change events, byte/bit waveform, two-frame bit activity comparison, custom signal preview
- Signal Processing: IdentifiedSignals import, decoded signal summaries, on-demand DBC-free candidate detection
- Signal Waveforms: selected physical signals, time range, downsampled plots, readable points, basic transition latency
- PCAP/PCAPNG + TECMP + CAN/CAN-FD/LIN/Ethernet/Analog/Unknown baseline from the Rust core

## Performance changes
- Signal candidate detection is no longer automatic at log load
- Filtered rows are cached
- Frame table is virtualized
- Waveforms use min/max downsampling before painting
- Dataset remains shared through Arc; UI avoids copying frame payloads for display

## Remaining parity work
- Exact Python HTML report templates
- Exact EventLog adjacent-trigger heuristics and all schema aliases
- Editable VehicleDB grid semantics
- Complete agent/LLM panels
- All XLSX export variants
- Full benchmark/evidence report UI
- Pixel-level visual parity with Streamlit theme


---

## Imported history: README.md

# LogLens Rust Replica v0.13.0

## v0.13.0 Hybrid Native Engine
LogLens now uses a Rust + C++17 hybrid architecture. C++ handles high-volume PCAP/PCAPNG scanning, batched payload-change analysis, and waveform min/max downsampling through a stable C ABI. Rust remains responsible for the desktop UI, project/configuration workflow, TECMP/protocol normalization and orchestration. The existing Rust capture reader remains as a compatibility fallback. See `V0130_HYBRID_NATIVE_ENGINE.md`.

## v0.12.3 icons, Tool Log and Recent Sessions
- Replaces symbol-font toolbar icons with reliable text/ASCII controls to prevent box/tofu glyphs.
- Adds a resizable Tool Log panel with capture/config/session/error logging.
- Moves Recent Sessions to persistent per-user application storage and repairs stale relative paths.
- Deduplicates reopened captures and sorts sessions by last-opened time.
- See `V0123_ICONS_TOOLLOG_SESSIONS.md`.


**v0.10.2:** Toolbar and control organization was redesigned into compact File, Replay, View, Trace, Graphics, Filter, Navigation and Cursor groups. Replay timeline/status now has a dedicated row to avoid wrapping glitches. See `V0102_TOOLBAR_ORGANIZATION.md`.

**v0.10.1:** Graphics configuration now persists across replay restarts and reloads of the same capture. See `V0101_GRAPHICS_PERSISTENCE_FIX.md`.

# v0.9.5.1 compile-fix build

# LogLens Rust Replica v0.9.5

CANoe-style desktop log analysis with Overwrite Trace, offline replay, multi-frame/channel Graphics, event triggers, pan/zoom and detachable panes.

See `V094_FEATURES.md` for this revision.


## v0.9.2 timestamp corrections
- PCAP/PCAPNG outer capture timestamps are the canonical absolute timeline.
- Prevents TECMP device-uptime H2 values from appearing as capture start time.
- Events/Triggers show absolute readable start/end timestamps.
- Selecting an event scopes offline replay to that event and seeks to its start.
- Replay stops at the selected event end; clearing event selection restores full-capture replay.


## v0.9.1 readable timestamps

The UI now uses the first recorded frame timestamp as the capture start and the last recorded frame timestamp as capture end. Trace rows show readable absolute timestamps (UTC for Unix-epoch timestamps), while relative seconds remain internal to replay, pan and zoom. Hover a timestamp to see raw nanoseconds and relative seconds. Delta-time values automatically use µs, ms or s.


## v0.9 — Waveform pan/zoom and offline replay

The CANoe-style Graphics workspace now has a synchronized time viewport across stacked lanes. Use the mouse wheel to zoom around the pointer, drag to pan, Shift+wheel to pan, or use Fit / +/- / left-right controls. A shared replay playhead, A/B cursors and Control Message event markers use the same time axis.

Local logs can now be replayed offline after loading. Use **Run Offline**, Pause/Resume, Stop, replay speed, and the timeline slider. During replay the Trace window reveals frames according to their recorded timestamps and Graphics can follow the playhead. This is file replay only; it does not transmit onto a physical network bus.

# LogLens Rust Replica v0.8.1

CANoe-style adjustable/detachable workspace. v0.8.1 fixes vertical resizing of detached analysis windows, especially Graphics.

# LogLens Rust Replica v0.8 — CANoe Workspace

v0.8 changes the default LogLens interaction model from tab-first analysis to a CANoe-style engineering workspace while keeping the v0.4 parser/analysis engine.

## Default workspace

- Measurement/replay toolbar: Start, Stop, Pause/Resume, replay speed
- Project / Network tree grouped by protocol and configured channel
- Channel Name Config, VehicleDB, EventLog and IdentifiedSignals access
- Central virtualized Trace window
- Right-side Frame Details + byte inspector
- Bottom multi-byte Graphics window
- A/B time cursors with delta-time display
- Trace context menu actions: Add changing bytes to Graphics, Analyze Bytes/Bits, Filter this ID
- Simplified navigation: Measurement, Analysis and Configuration. Legacy filtering/explorer/signal workflows are integrated into these engineering workspaces instead of exposed as duplicate top-level pages.

## Channel-name workflow

Load a CSV/XLSX containing `channel_id` and `channel_name`. Decimal and `0x...` hexadecimal IDs are supported. Names are used in Network Tree, Trace, filters and explorer channel selectors.

## Byte waveform workflow

1. Open a log.
2. Select a row in Trace.
3. Click bytes in the Details / Byte Inspector panel, or right-click Trace -> Add changing bytes to Graphics.
4. The Graphics window overlays the selected raw byte waveforms (0..255).
5. Set Cursor A and Cursor B to inspect timing difference.

## Build macOS Apple Silicon

```bash
source "$HOME/.cargo/env"
rustup target add aarch64-apple-darwin
./build_macos_app.sh
open dist/LogLens.app
```

## Build Windows with MinGW (no Visual Studio required)

```powershell
rustup toolchain install stable-x86_64-pc-windows-gnu
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

Executable:

`target\x86_64-pc-windows-gnu\release\loglens-replica.exe`

## Performance design retained from v0.4

- virtualized trace/frame rows
- cached filters
- shared `Arc<Dataset>` rather than cloning captures
- signal detection on demand
- waveform min/max downsampling

## Important

v0.8 is a CANoe-inspired workflow, not a copy of Vector CANoe proprietary UI/code. The goal is an engineering-tool interaction model suitable for LogLens.


## v0.8 additions
See `V06_FEATURES.md` for scrollable CANoe-style stacked signal lanes and TECMP Control Message (`0x00`) event-trigger mapping.


## v0.8 Organized Workspace
- Trace is always the primary work area.
- Details, Graphics, and Events are mutually exclusive inspector panes.
- No simultaneous right-details + bottom-graphics + event list layout.
- Trace-only mode for maximum log visibility.
- Adjustable inspector height and compact signal navigator.
- Events moved to a dedicated table pane; Project tree remains for navigation only.


## v0.8 workspace docking
The CANoe workspace now supports draggable splitters, bottom/right docking, and detachable floating Details, Graphics, and Events/Triggers tabs. See `V08_FEATURES.md`.

## v0.9.3 Updating Trace
The CANoe workspace Trace view now supports live-style updating during offline replay. Enable **Auto-follow** to keep the newest replayed frame in view. Use **Freeze View** to hold the current Trace rows while replay continues, then **Resume View** to catch up immediately. This is separate from Pause/Stop, which control the replay engine itself.


## v0.9.5 performance
See `V095_PERFORMANCE.md`. This release caches scope/graphics/event data, incrementally updates Overwrite Trace during replay, caches indexed waveform series, and limits UI replay refresh to ~30 FPS.

## v0.9.6 additions
- Trace **Changing only** filter for suppressing static frame/channel sources.
- Graphics **Changing only** source filter.
- Raw Automotive Ethernet from Ethernet-linktype PCAP/PCAPNG: VLAN, IPv4, UDP/TCP, ARP/IPv6 classification and basic SOME/IP Message-ID recognition.


## v0.9.7 — Static-frame filter correction

`Hide Static Frames` now classifies each unique `Protocol + Channel + Frame ID` using the **entire loaded capture from start to end**. A source is hidden only if every payload occurrence is identical. If the payload changes even once and later returns to its original value, the source remains visible. The classification is independent of replay position, selected event, manual time window, and current UI filters. The same precomputed classification is reused by Trace and Graphics.


## v0.9.8 — Global Project Tree scope

Project Tree channel selection is now a global source filter shared by Trace and Graphics. Unchecking a channel removes its frames from Trace and hides its waveform sources/lanes in Graphics. Existing Graphics selections are retained and become visible again if the channel is re-enabled. `Hide Static Frames` remains an additional filter after the Project Tree scope.


## v0.11.1 simple configuration
Use the templates in `templates/`: `project.csv`, `channels.csv`, `messages.csv`, `signals.csv`, and optional `events.csv`. Older configuration formats remain compatible.

## v0.11.2 UI consistency
Measurement, Analysis and Configuration share the same command/replay controls. Analysis and Configuration use a Graphics-style left navigator with a scrollable work surface, and controls have larger desktop hit targets. See `V0112_UI_STRUCTURE_SCROLLING.md`.


## v0.12.0 workspace cleanup

- Measurement owns replay and trace controls.
- Analysis has a dedicated analysis toolbar and navigator.
- Configuration has a dedicated editor toolbar and navigator.
- Removed duplicate replay/Graphics/Details/Events controls from non-measurement workspaces.
- Removed loaded-log summary cards to prioritize engineering work area.
- See `V0120_UI_CLEANUP.md`.


## v0.12.1 alignment and control sizing
- Enlarges the button widgets themselves instead of globally increasing UI padding.
- Keeps toolbars on one aligned row with horizontal scrolling on narrow windows.
- Uses consistent button widths/heights for Measurement, Analysis and Configuration.
- Uses full-width aligned navigation buttons in Analysis and Configuration.
- Removes duplicate Add/Save controls from inside configuration tables.
- Aligns configuration headers and editable cells to fixed column widths.
- Reduces panel padding so larger controls do not make the workspace look inflated.


## v0.47.8 Signal Configuration / Decode Fix
See `V0478_SIGNAL_CONFIG_DECODE_FIX.md`.

## v0.47.9 Physical Bit / Endian Fix
See `V0479_PHYSICAL_BIT_ENDIAN_FIX.md`.


---

## Imported history: README_V0.25.1_IMPLEMENTATION_LOCATION.txt

LogLens v0.25.1

WHERE TO FIND THE NEW IMPLEMENTATION
------------------------------------
Open LogLens -> Analysis -> Frame / Byte Analysis.

At the top, expand:
"Multi-Frame Signal Classification (ACTIVE / STATIC / COUNTER / CRC)"

The panel contains:
- multiple frame selection
- Hide STATIC
- Hide COUNTER
- Hide CRC candidates
- Analyze A-B range
- Class column: ACTIVE / STATIC / COUNTER4 / COUNTER8 / CRC_CANDIDATE
- Confidence, Changes, Activity %, Unique, Min, Max, Value

Implementation source:
- src/analysis.rs : byte_activity_indexed(), ScopedByteActivity
- src/app.rs      : Multi-Frame Signal Classification UI

Performance:
Only selected FrameIndex.by_key records are scanned. Results are cached.
Visibility toggles do not rescan the capture.


---

## Imported history: README_V0.26.0_GRAPHICS_CLASSIFICATION.txt

LogLens v0.26.0 Graphics classification + scroll fix

Graphics:
- Large checkbox targets for ACTIVE / STATIC / COUNTER / CRC?.
- Defaults: ACTIVE ON; STATIC/COUNTER/CRC OFF.
- Raw byte lane headers show classification; candidates show confidence.
- Hidden classes are skipped before waveform generation.
- Classification is cached per bus/channel/frame/byte and cleared with waveform cache.

Mouse:
- Plain wheel = vertical graph/lane scrolling. It never zooms.
- Ctrl+Wheel = X/time zoom.
- Alt+Wheel = independent Y zoom.
- Shift+Wheel = X/time pan.
- Middle drag = X pan.
- Right drag = Y pan.
- A/B measurement cursors retained.


---

## Imported history: README_V0.27.0_VALIDATED_CRC_COUNTER.txt

LogLens v0.27.0 - CRC/Alive Counter validation update

Validated against the supplied Event_Marker_Implementation.pcapng.

Key fixes
- Alive-counter detector no longer requires 95% perfect +1 transitions.
- It accepts small modular forward jumps (1..3) caused by missed/capture-gap messages.
- Requires strong exact +1 behavior plus strong tolerant continuity to reduce false positives.
- Supports 8-bit and low-nibble 4-bit alive counters.
- Automotive B0/B1 prior: if B1 is a verified alive counter and B0 is high-change/high-diversity,
  B0 becomes a CRC/CHECKSUM CANDIDATE. This is evidence, not hardcoded proof.
- Static/low-diversity B0/B1 are not forced into CRC/counter classes.
- Generic high-entropy CRC candidate fallback remains for other layouts.

User-facing terminology
- SIGNAL ACTIVITY
- CONSTANT
- ALIVE COUNTER (4-bit/8-bit)
- CRC / CHECKSUM CANDIDATE
- Analysis table includes a Why? explanation.
- Graphics uses the same understandable labels.

Important
CRC/checksum classification is candidate-level unless an exact checksum/CRC algorithm is proven.


---

## Imported history: README_V0.28.0_NATIVE_DETACHED_WINDOWS.txt

LogLens v0.28.0 - CANoe-style detached windows

Changed:
- Details, Graphics and Events/Triggers now detach using egui/eframe native viewports.
- Detached pane becomes a separate operating-system window rather than an egui Window inside LogLens.
- Separate windows can be moved/resized independently and moved to another monitor.
- Each detached window has Dock Bottom and Dock Right controls.
- Closing the native window removes it from the detached-pane set.
- Existing graph state, A/B cursors, filters, CRC/counter classification and project state remain owned by the main app.

UI:
Open Window -> Detach Window
Tooltip explicitly describes separate OS window behavior.

Dependency:
eframe/egui 0.33 native multi-viewport API (show_viewport_immediate).


---

## Imported history: README_V0.29.0_UX_BYTE_SELECTION.txt

LogLens v0.29.0 UX Consolidation
- Per-frame B0..B63 byte selection.
- All / None / Signals Only / Invert.
- Byte selection affects Byte Analysis and raw-byte Graphics; Trace remains raw and complete.
- Cleaner user terminology: Byte Analysis instead of Multi-Frame Signal Classification / Frame-Byte Analysis.
- CRC/counter improvements from v0.27 retained.
- True native detached windows from v0.28 retained.
- Internal caches remain hidden implementation details.


---

## Imported history: README_V0.29.1_COMPILE_LOGO_FIX.txt

LogLens v0.29.1
- Fixed E0308 in selected_frame_bytes HashMap insertion: &str -> owned String.
- Added/normalized LogLens application icon asset.
- Embedded icon into native eframe viewport when main.rs uses default NativeOptions.
- Retains v0.29 per-frame byte selection and UX cleanup.


---

## Imported history: README_V0.29.2_TRACE_BYTE_CHECKBOX_LOGO.txt

LogLens v0.29.2
- Byte selection is now available from Trace.
- Selected Trace frame shows Bn + current hex-value selection tiles.
- Non-virtual Trace payload bytes are individually clickable.
- Clicking bytes changes Analysis/Graphics selection only; raw Trace data remains intact.
- Replaced tiny checkbox-only interaction with 72x30 selectable tiles for frame/byte selection.
- Increased general interaction height to 30px where the existing style hook is present.
- Fixed the real native logo bug: icon was decoded in main.rs but never passed to ViewportBuilder. It is now wired using with_icon(...).


---

## Imported history: README_V0.29.3_COMPILE_FIX.txt

LogLens v0.29.3
- Fixed E0599: selection_tile helper is now actually defined at module scope.
- Trace byte selection retained.
- Larger 72x30 selection hit targets retained.
- Native icon with_icon wiring retained.


---

## Imported history: README_V0.30.0_GRAPHICS_SOURCE_TREE.txt

LogLens v0.30.0 - Graphics Source Tree
- Byte selection moved out of Trace and into Graphics.
- Graphics hierarchy: Protocol > Channel > Frame > Byte.
- Each byte row shows classification: Signal, Counter, CRC/Checksum, Constant, Review.
- Entire 176x30 byte row is clickable, avoiding tiny checkbox hit targets.
- Per-source byte selection is independent for each frame/channel.
- Frame labels visually show none/all/partial state using checkbox/partial markers.
- Trace restored to raw payload display.
- Native tool icon now uses the project's existing assets/LogLens_Dark.png. Generated replacement icon removed.


---

## Imported history: README_V0.31.0_WINDOWS_RUNTIME_PERFORMANCE.txt

LogLens v0.31.0 - Windows Runtime Performance
Performance-focused release; UI structure retained.
- Graphics byte classification changed from one full indexed analysis per BYTE to one analysis per FRAME SOURCE, cached for all bytes.
- Closed Graphics frames no longer trigger classification merely because bytes were previously selected.
- Graphics starts with zero raw-byte lanes; only user-selected bytes are plotted.
- Lane ordering now uses each source's own byte mask instead of source x global-byte Cartesian combinations.
- Toggling one byte no longer clears every waveform cache or every lane.
- Cached waveform LOD reduced 12000 -> 6000 points using min/max preservation.
- Per-repaint lane rendering reduced 1800 -> 1200 points.
- Trace remains virtualized/raw.
- Original LogLens_Dark.png native icon retained.


---

## Imported history: README_V0.31.1_GRAPHICS_TREE_FIX.txt

LogLens v0.31.1
Graphics tree interaction fix: expansion independent from selection; frame select-all; byte-only toggle; stable open state; tri-state frame marker. v0.31 Windows performance optimizations retained.


---

## Imported history: README_V0.31.2_PROJECT_STYLE_GRAPHICS_TREE.txt

LogLens v0.31.2
Graphics > Signals source tree now uses the same egui interaction style as Project:
- standard CollapsingHeader arrows for Protocol, Channel and Frame
- standard checkboxes for selection
- frame expansion ID is stable and independent from selection
- byte checkbox changes only that byte and does not collapse the frame
- complete-frame selection via standard checkbox / All bytes
- partial selection is indicated in the frame selection row
- v0.31 Windows performance optimizations retained


---

## Imported history: README_V0.31.3_BYTE_MASK_PATCH.txt

LogLens v0.31.3
Fixed Graphics individual-byte mask initialization: first byte selection starts from zero instead of all bits. Whole-frame selection remains separate. Project-style tree and v0.31 performance optimizations retained.


---

## Imported history: README_V0.32.0_CANOE_GRAPHICS.txt

LogLens v0.32.0 - CANoe-style Graphics Workspace

Changes:
- compact Run/Pause/Stop + Fit/Zoom/Pan Graphics command bar
- wider Sources/Decoded signal navigator
- stacked independent signal lanes with shared time domain
- per-lane Y zoom/pan retained
- A/B synchronized cursors retained
- lane current/min/max value header
- CANoe-like grid and dedicated Y label gutter
- full-recording overview navigator with draggable visible window
- lane reorder retained
- raw byte classification and decoded signals retained
- v0.31.3 individual-byte mask fix retained
- Windows performance optimizations retained


---

## Imported history: README_V0.33.0_CURSOR_SEGMENT_TECMP_FIX.txt

LogLens v0.33.0
- Fixed A/B cursor coordinate mapping to use the waveform graph area rather than the outer lane.
- Cursor drag no longer jumps on ordinary waveform clicks; 12 px grab target.
- Cursor value lookup uses sample-and-hold (last sample at/before cursor).
- Added Save A→B Log: writes packet-preserving PCAPNG segment and sidecar .loglens_config folder containing project/channels/messages/signals/events and workspace state.
- Expanded TECMP handling to all data types defined by Technica libtecmp: Analog SLA is Analog; FlexRay/GPIO/UART are retained as TECMP instead of incorrectly becoming Unknown. Truly unrecognized data types remain Unknown.
- Existing v0.31.3 byte mask and v0.32 CANoe Graphics behavior retained.
Note: exact remaining Wireshark-vs-LogLens Unknown records still require the user's affected capture for byte-level validation.


---

## Imported history: README_V0.34.0_LIN_CSV_XLSX_EXPORT.txt

LogLens v0.34.0
1. TECMP 0x0004 is classified as LIN before payload parsing. A malformed/unhandled LIN payload remains LIN:RAW instead of leaking into TECMP/Unknown UI classification.
2. The same recognized-bus fallback is applied to CAN, CAN-FD and Ethernet.
3. Graphics A/B export now has:
   - PCAPNG A→B: lossless original packet segment
   - CSV A→B: all normalized records in the cursor range
   - Excel A→B: Summary + Frames + per-protocol sheets
4. CSV/XLSX include sequence, absolute timestamp ns, relative time, protocol, channel, direction, frame ID/key, DLC, TECMP data type, decoded payload and raw payload.
5. Existing v0.33 cursor fixes and v0.31.3 byte-mask fix are retained.

Validation note:
- Static source checks completed in this environment.
- Cargo is not installed here, so a full Rust compile could not be executed.
- Exact LIN ID/payload parity with Wireshark still needs the affected capture if its vendor LIN payload layout differs from the current parser.


---

## Imported history: README_V0.35.0_SELECTED_MAPPED_EXPORT_UX.txt

LogLens v0.35.0 - Selected + Mapped Export UX

Implemented:
- CSV and Excel export now use A/B time range AND the current Graphics selection.
- Selecting a raw message/byte selects that parent frame for export.
- Selecting a configured signal selects only its matching parent frames.
- Empty selection is blocked instead of silently exporting the whole A/B range.
- CSV Selected A→B exports configured/mapped signal samples (raw value + physical value + unit).
- Excel Selected A→B contains Summary, Selected Frames, Mapped Signals, and Signal Configuration sheets.
- Mapping reuses LogLens SignalDefinition and decode_signal logic: channel, frame ID, start byte/bit, length, endian, signed, factor, offset, unit.
- Graphics shows a live export-selection count.
- PCAPNG is explicitly labelled packet-preserving because message/byte-level removal from a TECMP container would require packet reserialization; CSV/XLSX are the precise selected-data exports.
- v0.34 LIN/TECMP normalization, v0.33 cursor fix, and v0.31.3 byte-mask fix retained.

Validation:
- Static brace/source validation performed.
- Full Cargo compilation was not available in this environment.


---

## Imported history: README_V0.36.0_INTEGRATED_ANALYSIS_WORKFLOW.txt

LogLens v0.36.0 - Integrated Analysis Workflow

Implemented in this upgrade:
- One Export… entry point replaces separate CSV/XLSX/PCAPNG buttons.
- Export Manager previews Range, Content, Mapping Status and Format before writing a file.
- Range options: Cursor A→B, visible Graphics range, full recording.
- Content options: current shared Graphics selection or all data in range.
- Mapping preview reports selected frames, configured signals, mapped signals and mapped samples.
- Empty current selection is visibly blocked instead of silently exporting unrelated traffic.
- Excel/CSV continue to use the current LogLens signal configuration and common decoder.
- PCAPNG limitation is shown in the dialog: original packet containers are preserved; message/signal-level selection belongs to decoded exports.
- Existing event→cursor/time navigation remains integrated with the Graphics timeline.
- Existing Project Tree source scope remains shared by Trace and Graphics.
- v0.35 selected/mapped export, v0.34 LIN normalization, cursor and byte-selection fixes retained.

Deferred from v0.36 to avoid destabilizing the build:
- asynchronous export worker/progress cancel
- single-file .loglens workspace packaging
- full bidirectional row highlighting across every panel
- waveform LOD pyramid

Validation:
- Static source/bracket checks completed.
- Cargo is not installed in this execution environment, so full Rust compilation was not executed here.


---

## Imported history: README_V0.37.0_ANALYSIS_INTELLIGENCE.txt

LogLens v0.37.0 — Analysis Intelligence

Added:
- Timing / Jitter analysis for the currently selected frame.
- Median/mean/min/max cycle time, peak-to-peak jitter and gap/late-frame candidates.
- Jump-to-anomaly navigation.
- Counter / CRC Intelligence page using the existing byte classifier.
- 4-bit and 8-bit alive-counter integrity analysis with exact +1 continuity and discontinuity timestamps.
- CRC/checksum remains explicitly a candidate until its algorithm is proven.
- Cursor A↔B comparison for raw bytes.
- Cursor A↔B comparison for configured physical signals and units.
- Existing Signal Discovery remains available and uses the current analysis workflow.
- v0.36 Export Manager, v0.35 selected/mapped export and v0.34 LIN normalization retained.

Scope deliberately not included yet:
- CRC polynomial brute-force search.
- DBC/ARXML import.
- AI/LLM analysis.
- Live CAN hardware.
- Automatic semantic signal naming.

Validation:
- Static source/brace checks performed.
- Full Cargo compilation is not available in this environment.


---

## Imported history: README_V0.38.0_PERFORMANCE_UI_HARDENING.txt

LogLens v0.38.0 — Performance & UI Hardening

Graphics UI cleanup
- v0.38 defaults the Graphics information-density concept to Compact.
- Fit All shortened to Fit and zoom labels compacted.
- Permanent mouse instruction string reduced; detailed controls are intended for the overflow/help menu.
- Added GraphicsDensity state (Compact / Standard / Detailed) and compact overflow-menu helper.
- Existing selection, cursors, events, mapping and Export Manager behavior retained.
- This release establishes the UI state needed to progressively move verbose lane metadata into Standard/Detailed/Inspector views.

Performance foundation
- Added TimeRangeIndex with binary partitioned A→B retrieval per frame key.
- Added multi-resolution LodSeries builder using min/max envelope preservation.
- Added PerformanceCounters foundation.
- Existing v0.37 timing/counter/A-B intelligence retained.

Important validation status
- Static source checks performed.
- Full Cargo compilation was not available in this execution environment.
- TimeRangeIndex/LOD utilities are added as v0.38 engine foundations; not every existing Graphics/Trace code path has been migrated to them yet.
- Background export/task manager and crash-recovery autosave are NOT claimed as complete in this package.

Next validation on Mac
1. Build release.
2. Open the same PCAPNG used with v0.37.
3. Verify Graphics toolbar and selection behavior.
4. Verify Timing/Jitter, Counters/CRC and A↔B pages.
5. Verify CSV/XLSX/PCAPNG Export Manager.
6. Profile a 200–300 MB log before migrating remaining rendering paths to the LOD/index utilities.


---

## Imported history: README_V0.38.1_GRAPHICS_UI_FIX.txt

LogLens v0.38.1 Graphics UI Fix
- Actual Graphics renderer rewritten to a compact two-row header.
- Capture/visible/export-selection permanent rows removed.
- Filters, lane height, Clear and mouse help moved to overflow.
- Compact/Standard/Detailed now changes waveform lane text.
- Compact removes per-lane A/B/min/max text; current value remains.
- Standard adds min/max; Detailed adds A/B plus min/max.
- Raw lane labels shortened in Compact/Standard.
- Duplicate overview A/B/visible summary removed.
- v0.38 performance foundations retained.
Validation: static source/brace checks passed; target Cargo build still required.


---

## Imported history: README_V0.38.2_WORKSPACE_DEDUPLICATION.txt

LogLens v0.38.2 — Workspace Deduplication
- Removed top-level Details button; contextual panel is now Inspector.
- Removed top-level Events button; Events is now under Analysis.
- Removed Project Tree > Analysis > Graphics duplicate shortcut.
- Kept one top-level Graphics workspace entry.
- Inspector tabs now expose only Inspector and Graphics.
- Only Graphics remains detachable; duplicate Details and Events detached windows removed.
- Configuration > Events remains because it edits event definitions.
- Graphics Events toggle remains because it controls waveform markers.
- v0.38.1 compact Graphics UI retained.
Validation: static source checks passed; full target Cargo build required.


---

## Imported history: README_V0.38.3_WORKSPACE_START_LAYOUT_FIX.txt

LogLens v0.38.3 — Workspace + Start Layout Fix

This release reapplies the de-duplication against the actual source and aligns the startup window with the main LogLens UI.

Workspace:
- Removed top-level Details.
- Details is now contextual Inspector.
- Removed top-level Events workspace.
- Events added under Analysis.
- Removed Project Tree > Analysis > Graphics duplicate shortcut.
- Kept one main Graphics entry.
- Inspector tabs expose Inspector + Graphics only.
- Only Graphics remains detachable.
- Configuration > Events remains for editing event definitions.
- Graphics Events toggle remains for event-marker overlays.

Startup:
- Replaced centered welcome/marketing layout.
- Added LogLens / START application-style header.
- Added Open Capture primary panel.
- Added aligned Recent Sessions panel.
- Added Configuration strip using the same button sizing/hierarchy.
- Added workflow line and bottom Ready/version status bar.
- Reduced empty space and oversized typography.

Validation:
- Static brace/source assertions passed.
- Full Cargo compilation must still be run on the target Mac.


---

## Imported history: README_V0.39.0_TRACE_NAVIGATION_INTEGRATION.txt

LogLens v0.39.0 — Trace & Navigation Integration

Implemented:
- One persistent Home button in the loaded-workspace header.
- Home returns to the unified Start page without unloading Dataset/capture state.
- Start page exposes Return to Workspace when a capture is already active.
- Opening a capture/recent session exits Start and resumes the workspace.
- Trace toolbar reorganized around Mode, View, Search, Hide static and state.
- Trace search widened and labelled for ID/channel/protocol/data.
- Added compact Trace visible/total frame status.
- Added selected frame/time status when a Trace row is selected.
- Added compact Trace query-time feedback.
- Existing indexed/virtual chronological Trace path retained.
- Existing Inspector/Graphics integration retained.
- v0.38.3 workspace deduplication and unified Start layout retained.

Next v0.39.x hardening:
- Shared selection object across Trace/Graphics/Analysis/Events.
- Context-menu actions: Set A, Set B, Show in Graphics, Analyze selection, Export selection.
- Remove remaining full Dataset clones in Analysis/Events.
- Verify target Cargo build and fix compiler/API differences if any.

Validation:
- Static source assertions and brace balance passed.
- Full Cargo build must be run on target macOS environment.


---

## Imported history: README_V0.39.1_HOME_MEASUREMENT_CLEANUP.txt

LogLens v0.39.1 — Home Visibility + Measurement Cleanup
- Home is now in the visible workspace tab strip before Measurement.
- Home preserves the loaded capture and returns to Start.
- Removed Channels/Messages/Signals/Events import buttons from Measurement.
- Removed duplicate Configuration group from Measurement Project Tree.
- Configuration is the single configuration-management workspace.
- Measurement retains capture, replay, Project, Trace and Graphics operational controls.
Static checks passed; run the target macOS Cargo build for compile validation.


---

## Imported history: README_V0.39.2_HOME_COMPILE_EXPORT_WINDOW.txt

LogLens v0.39.2 — Home Compile Fix + Export Window

- Added missing show_start_page field to LogLensApp.
- Added show_start_page=false initialization.
- Home remains a single button in the workspace strip.
- Update routing now allows Start to be shown while Dataset remains loaded.
- Return to Workspace resumes the current loaded session.
- Export is treated as a movable/resizable utility window, not another duplicated top-level workspace.
- Existing Export Manager window patched: False
- Start routing gate patched: True

Static checks passed; target Cargo build required.


---

## Imported history: README_V0.39.3_HOME_NEW_CAPTURE_RECENT_SESSIONS_FIX.txt

LogLens v0.39.3 — Home / New Capture / Recent Sessions Fix

- Home refreshes Recent Sessions before showing Start.
- Open Capture works while another capture remains loaded.
- Start closes only after a new file is actually selected.
- Cancelling the picker keeps Home visible.
- Opening a valid Recent Session switches back to the workspace.
- Missing recent files stay on Home and report the error.
- Return to Workspace resumes the existing capture.
- Current capture is explicitly shown as retained while browsing Home.
Static checks passed; target Cargo build required.


---

## Imported history: README_V0.39.4_STABILIZATION_EXPORT_WINDOW.txt

LogLens v0.39.4 — Stabilization + Separate Export Window

Home / capture lifecycle
- Fixed stale Start version display: v0.39.4.
- load_path now returns whether the background load was accepted.
- Home only closes after a new capture load request is accepted.
- If another capture is already loading, Home remains visible and reports the condition.
- Recent Session only leaves Home when its load request is accepted.
- Existing Dataset remains available while browsing Home.

Export
- Export is rendered outside the CentralPanel/workspace content.
- It is one independent non-modal egui utility window.
- Window title: LogLens - Export.
- Movable, resizable, collapsible and independently closable.
- Default size 720x560; minimum width 560.
- Export retains current selection, A→B, visible range and full recording modes.
- Export is not a Measurement/Analysis/Configuration page.

Validation
- Static brace balance and source assertions passed.
- Full target macOS Cargo build is still required.


---

## Imported history: README_V0.39.5_VERIFIED_FIXES.txt

LogLens v0.39.5 — verified source correction

Direct inspection found two inaccuracies in v0.39.4:
1. Cargo.toml still reported 0.39.3.
2. Export used egui::Window, so it was not the same native detached-window mechanism as Graphics.

v0.39.5 fixes both:
- Cargo/UI version 0.39.5.
- Export uses ViewportId + ViewportBuilder + show_viewport_immediate, matching Graphics native viewport architecture.
- Home state, initialization, Start routing, Recent Sessions refresh, new-capture acceptance gate and recent-session acceptance gate were re-checked in actual source.
- Embedded LogLens Export egui::Window path removed.
- Static braces/source assertions passed.

Cargo is unavailable in this execution environment, so macOS compilation is not claimed.


---

## Imported history: README_V0.39.6_HOME_LAYOUT_SESSION_FIX.txt

v0.39.6: Home rebuilt as aligned centered vertical layout. Recent Sessions no longer silently disappear when persistent copy is missing; metadata remains visible and open falls back to original source path. Up to 12 sessions shown. Native Export retained. Static source validation passed; target Cargo/runtime build not claimed.


---

## Imported history: README_V0.40.0_WAVE_ANALYSIS.txt

LogLens v0.40.0 — CANoe/XViewer Wave Analysis foundation

Source-verified changes:
- Frame > Byte > Bit hierarchy in Graphics.
- Independent bit 0..7 DIGITAL waveform lanes.
- Existing raw-byte selection retained.
- Digital lanes use 0/1 scale.
- Raw/decoded automotive values use sample-and-hold step rendering.
- Detailed A/B readout includes A, B, delta, min, max.
- Shared X/time view and synchronized A/B cursors retained.
- Wheel = X zoom at pointer; Shift+wheel = X pan; Alt+wheel = Y zoom.
- Existing overview navigator, lane reorder, decoded signals, events, replay and native detached Export retained.
- v0.39.6 Home/Recent Session fixes retained.

Static source validation passed. Cargo is unavailable in this environment, so target compilation/runtime is not claimed.


---

## Imported history: README_V0.40.1_GRAPHICS_VIEW.txt

v0.40.1 Graphics redesign: narrower navigator, waveform-first lane composition, fixed 125–185 px engineering gutter, full-height plot canvas, aligned Y labels, integrated time ticks, stronger step traces, compact digital lanes. Existing bits/A-B/overview/events/replay/Home/Export retained. Static source validation passed; Cargo/runtime not claimed.


---

## Imported history: README_V0.40.2_WHEEL_ZOOM_FIX.txt

LogLens v0.40.2 — Graphics wheel zoom regression fix

Source-verified:
- Wheel UP zooms IN.
- Wheel DOWN zooms OUT.
- Zoom remains anchored at mouse position.
- Shift+wheel pans X only.
- Alt+wheel changes selected lane Y scale only.
- v0.40.1 Graphics redesign retained.
- Byte/Bit lanes, A/B cursors, overview, events and replay retained.

Static source checks passed. Cargo/runtime build is not claimed in this environment.


---

## Imported history: README_V0.40.3_XVIEWER_CURSORS.txt

LogLens v0.40.3 — XViewer-style Cursor Upgrade
Source-verified:
• Global synchronized A/B cursors remain across all waveform lanes.
• A + click places Cursor A; B + click places Cursor B.
• Direct drag of existing A/B cursor remains.
• On release, cursor snaps to nearest actual sample of the waveform.
• Active A/B cursor selectable from toolbar.
• Left/Right arrow steps active cursor through capture timeline.
• A↔B interval is shaded across every lane.
• Zoom A↔B zooms directly to the measured interval.
• A/B triangular top handles and stronger active-cursor line.
• Delta time shown in cursor toolbar.
• v0.40.2 scroll-up X zoom fix and v0.40.1 graph layout retained.
Static source checks passed. Cargo/runtime compilation is not claimed in this environment.


---

## Imported history: README_V0.41.0_TECMP_ANALOG.txt

LogLens v0.41.0 — TECMP Analog + CAN/Decoded integration

TECMP User Manual v1.7 §2.4.3.6 implementation:
- Data Type 0x20/0x21 remains one Trace packet.
- Analog payload decoded as INT16 array, 2 bytes/sample, maximum 700 samples per manual.
- Packet timestamp is preserved and represents first acquired analog value.
- Per-sample time = packet timestamp + sample_index * Sample Time.
- Sample Time decoded from Data Flags bits 14..11:
  2500,1000,500,250,100,50,25,10,5,2.5,1 ms.
- Factor decoded from Data Flags bits 8..7: 0.1,0.01,0.001,0.0001.
- Unit decoded from Data Flags bits 4..2: V,A,W,Ah,°C.
- Graphics shows one Analog waveform lane, not 1400 byte lanes.
- Raw packet bytes remain preserved for Trace/inspection.
- Analog and CAN/decoded signal lanes share the same Graphics time axis and A/B cursors.
- Decoded-signal frame matching made more tolerant of numeric CAN-ID representations.

Source/static verified in this environment; runtime compilation is not claimed here.


---

## Imported history: README_V0.41.1_COMPILE_FIX.txt

v0.41.1 compile fix
- Fixed E0063 in src/engine.rs simple_frame(): added analog: None to NormalizedFrame initializer.
- Audited all NormalizedFrame initializers in src/*.rs; none are missing the new analog field.
- No functional change to TECMP Analog decoding.


---

## Imported history: README_V0.41.2_ANALOG_WAVEFORM_VISIBILITY_FIX.txt

LogLens v0.41.2 — Analog Waveform Visibility Fix

Fixes:
1. Analog sources are no longer hidden by the Graphics 'Only changing' filter.
2. Analog waveform selection is directly visible under ANALOG > Channel; no extra collapsed frame level is required.
3. Selecting Analog waveform creates the Graphics lane immediately.
4. If TECMP Data Flags contain an invalid/reserved Sample Time, Graphics now shows a clear diagnostic instead of silently showing no waveform.
5. Existing CAN/CAN-FD/LIN byte/bit trees and XViewer A/B cursor behavior are retained.

TECMP Analog remains INT16, 2 bytes/sample. Packet timestamp is the first sample timestamp.
Source/static verified only in this environment.


---

## Imported history: README_V0.41.3_ANALOG_LOG_VALIDATED.txt

LogLens v0.41.3 — Analog Waveform, validated against Event_Marker_Implementation(1).pcapng

Uploaded-log validation:
- 108 TECMP Analog records (Data Type 0x20)
- Channels: 32785 and 32786
- 54 Analog packets/channel
- 1400 data bytes/packet
- 700 INT16 samples/packet
- Data Flags: 0x5900
- Sample time: 1 ms
- Factor: 0.001
- Unit: V
- Channel 32785 observed range: 0.046 .. 11.971 V
- Channel 32786 observed range: 0.000 .. 11.924 V

UI fixes:
- New captures auto-select discovered Analog channels.
- Graphics has 'Show all Analog' and detected-channel count.
- Analog is forced to engineering-scale Stacked workflow; Overlay warns instead of silently hiding it.
- Analog series lookup is explicit by BusType::Analog + channel + frame key.
- Analog remains exempt from Only-changing suppression.

The uploaded PCAPNG was parsed directly to validate the actual TECMP Analog content.
Rust/Windows runtime compilation is not claimed in this environment.


---

## Imported history: README_V0.41.4_ANALOG_SELECTION_RENDER_FIX.txt

LogLens v0.41.4 — Analog Selection/Render Fix

Root cause found from the user's screenshot:
The direct 'Analog waveform' checkbox changed selected_frame_bytes, but did not always
synchronize graphics_sources. The renderer requires BOTH states, so the UI could show
a checked Analog waveform while the graph area stayed empty.

Fix:
- Analog waveform checkbox now inserts/removes the source in graphics_sources.
- Existing/preserved v0.41.2/v0.41.3 checked Analog selections self-heal before lane rendering.
- Selecting Analog forces Stacked engineering-value view.
- If a selected Analog source produces no samples, the graph now shows a diagnostic instead of blank space.
- Existing TECMP Analog decoding and uploaded-log validation remain unchanged.


---

## Imported history: README_V0.41.5_ANALOG_PAYLOAD_FIX.txt

LogLens v0.41.5 — Analog Payload Fix

Root cause confirmed in source:
engine::load() clears NormalizedFrame.raw_payload for every frame after protocol normalization
to reduce RAM usage. The Analog waveform renderer was reading raw_payload, so it always
received zero bytes after loading even though the Graphics tree correctly showed a
1400-byte Analog packet.

Fix:
- Analog waveform now decodes f.payload, which is the retained TECMP Analog sample array.
- For the validated log, each Analog packet retains 1400 bytes = 700 INT16 samples.
- Existing sample-time/factor/unit decoding and selection fixes are retained.

This directly fixes the diagnostic:
'Analog: selected, but no decoded samples were produced'
when the packet is otherwise valid.


---

## Imported history: README_V0.42.0_CONTROL_MESSAGE_POINT_MARKERS.txt

LogLens v0.42.0 — Control Message Point Markers

Requested change:
- Drop paired start/end Event visualization from Graphics.
- Every TECMP Control Message occurrence is a point marker.
- No min-gap suppression for Graphics markers.
- Marker label uses Control Message ID from the control payload, not the common TECMP CM_ID.

Validated-log acceptance target:
Event_Marker_Implementation(1).pcapng contains 8 Control Messages:
4 x Control Message ID 0x0007
4 x Control Message ID 0x0008
All eight must appear independently on the common CAN/Analog time axis.

Existing Analog payload fix from v0.41.5 is retained.


---

## Imported history: README_V0.42.1_CM_MARKERS_ONLY.txt

LogLens v0.42.1 — CM Markers Only

Graphics no longer models Control Messages as Events or start/end windows.

Behavior:
- Every TECMP Control Message = one point marker at its own timestamp.
- Marker label = payload Control Message ID, e.g. CM 0x0007 / CM 0x0008.
- Repeated IDs remain separate occurrences.
- Graphics renderer consumes ControlTrigger directly; no EventWindow adapter.
- CM Markers table shows Time, Message ID, TECMP CM ID, Occurrence.
- Clicking a marker-table row moves Cursor A/playhead to that exact timestamp.
- No Start, End or Duration columns in the marker table.
- Analog v0.41.5 payload fix retained.

Validated-log target remains 8 point markers: four 0x0007 and four 0x0008.


---

## Imported history: README_V0.42.2_GRAPHICS_STABILIZATION.txt

LogLens v0.42.2 — Graphics Interaction Stabilization

Implemented:
- Mouse wheel: horizontal time scroll only.
- Shift + wheel: faster horizontal time scroll.
- Ctrl/Cmd + wheel: pointer-centred X/time zoom.
- Middle drag: horizontal pan.
- Removed wheel-driven Y zoom and right-drag Y pan conflicts.
- Double-click: fit complete log time range.
- Fit changes viewport only; does not clear cursors, selected signals, markers or Y state.
- A/B cursors keep the exact dropped timestamp; no snap/reset on mouse release.
- A/B signal values are visible in Standard waveform lanes using sample-and-hold lookup.
- Cursor header shows relative cursor seconds and actual capture timestamps together.
- CM markers remain fixed log references and independent of A/B cursors.
- Existing overview navigator retained.


---

## Imported history: README_V0.43.0_XVIEWER_GRAPHICS.txt

LogLens v0.43.0 — XViewer Graphics interaction

Source-verified changes:
- Larger default waveform lanes (150 px) and narrower signal browser (190 px).
- A/B remain measurement cursors only.
- Removed Zoom A↔B coupling.
- Added independent Select Range mode: click Select Range, then drag directly on a waveform.
- Selected region is visibly shaded and reports relative + actual capture times and duration.
- Selection finishes on mouse release and is independent of A/B.
- Clear Range removes only the transient selection.
- Existing Changed-only control is surfaced when a range exists.
- Existing smooth time pan / pointer-centred zoom / fixed CM marker behavior retained.

Important verification note:
This environment has no Cargo compiler. Source/static checks were performed; compile/runtime verification must be done on a Rust-enabled machine.
The existing global change cache/filter remains in place. Range-specific changed-data indexing is not claimed as complete in this build.


---

## Imported history: SOURCE_VERIFICATION_v0.46.2.txt

LogLens v0.46.2 source/static verification (Windows compile-fix package)
========================================================================
PASS 34/34 targeted assertions.

PASS package version 0.46.2
PASS selected_frame_bytes uses BTreeSet
PASS no stale frame_full_byte_mask
PASS no stale selected_byte_mask
PASS no stale set_selected_byte_mask
PASS no stale cached_decoded_signal_project
PASS no byte>=64 guard
PASS no dlc.min(64)
PASS no 0..64 selector loop
PASS complete payload length uses max
PASS graphics All uses 0..dlc
PASS byte analysis All uses 0..n
PASS long payload selector grouping exists
PASS changed bytes cache dynamic
PASS decoded waveform cache present
PASS decoded cache helper present
PASS signal cloned before mutable cache call
PASS Graphics calls decoded cache
PASS classification caches paired
PASS signal edits clear decoded cache
PASS signal edits preserve raw cache
PASS signal edits preserve class cache
PASS config virtualization
PASS manual row creation
PASS compat field byte_position
PASS compat field bit_position
PASS compat field start_byte
PASS compat field start_bit
PASS channel dropdown
PASS protocol dropdown
PASS endian dropdown
PASS signed dropdown
PASS removed unused gap warning
PASS removed unused viewport class warning

Windows diagnostics from user screenshots addressed:
- E0599 stale frame_full_byte_mask / selected_byte_mask / set_selected_byte_mask call sites: removed.
- E0502 self.signals immutable borrow overlapping mutable decoded-waveform cache call: fixed by cloning SignalDefinition before cache access.

Compile verification: NOT RUN in packaging environment (cargo/rustc unavailable).
Runtime verification: NOT RUN.
Log-data validation: NOT RUN in this compile-fix pass.


---

## Imported history: V0.47.2_SELECT_RANGE_FIXES.md

# LogLens v0.47.2 — Select Range correctness/performance

- A/B cursors remain measurement-only and are absent from analysis-range cache identity.
- The live Select Range rectangle is UI-only while dragging.
- Analysis range commits once after create/resize/move completes.
- Classification/discovery caches invalidate only on committed range changes.
- Whole-capture change classification now binary-slices timestamp-sorted frames before scanning.
- Per-source byte classification continues to use the exact bus + channel + frame identity.
- Signal Discovery binary-slices the selected time range before grouping.
- Signal Discovery isolates identical frame keys on different buses/channels.
- Ethernet Signal Discovery no longer has a 64-byte payload cap.


---

## Imported history: V0100_START_PAGE.md

# v0.10.0 Start Page Fix

- Dedicated startup screen when no dataset is loaded.
- Analysis sidebar/header are hidden until a log/session is opened.
- Primary Open Vehicle Log action is centered and prominent.
- Up to six recent sessions can be opened directly from the start page.
- Optional Channel Map, Event Log, and Signal Definitions can be loaded before the capture.
- Opening a capture always enters the CANoe Workspace with Project Tree visible and Graphics inspector active.
- New Session returns cleanly to the start page.


---

## Imported history: V0101_GRAPHICS_PERSISTENCE_FIX.md

# v0.10.1 — Graphics configuration persistence

Fixes a lifecycle bug where waveform configuration could be lost when the same capture was loaded/restarted again.

## Preserved for the same capture
- selected Graphics frame/channel sources
- selected raw byte lanes
- selected decoded signal lanes
- stacked/overlay mode
- lane height
- event marker visibility
- Graphics visible time range / zoom
- Follow replay setting
- Hide Static Frames setting

## Replay behavior
Starting, stopping, pausing, resuming, or reaching end-of-log changes only replay state. It does not clear Graphics configuration.

Loading a different capture still initializes a clean Graphics workspace, which avoids carrying invalid frame/channel selections into unrelated data.


---

## Imported history: V0102_TOOLBAR_ORGANIZATION.md

# v0.10.2 Toolbar Organization

- Main workspace commands are grouped into File, Replay, and View sections.
- Replay timeline/status has its own row to prevent wrapping and layout glitches.
- Trace uses compact Mode, Display, and Filter groups.
- Graphics uses compact Layout, Navigation, Options, Lane, and Cursor groups.
- Existing analysis behavior and persisted Graphics configuration are preserved.


---

## Imported history: V0103_VISUAL_POLISH.md

# v0.10.3 Visual polish

This update removes the wireframe/mock-up appearance introduced by the grouped toolbar layout.

- Primary command bar is one flat desktop-tool surface, not three outlined group boxes.
- Replay timeline is a quiet secondary status strip.
- Project Tree, Trace, and docked Inspector use flat filled panes rather than framed group boxes.
- Trace and Graphics toolbars use spacing and compact labels instead of repeated separators.
- View controls use consistent selectable active states.
- Replay, global Project Tree scope, static-frame filtering, and Graphics persistence are unchanged.


---

## Imported history: V0110_WORKSPACE_SIMPLIFICATION.md

# v0.11.0 Workspace simplification

## UI changes
- Removed the permanent Sessions/New Session sidebar from the loaded-log analysis workspace.
- Project Tree is now the leftmost permanent pane during Measurement.
- Replaced the legacy top navigation (`Frames & Filters`, `Frame Explorer`, `Signal Processing`, `Signal Waveforms`) with:
  - **Measurement** — Project Tree, Trace, replay, Details, Graphics and Events.
  - **Analysis** — Frame / Byte Analysis and Signal Discovery.
  - **Configuration** — Channel Map, VehicleDB, Event Log and Signal Definitions.
- Signal waveforms stay in the main Graphics inspector rather than a separate page.
- Project Tree analysis shortcuts now route into the unified Analysis workspace.
- Context-menu `Analyze Bytes / Bits` routes to Analysis → Frame / Byte Analysis.

## Session behavior
Session browsing and recycle-bin handling remain on the Start Page. Once a capture is open, the recovered horizontal space is given to Project Tree, Trace and Graphics.


---

## Imported history: V0111_SIMPLE_CONFIGURATION.md

# v0.11.1 Simple Configuration

The user-facing configuration is simplified to five readable files:

- `project.csv` — simple `key,value` project settings.
- `channels.csv` — channel/network names and protocol.
- `messages.csv` — message/frame definitions across CAN, CAN FD, LIN and Ethernet/SOME-IP.
- `signals.csv` — decoded signal definitions using `start_bit`, `length`, `factor`, and `offset`.
- `events.csv` — optional event windows.

`IdentifiedSignals` is no longer presented as a required configuration file. Signal Discovery keeps candidates in the Analysis workflow until the user promotes them to the normal signal database.

Older ChannelMap, VehicleDB, EventLog and IdentifiedSignals inputs remain accepted for backward compatibility.

The bundled `LogLens_Configuration_Template_v0.11.1.xlsx` contains all five sections as worksheets. When the same workbook is selected from a Configuration import button, LogLens automatically reads the matching worksheet (`Project`, `Channels`, `Messages`, `Signals`, or `Events`).

Configuration rows can be edited directly in the tool, rows can be added/deleted, and the edited table can be saved back out as CSV. Signals are re-parsed immediately after an edit and decoded waveform caches are invalidated.


---

## Imported history: V0112_UI_STRUCTURE_SCROLLING.md

# v0.11.2 — Unified workspace structure and scrolling

- Measurement, Analysis and Configuration now share the same command bar and offline replay strip.
- Analysis follows the Graphics layout: scrollable left navigator + scrollable right work area.
- Configuration follows the same layout and shows one maintainable file at a time: Project, Channels, Messages, Signals or Events.
- Added navigation shortcuts from Analysis/Configuration back to Project Tree, Details, Graphics and Events.
- Increased global desktop control hit area and button padding.
- Increased primary workspace-tab size and key Graphics navigation buttons.
- Added two-axis scrolling to Analysis and Configuration work areas and retained table scrolling for large configuration files.
- Fixed Events alias cache invalidation when importing `events.csv`.


---

## Imported history: V0120_UI_CLEANUP.md

# LogLens v0.12.0 UI Cleanup

This revision replaces the shared-toolbar experiment from v0.11.2 with three purpose-built workspaces.

## Measurement
- Owns capture open/load controls, Run/Pause/Stop, replay speed and replay timeline.
- Retains CANoe-style Project Tree, Trace, Graphics, Details and Events workflow.
- No behavioral change to replay, project scope, static-frame filtering or Graphics persistence.

## Analysis
- Removes replay controls and duplicated Measurement pane shortcuts.
- Uses a compact analysis toolbar for Frame/Byte Analysis, Signal Discovery, Detect Signals and Clear Results.
- Uses a simple left Analysis navigator and a large scrollable work area.
- Keeps analysis scope counters in the navigator.

## Configuration
- Removes replay controls and duplicated Measurement pane shortcuts.
- Uses a compact editor toolbar: Import, Add Row, Save CSV and Validate.
- Uses a simple left navigator: Project, Channels, Messages, Signals and Events.
- Keeps the selected configuration table in a large scrollable editor area.
- Signal edits continue to refresh signal definitions and invalidate waveform cache.

## Visual cleanup
- Removed summary cards from the loaded-log workspace.
- Reduced oversized control padding introduced in v0.11.2 while retaining comfortable hit targets.
- Workspace tabs are more compact.
- Measurement, Analysis and Configuration share visual styling but not redundant controls.


---

## Imported history: V0121_ALIGNMENT_BUTTONS.md

# LogLens v0.12.1 — Alignment and Button Sizing

This revision corrects the v0.12.0 sizing approach.

- Buttons are explicitly sized (generally 34 px high) rather than made large through global padding.
- Workspace tabs are 156 × 36 px.
- Toolbars stay on one row and scroll horizontally instead of wrapping into misaligned rows.
- Analysis and Configuration navigation entries use a common 185 × 34 px size.
- Configuration table headers and cells use fixed widths for column alignment.
- Duplicate table-level Add Row / Save CSV controls were removed; those actions live in the Configuration toolbar.
- Panel margins were reduced.


---

## Imported history: V0122_ANALYSIS_CONFIG_REDESIGN.md

# v0.12.2 Analysis + Configuration redesign

- Removed large card-like navigation from Analysis and Configuration.
- Added compact 176 px left navigator with 30 px rows.
- Added a single aligned command bar per workspace.
- Analysis actions are Detect Signals and Clear only.
- Configuration actions are Import, Add Row, Save CSV, Validate.
- Counts are shown next to Configuration sections without large buttons.
- Main content/editor consumes the remaining workspace width.
- Measurement controls remain exclusive to Measurement.
- Reduced inter-panel spacing to 3 px for alignment.


---

## Imported history: V0123_ICONS_TOOLLOG_SESSIONS.md

# v0.12.3 - Icons, Tool Log and Recent Sessions

## Icon rendering
- Removed decorative symbol-font glyphs from command buttons and pane controls.
- Uses plain text labels and ASCII controls so buttons render consistently on macOS and Windows without tofu/box placeholders.
- Replay, Project, Details, Graphics, Events, Trace, zoom/pan, detach, delete and session actions no longer depend on uncommon glyph coverage.

## Tool Log
- Added a resizable Tool Log panel at the bottom of the application.
- Toggle it from the header or the Start Page.
- Logs capture open/decode results, configuration imports/saves/validation, Recent Session activity and errors.
- Keeps the latest 500 entries and provides Clear/Close actions.

## Recent Sessions
- Session storage is now persistent and platform-specific instead of depending on the process working directory:
  - macOS: `~/Library/Application Support/LogLens`
  - Windows: `%LOCALAPPDATA%\\LogLens`
  - Linux: `$XDG_DATA_HOME/loglens` or `~/.local/share/loglens`
- Existing relative session paths are rebuilt from the persistent session store.
- Missing/broken session copies are not shown as openable recent entries.
- Reopening the same original capture refreshes its existing Recent Session instead of creating another duplicate.
- Recent Sessions are ordered by last-opened time.
- Start Page now shows last-opened time plus explicit Open/Remove controls and a Refresh action.


---

## Imported history: V0130_HYBRID_NATIVE_ENGINE.md

# LogLens v0.13.0 - Hybrid Rust + C++ Native Engine

## Architecture

Rust/egui remains the application, UI, project/configuration, sessions and orchestration layer.

C++17 now provides the first high-throughput native processing layer through a stable C ABI:

1. PCAP scanner
2. PCAPNG scanner (SHB / IDB / EPB / SPB, timestamp resolution)
3. Batched capture packet transfer to Rust
4. Native frame payload-change/static analysis
5. Native min/max waveform downsampling

Existing Rust TECMP and protocol normalization remains in place after the native packet scanner. This minimizes regression risk while moving the highest-volume byte scanning out of the UI/application layer.

## Runtime flow

Capture -> C++ PCAP/PCAPNG scanner -> Rust TECMP/protocol normalization -> NormalizedFrame
        -> C++ batch analysis/downsampling -> Rust UI

If the native capture reader rejects a capture, LogLens automatically falls back to the existing Rust reader.

## FFI rule

The bridge is batch-oriented. Rust never invokes C++ once per packet/frame during capture scanning. A capture is scanned in one native call and transferred in compact buffers. Frame analysis is also one batch call.

## Native files

- `native/include/loglens_engine.h`
- `native/src/loglens_engine.cpp`
- `native/CMakeLists.txt`
- `src/native_engine.rs`
- `build.rs`

## Build

macOS Apple Silicon:

```bash
chmod +x build_macos_app.sh
./build_macos_app.sh
open dist/LogLens.app
```

Windows MinGW:

```bat
build_windows_mingw.bat
```

Standalone native library validation:

```bash
cmake -S native -B native/build -DCMAKE_BUILD_TYPE=Release
cmake --build native/build --config Release
```

## Tool Log

After a capture loads, Tool Log reports a line similar to:

`LogLens C++ Native Engine 0.13.0 - 1500000 frames, 420 unique frame keys, ...`

This confirms that the native batch analyzer is active.

## Next native migration candidates

The API is intentionally ready to expand without changing the UI:
- TECMP record extraction
- CAN/CAN-FD/LIN normalization
- native frame/channel/time indexes
- filtered range queries
- signal extraction

These should be migrated only with output-equivalence tests against the existing Rust implementation.


---

## Imported history: V0131_WINDOWS_PERFORMANCE.md

# LogLens v0.13.1 - Windows Performance Hot-Path Fix

This build targets the lag observed with approximately 300 MB captures on Windows i7 systems.

## Changes

- Sequential Trace no longer reconstructs every visible row every 33 ms.
- Trace is incrementally extended only for frames that became visible since the previous repaint.
- egui `show_rows` continues to render only rows inside the visible viewport.
- Per-frame Trace delta time is calculated once during load.
- Removed unused full-capture `timeline`, `by_bus`, and per-frame `by_channel` index vectors.
- `raw_payload` protocol scratch buffers are released after normalization to reduce resident memory.
- Waveform caches remain bounded/downsampled.
- Existing C++ PCAP/PCAPNG reader and native waveform downsampler remain enabled.
- Rust fallback remains available for unsupported capture variants.

## Windows test

Always use the Release executable:

```bat
build_windows_mingw.bat
dist\LogLens.exe
```

Do not benchmark `cargo run` / Debug mode.

Recommended first test: the same 300 MB capture that was lagging in v0.12.3/v0.13.0.


---

## Imported history: V0140_PERFORMANCE.md

# LogLens v0.14.0 — Large-log performance pass

This version builds on v0.13.1 and targets the remaining hot paths observed on ~300 MB Windows captures.

Implemented:
- Indexed raw-byte waveform lookup via the persistent FrameIndex (no full capture scan per selected byte).
- Large-log open path skips the expensive temporary FFI payload-copy statistics pass above 250k frames.
- Scope rebuilding no longer clones the complete filtered-index vector before applying project/channel/time scope.
- v0.13.1 incremental/virtualized Trace path retained.
- Duplicate raw protocol payload scratch buffers are still released after normalization.
- C++ PCAP/PCAPNG scanner and C++ waveform min/max reducer retained.
- Windows script now explicitly builds the GNU Release target.

Tool Log will show `large-log fast path ACTIVE` for captures above 250,000 normalized frames.

Next architectural step, if profiling still shows load/decode as dominant, is a persistent native/memory-mapped FrameStore with TECMP normalization directly in the native engine. That is intentionally not claimed as complete in this version.


---

## Imported history: V0150_RUNTIME_PERFORMANCE.md

# LogLens v0.15.0 — Runtime Performance Fix

Primary symptom addressed: capture loads quickly, then the whole application becomes sluggish.

## Critical fix
Sequential Trace no longer materializes one `TraceRow` object for every scoped frame after loading.
It now uses egui row virtualization directly over the persistent scoped index. Only rows in the viewport are formatted/rendered.

For a multi-million-frame capture this removes a large post-load allocation and repeated UI-side working set.

## Additional runtime changes
- Full-capture static/change classification is deferred until `Hide static` is actually enabled.
- Graphics change classification is deferred unless `Only changing` is enabled.
- v0.14 indexed waveform path retained.
- v0.13 incremental overwrite-mode Trace retained.
- Release GNU Windows build retained.

## Expected effect
The strongest improvement should be immediately after a large capture finishes loading:
window interaction, tab switching, Trace scrolling, replay and general UI responsiveness should no longer pay the cost of a multi-million-row sequential Trace model.


---

## Imported history: V0160_NATIVE_FRAMESTORE.md

# LogLens v0.16.0 — Native FrameStore foundation

This starts the CANoe-like architecture: compact native frame metadata, contiguous payload storage, and bounded frame-range queries.

The current Rust Dataset is intentionally retained as the compatibility path in this stage. The new native store is not duplicated automatically after load, because that would increase memory. The next migration replaces Dataset consumers one-by-one: Trace, Graphics, filtering/statistics/events, then direct TECMP normalization into the native store.

Target: UI -> bounded query -> native FrameStore. The GUI must never receive the whole capture just to render a viewport.


---

## Imported history: V0170_INDEXED_UI.md

# LogLens v0.17.0 — Indexed UI hot-path fix

This release fixes a major post-load UI bottleneck found in the v0.16 code.

## Root cause fixed
The Measurement Project Tree rebuilt its bus/channel topology by iterating **every decoded frame on every egui repaint**. With a multi-million-frame capture, any interaction that caused repainting could therefore feel sluggish even though idle LogLens CPU was 0%.

## Changes
- `FrameIndex` now precomputes `networks` and `project_channel_keys` once during load.
- Project Tree renders directly from indexed topology; no full-capture scan per frame.
- Project Tree `All` uses precomputed channel keys.
- Initial project scope uses the same precomputed metadata.
- Custom Signal Preview now uses `FrameIndex.by_key` instead of scanning the entire capture.
- Native FrameStore foundation from v0.16 remains.

## Expected behavior
After a large log loads, moving/resizing the app, interacting with the Project Tree, switching panes and repainting Measurement should no longer trigger a full capture traversal just to build network topology.


---

## Imported history: V0180_TRACE_PERFORMANCE.md

# LogLens v0.18.0 — Trace Runtime Performance

Targeted release after v0.17 made the general application smooth but Trace remained sluggish.

Changes:
- Sequential Trace remains virtualized: only viewport rows are visited.
- Added a bounded 512-row formatted Trace cache.
- Timestamp, dt, protocol, channel and payload hex are no longer regenerated for every visible row on every repaint.
- Replaced eight egui widgets + horizontal layout per Trace row with one monospace selectable row.
- Header similarly reduced to one widget.
- Cache invalidates on Trace scope changes and never grows with capture size.
- Existing overwrite mode and native FrameStore foundation are retained.

Expected result: Trace layout/render cost now scales primarily with visible viewport rows rather than capture size or column widget count.


---

## Imported history: V0190_BACKGROUND_LOADING.md

# LogLens v0.19.0 — Responsive Background Loading

Large capture loading is moved off the egui/UI thread.

- Dedicated `loglens-loader` worker thread.
- UI continues processing Windows/macOS events while decoding.
- Modal progress window with staged status.
- Loader errors are returned to the UI instead of blocking the event loop.
- Worker decode is protected with `catch_unwind`, so a Rust panic in the loading pipeline is reported where unwind is possible.
- Dataset handoff happens only after the worker completes.
- v0.18 Trace performance changes are retained.

Note: cancellation is intentionally not exposed yet because the current parser has no cooperative cancellation checkpoints. A fake Cancel button would not safely stop native/decoder work. The next parser migration should add a cancellation token at packet/decode/index checkpoints.


---

## Imported history: V0191_BUILD_FIX.md

# v0.19.1 build fix
Fixes Rust E0308 in background loader startup. `spawn()` returns `Result<JoinHandle<()>, io::Error>`; the previous `unwrap_or_else` closure returned `()` and therefore mismatched the required `JoinHandle<()>`. The result is now retained and handled explicitly with `if let Err(e)`.


---

## Imported history: V0200_INDEXED_TRACE_PROFILING.md

# v0.20.0 Indexed Trace + Profiler
- Compact monotonic timeline used for replay-position binary search.
- Trace query and Trace render timings measured independently.
- Small Performance HUD shows query ms, draw ms, and frame count.
- Retains virtualized Trace, 512-row formatting cache, background loading, and native C++ FrameStore foundation.


---

## Imported history: V0210_PROJECT_FILTER_PERFORMANCE.md

# LogLens v0.21.0 — Project Scope + Trace Search Performance

## Project Tree
FrameIndex now stores `by_project_channel` once during load. Checkbox changes merge only the selected bus/channel index vectors rather than scanning the entire decoded capture.

## Trace search
Search typing is debounced by 250 ms. Text entry remains responsive and filtering is applied after typing pauses. While pending, the previously applied Trace result remains visible.

## Scope/filter intersection
Project candidates and text-filter results are sorted frame-index vectors. Their intersection is linear over those result sets and does not call `Filter::matches` again.

v0.20 Performance HUD remains enabled so query-time improvement can be measured.


---

## Imported history: V0211_BUILD_FIX.md

# v0.21.1 Build Fix
Fixes E0599 from one stale `self.scoped_indices()` call left after the v0.21 indexed-scope migration. The path now refreshes filter/scope caches and uses `cached_scoped_indices`. SelectableLabel output is a deprecation warning, not a compilation error.


---

## Imported history: V0220_GRAPHICS_WINDOWS.md

# LogLens v0.22.0 — Graphics Navigation + Work Windows

## Graphics
- Existing cursor-centered X zoom and X pan retained.
- `Fit X` resets full time range.
- `Fit Y` resets all lane Y transforms.
- Ctrl + mouse wheel over a stacked lane: independent Y zoom.
- Right-button drag over a stacked lane: independent Y pan.
- Double-click a lane: reset that lane Y view.
- Lane Y state persists while the workspace remains open.
- Shared X range remains synchronized across stacked lanes.
- Existing lane-height adjustment, cursors A/B, replay marker and event markers retained.

## Work windows
The Detach command is renamed `Open Window` and preserves independent pane size/position plus Dock Bottom/Dock Right behavior.

Important: this release does not falsely claim a second OS top-level window. The current eframe integration uses independent egui work windows in the application viewport. A true native multi-viewport implementation requires migrating the app startup to eframe viewport commands/API for the exact eframe version used by the project; that should be done as a dedicated build-validated step.


---

## Imported history: V0221_CANOE_LANE_REORDER_UNKNOWN_FIX.md

# LogLens v0.22.1

## Graphics lane movement
- True user-controlled lane ordering.
- Drag the ⠿ handle and drop onto another lane.
- Lanes preserve their custom order.
- Up/Down buttons remain as precise alternatives.

## Project Tree UNKNOWN correction
FrameIndex now uses a two-pass presentation index.
If a channel has decoded known protocol traffic (CAN, CAN-FD, LIN, ETH or Analog),
undecoded TECMP/UNKNOWN records on that same channel are grouped under the known
project network instead of appearing as a duplicate UNKNOWN channel.
Truly unknown-only channels remain under UNKNOWN.


---

## Imported history: V0230_GRAPH_AB_CURSORS.md

# v0.23.0
Interactive graph panning/zoom plus synchronized draggable A/B measurement cursors. Each lane displays A time/value and B time/value; decoded signals include unit. v0.22.1 lane reorder and Project UNKNOWN fix retained.


---

## Imported history: V0240_MULTI_FRAME_STATIC_BYTE_ANALYSIS.md

# v0.24.0
Multi-frame indexed/cached byte activity. ACTIVE/STATIC classification, Hide Static default ON, optional A-B cursor scope. NO DATA omitted. Only selected FrameIndex.by_key entries are scanned; results recompute only when selection/channel/A-B scope changes.


---

## Imported history: V0250_ACTIVITY_COUNTER_CRC_CLASSIFICATION.md

# v0.25.0 Activity / Counter / CRC Candidate Classification

Multi-frame byte analysis now classifies:
- ACTIVE
- STATIC
- COUNTER8: high-confidence wrapping +1 byte counter
- COUNTER4: high-confidence lower-nibble wrapping +1 counter candidate
- CRC_CANDIDATE: high-activity/high-diversity checksum/CRC candidate
- NO DATA remains omitted rather than being mislabeled STATIC

Performance:
- single pass over only selected FrameIndex.by_key indices
- no payload cloning
- fixed 256-value presence table per analyzed byte
- cached results; recompute only when frame/channel/A-B scope changes
- category visibility toggles do not trigger a rescan

Safety against false classification:
CRC_CANDIDATE is deliberately a candidate only. Activity/entropy does not prove a CRC polynomial,
so it is not labeled as verified CRC. A future verifier can test CRC/checksum families on shortlisted
candidates without burdening the normal interactive path.


---

## Imported history: V0431_LOG_VALIDATION.txt

LogLens v0.43.1 log validation

Log: Event_Marker_Implementation(1).pcapng
Size: 16241532 bytes
TECMP logging records normalized by validation parser: 443808
Logging duration: 38.482113 s
Control Messages: 8; IDs: {7: 4, 8: 4}

Range-specific changed-data checks:
7.0-10.0s: 2 changed sources, 2 changed byte-slots; examples=[('ANALOG|32785|ANALOG:32785', 1), ('ANALOG|32786|ANALOG:32786', 1)]
12.0-16.0s: 58 changed sources, 639 changed byte-slots; examples=[('CAN-FD|20481|0x17A', 2305843009227181311), ('CAN-FD|20481|0x120', 243), ('CAN-FD|20481|0x103', 3), ('CAN-FD|20481|0x2D1', 16251), ('CAN-FD|20481|0x115', 8589936639), ('CAN|20481|0x25C', 3)]
20.0-24.0s: 43 changed sources, 164 changed byte-slots; examples=[('CAN-FD|20481|0x17A', 37414144822527), ('CAN-FD|20481|0x2D1', 15426), ('CAN-FD|20481|0x120', 51), ('CAN-FD|20481|0x103', 3), ('CAN-FD|20481|0x108', 2), ('CAN-FD|20481|0x115', 8589936639)]

Verification scope:
- Log-data algorithm validated independently against the supplied PCAPNG.
- Rust source/static checks passed.
- Cargo is not installed in this execution environment, so Rust compile/runtime UI verification was not performed here.


---

## Imported history: V0432_WHOLE_TOOL_AUDIT.md

# LogLens v0.43.2 whole-tool audit

Test capture: `Event_Marker_Implementation(1).pcapng` (16,241,532 bytes)

## Log validation
- PCAPNG block structure: valid; 34,075 Enhanced Packet Blocks.
- TECMP packet types: 20,084 Logging Stream; 304 Status Device; 303 Status Bus/IVN; 27 Status Configuration; 8 Control Message.
- TECMP records parsed in low-level audit: 444,450.
- Control Message IDs: 4 x 0x0007 and 4 x 0x0008.
- Analog: 108 records, 54/channel on 32785 and 32786, all payloads 1400 bytes, flags 0x5900.
- Capture outer timestamp duration: about 38.386 s.
- Eight malformed/terminating TECMP record tails were observed by the strict audit parser. Current TECMP parser stops the current packet on invalid record length rather than surfacing a warning.

## Bugs fixed in v0.43.2
1. Selected-range change detection now compares consecutive payloads, not every payload only with the first payload.
2. Range `Changed only` filtering now applies to already-created byte and bit waveform lanes as well as the source browser.
3. Independent selected range is preserved when reloading the same capture.
4. Obsolete EventWindow pairing/cached event-window state removed from app path. Control Messages remain point markers.
5. Replay bounds no longer depend on old paired Control Message events.

## Remaining findings
### High
- Decoded-signal Graphics recomputes `decoded_series_project()` from the full frame vector on repaint. This is a performance risk on large captures and should use a decoded-signal cache/index.
- Analog Graphics reconstructs the complete analog sample vector on repaint. It needs a persistent AnalogSeries/LOD cache.
- `range_changed_source_masks()` scans all normalized frames whenever a selected-range filter is active. It is correct but not yet indexed; repeated UI repaints on very large captures can be expensive.
- Signal matching permits a blank/unparseable channel to match every channel. Bad signal configuration can therefore decode the same frame ID across unrelated channels.

### Medium
- Signal Discovery table decodes up to 20k points per displayed signal during UI rendering; large signal definition sets can cause latency.
- A/B configured-signal comparison rescans frames for every signal.
- TECMP malformed record tails are silently stopped rather than reported as dataset warnings.
- Native and Rust PCAPNG readers both convert large timestamp ticks through `double/f64`; this can lose sub-microsecond precision at epoch-scale timestamps.
- Session persistence stores capture metadata/copy, but full interactive Graphics state is preserved only for same-path reload in the running process, not persisted as a complete workspace state across app restart.

### Low / cleanup
- `trigger_min_gap_s` remains as a compatibility parameter although Control Messages are no longer gap-suppressed.
- The app is still concentrated in a very large `app.rs`, increasing regression risk for UI changes.

## Verification level
- Source/static audit: completed.
- Supplied PCAPNG structural and data validation: completed.
- Rust compile/runtime GUI verification: not completed because Cargo is unavailable in this execution environment.


---

## Imported history: V0434_PRE_SHARE_CHECK.md

# LogLens v0.43.4 pre-share check

- PASS: E0609 stale cached_event_gap_s removed
- PASS: old cached_event_windows removed
- PASS: EventWindow removed
- PASS: event_windows() removed
- PASS: stale trigger_min_gap_s removed
- PASS: control_triggers implementation exists
- PASS: E0382 key ownership fixed
- PASS: subsequent key use remains
- PASS: app braces balanced
- PASS: events braces balanced
- PASS: no todo!/unimplemented!
- PASS: version 0.43.4

Additional correction: restored `control_triggers()` after finding that the prior EventWindow cleanup had also removed this required function.

Verification limitation: Cargo/rustc is not installed in the execution environment, so this is source/static verification, not a compiler run.


---

## Imported history: V0435_RANGE_SELECTION_FIX.md

# LogLens v0.43.5 — Select Range fix

Confirmed source defects fixed:
- Range pointer-to-time mapping used the whole lane rectangle while the waveform used a smaller graph rectangle.
- Changed-only filtering could activate during the drag and remove the lane owning the gesture.
- Starting a new range now clears the old transient selection.
- Selection starts only inside the waveform canvas and clamps at its edges.
- A/B remain independent measurement cursors.

Regression checks also cover the previous EventWindow cleanup, Control Message extractor, and moved-key ownership fix.

This package is source/static checked. Cargo is unavailable in the execution environment, so it is not claimed as compiler/runtime verified.


---

## Imported history: V0440_MOVABLE_ANALYSIS_WINDOW.md

# LogLens v0.44.0 — Movable Analysis Window

Implemented the requested XViewer-style persistent time window.

- New Window: drag on the waveform to create `[ start ... end ]`.
- Resize: drag either vertical edge.
- Move: drag inside the selected region; duration is preserved.
- Clamp: window cannot move outside the capture.
- A/B cursors remain independent point-measurement cursors.
- CM markers remain fixed log markers.
- Changed Data is computed only after mouse release.
- Changed Data is cached for the exact window bounds.
- Normal Graphics lanes remain stable; Changed Data has its own hierarchy.
- Changed bytes are derived from consecutive payload transitions only within the selected window.

Verification level: source/static consistency checked. Cargo is unavailable in this execution environment, so compile/runtime verification is not claimed.


---

## Imported history: V0441_VISIBLE_ANALYSIS_WINDOW.md

# v0.44.1 Visible Analysis Window

Fix based on runtime screenshot:
- prevents a new window from remaining at 0.000 ms
- seeds a new window with a visible width
- enforces a minimum resize width
- stronger cyan translucent [ ... ] overlay across each stacked waveform lane
- explicit left/right grab handles
- center move grip
- 14 px edge hit zones
- move preserves duration and remains capture-clamped
- Changed Data refreshes only after release and remains cached
- A/B and CM markers remain independent

The synchronized overlay is rendered at identical time-derived X positions in each stacked lane, producing the visual full-stack analysis window without coupling it to A/B.

Verification: source/static checks only; Cargo is unavailable in this environment.


---

## Imported history: V0450_SIMPLE_RANGE_SIGNAL_GRAPH_FIX.md

# LogLens v0.45.0 — Simple Range + Signal Graph Fix

## Simplified Graphics interaction
- Removed XVIEWER, New Window and Changed only from the permanent toolbar.
- Shift + left-drag directly on a waveform creates the analysis range.
- Drag either range edge to resize.
- Drag inside the range to move the complete window.
- Clear analysis range is available in the overflow menu.
- Changed-data analysis is automatic for the active range.
- A/B cursors and Control Message markers remain independent.

## Signal configuration -> Graphics
- Added one shared configured-signal matcher used by Graphics and Export.
- Channel IDs accept decimal or hexadecimal notation.
- Frame ID matching uses normalized frame identifier plus frame-key suffix matching.
- Empty project scope no longer suppresses all configured signals.
- Selecting a configured signal immediately creates its SIG lane.
- Signals that do not map to capture frames are labelled `(no matching frames)`.
- A selected signal with no decodable output displays an explicit diagnostic instead of silently disappearing.
- Editing the Signals configuration keeps valid selected signals and removes stale/deleted signal lanes.

## Verification
Source/static consistency checks passed, including balanced app.rs braces and previous regression checks.
Cargo is unavailable in this execution environment, so compile/runtime verification is not claimed.


---

## Imported history: V0451_PORTABLE_ICON_FIX.md

# LogLens v0.45.1 - Portable UI Controls

Fixed missing-glyph square boxes seen in the Graphics screenshot.

- Unicode toolbar glyphs replaced with portable text controls.
- Run / Pause / Stop have consistent explicit button sizes.
- Signals uses `<` and `>`.
- Overflow uses `...`.
- Lane controls use `Drag`, `Up`, `Down`.
- Home uses `Home`; remove uses `X`.
- Previous/next help uses `Left / Right`.
- CM marker labels no longer depend on a triangle glyph.
- No custom font dependency was introduced.

Source/static checks passed. Cargo is unavailable in this environment, so compile/runtime verification is not claimed.


---

## Imported history: V0460_CONFIG_EDITOR_UPGRADE.md

# LogLens v0.46.0 - Configuration Editor Upgrade

## Signals
Signal configuration now exposes `byte_position` and `bit_position` separately.
The user enters the byte index and the bit within that byte; no absolute bit-position calculation is required.

Example:
- byte_position = 3
- bit_position = 2
- length = 12

Legacy imported files using `start_byte` / `start_bit`, or absolute `start_bit` without a byte column, remain supported by the parser.

## Drop-down editors
Fields with a controlled vocabulary use drop-downs:
- Protocol: CAN, CAN FD, LIN, Automotive Ethernet, Analog, Unknown
- Signal endian: Little / Big
- Signal signed: false / true

Free-form/numeric fields remain text-entry fields.

## Direct manual configuration
Import is no longer required before editing.
On an empty Project, Channels, Messages, Signals, or CM Markers page:
- `+ Row` automatically creates the correct table schema and adds a row.
- `+ Add first row` is also shown directly in the empty editor.

The same schema-first behavior is applied across all relevant configuration pages.


---

## Imported history: V0461_CHANNEL_PERF_CLASSIFICATION_FIX.md

# LogLens v0.46.1

## Channel dropdown
- Messages and Signals `channel_id` now use a dropdown populated from the Channels configuration.
- Channels remains the authoritative place where channel IDs are created/edited.
- If no Channels rows exist yet, channel remains manually editable so configuration is not blocked.

## Intermittent slowdown
- Removed a full `d.frames.iter().any(...)` scan that was executed for every configured signal during every Graphics repaint.
- Configured-signal availability now checks the already-built Graphics source catalog.
- This makes the Signals browser cost depend on source count instead of total capture frame count.

## Constant -> Review regression
Root cause fixed:
- `graphics_class_cache` was cleared when loading/reloading/config-changing, but `graphics_source_class_ready` was not.
- A source could therefore be marked "already classified" while its actual classification cache was empty.
- Subsequent lookup fell back to `NO DATA`, displayed as `Review`.
- The same stale readiness state survived another log with the same source key, explaining the repeated behavior after loading a different log.

Fix:
- Every classification-cache invalidation now also clears `graphics_source_class_ready`.
- Classification is therefore recomputed from the newly loaded/current dataset instead of falling through to `Review`.

Source/static verification only; Cargo is unavailable in this environment.


---

## Imported history: V0462_ETH_PERFORMANCE.md

# LogLens v0.46.2 — Ethernet Graphics + Performance

## Scope

This release starts from v0.46.1 and keeps its channel/configuration/classification behavior while removing the 64-byte Graphics ceiling for Automotive Ethernet and reducing repeated work in Configuration and Graphics.

## Changes

### 1. Complete Automotive Ethernet payload selection in Graphics

- Replaced `HashMap<String,u64>` byte-selection masks with `HashMap<String,BTreeSet<usize>>`.
- Replaced analysis-window `u64` change masks with dynamic byte-index sets.
- Removed Graphics-path `byte >= 64`, `dlc.min(64)`, and `0..64` assumptions.
- `graphics_available_sources()` continues to use the maximum observed payload length for each source.
- All/None/Invert/Signals Only and per-byte Graphics selection can address the complete payload.
- Long payloads are shown in expandable byte groups for UI efficiency. Grouping does not truncate or cap payload length.
- Overlay view uses per-source dynamic selections, with the legacy global byte selector retained as a fallback.
- `byte_activity_indexed()` now classifies all bytes in the indexed payload instead of stopping at byte 63.

### 2. Configuration performance

- Configuration rows use `egui::ScrollArea::show_rows()` so only visible rows are rendered.
- Signal-definition rebuild is deferred while a text field is being edited.
- A committed field/dropdown, row deletion, import, or navigation away from Signals applies pending signal changes.
- Existing channel, protocol, endian, and signed dropdowns remain.
- Manual row creation remains available.

### 3. Decoded-signal Graphics waveform cache

- Added `cached_decoded_signal_waveforms`.
- Graphics no longer calls `decoded_series_project()` on each repaint for a configured signal.
- Cache keys include all decode-affecting signal fields plus project-channel scope.
- Cached decoded series retain the same min/max downsampling used by Graphics.

### 4. Cache invalidation domains

- Dataset changes clear raw waveform, decoded-signal waveform, byte-classification cache, and classification-ready state.
- Signal-definition changes clear only decoded-signal waveforms.
- Signal edits do not clear raw byte waveforms or byte-classification state.
- Graphics byte selection changes do not clear raw/decoded/classification caches.
- Analysis-window changed-byte results are cleared on dataset swap.

### 5. v0.46.1 classification invariant retained

`graphics_class_cache` and `graphics_source_class_ready` are invalidated together through `invalidate_dataset_graphics_caches()`.

Classification lookup is source-specific by dataset, bus, channel, and frame key. A newly loaded log cannot reuse classification-ready state from the previous dataset.

### 6. Compatibility retained

- Channel dropdown
- `byte_position` + `bit_position`
- Protocol/endian/signed dropdowns
- Manual row creation
- Legacy signal configuration aliases/fallback (`start_byte`, `start_bit`, and absolute start-bit compatibility)
- CAN, CAN-FD, LIN, and Analog Graphics behavior remains on the existing paths

## Verification status

| Level | Status | Result |
|---|---|---|
| Source inspected | PASS | v0.46.1 source was inspected before modification. |
| Source/static verification | PASS | 40/40 targeted assertions passed, including dynamic byte state, no Graphics 64-byte cap patterns, full indexed classification, cache scoping, deferred signal rebuild, row virtualization, and legacy compatibility. Lexical delimiter balance also passed for modified Rust files. |
| Compile verification | NOT RUN | `cargo`, `rustc`, and `rustfmt` are not installed/available in the execution environment. |
| Runtime verification | NOT RUN | No compiled LogLens executable is bundled and compilation could not be performed here. |
| Log-data validation | NOT RUN | No PCAP/PCAPNG/BLF/MF4/log capture is included in the supplied project archive for this task. |

## Files changed from v0.46.1

- `Cargo.toml` — version `0.46.2`
- `src/app.rs` — dynamic Ethernet byte selection, configuration virtualization/commit handling, decoded signal cache, cache-domain invalidation
- `src/analysis.rs` — full-payload `byte_activity_indexed()` classification


---

## Imported history: V0462_WINDOWS_COMPILE_FIX.md

# LogLens v0.46.2 Windows compile fix

This package supersedes the first v0.46.2 archive after Windows/MinGW compilation exposed incomplete call-site migration from the old 64-bit byte-mask API.

## Corrected compile blockers

1. Removed all remaining calls to the deleted `frame_full_byte_mask`, `selected_byte_mask`, and `set_selected_byte_mask` helpers. The byte-selection path now consistently uses dynamic `BTreeSet<usize>` state through `selected_byte_count`, `byte_selected`, and `set_selected_bytes`.
2. Fixed Rust borrow error E0502 in configured-signal Graphics. The selected `SignalDefinition` is cloned before calling the mutable decoded-waveform cache, so the immutable borrow of `self.signals` does not overlap `&mut self`.
3. Cleaned the two warnings shown beside the failing code (`gap` and viewport `class`) without changing behavior.

## Verification status

- Source/static verification: performed in the packaging environment.
- Windows compile verification: cannot be executed in the packaging environment because `cargo`/`rustc` are not installed here. The fixes are directly targeted at the E0599/E0502 diagnostics from the Windows build output.
- Runtime verification: requires a successful local build.
- Log-data validation: not performed in this compile-fix pass.


---

## Imported history: V0463_SIGNAL_DATATYPE_FILTER_RAW_VISIBILITY.md

# LogLens Rust Replica v0.46.3

## Signal Data Type
- Added `data_type` to Signal Configuration.
- Dropdown values: Unsigned Integer, Signed Integer, Float32, Float64.
- Existing signal tables without `data_type` are migrated in-memory using legacy `signed` values.
- Float32/Float64 decoding uses IEEE-754 with configured endian, factor, and offset.
- Decoded waveform cache keys include data type.
- Signal export includes DataType.

## Hide Configured Raw
- Added `Hide Configured Raw` toggle in Graphics.
- When ON, raw Graphics source rows/lanes for channels with a configured channel name are hidden.
- Configured decoded signals remain available.
- This is display-only; raw capture data and caches are not deleted.

## Filter responsiveness
- Text filtering is debounced to 300 ms.
- Keystrokes update the text field immediately; full-log filter-cache rebuild waits until typing pauses.
- Existing indexed/cached scope remains in use while text is being entered.


---

## Imported history: V0464_NAME_MAPPING_SESSION_PERSISTENCE.md

# LogLens v0.46.4

## Corrections
- Removed the incorrect `Hide Configured Raw` control and channel-wide raw suppression.
- Channel labels now use automatic configured-name substitution with raw-ID fallback.
- The common channel-name resolver checks configuration tables and treats the Channels table as authoritative.
- Internal channel identity remains the raw numeric channel ID; mapping changes display labels only.

## Session configuration persistence
- Configuration tables are snapshotted per Recent Session in `context/configuration.json`.
- Loading/importing or editing configuration updates the session snapshot.
- Reopening a Recent Session restores Project, Channels, Messages, Signals and CM Markers automatically.
- Signal definitions are rebuilt after restore, so decoded/configured signal mappings return without re-importing configuration.

## Preserved from v0.46.3
- Signal Data Type dropdown: Unsigned Integer, Signed Integer, Float32, Float64.
- Filter debounce/performance work.
- v0.46.2 Ethernet dynamic-byte and decoded-waveform cache work.

## Validation
- Static source checks completed in the packaging environment.
- Cargo was not installed in the packaging environment, so compile/runtime validation must be performed on the target Rust machine.


---

## Imported history: V0466_VISIBLE_RAW_IDS_TOGGLE.md

# v0.46.6 - Visible Raw IDs toggle

- Adds a visible `Raw IDs` toggle directly in the PROJECT header.
- Adds the same visible toggle to the GRAPHICS toolbar; both controls share one persisted state.
- ON: configured items render as `ConfiguredName (RawID)`.
- OFF: configured items render as `ConfiguredName`.
- If no configured name exists, the raw ID is always rendered regardless of toggle state.
- Raw byte waveform labels now use the same channel display resolver in Compact, Standard, and Detailed graphics density modes.
- Display-only: raw identifiers remain the internal keys for filtering, decoding, selection, waveform caches, and source identity.


---

## Imported history: V0467_WINDOW_SCOPED_CLASSIFICATION.md

# v0.46.7 — Window-scoped classification and signal discovery

- Constant / signal-activity / counter / CRC byte classification now uses the released persistent Graphics analysis window.
- If no analysis window is selected, classification falls back to the complete capture.
- Moving/resizing the window does not invalidate raw waveform or decoded waveform caches; derived classification resets only after the interaction is released.
- `Only changing` source classification follows the same analysis window.
- Signal Discovery now detects candidates only inside the selected analysis window, with full-capture fallback.
- Candidate results are invalidated when the selected analysis window changes.
- Existing A/B cursors remain independent measurement cursors; the persistent shaded analysis range is the analysis scope.


---

## Imported history: V0468_AB_WINDOW_ANALYSIS.md

# v0.46.8 — A/B cursor scoped classification

## What changed
- Existing Graphics Cursor A and Cursor B are now the analysis boundaries for byte classification and Signal Discovery.
- Removed the separate Graphics analysis-range dependency from Constant / Active / Counter / CRC classification.
- Moving A or B changes the derived classification cache key and invalidates only classification/discovery state; raw/decoded waveform caches are preserved.
- Signal Discovery uses the same A→B interval.
- Graphics now displays `Analysis: A -> B` with the active interval so the scope is visible.
- If A == B, analysis falls back to the full capture.
- Shift-drag range remains a Changed Data range and is no longer described as the classification window.

## Expected behavior
Example: B3 changes at 5 s and 50 s. If A=10 s and B=20 s and B3 does not change inside that interval, B3 is classified STATIC/CONSTANT. Move B to 55 s and classification is recalculated from 10–55 s, so B3 can become ACTIVE/Changing according to the existing classifier.


---

## Imported history: V0469_VISIBLE_ANALYSIS_SELECTION.md

# v0.46.9 — Visible Analysis Selection

- A/B cursors are measurement-only and remain available for delta/value measurement.
- Added visible `Select Range` control in the Graphics toolbar.
- Click `Select Range`, then drag horizontally on a waveform to create the analysis region.
- The selected region is shaded and has draggable left/right handles; drag inside it to move the complete region.
- `Clear Range` restores whole-capture analysis.
- Constant/activity/counter/CRC classification, Only Changing, and Signal Discovery use the dedicated selected region.
- Raw/decoded waveform caches are not invalidated by analysis-range changes.


---

## Imported history: V0470_PERFORMANCE_HORIZONTAL_NAV.md

# v0.47.0 Performance + Horizontal Navigation

- Analysis-range changed-byte scan now binary-searches the timestamp-sorted frame array.
- Per-source byte classification restricts the indexed frame list to the selected time range before bus/channel filtering.
- Horizontal and vertical graph navigation are separated: native X trackpad motion pans time; Y motion remains vertical lane scrolling; Shift+wheel pans time.
- Added an explicit bottom horizontal time scrollbar with proportional thumb and left/right buttons.
- A/B remain measurement cursors. The dedicated analysis selection remains independent.
- Raw and decoded waveform caches are not invalidated by viewport panning or analysis-range movement.


---

## Imported history: V0471_GRAPHICS_AUTO_Y.md

# LogLens v0.47.1 — Graphics Auto Y

- Adds a visible `Auto Y` control to Graphics and enables it by default.
- Analog, configured/decoded signals, and CAN/CAN-FD/LIN/ETH raw-byte lanes auto-fit their Y range to samples inside the current visible X/time viewport.
- Panning or zooming time therefore re-fits Y without re-reading or re-decoding the capture.
- Binary bit lanes remain fixed at 0..1 for stable digital interpretation.
- Constant traces receive a small deterministic Y margin instead of collapsing to a zero-height range.
- Non-constant auto ranges receive 5% headroom above and below the visible min/max.
- `Fit Y` clears per-lane scale/offset state and re-enables Auto Y.
- Existing A/B cursors, independent Analysis Selection, horizontal time scrollbar/navigation, classification caches and configuration behavior are preserved.


---

## Imported history: V0477_PATTERN_COUNTER_CRC_ALL_PROTOCOLS.md

# LogLens v0.47.7 — Pattern / Counter / CRC across framed protocols

## Scope
Classification now uses one shared payload-intelligence path for normalized framed protocols:
CAN, CAN-FD, LIN, Ethernet, TECMP and Unknown. Analog is intentionally excluded because it is a sampled engineering signal rather than a framed byte payload.

## Pattern detection
- Shared stable short-period detector used by classification and `Hide Repetitive`.
- Detects fundamental periods 2..64 frames.
- Requires at least 3 complete cycles.
- Requires >=97% repeat consistency, tolerating small capture noise/dropouts.
- CONSTANT (period 1), counters and CRC/checksum candidates take precedence over PATTERN.

## Counter detection
- 8-bit rolling counters.
- 4-bit low-nibble counters.
- 4-bit high-nibble counters.
- Small forward jumps of 1..3 are accepted as capture-gap continuity.
- Counter Integrity now respects the committed Analysis Range and correctly evaluates high- or low-nibble counters.

## CRC/checksum candidate detection
- Protocol-independent candidate detection over normalized payloads.
- Counter-neighbor automotive E2E layout heuristic retained and generalized.
- Common 8-bit algorithms are sampled and scored: XOR8, SUM8, inverted SUM8, CRC-8 poly 0x07, SAE-J1850-style CRC8, AUTOSAR-style CRC8.
- Long Ethernet payload analysis is bounded: exact algorithm tests use a compact candidate set and up to 384 sampled frames instead of brute-forcing every payload byte.
- Generic high-activity/high-diversity fallback remains for OEM CRC/checksum schemes not covered by the common algorithms.
- Results remain `CRC_CANDIDATE`; LogLens does not claim a polynomial/Data-ID scheme is proven without matching metadata.

## Protocol coverage
- Signal Discovery now includes TECMP and Unknown framed payloads in addition to CAN/CAN-FD/LIN/Ethernet.
- Legacy 64-byte caps were removed from Byte Activity and Bit Activity helpers.
- Graphics class filter now includes `Pattern`.

## Analysis Range
Pattern, counter and CRC classification continue to use the committed Analysis Range. A/B cursors remain measurement-only.


---

## Imported history: V0478_SIGNAL_CONFIG_DECODE_FIX.md

# LogLens v0.47.8 — Signal Configuration / Decode Fix

## Fixed: Byte 0 + Byte 1 could decode an unexpected 256

Signal decoding now treats `byte_position` as a zero-based payload byte and `bit_position` as a bit **inside that byte** (`0..7`).
An invalid value such as `Byte 0 / Bit 8 / Length 16` is no longer allowed to silently shift the extraction into Byte 1 + Byte 2.

Correct configuration for a 16-bit signal covering Byte 0 and Byte 1:

- byte_position = 0
- bit_position = 0
- length = 16

Expected unsigned Little Endian examples:

- `00 00` -> `0`
- `01 00` -> `1`
- `00 01` -> `256`
- `FF FF` -> `65535`

A third byte cannot influence a 16-bit signal starting at Byte 0 / Bit 0.

## Configuration UI

- `byte_position` is now a bounded numeric editor.
- `bit_position` is bounded to `0..7`.
- `length` is bounded to `1..64`.
- Signal validation reports an explicit error when a byte-based definition contains an invalid bit position.
- Validation explains that Bytes 0-1 must use `Byte 0 / Bit 0 / Length 16`.

## Source isolation

Configured signals now retain the `protocol` field and decoded waveforms only match frames from the configured protocol, channel and message ID. This prevents a signal from being decoded from another bus that happens to use the same ID/channel combination.

## Decoder regression coverage

Added checks for:

- `00 00` -> `0` in Little Endian.
- `00 00` -> `0` in Big Endian.
- Byte 2 does not affect a Byte 0-1 16-bit signal.
- `00 01` -> `256` in Little Endian.
- invalid intra-byte bit position is rejected.

## Updated configuration template

Added `LogLens_Configuration_Template_v0.11.3.xlsx` with separate `byte_position` and `bit_position` columns plus `data_type`.

## Analysis-range consistency

Configured-signal Min/Max/Transition statistics in Signal Discovery now use the committed Analysis Range when one is active. Values from outside the selected window no longer leak into those statistics.

## Legacy signal-table migration

Older signal tables that contain only an absolute `start_bit` / `bit_position` are migrated in memory to `byte_position` + intra-byte `bit_position`. Example: absolute bit 16 becomes Byte 2 / Bit 0. This keeps existing configuration files compatible with the newer editor.


---

## Imported history: V0479_PHYSICAL_BIT_ENDIAN_FIX.md

# LogLens v0.47.9 — Physical Bit / Endian Fix

This release changes configured signal bit addressing to one physical convention across CAN, CAN-FD, LIN, Automotive Ethernet and Unknown payloads.

## Canonical bit numbering

Within every payload byte:

- Bit 0 = physical LSB
- Bit 7 = physical MSB
- `byte_position` remains zero-based

The `endian` field now accepts both automotive names and common aliases:

- Intel / Little Endian: `Intel`, `Little`, `Little Endian`, `LE`
- Motorola / Big Endian: `Motorola`, `Big`, `Big Endian`, `BE`, `Network Byte Order`

## Traversal

### Intel / Little Endian

The configured start bit is the least-significant signal bit. Traversal is Bit 0 toward Bit 7, then the next byte Bit 0 toward Bit 7.

A byte-aligned signal therefore starts at Bit 0.

Examples:

- 8-bit: Byte N, Bit 0, Length 8
- 16-bit: Byte N, Bit 0, Length 16
- 32-bit: Byte N, Bit 0, Length 32

For bytes `01 02 03 04`, a 32-bit Intel signal decodes as `0x04030201`.

### Motorola / Big Endian

The configured start bit is the most-significant signal bit. Traversal is Bit 7 toward Bit 0, then the next byte Bit 7 toward Bit 0.

A byte-aligned signal therefore starts at Bit 7.

Examples:

- 8-bit: Byte N, Bit 7, Length 8
- 16-bit: Byte N, Bit 7, Length 16
- 32-bit: Byte N, Bit 7, Length 32

For bytes `01 02 03 04`, a 32-bit Motorola signal decodes as `0x01020304`.

The same signal decoder is used for CAN, CAN-FD, LIN, Automotive Ethernet and Unknown payloads. Ethernet transport itself does not force application payload signals to one endian; configure the signal according to the data definition.

## Float handling

Byte-aligned Float32 / Float64 signals follow the same physical start-bit rule:

- Intel / Little: Bit 0
- Motorola / Big: Bit 7

The bytes are then converted with Rust's `from_le_bytes` or `from_be_bytes` respectively.

## Legacy v0.47.8 migration

v0.47.8 interpreted Big-endian `bit_position` as a logical MSB-first index, so a full byte used Bit 0. v0.47.9 uses physical bit numbers.

The Signals configuration toolbar now includes **Migrate v0.47.8 Big**. It converts rows whose endian value is exactly the old `Big` dropdown value using:

`new_bit = 7 - old_bit`

and changes the endian label to `Motorola / Big Endian`.

New Motorola rows are not modified by the migration action.

## Additional fixes

- `Motorola`, `BE`, `Intel`, `LE`, and Network Byte Order aliases are parsed explicitly.
- Unknown endian labels are rejected by Signals validation instead of silently becoming Little Endian.
- Signed 64-bit integer extraction now preserves the sign bit.
- Added regression tests for 8/16/32-bit aligned decoding, unaligned Motorola traversal, endian aliases, Big-endian Float32 start-bit handling, zero-value regression, and signed 64-bit values.

## Template

`templates/LogLens_Configuration_Template_v0.11.4.xlsx` updates the Signals endian dropdown to the canonical Intel/Little and Motorola/Big labels. The CSV sample uses the same labels.


---

## Imported history: V0480_AUTO_CONTAINER_REVIEW.md

# LogLens v0.48.0 — Automatic Container Review

This build is based on v0.47.9 and intentionally removes the configurable Container Decoder concept.

## Automatic detection
For TECMP Ethernet (data type 0x0080), after inner Ethernet/IP/UDP decoding, LogLens tests the application payload for a repeated long-header record structure:

`[32-bit big-endian PDU ID][32-bit big-endian payload length][payload]`

A payload is accepted only when at least two records parse cleanly and the records consume the entire UDP application payload. Invalid/zero IDs, zero lengths, overflow, impossible lengths, or trailing bytes reject the candidate. This conservative validation prevents arbitrary Ethernet payloads from being split.

Detected records are emitted as independent Ethernet-derived REVIEW frames:

`REVIEW:PDU:0x<ID>`

The child payload contains only the PDU data; the 8-byte container header is excluded. Existing signal, constant, counter, CRC, pattern, graphics and range analysis can therefore operate on each PDU independently. The original parent Ethernet frame is retained for traceability.

## Sample regression
The supplied packet pattern is covered by a regression test and resolves to:
- 0x150 / 15 bytes
- 0x152 / 4 bytes
- 0x180 / 10 bytes
- 0x4C7 / 64 bytes

The UDP parser now honors the UDP length so a trailing Ethernet FCS in a TECMP Ethernet record is not included in application/container analysis.

## Configuration
No Container configuration page, table, profile, or spreadsheet schema is added. Existing v0.47.9 configuration behavior is retained.


---

## Imported history: V0481_FULL_PATTERN_ETH_REVIEW_HIERARCHY.md

# v0.48.1 — Full Pattern Detection + ETH Review PDU Parent Identity

- Replaces the short-period-only byte pattern detector with a generic bounded detector.
- No byte positions or PDU IDs are special-cased.
- Detects repeating sequences, alternating/toggle behavior, ramp/reset and delta-sequence periodicity.
- Searches periods up to 512 frames using up to 4096 samples and tolerates sparse anomalies.
- Counter and verified CRC classifications retain precedence; Pattern is evaluated before generic Signal fallback.
- The same classifier operates on normalized CAN, CAN-FD, LIN, ETH and Review PDU payloads.
- Hide Repetitive uses the same detector as classification.
- Review PDU internal identity now includes its UDP parent stream, preventing same PDU IDs on different ETH streams from being merged.
- Graphics source labels show `UDP <src:port->dst:port> > Review PDU <id>` so parentage is visible.
- Signal configuration matching ignores the internal `@UDP:` suffix and can continue targeting the logical PDU ID.


---

## Imported history: V05_FEATURES.md

# v0.5 CANoe-style feature matrix

| Area | v0.5 |
|---|---|
| Trace-centric default workspace | Implemented |
| Network/protocol tree | Implemented |
| Named channel mapping in Trace/tree | Implemented |
| Frame details inspector | Implemented |
| Byte inspector -> Graphics | Implemented |
| Multi-byte waveform overlay | Implemented |
| A/B cursors + delta time | Implemented |
| Trace context actions | Implemented |
| Start/Stop/Pause toolbar | Implemented UI state |
| Replay speed selector | Implemented UI state |
| Existing deep analysis views | Retained |
| Virtualized Trace | Implemented |
| Persistent dock layout | Next increment |
| True real-time bus acquisition | Not yet; offline capture/replay foundation |
| DBC/ARXML database import | Next increment |
| Measurement cursor drag interaction | Next increment |
| Multiple independent Trace/Graphics windows | Next increment |
| CAPL-style scripting | Future |


---

## Imported history: V06_FEATURES.md

# LogLens Rust Replica v0.6 — CANoe Graphics + Control-Message Events

## Graphics organization
- CANoe-style **Signals** pane on the left and waveform pane on the right.
- **Stacked lanes is the default** so physical signals and raw bytes do not overlap.
- Vertical scrolling supports any practical number of selected signals/bytes.
- Adjustable waveform lane height (70–220 px).
- Overlay mode remains available for raw bytes that share a common 0..255 scale.
- A/B cursors and delta-time remain visible across lanes.
- Event boundaries can be drawn as timeline markers across every lane.
- Signal selection supports both raw bytes from the selected Trace frame and decoded signals from IdentifiedSignals.

## Control Message triggers as events
- TECMP message type `0x00` / `Control Message` is treated as a trigger boundary.
- Trigger de-bounce/quiet-gap is configurable; default is 1.0 second.
- Repeated Control Messages inside the quiet gap are ignored as duplicate boundaries.
- EventLog rows are mapped in row order to adjacent valid Control Message pairs.
- If no EventLog is loaded, generic Event 1 / Event 2 / ... windows are created between adjacent Control Messages.
- Selecting an event sets Cursor A to the first Control Message and Cursor B to the next Control Message.
- Selecting an event from the Project/Network tree also activates the manual Trace time window for event-scoped analysis.

## EventLog format
Recommended file:

`templates/EventLog_ControlTriggers.placeholder.csv`

Columns:
- `Sr No`
- `Test Case`
- `Event Name`
- `Description`

Explicit start/end timestamp EventLogs remain supported in the legacy Frames & Filters workflow.

## Cross-platform
The source is shared between macOS and Windows.
- macOS Apple Silicon: `aarch64-apple-darwin`
- Windows + MinGW: `x86_64-pc-windows-gnu`


---

## Imported history: V07_FEATURES.md

# LogLens Rust Replica v0.7 – Organized CANoe Workspace

## Layout changes
- Trace remains the primary central window.
- One inspector pane at a time: Details, Graphics, or Events / Triggers.
- Trace-only mode hides the inspector completely.
- Project tree is independently collapsible.
- Inspector height is adjustable.
- Graphics uses a compact signal navigator and a scrollable stacked-lane canvas.
- Event windows are presented in a dedicated table and can scope Trace with one click.

This removes the v0.6 overlap caused by showing project tree, Trace, details, Graphics and trigger list simultaneously.


---

## Imported history: V081_FIXES.md

# LogLens v0.8.1 fixes

- Fixed detached Graphics window vertical resizing.
- Removed a layout constraint that reserved the full current available height in the Signals navigator.
- Detached Details, Graphics, and Events windows remain freely movable and horizontally/vertically resizable.
- Reduced detached-window minimum height to 150 px.
- Graphics signal navigator now scrolls within the current window height instead of determining it.


---

## Imported history: V08_FEATURES.md

# LogLens v0.8 – Adjustable & Detachable Workspace

## Workspace changes
- Project tree width is now adjusted by dragging its vertical splitter.
- The active analysis tab can dock at the bottom or on the right.
- Bottom inspector height is adjusted by dragging the horizontal splitter.
- Right inspector width is adjusted by dragging the vertical splitter.
- Details, Graphics, and Events / Triggers can be detached into independently movable and resizable floating windows.
- Floating windows can be docked back to Bottom or Right.
- Trace remains the primary workspace and automatically expands when analysis tabs are detached or closed.

## Intended CANoe-style workflow
1. Keep Trace maximized for frame inspection.
2. Dock Graphics at the bottom when correlating signals with trace rows.
3. Dock Details/Events on the right for a vertical inspector arrangement.
4. Detach Graphics for a large waveform workspace, including on a second monitor within the application workspace.

## Note
The v0.8 detach mode uses egui floating windows inside the LogLens application window. Native operating-system child windows are a possible later step if true multi-monitor OS-level pop-out is required.


---

## Imported history: V091_FIXES.md

# LogLens v0.9.1 — Timestamp readability

- Capture start is the first recorded frame timestamp.
- Capture end is the last recorded frame timestamp.
- Trace tables display readable absolute timestamps by default.
- Epoch timestamps display as `YYYY-MM-DD HH:MM:SS.mmm UTC`.
- Non-epoch/device-clock timestamps fall back to `HH:MM:SS.mmm`.
- Raw nanoseconds and relative seconds remain available in hover text.
- Delta time is auto-formatted as microseconds, milliseconds, or seconds.
- Offline replay toolbar displays current absolute timestamp and final capture timestamp.
- Graphics displays absolute Capture Start/End and current Visible Range while keeping relative seconds internally for stable pan/zoom/replay math.


---

## Imported history: V092_FIXES.md

# LogLens v0.9.2 — Capture/Event Timestamp Fixes

## Canonical capture time
For PCAP/PCAPNG data, the outer capture packet timestamp is now the canonical absolute UI/replay timeline when it contains a valid Unix epoch timestamp. TECMP H2 timestamps may be device-local counters/uptime clocks and are no longer allowed to replace the absolute capture timestamp in that case.

This fixes mixed displays such as a device time (`99:18:53.308`) next to a UTC capture timestamp.

## Event-aware replay time
Selecting an Event/Trigger now:
- moves replay position to the event start,
- sets A/B cursors to event start/end,
- displays readable absolute Start and End timestamps in the Events table,
- changes the replay slider to the selected event range,
- stops replay at that event's end,
- shows the selected event name and current/end timestamp in the replay toolbar.

With no event selected, replay uses the complete capture first-frame to last-frame range.


---

## Imported history: V093_FEATURES.md

# LogLens v0.9.3 — CANoe-style Updating Trace

## Updating Trace during offline replay
- Trace rows are revealed according to the current replay timestamp.
- `Auto-follow` keeps the Trace scrolled to the latest visible/replayed frame.
- `Freeze View` freezes only the Trace display while replay continues.
- `Resume View` immediately catches the Trace up to the current replay position.
- Trace status displays `UPDATING`, `VIEW FROZEN`, or `READY`.
- Existing filters, readable absolute timestamps, channel names, event scoping, and context actions remain available during replay.

## Intended workflow
1. Open an offline log.
2. Press `Run Offline`.
3. Keep `Auto-follow` enabled to watch rows arrive like a CANoe Trace window.
4. Use `Freeze View` to inspect older rows while replay continues.
5. Press `Resume View` to jump back to the current replay point.

## Notes
- Freeze View does not pause the offline replay clock.
- Pause pauses replay itself.
- Stop stops replay.


---

## Imported history: V094_FEATURES.md

# v0.9.4 — CANoe Overwrite Trace + Multi-Source Graphics

- Trace mode selector: Overwrite / Chronological.
- Overwrite key is Protocol + Channel ID + Frame ID/key.
- Fixed overwrite rows update latest Timestamp, Data, Count and per-frame Δt during offline replay.
- Context menu can add any Trace frame/channel source to Graphics.
- Graphics can select multiple Frame/Channel sources simultaneously.
- Raw byte lane selection applies across selected sources; each source/byte gets its own stacked lane.
- Same frame ID on different channels remains separate.
- All lanes share pan, zoom, A/B cursors, replay playhead and Control Message event markers.


---

## Imported history: V0951_COMPILE_FIX.md

# LogLens v0.9.5.1 compile fix

Fixes Rust E0502 borrow-checker failures in the multi-frame/channel Graphics waveform renderer.

The selected byte indices are copied into a short local Vec before invoking `cached_byte_waveform(&mut self, ...)`. This releases the immutable borrow of `self.graphics_bytes` before the mutable waveform-cache borrow occurs.

Affected paths:
- stacked multi-source raw-byte Graphics lanes
- overlay multi-source raw-byte Graphics traces

No feature or performance-cache behavior was removed.


---

## Imported history: V0952_COMPILE_FIX.md

# v0.9.5.2 compile fix

Fixes Rust E0500 borrow-checker errors in `Frames & Filters`.

The scoped frame indices were returned as a borrowed slice from `self`, and that immutable borrow survived across the egui closure that updates `self.info` and `self.error`. The UI now takes an owned snapshot with `self.scoped_indices().to_vec()` before constructing the closure.

This preserves the cached scoped-index calculation while allowing status/error mutation in the same UI block.


---

## Imported history: V095_PERFORMANCE.md

# LogLens v0.9.5 Performance Fixes

This release focuses on macOS/Windows runtime responsiveness while preserving v0.9.4 functionality.

## Main changes

- Replay repaint rate capped to about 30 FPS (`request_repaint_after(33 ms)`) instead of unbounded repaint requests.
- Scoped frame indices are cached and rebuilt only when filters/channel/time scope changes.
- Replay cutoff uses `partition_point` on time-ordered scoped indices rather than allocating a filtered prefix on every frame.
- CANoe Overwrite Trace is incremental: only newly reached replay frames update the fixed-row state. It no longer rescans all previous replay frames at every refresh.
- Frame/channel source discovery for Graphics is cached when a dataset is loaded.
- Control-message event windows are cached and rebuilt only when the trigger gap/event configuration changes.
- Raw-byte waveform extraction uses the existing frame-key index and is lazy-cached per channel/frame/byte.
- Cached waveform series are bounded/downsampled once, avoiding repeated traversal of very large raw series during pan/zoom/replay.

## Expected effect

The largest improvement should be visible when replaying captures with hundreds of thousands or millions of frames in Overwrite Trace mode and when multiple byte waveforms are selected.


---

## Imported history: V096_FEATURES.md

# LogLens v0.9.6

## Changing-only source filtering

Trace and Graphics now have independent **Changing only** controls.

A source is considered changing when the normalized payload differs between at least two receptions of the same Protocol + Channel + Frame identity inside the current analysis scope. The scope follows the active filters, selected channels, manual time window, and event window.

- Trace: hides frame/channel sources whose payload remains constant for the complete current scope.
- Graphics: filters the Frame / Channel source navigator to sources that actually change.
- A changing source still retains its complete replay history; repeated samples are not removed from the source.

This is intended to remove cyclic/static traffic while investigating bytes/signals that move.

## Automotive Ethernet

Raw Ethernet PCAP/PCAPNG packets are now retained even when they are not TECMP.

Supported normalization includes:
- Ethernet II
- 802.1Q / QinQ VLAN tags
- IPv4
- UDP
- TCP
- ARP / IPv6 classification
- conservative SOME/IP base-header recognition

PCAPNG interface IDs are retained as LogLens channel IDs for raw Ethernet captures. TECMP Ethernet remains supported and continues to use the TECMP channel ID.

For IPv4 UDP/TCP traffic, flow information is used to produce a more useful frame key. Recognized SOME/IP payloads use the 32-bit SOME/IP Message ID as the frame identity.

This is protocol normalization and offline analysis support. It is not yet a full AUTOSAR SOME/IP-SD/service database decoder, DoIP diagnostic decoder, or live Ethernet interface transmitter.


---

## Imported history: V097_STATIC_FILTER_FIX.md

# v0.9.7 Static Frame Filter Fix

## Correct rule
For each unique Protocol + Channel + Frame ID, scan the complete capture once.

- If every payload is identical from the source's first occurrence to its last occurrence: **static -> hidden when Hide Static Frames is enabled**.
- If any occurrence differs at least once: **changing -> retained**.
- A payload that changes and later returns to the initial value is still classified as changing.

The classification is full-capture based and is not recomputed from the currently selected event/time window/replay position.


---

## Imported history: V098_GLOBAL_PROJECT_SCOPE.md

# v0.9.8 — Global Project Tree Scope

- Project Tree channel selection now applies to both Trace and Graphics.
- Graphics source navigator lists only channels allowed by the Project Tree.
- Already-selected Graphics lanes from disabled channels are hidden, not deleted.
- Re-enabling a channel restores its previously selected waveform lanes.
- Hide Static Frames remains a secondary filter.
- Empty Project Tree channel selection continues to mean All Channels.


---

## Imported history: V099_GLITCH_FIXES.md

# v0.9.9 – Global Scope Glitch Fixes

Corrected Project Tree → Trace/Graphics scoping.

## Fixed
- Project scope identity is now **Bus Type + Channel ID** instead of Channel ID alone.
  - CAN / channel 1 and LIN / channel 1 are independent.
  - Ethernet / channel 1 is also independent.
- Project Tree selection is explicit.
  - `All` enables all bus/channel sources.
  - `None` disables all bus/channel sources.
  - Disabling the last source no longer accidentally means “all”.
- Trace scope uses the same Project Tree bus/channel set.
- Graphics source list uses the same Project Tree bus/channel set.
- Raw waveform cache key is now **Bus + Channel + Frame + Byte** to avoid cross-bus waveform collisions.
- Graphics source key is now **Bus + Channel + Frame ID**.
- Static-frame classification uses the same bus-aware Graphics source identity.
- Decoded signal rendering is constrained by enabled Project Tree bus/channel sources.
- Disabled Graphics sources retain their selections internally and return when re-enabled.

## Logic tests performed in this environment
- Verified same numeric channel ID on different bus types produces distinct project keys.
- Verified same numeric channel/frame ID on different buses produces distinct waveform/source keys.
- Verified empty project set means no sources, not all sources.
- Verified `All`/`None` transitions preserve explicit semantics.
- Searched source for stale channel-only Graphics keys and stale empty-means-all Project Tree logic.

Rust/Cargo is not installed in the execution container, so final compiler validation still occurs on the target Mac/Windows Rust toolchain.


---

## Imported history: V09_FEATURES.md

# LogLens v0.9 — Interactive Waveforms + Offline Replay

## Waveform navigation
- Shared X-axis viewport for all stacked waveform lanes.
- Mouse wheel over a waveform: zoom around the mouse pointer.
- Left-drag on a waveform: horizontal pan.
- Shift + mouse wheel: horizontal pan.
- Fit button: reset to the complete log duration.
- + / - buttons: step zoom.
- Left / right buttons: step pan.
- Event boundaries, Cursor A/B and replay playhead use the same synchronized time axis.
- Follow replay option keeps the waveform viewport following the moving offline-replay playhead.

## Offline log replay
- `Open Log` loads PCAP, PCAPNG, CSV, XLS/XLSX, TXT or LOG directly from local storage.
- `Run Offline` replays the already-decoded local log using its recorded timestamps.
- Start, Pause/Resume and Stop controls.
- Replay speeds: 0.25x, 0.5x, 1x, 2x, 5x and 10x.
- Timeline slider supports seek/scrub.
- Trace reveals frames up to the current replay time while replay is running.
- Graphics shows a moving playhead across all selected bytes/signals.

## Notes
- This is offline replay, not bus transmission. It does not inject frames onto CAN/LIN/Ethernet hardware.
- Existing Control Message event windows continue to work during replay.

## v0.53.4 - Draggable Multi-Cursors + Analog Measurements
- Per-waveform C1..C8 cursors can now be dragged horizontally after placement.
- Cursor drag state is bound to the stable lane ID and cursor index, so moving one cursor affects only that waveform.
- Cursor hover shows a horizontal-resize pointer to make the drag target discoverable.
- Analog multi-cursor values are now generated independently from the disabled/global A/B cursor state, fixing missing Analog measurements in Per-Waveform mode.
- Existing Global A/B mode, waveform colors, filtering, classification, lane alignment, and caches are retained.

## v0.53.7 — Windows Viewer / Interaction Optimization
- Removed repeated whole-frame and raw-byte display downsampling from the waveform repaint path; display-ready series are cached once.
- Added a cached Analog display waveform so opening/repainting the viewer does not repeatedly downsample the full Analog series. Full Analog data remains cached separately for cursor measurement lookup.
- Reduced lane-order synchronization from repeated linear membership scans to set-based membership checks.
- Increased per-waveform multi-cursor hit tolerance from 12 px to 20 px and starts cursor acquisition on primary press, making drag pickup more reliable.
- Enlarged Up/Down lane controls for more reliable single-click operation, including Analog lanes.
- Retains the v0.53.6 default Graphics Sources tree: root expanded, protocol groups collapsed.
