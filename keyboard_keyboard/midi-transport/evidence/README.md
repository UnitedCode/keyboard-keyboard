# MIDI experiment evidence — 2026-11-08

- `host-tests.log`: 11 passing public USB-host scenarios, using the real USB
  stack/class and output queues, not a replacement encoder.
- `seed-host-tests.log`: 19 passing recorded-evidence and velocity scenarios
  after incorporating main's concurrent velocity update (`36493c3`).
- `output-both.png`, `both-midi2.png`: 4x framebuffer renders of the actual
  firmware settings/main-screen code. These are previews, not physical OLED photos.
- `firmware.sha256`: exact release ELF supplied unchanged to the virtual Seed.
- `boot-scenario.csv`: bounded 130 ms virtual boot attempt.
- `seed-model-blocker.log`, `seed-runtime.log`: **blocked** actual ARM run.
  USART1 TXE interrupt enable is rejected by the polling-only carrier fixture;
  USB2/CRS/DBGMCU also produce unsupported-model warnings. No timing/MIDI-delivery
  pass is claimed for this ELF, and no warning was suppressed.

Firmware build, Clippy and format checks passed. Existing historical Seed evidence
host tests also passed (19 tests; the separate live-firmware test is ignored by its
normal command). Those historical recordings do not validate the new firmware.
The existing unused `commit` manifest keys and linker `Reset_Handler` entry warning
remain unchanged; the image has its flash vector table as before.

The firmware fits the declared STM32H750 memory: text 81,940 bytes, data 8 bytes,
BSS 53,836 bytes (`rust-size`). Physical USB/DIN interoperability, menu behavior
on hardware, scan jitter, power/thermal behavior and musical acceptance are pending.

See [the branch guide](../../code/USB_MIDI.md) for reproducible commands and scope.
