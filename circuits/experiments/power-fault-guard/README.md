# Sensor power-fault guard — separate virtual candidate

This refines the rejected [split-rail allocation](../power-offload/README.md).
Both that experiment and the original `keyboard-current.net` remain frozen.
The candidate keeps every original Hall part and 4.7 nF filter; it does not stack
the low-current Hall or capacitor experiments. No production firmware, original
schematic, PCB or manufacturing output is changed.

## Circuit / firmware contract

Two TPS3808G33DBVR monitors replace the unselected `PG1`: one senses Seed 3.3 V,
the other the sensor rail. Both are powered by Seed 3.3 V. Their open-drain outputs
share `POWER_VALID`, the existing 10 kΩ pullup, TMUX enables, and the formerly
unused **Daisy D10 / PB5 / A1 pad 11**. Each has 100 nF supply bypass; CT is open.
The functional KiCad allocation and exact original-pin changes are reviewable
in `power-guard-allocation.kicad_sch` and the TSVs. Symbols remain functional
terminals, not production-qualified library/footprint definitions.

The isolated Rust firmware is under `firmware/`. It shares unchanged production
switch/filter/calibration/display modules by explicit path. Only its entry point,
MIDI sender and new guard differ. Its Cargo.lock retains the original library
commits. The ordinary firmware target is unchanged and is the failure control.

1. Record power validity before, during and after boot calibration. Invalid boot
   samples leave the candidate locked out; bad-rail anchors are never used for notes.
2. Check validity before and after each ADC scan. A detected fault latches lockout,
   clears pending events, resets switch state, and cancels recalibration.
3. Send explicit NoteOff for tracked pitches, sustain off, All Sound Off, All Notes
   Off and neutral bend/modulation on all channels, outside the scan queue lock.
4. Wait for UART completion after each MIDI message. Clearing a software queue alone left
   several future notes in the transmit FIFO; the retained failed brownout proves it.
5. Restoring power never rearms the candidate. Use a **full Seed power cycle with
   stable rails and released keys**. Warm software reset remains unqualified below.

MIDI has finite wire time: a 90-pitch panic is 510 bytes / 163.2 ms at 31,250 baud,
before software scheduling overhead. The 14-key ARM fault gate allows a 3 ms
detection / already-committed-message window, records
those events separately, and requires panic completion within 170 ms. It does
not claim instantaneous silence or control over an unpowered/failed receiver.

## Native results and hardware limits

The exact original comparison changes **8 failures to 0** across ten cases.
The broader matrix has **3,600 guarded cases, zero failures**, and 1,788 failing
unguarded controls. It sweeps five rest voltages, idle/held keys, six phases,
25 µs–20 ms dips, threshold/release corners and ADC values 0/2048/4095. These
are scenario counts, not failure probabilities. Production kernels execute on
the host; full ARM captures are separate evidence.

Startup/ramp/chatter and Seed-undervoltage cases are recorded separately.
`startup-margins.csv` also retains an inconvenient result: the module's −4%
output, **3.168 V**, fails to arm in one supervisor corner. The worst modeled
rising threshold is about **3.194 V**. The candidate requires at least 3.2 V at
the sense point to guarantee arming across this envelope; rail tolerance/part
selection must be resolved before physical promotion. Lowering the trip point
below the Hall family's 3.0 V operating minimum is not an acceptable workaround.

The supervisor model uses a declared 20 µs assertion, 12/28 ms release, ±1.5%
threshold corners and 1–2.5% hysteresis. These timing assumptions are conditioned
on datasheet overdrive/load, not a transistor model or a guarantee for arbitrary
slow dips. Native ngspice checks the pullup's DC high/sink margins with declared
5 µA total leakage and 0.4 V sink; it does not simulate a vendor supervisor.

## Complete ARM evidence

See `device/` for final captures and `retained-failures/` for failed predecessors
and the original-firmware control. Each keeps its original report, MIDI, supervisor
edges, source fingerprints and display; full raw captures/ELFs are archived locally.
The ARM runner uses actual GPIO-selected RC acquisition, paced UART and OLED, with
an explicit ideal mid-bias isolation boundary. Added TMUX/bias loading, regulator
dynamics, real ADC aperture and Hall magnet/noise behavior remain unmodeled here.
The supervisor comes from declared rail waveforms; no private firmware state,
startup timer or PC is patched.

**Warm reset is a retained failure**, not a passing negative control: after the
fault was safely silenced, the virtual SYSRESETREQ run stopped in flash startup
with an IS25LP instruction-lane mismatch. Firmware/model attribution is unresolved.
Do not advertise warm-reset recovery. The separate fresh cold-start case checks
normal playing; it is not a continuous physical power-cycle measurement.

**Acceptance: virtual fault-logic candidate only.** Physical promotion remains
blocked by startup rail margin, warm reset, partial-power/overvoltage behavior,
unmeasured signal loading and actual supervisor/receiver timing. Complete loss of
Seed power cannot be repaired by firmware transmitting a panic after it is dead.

Sources: [TI TPS3808 pin/electrical/timing tables](https://www.ti.com/lit/ds/symlink/tps3808.pdf),
[MagnTek operating range](https://xonstorage.z8.web.core.windows.net/pdf/magntek_mt9105et_apr22_xonlink.pdf).
[Independent verification](verify.md) includes exact commands and expected failures.
