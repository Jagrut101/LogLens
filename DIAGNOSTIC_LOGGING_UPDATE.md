# LogLens v0.54.5 — Persistent Diagnostic Logging

## Purpose
Keep enough evidence to diagnose LogLens failures after the application has recovered or restarted, without duplicating the user's raw vehicle capture into the diagnostic files.

## Windows location
`%LOCALAPPDATA%\LogLens\logs`

If that directory cannot be created, LogLens falls back to the operating-system temporary directory under `LogLens\logs`.

## Files
- `loglens_YYYY-MM-DD.log` — normal INFO/WARN/ERROR operational diagnostics.
- Additional timestamped `loglens_...log` files are created when the active file reaches 5 MiB.
- `crash_YYYY-MM-DD_HHMMSS_mmm.log` — panic report with operation/thread/location/backtrace.

Retention is bounded:
- maximum 5 normal diagnostic files, each capped at 5 MiB;
- maximum 5 crash-report files.

## What is recorded
- LogLens version, OS/architecture and process ID;
- installed/available RAM and active resource-safety caps;
- capture load start, file size, loader outcome and normalized-frame count;
- session/configuration errors already surfaced through Tool Log;
- resource-protection warnings;
- worker failures and contained UI/analysis panics;
- panic thread, operation context, source location and backtrace;
- normal application shutdown.

## Data handling
The diagnostics subsystem does **not intentionally log CAN, CAN-FD, LIN, Ethernet or Analog payload bytes**. It records operational metadata needed for support, such as filenames/paths, frame counts, resource limits and error messages. Diagnostic messages are capped at 16 KiB.

## UI
Open **Tool Log** and use **Open Diagnostics Folder** to open the folder directly. The existing **Clear** button clears only the on-screen/in-memory Tool Log; it does not delete persisted diagnostics.

## Failure isolation
Diagnostic I/O is best-effort. Failure to create, rotate, write or open a diagnostic file/folder does not intentionally terminate LogLens. This keeps the diagnostic subsystem from becoming another crash source.
