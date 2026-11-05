# LogLens v0.54.4 — Resource Safety and Crash Recovery

## Goal
Prevent avoidable process termination on typical Windows i5/i7 machines with 8 GiB or 16 GiB RAM while preserving normal LogLens behavior.

## Resource profile
LogLens checks installed and currently available physical RAM. The limits are intentionally conservative because the decoded dataset, indexes, UI state and graphics caches coexist in memory.

| Memory condition | Packet/row safety cap | Native whole-file reader max | XLS/XLSX max | Analog cached points | Waveform cache entries |
| --- | ---: | ---: | ---: | ---: | ---: |
| <=10 GiB installed (8 GiB class) | 750,000 | 192 MiB | 256 MiB | 350,000 | 96 |
| <=20 GiB installed (16 GiB class) | 1,500,000 | 384 MiB | 512 MiB | 750,000 | 160 |
| >20 GiB | 2,000,000 | 768 MiB | 1,024 MiB | 1,500,000 | 256 |
| <1.5 GiB currently available | 250,000 | 64 MiB | 128 MiB | 150,000 | 48 |
| <3 GiB currently available | 500,000 | 128 MiB | 192 MiB | 250,000 | 72 |

The effective value is the safer of the installed-RAM tier and the current-available-RAM tier.

## Failure handling
- Loader errors/corrupt captures -> return `Result` and display the error.
- Loader panic -> caught in the loader worker (release builds now unwind).
- Loader channel disconnect -> loading job is ended and an error is displayed.
- UI/analysis panic -> caught around the eframe update; replay is stopped and derived graphics caches are cleared before continuing.
- Oversized packet/block declarations -> rejected as corrupt/unsafe before allocation.
- Large PCAP/PCAPNG -> streaming fallback avoids loading the complete file into RAM.
- Large text logs -> line-by-line import.
- Large spreadsheets -> refused before calamine opens them if they exceed the safe resource profile.
- Large Analog sources -> bounded uniform decimation before cache insertion.

## Important limitation
Rust process-level out-of-memory can abort before normal panic recovery. The purpose of the resource guards is to prevent LogLens from reaching that state during supported workloads. No application can guarantee survival if Windows itself has no commit/pagefile capacity or if a native/driver failure terminates the process externally.
