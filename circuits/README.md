# Keyboard Keyboard circuit lab

For production firmware connected to the circuit, display and MIDI output,
use the [complete virtual-device guide](virtual-devices.md).

Reproduce the existing board before changing its design:

```sh
LAB_HOST=$(rustc -vV | sed -n 's/^host: //p')
export CARGO_TARGET_DIR="$PWD/keyboard_keyboard/code/target"
export CARGO_INCREMENTAL=0
mkdir -p circuits/results
cargo run --locked --manifest-path tools/circuit-lab/Cargo.toml --target "$LAB_HOST" -- \
  verify keyboard-current circuits/baselines/keyboard-current.net \
  circuits/fixtures/reference.fixture circuits/results/current-first
```

Run the same command with `keyboard-pre-repair` and its matching `.net` to test
the earlier design. Always choose a **fresh** output directory. A failed run is
preserved. Install the ngspice shared library first; use `NGSPICE_LIB_DIR` for a
custom library directory. `shasum` is needed for source fingerprints.

| Directory | Purpose |
| --- | --- |
| `baselines/` | Frozen values and all net/pin connectivity, with source SHA256 |
| `fixtures/` | Declared electrical assumptions, separate from connectivity |
| `experiments/` | Copies for independent changes and comparisons |
| `results/` | Ignored, reproducible circuit decks, vectors, logs and coverage |
| `evidence/` | Small retained reports and relevant measurements |
| `../tools/circuit-lab/` | Thin Rust consumer of an immutable engine revision |

## Two existing revisions

`keyboard-pre-repair` was exported from the schematic at
`7fa61c4b2679260611774365b239bbc2114cddcf`, immediately before the decoder/copper
repair. It retains U1 pins 7/Y7 and 9/Y6 on ground. The current profile uses
the repaired schematic at `36493c3` (464 components, 174 nets). The old profile
has 172 nets. Source hashes distinguish both. These are repository-design
baselines; which revision and rework are physically fitted remains unverified.
The historical manufacturing exports stay unchanged.

The lab walks the real nets to locate every HE1–HE100 and RV1–RV12 input; it
does not assume Hall numbering equals mux-channel order. It checks all 112
routes, physical ADC pads, shared address lines, power pins, mux inhibition and
decoder connections. Open Hall, ADC and enable faults must be detected.

The full ngspice acquisition network includes all fifteen 4051 muxes (120
switch paths), shared ADC nets, grounded unused inputs and the schematic's
capacitors. Unique input voltages identify all 112 controls. The fixture drives
one mux channel at a time and an ideal decoded enable sequence, with a 5 us
slot and sampling at 90% of the slot. This is a declared acquisition stress
fixture, not a measurement of firmware or ADC sample timing.

The reference fixture uses 1 kΩ source resistance, 100 Ω switch resistance and
50 pF ADC loading. With the fitted schematic's 4.7 nF input filters it predicts
17 readings outside a 1 mV settling target (maximum about 8.5 mV). These are
reported design-gate failures, retained alongside the expected model regression
pass. Change a copied fixture to compare source/loading assumptions; do not
infer the fitted Hall output impedance or key velocity from this result.

Additional checks cover a disabled analog switch, simultaneously enabled shared
muxes, grounded unused decoder outputs, and a sensor-only supply envelope.
The latter uses 6–7 mA per sensor from the existing hardware reference and
**provisional** 0.5/1.0 A source limits. It excludes Seed, LED, mux and pot load,
and makes no claim about the fitted regulator's capacity or thermal shutdown.

The pre-repair design remains `DESIGN_ACCEPTANCE=FAIL`. A regression pass
requires its grounded-output fault to remain visible. The repaired design is
`UNVERIFIED`, with acquisition and supply margin still open. The 33 Ω decoder
driver impedance is provisional; resulting short current is a warning fixture,
not a transistor model or measured current.

## Test, change, compare

```sh
cargo test --locked --manifest-path tools/circuit-lab/Cargo.toml --target "$LAB_HOST"
cargo clippy --locked --manifest-path tools/circuit-lab/Cargo.toml --target "$LAB_HOST" --all-targets --all-features -- -D warnings
cargo fmt --manifest-path tools/circuit-lab/Cargo.toml -- --check
```

Use `--no-default-features` for connectivity and host boundary tests without
ngspice. Use the generic runner's `check-source BASELINE SCHEMATIC` to detect
source drift. To create a new experiment, copy the baseline and fixture under
`experiments/my-design/`, document the expected change, and run with those files.
For schematic edits, export KiCad XML with
`kicad-cli sch export netlist --format kicadxml -o new.xml design.kicad_sch`, then
run `import new.xml design.kicad_sch experiments/my-design/design.net` through
the runner. The profile contract deliberately rejects unexpected topology;
create a new profile for a new architecture.

The engine is [Circuit Workbench in Synthphone](https://github.com/UnitedCode/synth-phone-e-v2-rust/tree/main/tools/circuit-lab),
pinned by Git revision. Its future changes do not affect this project until
`Cargo.toml` and the lockfile are explicitly updated. Do not replace the pin with
a floating branch or a permanent sibling path dependency.

## Firmware boundary and coverage

`circuit_workbench::carrier::KeyboardCarrier` takes the frozen net graph and
accepts per-control `set_microvolts("HE1", value)` or `"RV1"`. It reads the
actual GPIO-selected channel/decoder state and supplies the routed ADC voltage.
The `daisy` feature implements the pinned SDK's `machine::Carrier` trait for
`Machine::attach`. It refuses the old shorted-output profile, which needs the
SPICE fault model rather than an ideal digital approximation. OLED writes are
counted/acknowledged, not rendered. Host tests exercise all 112 routes.

This original adapter is electrically ideal. The new
[device runner](virtual-devices.md) instead evolves circuit voltages from
actual firmware GPIO activity and feeds them into its ADC conversions. Existing
[firmware tests](../tools/seed-tests/README.md) remain a separate evidence source.
Magnetic travel curves, temperature, true ADC sample/hold behavior, optocoupler,
USB integrity, and complete power consumption are not modeled. `coverage.csv`
lists the scope for every component, including inventory-only components.

Pin behavior follows the [TI 4051 datasheet](https://www.ti.com/lit/ds/symlink/sn74lv4051a.pdf)
and existing [hardware reference](../keyboard_keyboard/code/HARDWARE.md).
The resistance/loading values above are explicit fixtures, not guaranteed
datasheet maxima or extracted PCB parasitics.
