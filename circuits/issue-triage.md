# Circuit improvement queue

The new [power/noise studies](verify-power-noise.md) refine the priorities below:
retain 4.7 nF pending dynamic source/noise measurements; the separate 4 Ω DC-test
fixture passes with original capacitors. A lower-current Hall candidate reduces
the datasheet-based sensor budget but remains conditional on magnets/output/fit.
The split-rail allocation is **rejected** after eight native firmware power-loss
counterexamples. No physical hot-room cause or regulator rating has been proved.

| Priority | Issue | Decision |
|---|---|---|
| 1 | Power headroom / hot-room report | 100 sensors imply 0.60–0.70 A before other loads. The fitted Seed regulator/source, rail drop and temperature have not been measured. The earlier 0.5 A envelope is hypothetical, not a rating. Do not claim overload proved or select a regulator by changing the fixture. |
| 2 | Acquisition margin | Compare 1 nF and 470 pF independently against the frozen 4.7 nF design. Only 470 pF clears the original 1 mV scan gate; weaker noise filtering keeps it conditional. |
| 3 | MIDI/USB electrical fidelity | Complete firmware captures use an ideal UART boundary and do not qualify optocoupler CTR/edge timing or USB signal integrity. These need independent receiver/interface models. |

The old grounded decoder outputs were repaired before these experiments. Their
historical negative control remains frozen and is not counted as a new fix.

The production schematic, PCB and firmware remain unchanged. The separate
[470 pF schematic](experiments/filter-470p/README.md) is a reviewable virtual
candidate for the builder. No PCB routing, Gerbers or manufacturing package is
created. The earlier complete-device baseline remains the control.

For a future separate power candidate, first establish the actual Seed revision,
input source and load envelope. A dedicated 3.3 V rail must isolate regulator
outputs and account for common ground, ADC limits and power sequencing/backfeed.
That work cannot be validated by the present stiff-rail acquisition model.
