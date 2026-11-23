# Short functional countercheck

The unchanged keyboard ARM ELF runs for 260 ms with this independent 1 nF
candidate. HE1 rises at 170 ms and releases at 220 ms. The firmware detects its
note; the captured MIDI includes the matching release. The startup marker,
continuous ADC scanning, OLED, GPIO configuration, audio clock and CPU fault
gates pass. This is not the post-splash settings or all-controls acceptance run.

The analog comparison still has four readings outside 1 mV. A functional note
pass does not change that incomplete analog result. Full raw files are archived
under `audio-lab-results/circuit-improvements-2026-11-22/keyboard/filter-step/` in
the Synthphone repository. Compact observations are committed here.
