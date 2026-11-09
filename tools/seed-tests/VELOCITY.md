# Velocity sensing investigation

The reported symptom was that soft and hard presses sounded too similar.
The simulation reproduces a concrete software mechanism: with 600 ADC counts
of linear full travel, 2/4/8/16 ms strokes all produce velocity 124 in the original
firmware. Both thresholds are confirmed in separate three-scan phases; the
second phase imposes a minimum measured interval of 3 ms. An 80 ms linear
response then maps most ordinary intervals close to maximum velocity.

The final implementation keeps the existing four-sample trigger smoothing,
three consecutive confirmations and trigger/release thresholds. A separate
two-sample measurement filter records interpolated threshold crossings before
confirmation. This separates measurement from note validation. A provisional
three-anchor response spreads the useful synthetic interval range. Calibration
entries can override thresholds and timing anchors per key; boot recalibration
still measures resting baselines only and does not learn travel or playing style.

The raw, four-sample and two-sample alternatives were all tested. Raw crossings
gave better clean timing but up to 26 velocity steps of alignment variation in
the specified stress envelope. Two-sample measurement reduced this to 10 while
retaining more fast-stroke separation than four samples. It is a compromise:
its worst transit error under stress is greater than raw measurement. The full
results and limitations are in [the recorded summary](evidence/velocity/summary.md).

Two additional regressions are covered: millisecond-counter rollover previously
used saturating subtraction, and the original FirstActuated phase shared a
debounce counter between release and second-threshold conditions. Opposite
conditions now reset each other's confirmation.

## Reproduce

From `tools/seed-tests`, use your host Rust target if an ancestor Cargo config
selects an embedded target (for this Mac, `--target aarch64-apple-darwin`):

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo run --release --locked --bin velocity-sim
cargo run --release --locked --bin velocity-sim -- sweep /path/to/fresh-results
```

`sweep` requires a fresh directory and writes `summary.md` and the complete
per-condition `matrix.tsv`. It exits with failure if any condition loses or
duplicates a Note On/Off. The six approaches each see the same 13,824 presses.
Reported timing error is against the mathematical noiseless ADC crossings,
not against an invented ground-truth musical velocity. Scan alignment is swept;
noise is one repeatable bounded sequence, not an empirical sensor distribution.

The tool compiles the production filter, configuration and state machine directly
by path. The frozen negative control is the original `switch.rs` from commit
`f308a9a9d1fdfa699a41b3a5f4ba377c88997d50`, SHA256
`c4a333d615439dad862760b6521e85dd2b76285327f16585ba91b673bf09e807`.
Its original 80 ms window and filter shift are supplied in the host wrapper.
The curve-only comparison reads the old elapsed interval without changing its
state transitions. These host tests do not execute the whole ARM firmware.
The previous virtual Seed recordings remain historical evidence; the ignored
full-firmware integration test has not been rerun for this change.

## Calibration from real measurements

For each representative key, measure rest, fully depressed ADC and peak resting
noise in ADC counts. Leave all musical keys untouched during boot or snapshot
baseline calibration. A held key becomes a false resting reference; snapshot
recalibration has not been redesigned here. To check an endpoint calibration:

```sh
cargo run --release --locked --bin velocity-sim -- calibrate 2000 2600 12
```

Those numbers are an example, not measurements of this keyboard. The helper
places release/first/second thresholds at 1/6, 1/4 and 5/12 of electrical travel,
preserving the original values for a 600-count key. It rejects out-of-range
endpoints, zero travel and release thresholds at or below four times peak noise.
Copy validated per-key values into `SWITCH_CALIBRATIONS` in the firmware's
`src/velocity.rs`. Entries currently retain the original thresholds. The table
is compiled into firmware; no persistence or automatic endpoint learning is claimed.

Endpoint scaling reduced clean linear span bias to at most two velocity steps
in the tested 350–1500 count range. It does not correct a nonlinear magnetic
distance curve; identical physical key speeds can still produce different
electrical transit times. Per-key timing anchors should be based on repeated
soft, medium and hard strokes, not just rest/bottom measurements.

Recorded single-key ADC data can be replayed without changing firmware code:

```sh
cargo run --release --locked --bin velocity-sim -- replay key.csv 2000
```

The argument is the measured resting baseline. CSV header is `tick_ms,adc`;
rows are consecutive 1 ms ticks and raw 12-bit ADC samples, with resting samples
before the first stroke. The tool rejects missing/nonconsecutive ticks rather
than inventing samples. It prints Note On tick, measured transit microseconds,
velocity and release ticks. The baseline stays fixed during replay; the complete
firmware's baseline tracker, mux timing, UART and task scheduling are not run.

## Physical acceptance still needed

There were no recorded keyboard traces available in this investigation. The
default 250/3000/24000 µs timing anchors (velocities 127/64/1) spread the synthetic
test envelope; they are not a measured player calibration. Interpolation estimates
subscan crossing times from ADC slope; it does not turn a 1 kHz scanner into a
microsecond-resolution sensor. Actual timer overruns remain a source of error
because firmware time advances once per interrupt.

Confirm the keyboard's identity before flashing. A connected ST-Link probe does
not distinguish this controller from the adjacent synthphone project. Build the
release ELF, flash the identified keyboard, then compare repeated soft/medium/hard
notes and rapid repeats across its physical keys and listening instrument.
Measure scan cadence and resting noise under chords, supply and temperature
changes. Set per-key travel and response anchors from those recordings and
replay them before accepting a final calibration. A synthetic pass establishes
software behavior within this envelope, not expressive feel on the instrument.
