# Filter noise / settling comparison

**Keep the original 4.7 nF design as the default.** Smaller capacitors improved
the previous 1 kΩ-source stress test, but that is not proof the real Hall driver
has that impedance or needs a capacitor change.

The MagnTek table gives 2 Ω typical / 4 Ω maximum output resistance at its stated
2.5 V DC output test point and limited output current. A new, separate **4 Ω
fixture** gives zero original-filter scan errors above 1 mV. The additional
current-limited ngspice charge test also passes all three capacitor values after
4.5 µs. Neither test establishes the assembled device's dynamic output impedance,
sensor feedback response or ADC aperture. The earlier 1 kΩ results remain frozen.

## Noise method

- Compare original / 1 nF / 470 pF graphs; validate each exact allowed delta first.
- Apply post-sensor sine interference through a continuous selected-channel
  two-node RC model. Cross-check it against ngspice; include burst transients.
- Sweep source impedance (4/100/1000/3000 Ω), capacitance tolerance, frequency,
  phase and amplitude, with separate localized-noise and correlated-reference
  hypotheses. The 4 Ω slice retains the datasheet test-condition caveat.
- Replay both an ideal 1 ms clock and an actual HE1 scan clock cropped after the
  first 200 ms boot-calibration window. Runtime gaps outside 0.5–2 ms are rejected.
- Execute unchanged production `ChannelFilter`, `SwitchState` and calibration
  modules. Use the firmware's scan-count tick, not rounded physical timestamps.
  The inline baseline-tracking and analog-control rules are host mirrors, with
  source fingerprints; they are not new full-ARM/UART observations.

The full sweep has **21,504 cases** plus **96 clean-signal controls**. Inspect
`comparison/report.txt`, `noise-cases.csv`, `analog-controls.csv` and
`analog-summary.txt`. At or below 50 mV peak in this grid there are no false/missing
HE1 note pairs, but some analog-control cases still generate CC1 or bend changes.
Thus a note-only pass is not a quiet-instrument gate. The CC/bend monitor uses
representative HE1 timing, a quiet counterpart for localized pickup, and identical
simultaneous counterparts for ideal common ripple; actual cross-channel phases
are not modeled.

A matched stress case uses an ideal 1 ms clock, Rs=100 Ω, −10% capacitance,
400 kHz, phase π/2 and 400 mV peak localized interference. The original filter
produces no false note, while 470 pF does. This is an injected fault case,
**not a measured noise amplitude** or a failure probability. Correlated input and
ADC-reference noise can change rankings; reference decoupling is not modeled.

Limitations include real ADC aperture/sample-and-hold, switched-mux interference,
Hall intrinsic noise spectra, wiring/EMI and temperature. The actual source/noise
measurements should choose the next design, rather than optimizing a provisional
fixture. The standalone supply and lower-current sensor studies retain 4.7 nF.

Source: [MagnTek MT910X electrical table](https://xonstorage.z8.web.core.windows.net/pdf/magntek_mt9105et_apr22_xonlink.pdf).
[Independent verification](../../verify-power-noise.md) gives exact commands,
expected controls and an optional way to regenerate the clock from an ARM capture.
