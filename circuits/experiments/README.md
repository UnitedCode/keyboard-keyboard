# Experiments

Latest: [power allocation rejection](power-offload/README.md),
[lower-current Hall candidate](low-power-hall/README.md), and
[filter noise / source-impedance comparison](filter-noise/README.md).
Use the [independent verification guide](../verify-power-noise.md) to reproduce
each separately. The original 4.7 nF remains the default.

Copy a baseline and `../fixtures/reference.fixture` into a named directory here.
Record the question, changed values, expected result and measured comparison in
a README. Run the lab with these copied files and a fresh result directory.

Do not edit the baseline to make a proposed repair pass. Preserve pre-repair
faults as expected findings. New topology needs a new profile and expectations.
The lab's engine is pinned to one revision in `tools/circuit-lab/Cargo.toml`;
upgrading Synthphone or Virtual Daisy elsewhere cannot change this test suite.

## First improvement comparisons

- [1 nF trial](filter-1n/README.md): retained incomplete scan improvement.
- [470 pF candidate](filter-470p/README.md): separate schematic, complete scan
  improvement and explicit filtering tradeoff.
- [Issue ranking](../issue-triage.md): power remains the highest unresolved risk.

Both start from `keyboard-current.net`; their exact allowed delta is checked
before comparisons. `COMPARISON=PASS` means the comparison executed, not physical
acceptance. Read the scan gate, step errors and design acceptance independently.
