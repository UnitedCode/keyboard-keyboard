# Complete keyboard firmware regression

This runs the same immutable production ARM ELF as the original complete-device
baseline against the independent 470 pF candidate for **3.4 seconds of virtual
time**. No firmware timers, instruction pointer or private application state are
patched. The real three-second splash and later settings interactions execute.

Acceptance checks cover 70 melodic keys, 20 drum pads, all twelve inverted pots,
program changes, pitch bend, modulation, all-channel notes-off, recalibration,
settings-screen interaction, balanced note releases, scan deadlines, OLED, GPIO
configuration, sustained audio clock and Cortex-M faults. The report records the
result, exact firmware/circuit/fixture hashes, scan gap and model revision.
`analysis.txt` independently summarizes the recorded MIDI and PCM observations.

The event-driven acquisition model uses the capacitors from the candidate graph,
with the same provisional source/mux/ADC assumptions as the original. Hall
outputs are declared voltages; magnetic travel, analog noise, temperature,
regulator behavior and physical latency are not calibrated by this test.
Functional success does not waive the noise/source-impedance conditions in the
parent experiment README or fix the unresolved hot-room/power concern.

Compact report, MIDI, stimuli, GPIO configuration, display and runner fingerprints
are committed here. Full raw ADC/GPIO/PCM, ELF, OLED RAM and flash capture are
preserved locally under the Synthphone repository's
`audio-lab-results/circuit-improvements-2026-11-22/keyboard/filter-470p-complete/`.

Result: **ARM_DEVICE=PASS**, 90 note-on/off pairs, zero unmatched releases or held
notes, all twelve pot extremes observed. The run made 3,257 scans and 349,284 ADC
observations; maximum modeled scan gap was 1,004,913 ns (below its 2 ms gate).
This does not establish measured physical latency or noise performance.
