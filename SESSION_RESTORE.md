# LogLens v0.54.1 — Resume analysis per log

Open a log and perform analysis normally. To work on another log, use the Start page / Open Capture. To return, select the previous log under Recent Sessions. The saved channel selection loads automatically and the analysis workspace is restored once decoding succeeds.

Each session stores its configuration and analysis separately, including selected channels, frames, bytes, bits and signals, lane order and scaling, global and per-waveform cursors, analysis range, zoom, filters, analysis page, and replay position. Replay returns stopped. Changing logs saves the outgoing session immediately. Normal app close also flushes the latest changes. Idle autosave runs approximately every 900 ms.

The outgoing session remains active while a new capture loads. A failed load leaves it intact. New logs start with fresh analysis settings; existing logs restore their saved configuration and workspace. Existing v0.54.0 workspace files remain readable. Session JSON files are replaced atomically, and reopening an app-managed capture no longer copies it onto itself.

Storage location on macOS: ~/Library/Application Support/LogLens. Windows uses LOCALAPPDATA/LogLens; Linux uses XDG_DATA_HOME/loglens or ~/.local/share/loglens.

Build using Cargo or the included platform build scripts. This ZIP contains source code, not a prebuilt application.

Validation: five new persistence/lifecycle tests pass, covering A → B → A, failed loading, closing/restarting, safe managed-file reopening, and old workspace compatibility. Full binary test suite: 23 passed, 1 pre-existing failure in protocols::tests::detects_sample_long_header_container_as_review_pdus. No real user sessions were used by the tests. GUI interaction and Windows/Linux execution were not tested. CSV parsing and PCAPNG export issues identified in the previous review are outside this session-restoration update.
