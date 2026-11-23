# Isolated candidate: faster Hall acquisition

`keyboard-filter-470p.kicad_sch` is a separate copy of the original schematic with
only the **100 Hall output filter capacitors changed from 4.7 nF to 470 pF**.
The 100 nF supply decoupling, all wiring, sensors, pots and firmware are unchanged.
`changes.csv` lists every changed reference. An independent KiCad netlist export
matches `candidate.net` exactly. No PCB layout or fabrication files are generated;
the builder will make the physical PCB.

## Comparison with the original

| Declared test | Original 4.7 nF | 1 nF trial | 470 pF candidate |
|---|---:|---:|---:|
| 112-control, 5 µs slot scan: errors >1 mV | 17 | 4 | 0 |
| Full-scale step error at 4.5 µs, Rs=1 kΩ, Ron=100 Ω, C=+10% | 1.395 V | 66.35 mV | 1.203 mV |
| Unmuxed filter cutoff, Rs=1 kΩ, nominal C | 33.9 kHz | 159 kHz | 339 kHz |
| Unmuxed voltage gain at 100 kHz, Rs=1 kΩ, nominal C | 0.321 | 0.847 | 0.959 |

The 470 pF candidate meets the **original unique-voltage scan** gate in this
fixture. It does **not** meet a 1 mV full-scale-step gate at +10% capacitance.
The additional step sweep uses Rs=100/1000/3000 Ω, Ron=100/500 Ω and C=±10%;
`filter-corners.csv` records every result. Those source and ADC parameters are
provisional, not measured sensor curves or a calibrated ADC sample/hold model.
The 50 pF ADC load and 1 GΩ bleed match the existing lab assumptions.

**Decision: conditional virtual candidate, not a blanket production fix.** Smaller
capacitors substantially weaken high-frequency filtering. The existing full
firmware baseline already worked with 4.7 nF. There is no evidence here that the
old capacitors caused a physical missed key, hot-room failure or reset. Retain
the original until actual noise/source-impedance measurements justify the tradeoff.
An eventual firmware settling-delay change is another option to compare separately.

Complete virtual-device evidence is retained alongside the analog results. It
uses the same immutable production ELF as the original device, preserving real
scan timing and the three-second splash. Source changes are not substituted for
a firmware execution test.

## Reproduce

```sh
export CARGO_TARGET_DIR="$PWD/keyboard_keyboard/code/target"
export CARGO_INCREMENTAL=0
cargo run --release --locked --manifest-path tools/circuit-lab/Cargo.toml \
  --target aarch64-apple-darwin --features device -- compare keyboard-filter-470p \
  circuits/baselines/keyboard-current.net circuits/experiments/filter-470p/candidate.net \
  circuits/experiments/filter-470p/assumptions.fixture circuits/results/filter-470p-new
```

Use your host target on other systems. `device keyboard-filter-470p CANDIDATE
FIXTURE ELF rev4 NEW_DIR baseline` runs the complete original acceptance sequence.
Each trial needs a fresh output directory. `candidate` and `schematic` commands
regenerate isolated copies; `check-schematic` verifies KiCad-exported connectivity.

The source SHA in the original baseline still belongs to the original schematic;
`schematic-check.txt` separately fingerprints this candidate. Every comparison
retains the exact parent, candidate, fixture, decks, vectors and runner hashes.

KiCad 9.0.8 ERC retains exactly the original six errors and five warnings
(normalized severity/type/description sets match). Existing external-power,
unused-connector and library/net-name findings remain visible in the paired ERC
reports. The copied project settings and relative library tables keep the
original HE60 library shared, without a second library copy. This is not a clean
manufacturing ERC approval.
