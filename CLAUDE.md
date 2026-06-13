# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this project is

Hall-effect MIDI keyboard controller firmware for an Electrosmith Daisy Seed (STM32H750V). The PCB hosts up to 15 analog muxes feeding 100 MT9105ET hall-effect sensors into the Daisy ADC, plus 12 trim potentiometers. Firmware outputs MIDI over USART1 at 31250 baud, reads pots as CC messages, and drives a 128×32 SSD1306 OLED. Keys are laid out in a Wicki-Hayden isomorphic layout. Switches 81–100 are drum pads on MIDI channel 10.

All firmware lives in `keyboard_keyboard/code/`.

## Setup

```sh
rustup target add thumbv7em-none-eabihf
rustup component add llvm-tools-preview
cargo install cargo-binutils
# Install probe-rs / cargo-embed: https://probe.rs/docs/getting-started/installation/
```

## Commands

Run from `keyboard_keyboard/code/`:

```sh
cargo check                  # type-check (no target device needed)
cargo build                  # compile for thumbv7em-none-eabihf
cargo embed                  # flash to connected Daisy Seed (reads Embed.toml)
```

RTT logs stream automatically when `cargo embed` is running. Set `DIAG_LOGGING = true` in `src/main.rs` to enable verbose per-switch ADC and baseline logs.

GDB attach (separate terminal after `cargo embed`):
```sh
arm-none-eabi-gdb target/thumbv7em-none-eabihf/debug/app
# inside gdb:
target remote :1337
```

## Architecture

### RTIC task structure (`src/main.rs`)

The entire application is one RTIC `mod app`. Tasks:

| Task | Trigger | Priority | Role |
|------|---------|----------|------|
| `init` | boot | — | Calibration, peripheral init |
| `timer_handler` | TIM2 (1 kHz) | 15 | ADC scan, switch state machine, pot scan |
| `process_events` | software-spawned | 2 | Dequeue events → MIDI output |
| `audio_handler` | DMA1_STR1 | 8 | Audio passthrough (no-op) |
| `display_init` | software-spawned | 1 | One-shot SSD1306 init |

`timer_handler` is the hot path. It runs every 1 ms, scans all 13 muxes × 8 channels, runs the switch state machine for all 100 switches, and scans 12 pots every 10 ms. Results go into a `heapless::spsc::Queue<(usize, SwitchEvent), 64>` shared with `process_events`.

### Mux scanning

Three select lines (Daisy7/8/9 → `MUX_SELECT_0/1/2`) address one of 8 channels across all muxes simultaneously. AM1–AM9 each have a dedicated ADC pin (Daisy15–23 / A0–A8). AM10–AM13 share ADC pins A9/A10 via a 74HC138 decoder (U1) enabled by Daisy2/3/4 (`ENB_C/B/A`). AM14–AM15 (pots) share A11 via the same decoder and are read via Daisy28 (pad 35 — requires a bodge wire from pad 33 → pad 35 on the Daisy Seed).

`SWITCH_MAP: [(mux_index, channel); 100]` — maps each switch index to its `(mux, channel)` location. `SWITCH_TO_NOTE` maps switch index → MIDI note. `HE_NUM` maps switch index → HE sensor number (for log messages).

### Switch state machine

Each switch runs a three-state machine: `Idle → FirstActuated → FullyActuated`. A press requires crossing `FIRST_DELTA` (150 ADC counts above baseline), then `SECOND_DELTA` (250 counts). Velocity is derived from elapsed ms between the two thresholds within a `VELOCITY_WINDOW_MS` (80 ms) window. Release is detected when delta falls below `RELEASE_DELTA` (100 counts). All transitions require `DEBOUNCE_TICKS` (3) consecutive confirmations. Detection uses `abs_diff` so either magnet polarity works.

Each switch also has a 4-sample ring-buffer averaging filter (`ChannelFilter`) primed with the boot calibration baseline.

### Boot calibration

`init` collects 64 ADC samples per (mux, channel) slot before starting the timer, averages them into `baselines[switch_idx]`, and primes each filter with the result. This baseline is the resting ADC value with no key pressed.

### `src/midi_sender.rs`

Thin wrapper around `Tx<USART1>`. Encodes Note On/Off, Control Change, and Pitch Bend as 3-byte MIDI messages. Blocking byte-by-byte send (~320 µs/byte). Channel is set per-message by `process_events` (channel 0 for melody keys, channel 9 for drum pads).

## Hardware notes

See `keyboard_keyboard/code/HARDWARE.md` for full pin mapping and mux architecture. KiCad schematic: `keyboard_keyboard/kicad/keyboard_keyboard.kicad_sch`.

**PCB errata:** U1 (74HC138D) pin 5 (`E2~`, active-low enable) is incorrectly tied to +3.3V instead of GND — this permanently disables the decoder. Fix: solder bridge pad 5 → pad 4 (GND). AM10–AM13 (switches HE71–HE100) will not work until this is corrected.

**Daisy Seed errata:** Pad 33 (D26) has no ADC. AM14/AM15 pot reads require a wire from pad 33 → pad 35 (D28 / A11 / ADC11). Leave D26 unconfigured in firmware.

## Expanding to HE71–HE100

The `SWITCH_MAP` already includes entries for all 100 switches. To make them functional:
1. Apply the U1 bodge wire (see PCB errata above)
2. The decoder enable sequencing is already implemented in firmware
3. `NUM_MUXES = 13` and `NUM_SWITCHES = 100` are already set correctly
