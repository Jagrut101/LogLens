# v0.54.8 Start Classification Validation

Validation capture: `TC_UC_LL_01.01.pcapng` (user-provided).

Observed TECMP system interfaces:

| Interface ID | Start category | Observed records |
|---|---|---:|
| `0x000FFF00` | TECMP Status Bus/IVN | 239 |
| `0x000FFF01` | TECMP Status Configuration | 18 |
| `0x000FFF02` | TECMP Status Device | 239 |
| `0x000FFF03` | Control Messages | 1 |

The control record uses TECMP Message Type `0x00` and Control Message ID `0x0007`. It is no longer presented as `Unknown` by Start-page discovery.

Observed analyzable bus-data record groups in the same scan:
- CAN: 574,037
- CAN-FD: 302,324
- LIN: 52,271
- ETH: 397,399
- Analog: 84

LIN compatibility check:
- normal payload lengths observed: 1, 2, 3, 4, 5, 6, 7 and 8 bytes;
- LIN ID `0x00` specifically occurs with lengths 2, 5 and 8;
- v0.54.8 accepts normal LIN payload lengths 1..=8.
