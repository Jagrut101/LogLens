# LogLens v0.54.7 - TECMP LIN Decoder Correction

This update aligns LogLens LIN normalization with TECMP User Manual v1.7 sections 2.4.3 and 2.4.3.3.

## Corrected behavior

- Valid LIN ID `0x00` is retained as `LIN:0x00` when the TECMP Data section contains a valid LIN payload length (1 through 8 bytes).
- For LIN Rx (`Message Type 0x03`), Data Flags are evaluated before the Data section. Special conditions that can zero the Data section no longer appear as ambiguous `LIN:RAW:CHx`.
- WUP, Short WUP, SLEEP and No Slave Response are exposed as `LIN:EVENT:*`.
- Collision is exposed as `LIN:ERROR:COLLISION`.
- Parity and Checksum error traffic is isolated from the normal `LIN:0xNN` stream while preserving available bytes for trace diagnosis.
- Unexpected/truncated records use explicit `LIN:DECODE_ISSUE:*` keys instead of `Raw LIN`.
- LIN Replay Data (`Message Type 0x0A`) uses Tx semantics. WUP/Short-WUP are exposed as Tx events and zero-length payload is treated as the TECMP remove-slave-response operation.
- The Details inspector shows raw LIN Data Flags plus a decoded text description.
- Automatic signal discovery/activity-window analysis excludes LIN event/error/decode-operation records so they do not create misleading signal/CRC/counter candidates.

## Examples

- `ID=0x00, Length=2, Data Flags=0` -> `LIN:0x00`
- `Data Flags.WUP=1, Data fields all 0` -> `LIN:EVENT:WAKE_UP`
- `Data Flags.No Slave Response=1, Data fields all 0` -> `LIN:EVENT:NO_SLAVE_RESPONSE`
- `ID=0x00, Length=1, Checksum Error=1` -> `LIN:ERROR:CHECKSUM:0x00`
- `ID=0x00, Length=0, no special Rx flags` -> `LIN:DECODE_ISSUE:INVALID_LENGTH_0:0x00`
- LIN Replay `ID=0x00, Length=0` -> `LIN:TX:REMOVE_SLAVE_RESPONSE:0x00`


## v0.54.8 compatibility note
Testing with `TC_UC_LL_01.01.pcapng` showed valid checksum-bearing LIN responses with lengths 3, 5, 6 and 7 in addition to 1, 2, 4 and 8. LogLens therefore accepts normal LIN payload lengths 1 through 8 while keeping length 0 reserved for the TECMP event/error/replay semantics above.
