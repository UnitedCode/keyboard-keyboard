# Experimental USB MIDI 2.0 + DIN branch

Branch: `codex/usb-midi2-dual-output`. This is a firmware experiment for the
existing Daisy Seed connector; no PCB changes and no merge into main are part
of this experiment. Physical USB enumeration and musical use are not yet verified.

## Connections and screen

Use the **micro-USB connector on the Seed itself** to connect to a computer.
This uses PA11/PA12 and USB2/OTG_FS. Carrier pads 36/37 and J6/J7 are not used
by this implementation. Existing USART1 TX/RX and DIN connections stay MIDI 1.0.
The computer is the USB host; plugging two USB devices together is not supported.

Default output is **BOTH**. Hold HE74 to open settings. HE77/HE79 select the
previous/next item; HE78/HE80 decrease/increase its value. Select **MIDI OUTPUT**
and choose **DIN**, **USB**, or **BOTH**. Release HE74 to close settings.
Settings are held in RAM and reset at power-on, as with the existing menu.

The main-screen footer shows e.g. `BOTH U:M2`:

| Label | Meaning |
| --- | --- |
| DIN / USB / BOTH | Selected destinations for keyboard performance |
| U:WAIT | USB not configured, disconnected, or suspended |
| U:M1 | Legacy USB MIDI 1.0 selected by the host |
| U:M2 | USB UMP transport using MIDI 2.0 protocol |
| U:UMP1 | USB UMP transport using MIDI 1.0 protocol |
| ! | A transport queue overflow occurred since boot; recovery was scheduled |

USB stays enumerated for discovery even with DIN-only selected; performance is
routed according to the menu. The computer chooses the USB alternate setting
and UMP protocol. DIN has no plug-detection or capability handshake: the display
reports routing, not proof that a DIN receiver is present. Receiving the same
performance through both inputs in one DAW can double notes; select one there.

Changing output routes clears queued performance and schedules CC123 All Notes
Off on old/new destinations. Opening/closing settings also retains the existing
All Notes Off behavior. Select routing with keys released, then play again.
Held notes are not retriggered automatically after changing routes or reconnecting.

## Implemented

- USB AudioControl + MIDIStreaming descriptors: legacy alternate 0 and UMP
  alternate 1, one bidirectional group and one static keyboard function block.
- Group Terminal Block descriptor request, UMP endpoint/function discovery,
  endpoint/device identity and name, and a stable STM32 UID-based instance ID.
- UMP Stream Configuration requests for MIDI 1.0/2.0, and MIDI 1.0 USB fallback.
- Note on/off, CC, pitch bend, program changes, and bend-range RPN. Native MIDI 2.0
  registered-controller encoding is used for bend range rather than sending
  MIDI 1.0 RPN selector CCs as native MIDI 2.0 CC messages.
- Independent bounded DIN/USB queues. DIN TXE interrupts replace blocking UART
  writes; a blocked USB endpoint does not hold up DIN, or vice versa. A full
  destination queue discards its backlog and schedules All Notes Off on 16 channels.
- USB reset/reconnect/protocol changes discard stale performance and start with
  All Notes Off. Unknown incoming performance is ignored, not echoed to DIN.
- HSI48 clock recovery from the built-in connector's USB2 SOF and a bounded
  STM32 adapter for bulk endpoint DATA0/FIFO reset during SET_INTERFACE.

Current sensors/scan logic still generate 7-bit velocity/CC and 14-bit bend.
MIDI 2.0 conversion preserves those values; it does **not** add measured precision,
per-key aftertouch, MPE, or per-note pitch. MIDI-CI, profiles, property exchange,
JR timestamps, USB host mode, and a USB-to-DIN bridge are not implemented or
advertised as supported.

**Private test identity:** USB VID/PID `1209:0001` is pid.codes' shared Test PID.
It is not universally unique and must not be used for redistributed, sold, or
manufactured devices. Obtain an assigned identity before a public hardware release.
The UMP manufacturer ID is experimental/noncommercial `0x7D`.

## Verification

From the repository root:

```sh
cargo test --manifest-path keyboard_keyboard/midi-transport/Cargo.toml --locked
cargo clippy --manifest-path keyboard_keyboard/midi-transport/Cargo.toml --locked --all-targets -- -D warnings
cargo fmt --manifest-path keyboard_keyboard/midi-transport/Cargo.toml -- --check
cargo run --manifest-path keyboard_keyboard/midi-transport/Cargo.toml --example screen_preview -- /tmp/keyboard-screen-preview
```

Run firmware checks from `keyboard_keyboard/code`:

```sh
cargo build --release --locked
cargo clippy --locked -- -D warnings
cargo fmt -- --check
```

The simulated USB host exercises the actual `usb-device` stack and the same
USB class/output queues compiled into firmware, through control requests and
bulk packets. It is not an electrical USB PHY or a computer's USB class driver.
The screen preview executes the real firmware rendering/menu code on a framebuffer.

The unchanged ARM ELF was attempted on the virtual Seed. That run **cannot pass
this revision**: the fixture rejects USART1 CR1 `0x2000008D` (TXE interrupt enable),
and its generic USB model/CRS/DBGMCU coverage produces unsupported-access warnings.
The existing keyboard fixture supports polling UART only and does not drive a
USB host. Do not patch guest firmware, suppress those warnings, or report an
emulator timing pass for this image. Historical recorded regression tests remain
useful for their original firmware, not proof about this branch.

Before calling this hardware-ready, test enumeration/fallback on actual target
OS/DAWs, DIN and USB together under bursts, menu routing while notes/sustain are
active, reconnect/suspend and repeated alternate settings, UART throughput/scan
jitter, USB clock recovery, and warm ambient behavior. USB power/current margin
for the sensor-heavy carrier still needs physical verification; the USB descriptor
requests the USB 2.0 maximum 500 mA, not proof that the whole board fits that budget.
A visible ST-Link was not identified as this keyboard, so no device was flashed.

## Primary references

- [USB MIDI 2.0 class specification](https://www.usb.org/sites/default/files/USB%20MIDI%20v2_0.pdf)
- [UMP and MIDI 2.0 protocol v1.1.1](https://amei.or.jp/midistandardcommittee/MIDI2.0/MIDI2.0-DOCS/M2-104-UM_v1-1-1_UMP_and_MIDI_2-0_Protocol_Specification.pdf)
- [USB MIDI device design](https://midi.org/building-a-usb-midi-2-0-device-part-1)
- [UMP discovery](https://midi.org/building-a-usb-midi-2-0-device-part-2)
- [pid.codes Test PID conditions](https://pid.codes/1209/0001/)
