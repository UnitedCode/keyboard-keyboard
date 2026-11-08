# Keyboard slowdown diagnostic

This Rust tool runs the **unmodified release keyboard ELF** on Virtual Daisy's
Renode Cortex-M7, drives the carrier's GPIO-selected analog inputs, and measures
completed ADC sweeps and actual UART bytes. It does not call a host replacement
of the firmware's switch or MIDI logic. No production firmware or PCB is changed.

The paired experiment uses the same ELF and stimulus twice:

* A finite 16-byte UART FIFO plus shift register at the guest's configured baud.
* An instantaneous UART negative control, explicitly labelled as nonphysical.

After startup calibration and pot traffic, a quiet window establishes normal scan
timing. HE76 is then pressed, requesting CC123 on all sixteen MIDI channels.
Both runs must emit the complete 48-byte sequence. `verify` requires both runs to
stay within 2 ms between scans, with no deadline violations. The historical
`reproduce` mode instead requires a paced gap above 5 ms and a healthy control;
use the saved pre-fix ELF for that mode. An HE1 press/release and RV1 movement must produce a note on,
note off, and CC7=127. Missing output, broken startup, unknown MMIO warnings,
mismatched ELFs, or a stalled control fail the diagnostic.

## Verify the repaired firmware

Use Virtual Daisy revision `ecd818531a3656655341394f782acb89e0b7cdc5` or a
descendant with its keyboard runner (the `v0.1.1` tag predates it), a
portable Renode 1.17 runtime, .NET 8 SDK, CMake and a C compiler. Follow
[Virtual Daisy setup](https://github.com/UnitedCode/virtual-daisy#running-firmware).
Build Virtual Daisy with `cargo build --release --locked`. Build the keyboard:

```sh
cd keyboard_keyboard/code
cargo build --release --locked
```

From this directory, substitute absolute paths and use a **fresh** output folder:

```sh
cargo run --release --locked -- verify \
  /path/to/virtual-daisy/target/release/virtual-daisy \
  /path/to/Renode.app/Contents/MacOS \
  /path/to/keyboard-keyboard/keyboard_keyboard/code/target/thumbv7em-none-eabihf/release/app \
  /path/to/fresh-results rev7
```

Use `rev5` for the WM8731 profile or `rev7` for PCM3060, following Virtual Daisy's
board selection. Both were checked with the same keyboard ELF. On hosts where
an ancestor Cargo configuration selects an embedded target, append
`--target aarch64-apple-darwin` before `--` (or the appropriate host triple).
`VIRTUAL_DAISY_DOTNET` selects the SDK executable;
`VIRTUAL_DAISY_CORE_CACHE` reuses the verified native-engine build cache.

Results include the copied ELF and SHA256, conformance/model hashes, CSV input,
RTT output, runtime logs, timestamped MIDI bytes, baseline and event scan metrics,
and `diagnosis.txt`. `grade-fixed OUTPUT_DIRECTORY` rechecks the repaired-firmware
experiment; `grade` rechecks historical stall reproduction.

Use the same arguments with `verify-chord` to press/release forty keys together.
It requires 40 note-ons, 40 note-offs, no unmatched releases and no stranded notes,
with scan gaps below 2 ms in both UART controls. The old 32-event scan buffer
failed this test with only 32 on/off pairs.

`verify-overload` drives a deliberately unsustainable event rate into the paced
UART. It requires explicit overflow logging, a complete sixteen-channel All Notes
Off sequence, bounded scan gaps and no sounding notes at the end. The firmware's
bounded queue holds two full scans; on overflow it discards stale events, closes
settings and resets sounding notes. It cannot deliver an unlimited event stream
faster than MIDI's wire rate.

Run host checks with `cargo test --locked` and `cargo clippy --all-targets -- -D warnings`.
The explicit integration test uses the same public command:

```sh
KEYBOARD_VIRTUAL_DAISY=/absolute/path/to/virtual-daisy \
KEYBOARD_RENODE_RUNTIME=/absolute/path/to/runtime \
KEYBOARD_FIRMWARE_ELF=/absolute/path/to/app \
KEYBOARD_TEST_OUTPUT=/absolute/path/to/fresh-output \
KEYBOARD_SEED_PROFILE=rev7 \
cargo test --test firmware -- --ignored
```

## Continuous one-hour test

Replace `verify` with `verify-soak` and set a suitable explicit wall-time limit, e.g.
`VIRTUAL_DAISY_WALL_SECONDS=500000`. Each control executes **3,661 continuous
virtual seconds**, records minute snapshots, checks more than 3.5 million scans,
and repeats the burst after the first hour. It never patches the firmware tick,
PC, RAM, or skips its idle loop. At the observed emulation speed, this can take
days of host time for the pair. The short reproduction takes about a minute.

The recorded short runs say `continuous_one_hour_soak=NOT_RUN`. The long test is
implemented but was **not executed to completion**. A large-time UART unit test
only checks the model's timestamp representation; it is not evidence of an
hour of firmware execution. A future completed soak is reported as executed only
after the elapsed-time, liveness, MIDI and negative-control checks all pass.

RTT log draining advances the debugger read pointer in its ring buffer. It does
not fast-forward the firmware clock or alter application state to simulate aging.

## Meaning of the result

The pre-fix firmware sent MIDI with `nb::block!(tx.write(...))` while holding
`event_queue`'s RTIC lock. The queue is shared with the priority-15 scan task, so
that critical section prevented scanning while UART capacity was exhausted.
The repaired firmware dequeues under a short lock and transmits after releasing it.
The controlled experiment isolates that software path in the virtual system.
The CPU is not cycle calibrated; the measured gap is a simulation result.

The temporary event list now holds all 100 key transitions, 12 pots, pitch bend
and vibrato in a scan. Queue saturation has explicit receiver recovery. Transmission
still polls at low task priority; an IRQ/DMA transmit queue is a possible later
CPU-efficiency improvement. It is not required to fix the demonstrated scan stall.

This does **not** prove the cause of the OpenSauce hot-room behavior (duration unknown),
or exclude a second hardware fault. Voltage sources are ideal; Hall sensor
heating, regulator limits, shorts, analog crosstalk, temperature and supply droop
are unmodeled. Physical attribution still needs a repeatable cold/warm test with
3.3 V rail, current and component temperatures recorded alongside scan/MIDI
timing. An emulator pass alone cannot clear the PCB's electrical findings.
