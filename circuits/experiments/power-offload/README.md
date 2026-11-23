# Separate power-rail allocation — rejected for promotion

This starts from `keyboard-current.net` with **all original filter values intact**.
It sketches a new split-rail carrier using a D24V22F3 module for the Hall/mux/pot
rail, an SN74LVC244A for six controls, three TMUX1511PWR switches for twelve ADC
paths, and a buffered Seed mid-supply bias. The original shared power plane
cannot be split by a socket-only interposer. No PCB is drawn or manufactured.

`power-allocation.kicad_sch` is a **functional allocation**, with modeled terminals
and labeled boundaries. It is not a qualified production symbol/footprint set.
`PG1` is explicitly **UNSELECTED**; no supervisor timing or transistor behavior is
invented. Its exported connectivity matches the new-part allocation in the full
candidate graph. The TSV files list every component and original-pin change.

## What the tests found

- Offloading relocates the modeled Hall load; it does **not remove** its
  1.98–2.31 W of nominal 3.3 V dissipation. The bridge adds a declared 10 mA budget.
- The model credits only the 100 Hall sensors as load relief. Unmeasured auxiliary
  loads remain charged to the Seed, conservatively. No fitted-regulator rating
  is inferred from the old 0.5 A hypothetical source fixture.
- Source, wiring-loss and thermal envelopes have both passing and failing cases.
  They are sensitivity cases, not predicted temperatures of the actual keyboard.
- `normal-signal-bias.csv` exposes additional ADC loading from the 100 kΩ bias.
- The ideal-guardian leakage bound does not qualify real startup/brownout bands.
- More decisively, actual production `ChannelFilter`/`SwitchState` kernels produce
  **eight false/stuck-note cases** when a lost carrier rail leaves ADCs at 1.65 V
  but calibrated rest positions differ. Perfect signal isolation alone cannot
  fix that logic problem. The 1.65 V rest cases are the positive controls.

**Decision: do not promote or build this allocation.** A power-valid path must
suppress new notes and release/reset active notes independently of analog resting
voltage. Guardian selection/timing, signal loading, partial-power states and
actual module capacity at the selected input/temperature remain open. This
rejected experiment is retained so a later design cannot silently waive them.
The [lower-current Hall candidate](../low-power-hall/README.md) is a separate way
to reduce generated heat without introducing a second sensor rail.

## Thermal interpretation

`power-scenarios.fixture` declares unknown Seed/auxiliary loads, efficiencies,
ambient temperatures and thermal resistances. The project temperature budget is
85°C, not a component maximum. The thermal RC plots cross-check an analytic
constant-power model against ngspice over a **virtual** hour; this is not an
observed failure duration. Foldback, thermal shutdown, airflow, PCB spreading,
magnetic-temperature drift and detailed CPU/codec thermal coupling are unmodeled.

Changing only enclosure K/W compares a thermal assumption independently of a
circuit change: compare `original_local_c` across those rows. Compare
`original_local_c` and `split_local_c` **at the same K/W** for offloading alone.
Do not stack a better enclosure into the offload result and attribute all gains
to the regulator. Negative required-K/W cells mean the chosen budget is
unachievable under that load/local-resistance assumption.

Sources: [Pololu module](https://www.pololu.com/product/2857),
[TI TMUX1511](https://www.ti.com/lit/ds/symlink/tmux1511.pdf),
[TI SN74LVC244A](https://www.ti.com/lit/ds/symlink/sn74lvc244a.pdf).
Their stated operating conditions are not replaced by these simplified models.

[Independent verification](../../verify-power-noise.md) reproduces the rejection,
source/thermal cases, wiring checks and native firmware fault test.
