# Baseline run, 2026-11-22

Executed on macOS/aarch64 with ngspice 47. `keyboard-current/` holds the full
acquisition deck, source hashes, component coverage, 112 routed controls, sampled
voltages, sensor supply envelope and simulator banner. `keyboard-pre-repair/`
holds the independently executed old-board report and grounded-output fixture.
Both use `../../fixtures/reference.fixture`. The baseline and fixture hashes
are retained in each report. The engine source hashes are in the current run.

Validation: four Rust tests with all features (including the pinned SDK trait),
three without ngspice/SDK, formatting and all-feature clippy checks. All 112 host
carrier routes passed. Broken Hall, ADC, enable and mux-switch paths were caught;
shared-pair contention was detected. No production firmware was changed or flashed.

**Model regression passed; old-board design acceptance failed as expected.**
The repaired profile remains unverified. Both profiles predict 17 readings
outside the fixture's 1 mV target, maximum error 8.505 mV. This is retained as a
failed design gate, not converted into an electrical pass. The supply stress
results use hypothetical source limits and do not identify the fitted regulator.
The shorted-output check is separate from acquisition under a stiff ideal rail;
the resulting supply interaction is not co-simulated.

The historical profile comes from schematic commit
`7fa61c4b2679260611774365b239bbc2114cddcf`; the current schematic was present at
`36493c3`. Neither establishes the assembled board's exact fitted/reworked state.
There is no claimed ARM/SPICE coupled run or physical measurement.
