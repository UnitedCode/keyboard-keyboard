# Test the complete virtual Keyboard Keyboard

The actual main-branch ARM firmware scans the frozen 100-Hall/12-pot circuit,
drives its muxes and decoder, renders its OLED, and transmits real paced MIDI.
No host copy of the key, velocity, settings or MIDI firmware logic is substituted.

From a checkout containing this lab:

```sh
LAB_HOST=$(rustc -vV | sed -n 's/^host: //p')
export CARGO_TARGET_DIR="$PWD/keyboard_keyboard/code/target"
export CARGO_INCREMENTAL=0
(cd keyboard_keyboard/code && cargo build --release --locked)
mkdir -p circuits/results
cargo run --release --locked --manifest-path tools/circuit-lab/Cargo.toml \
  --features device --target "$LAB_HOST" -- device \
  keyboard-current circuits/baselines/keyboard-current.net circuits/fixtures/reference.fixture \
  keyboard_keyboard/code/target/thumbv7em-none-eabihf/release/app \
  rev4 circuits/results/device-first baseline
```

Use a **fresh** output directory and an installed native libngspice. This does
not flash anything. Keep experiment firmware and its source revision separate
from this repository-main baseline.

## Electrical coupling

GPIO changes select the real nets found in the KiCad snapshot. The carrier
evolves each populated input's actual schematic capacitor, declared source
impedance, mux resistance and shared ADC loading in virtual time. ADC reads
return those evolved voltages; no test calls the firmware's switch state machine.
The two-node RC solver is checked against an independent ngspice transient and
against partitioned-time evolution. A disconnected Hall source cannot instantly
follow its stimulus.

The initial sources are 1.65 V for Hall outputs and 0 V for pot wipers. Declared
actuation moves a Hall source to 2.2 V; pot movement reaches 3.3 V. These are
electrical fixtures, not fitted magnet/travel/velocity measurements. Decoder
selection follows firmware GPIO activity rather than a precomputed scan table.

SSD1306 commands update real GDDRAM and produce pixel artifacts. UART bytes are
observed after complete 31,250-baud frames. Cortex-M faults and stopped scanning
fail acceptance. A separate `boot` capture checks GPIO modes, alternate functions,
open-drain I2C, SAI flags and clock liveness.

## Baseline sequence

The run includes boot calibration, all 70 melodic keys and 20 drum pads,
release messages, all twelve pots, three program buttons, pitch bend, modulation,
all-channel All Notes Off, recalibration, settings open/edit/close and OLED
changes. It executes the real three-second splash delay and ends after 3.4
virtual seconds. Expect minutes of host time; the log reports both clocks.

Firmware output must include 90 note-on/off pairs, expected pitches/channels,
all pot controllers, program/expressive controls and bounded scan gaps. The
captured MIDI can be inspected independently with `analyze RESULT_DIRECTORY`.

To check the broken Hall path, use `device-batch` with the same inputs and final
arguments `circuits/results/open-hall-first open-hall`. Its individual device
acceptance must fail for the missing HE1 note, while the batch's negative-control
result must pass. The current profile refuses to quietly treat the historical
grounded decoder outputs as a healthy digital board; that revision remains in
the earlier standalone electrical fault suite.

## Results stay separate from design changes

Each capture retains the immutable ELF and hashes, circuit/fixture inputs, RTT,
GPIO/ADC/MIDI traces, display memory, stereo PCM, progress and diagnostics. No
firmware state, timer or PC is patched to shorten calibration, startup or idle.
The only observer write is the standard RTT consumer pointer.

The circuit solver is coupled to firmware; ngspice is its independent reference,
not a second complete SPICE solve for every ARM instruction. ADC loading is the
declared continuous 50 pF fixture, not a fitted switched-capacitor aperture model.
The rail is stiff in this acquisition model; sensor magnetic curves, regulator
thermal behavior, full current consumption, MIDI optocoupler and physical latency
remain uncalibrated. Clock rates and scan gaps are model observations.

Baseline evidence and failed gates are retained in
[the device evidence folder](evidence/2026-11-22-devices/README.md). Proposed
electrical/firmware improvements belong in separate experiments after reviewing
these findings. The engine and emulator remain pinned by immutable revisions.
