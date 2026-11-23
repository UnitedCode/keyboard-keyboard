# Complete virtual-device baseline, 2026-11-22

The unchanged keyboard main firmware was built from source revision
`ac4667ed3530993977d96eac43488c10a0f2ed93`. Its immutable ELF SHA256 is
`e1416750d97d39086ea4f6fa9cb9987d1fda0965b09c844c19ca9b3d645ab3f1`.
The existing current schematic and reference electrical fixture were connected
to a Rev4/AK4556 Virtual Daisy pinned to
`f7892c73b36960d729e6c3bf8bbd0ce3e65498a0`.

| Executed test | Result |
| --- | --- |
| Unmodified 3.4-second firmware run, including real splash delay | Completed 753,116,589 scheduler steps in 1,366.091 host seconds |
| All 70 melodic keys and 20 drums | 90 note-ons, 90 note-offs, expected pitches/channels, no unmatched releases or held notes |
| All twelve pots | Correct inverted endpoint: 0 V initially produces CC127; the 3.3 V stimulus produces CC0 |
| Programs, pitch bend, modulation, All Notes Off, recalibration and settings UI | Expected observed output; post-splash OLED changes |
| Scan liveness | 3,257 sweeps; maximum gap 1,004,913 ns, within the declared 2 ms model gate |
| Separate GPIO/clock audit | PASS: correct pin modes/AF/open-drain I2C, no Cortex-M/SAI faults, sustained audio clock |
| Open Hall source | Expected missing HE1 note detected; negative control PASS |

The initial grader incorrectly expected CC127 after raising the wiper voltage.
The original report in `complete-scan/` is preserved with those twelve failed
expectations. `recheck.txt` hashes that report and the actual MIDI trace, verifies
that no other original gate failed, and independently verifies the correct CC0
outputs and note balance. It reports `ARM_DEVICE_OBSERVATIONS=PASS` without
re-executing or modifying the CPU, circuit or capture. Future runs use the
corrected polarity expectation. This is an oracle correction, not waived input
coverage or a firmware change.

The first attempted attachment reached an unimplemented `UQADD16` instruction;
its failure report and stack are retained. [Virtual Daisy PR #12](https://github.com/UnitedCode/virtual-daisy/pull/12)
adds both packed halfword operations with actual Thumb regression tests. The
complete run then passed those instruction paths without patching the ELF.

The runtime consumes actual firmware GPIO changes and returns evolving RC/mux
voltages at ADC reads. RC behavior was checked independently against ngspice.
Five consumer host tests, broader engine/runtime checks, formatting and clippy
passed. Full traces and immutable inputs are preserved in the local archive;
small reports, MIDI output and observed display renders are committed here.

The rail, Hall output impedances, ADC capacitance and source waveforms are
declared fixtures. This does not identify the assembled board's revision,
regulator capacity, thermal behavior, magnetic response or physical latency.
No design, production firmware or manufacturing export was changed. Reset,
power-loss retention, extended soak and physical device acceptance remain unrun.
