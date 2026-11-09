# Velocity sensing simulation

Synthetic inputs; shared production Rust filter/state machine. No measured keyboard traces or player acceptance. Sampling is 1 kHz; jitter changes physical sample times while firmware still counts 1 ms ticks. CPU scheduling, mux settling, electrical crosstalk and temperature are not emulated.

## Controlled response

600 ADC counts full travel; linear magnetic transfer; no noise/jitter; sixteen scan alignments per duration. Values are MIDI velocity ranges. Curve-only changes the historical mapping; filtered/raw crossings interpolate before debounce. All variants retain the four-sample trigger filter and three-scan confirmation.

| Full travel (ms) | Legacy | Curve only | Four-sample crossings | Raw crossings | Two-sample crossings |
|---:|---:|---:|---:|---:|---:|
| 2 | 124–124 | 64–64 | 117–118 | 124–126 | 122–126 |
| 4 | 124–124 | 64–64 | 114–115 | 118–118 | 117–118 |
| 8 | 124–124 | 64–64 | 102–103 | 103–103 | 103–103 |
| 16 | 124–124 | 64–64 | 72–72 | 72–72 | 72–72 |
| 32 | 119–121 | 55–58 | 57–58 | 57–58 | 57–58 |
| 64 | 111–113 | 40–43 | 41–42 | 41–42 | 41–42 |
| 128 | 94–95 | 7–10 | 9–10 | 9–10 | 9–10 |
| 256 | 61–62 | 1–1 | 1–1 | 1–1 | 1–1 |

## Stress envelope

Eight travel durations × four spans (350/600/1000/1500 ADC counts) × three magnetic exponents (1/2/3) × three peak noise amplitudes (0/4/12 counts) × both polarities × three alternating timing jitters (0/±0.2/±0.4 ms) × eight scan alignments, for each algorithm. The deterministic noise sequence is shared across comparisons; these are specified stress cases, not a statistical model of real keyboards.

| Algorithm | Presses | Lifecycle failures | Worst transit error (µs) | Worst clean error (µs) | Worst alignment velocity spread |
|---|---:|---:|---:|---:|---:|
| legacy | 13824 | 0 | 6142.9 | 2866.7 | 5 |
| curve_only | 13824 | 0 | 6142.9 | 2866.7 | 6 |
| filtered_timing | 13824 | 0 | 6142.9 | 877.1 | 10 |
| raw_timing | 13824 | 0 | 5174.1 | 357.1 | 26 |
| two_sample_timing | 13824 | 0 | 6585.9 | 419.7 | 10 |
| endpoint_calibrated | 13824 | 0 | 6876.9 | 640.7 | 11 |

Transit error is against exact noiseless crossings of the specified ADC thresholds, not an assumed MIDI velocity. Velocity spread is across scan alignments for a fixed condition. Endpoint scaling cannot linearize the magnetic response.

Provisional response anchors: 250 µs → 127, 3000 µs → 64, 24000 µs → 1, piecewise linear interpolation. They spread the controlled envelope; simulations cannot establish the player's soft/medium/hard timing distribution. Existing trigger thresholds stay 100/150/250 counts. Endpoint calibration is opt-in and tested separately.
