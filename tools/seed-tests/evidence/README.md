# Measured short experiment — 2026-11-08

| Profile | Quiet max scan gap | All Notes Off max gap | Instant-UART control | Burst bytes |
| --- | ---: | ---: | ---: | ---: |
| rev5 / WM8731 | 1.000 ms | 10.209 ms | 1.000 ms | 48 / 48 |
| rev7 / PCM3060 | 1.000 ms | 10.209 ms | 1.000 ms | 48 / 48 |

Each run executed 350 ms of virtual time. The 100–125 ms window is the quiet
baseline; HE76 is driven from 130–180 ms and its observation window ends at
250 ms. HE1/RV1 positive controls run from 260–350 ms. Both paired runs use the
same ELF and scenario. Startup pot messages are recorded separately from the
48-byte event measurement. Original-keyboard source revision:
`58f0e0e8dd74ba3fe2107248ce6ff4699fbe5f99`.

Firmware SHA256:
`0c7c6fd23099b392999e0adbb065e9b309c7b01a4eb4326fb0dd2a8db0120af7`.

Virtual Daisy source: `ecd8185` (keyboard runner). Per-run conformance files
retain native model, managed bridge/engine, CPU manifest and platform hashes.
`firmware.sha256` files retain the original hash line; machine-specific source
paths are omitted. Runtime logs and UART/scan observations are otherwise copied
as recorded. ELF/build binaries are not duplicated in this evidence directory.

Recheck with `cargo run --release -- grade evidence/rev5` (or `evidence/rev7`).
See the parent README to rebuild and execute fresh controls.

The traces reproduce a UART-dependent software scan stall. They are not physical
measurements, and the CPU timing model is uncalibrated. The continuous 3,661 s
firmware scenario is implemented but **NOT_RUN**. No conclusion has been reached
about which fault caused the reported hot-room behavior (duration unknown), or whether
the board also has thermal/electrical faults.
