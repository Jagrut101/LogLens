# LogLens v0.54.8 — TECMP Start Classification

## Start-page clarity
- Start scan now classifies TECMP **Message Type before Data Type**.
- `Message Type 0x00` appears as **Control Messages**, not `Unknown`.
- Start channel selection is grouped into **BUS DATA**, **TECMP SYSTEM DATA**, and **OTHER**.
- `Unknown` is presented as **Unknown / Unsupported** and is reserved for genuinely unclassified content.
- TECMP interface/channel identifiers use 8-digit hexadecimal formatting so values such as `0x000FFF03` are easy to recognize.
- Control Messages show an explanatory note and tooltip that they are TECMP protocol-management traffic, not CAN/LIN/ETH signal payload data.

## Analysis safety
- TECMP Control and Status message payloads remain visible in Trace/Details but are excluded from automatic signal discovery, activity-range detection, counter/pattern analysis, and CRC candidate analysis.
- Control Messages normalize as TECMP system records instead of `UNKNOWN`.

## LIN compatibility carried forward
- Normal LIN payload lengths `1..=8` are accepted.
- This preserves valid 3/5/6/7-byte LIN frames observed and checksum-validated in `TC_UC_LL_01.01.pcapng`.
- Length `0` still uses the TECMP special-condition/replay handling introduced in v0.54.7.
