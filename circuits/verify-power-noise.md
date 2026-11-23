# Independently verify the power and filter experiments

Use this report's Git revision in a clean checkout/worktree. Fetch the repo and
create a worktree under `~/Documents/GitHub` at the report's linked commit; leave
your active firmware checkout intact. Check `df -h` first and retain 500 GB free.
Requires Rust, `shasum`, and native ngspice (`brew install ngspice` on macOS).
The engine and Virtual Daisy are pinned in Cargo files. No command below flashes
hardware. Use a new output directory each time; old results are never overwritten.

From the Keyboard Keyboard repository, build once and run the small checks:

```sh
export CARGO_TARGET_DIR="$HOME/Documents/GitHub/synth-phone-e-v2-rust/target"
export CARGO_INCREMENTAL=0
LAB_HOST=$(rustc -vV | sed -n 's/^host: //p')
mkdir -p circuits/results
cargo test --locked --manifest-path tools/circuit-lab/Cargo.toml --target "$LAB_HOST" --all-features
cargo build --release --locked --manifest-path tools/circuit-lab/Cargo.toml --target "$LAB_HOST" --features device
LAB="$CARGO_TARGET_DIR/$LAB_HOST/release/keyboard-circuit-lab"
```

The following use the unchanged original graph as the control. The candidate
validators compare every component/pin and reject unrelated changes.

```sh
"$LAB" noise-study circuits/baselines/keyboard-current.net \
  circuits/experiments/filter-1n/candidate.net circuits/experiments/filter-470p/candidate.net \
  circuits/fixtures/reference.fixture circuits/experiments/filter-noise/clock.csv \
  circuits/results/verify-noise-1
"$LAB" power-kernel circuits/results/verify-power-kernel-1
"$LAB" hall-kernel circuits/results/verify-hall-kernel-1
"$LAB" power-study keyboard-drv5053ra circuits/baselines/keyboard-current.net \
  circuits/experiments/low-power-hall/candidate.net circuits/fixtures/power-scenarios.fixture \
  circuits/results/verify-hall-power-1
"$LAB" power-study keyboard-guarded-offload circuits/baselines/keyboard-current.net \
  circuits/experiments/power-offload/candidate.net circuits/fixtures/power-scenarios.fixture \
  circuits/results/verify-offload-1
"$LAB" verify keyboard-current circuits/baselines/keyboard-current.net \
  circuits/experiments/filter-noise/datasheet-4ohm.fixture circuits/results/verify-4ohm-1
"$LAB" charge-study circuits/baselines/keyboard-current.net \
  circuits/experiments/filter-1n/candidate.net circuits/experiments/filter-470p/candidate.net \
  circuits/experiments/filter-noise/datasheet-4ohm.fixture circuits/results/verify-charge-1
```

Read the reports **and acceptance decisions**, not just exit status:

| Check | Expected result |
|---|---|
| Noise | 21,504 cases, 96 clean controls; zero HE1 note failures at ≤50 mV in this grid; strong interference counterexample retained |
| Analog controls | `analog-summary.txt` reports some CC1/bend changes; no-note pass does not mean quiet controls |
| Power-loss firmware kernels | Execution PASS, **8 failed fault cases**, allocation rejected |
| Hall firmware kernels | 144 cases, 36 strong-field passes, 60 weak-field failures; conditional magnetic qualification |
| Hall power | 1.98→0.891 W typical and 2.31→1.188 W hot-current scenarios for 100 sensors; whole-device limits remain conditional |
| Offload | `REJECTED_FOR_PROMOTION`; rail relocation does not remove sensor heat |
| 4 Ω acquisition / charge | Zero original-filter scan errors above 1 mV; all three capacitor charge gates pass at 4.5 µs |

Prove a test can fail: repeat `noise-study`, replacing its **1 nF candidate**
argument with `circuits/baselines/keyboard-current.net`, and choose a fresh output
folder. It must exit nonzero with `NOISE_STUDY=FAIL`. Likewise, passing the original
as the offload candidate must fail isolation. Do not alter a baseline for this.

The committed scan clock came from a previous complete ARM capture, cropped
after 200 ms to exclude boot-calibration scans. `clock.sources.txt` fingerprints
that capture. You can reproduce the clock from your own passing capture:

```sh
"$LAB" noise-clock /absolute/path/to/complete-arm-capture circuits/results/my-clock.csv
```

The noise test is a continuous selected-channel RC model and native execution of
production switch/filter modules, with host mirrors of inline baseline/CC rules.
ngspice cross-checks the analog response. The 4 Ω case is conditional on the
manufacturer's **DC** output test point, not a measured dynamic impedance. The
power model uses declared loads/efficiencies/K/W; changing cooling and circuit
at once would confound the comparison. Hand-check sensor heat with `100×3.3×I`.
Neither a virtual hour nor an above-budget thermal cell predicts actual time to
failure; shutdown/foldback/airflow detail is unmodeled.

## Independently check schematic connectivity

KiCad's CLI can export either full candidate drawing. For example:

```sh
kicad-cli sch export netlist --format kicadxml -o circuits/results/verify-hall.xml \
  circuits/experiments/low-power-hall/keyboard-drv5053ra.kicad_sch
"$LAB" check-schematic keyboard-drv5053ra circuits/experiments/low-power-hall/candidate.net \
  circuits/results/verify-hall.xml circuits/experiments/low-power-hall/keyboard-drv5053ra.kicad_sch \
  circuits/results/verify-hall-drawing.txt
```

Expected: `SCHEMATIC_CONNECTIVITY=PASS`. The separate power-allocation drawing
uses functional terminals, not production-qualified symbols. Neither connectivity
check is a manufacturing ERC/DRC or footprint approval. On macOS `kicad-cli` may
be `/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli`.

## Optional complete ARM reproduction

Follow [the complete-device guide](virtual-devices.md) to build the original
firmware ELF **from `keyboard_keyboard/code`**, using that firmware's existing
target cache. Do not run the embedded app with `cargo run` (that may flash).
Record its SHA256. Then use the host lab binary built above:

```sh
"$LAB" device keyboard-drv5053ra circuits/experiments/low-power-hall/candidate.net \
  circuits/fixtures/reference.fixture /absolute/path/to/keyboard-firmware.elf rev4 \
  circuits/results/verify-hall-arm-1 baseline
```

This executes the complete 3.4 s stimulus and may take about 20 minutes. Compare
`report.txt`, MIDI capture, display and fingerprints with the retained
[device evidence](experiments/low-power-hall/device/README.md). It applies a
declared 1.02 V rest, nominal −45 mV/mT and +17.4603 mT stroke, with conservative
clipping. Real magnets, output current limits/noise/delay and thermal drift are
not implemented in this ARM carrier. A different rebuilt ELF gets its own
fingerprint and may change timing; never claim it is the original immutable ELF.

## What still needs physical evidence

On the existing device, identify the Seed revision and fitted Hall part from
markings/order history. Record USB/source voltage and current, rail voltage at
the far sensors, ambient and component temperatures during the same sustained
key/pot sequence. Compare temperature rise only at matched load/ambient. Use
scope captures at Hall output, selected ADC input and ADC reference to distinguish
pickup, settling and supply ripple. Compare spectra with the same probe/ground
setup; avoid turning a probe loop into the measured noise source.

For the Hall candidate, verify magnet endpoints/polarity, calibrated rest over
temperature, velocity, output loading/noise and mechanical footprint before any
physical redesign. The production board and original firmware remain unchanged;
the user owns PCB design/fabrication. The rejected power allocation is not a build
recommendation. See also the [Synthphone receiver study](https://github.com/UnitedCode/synth-phone-e-v2-rust/blob/main/circuits/verify-power-thermal.md).
