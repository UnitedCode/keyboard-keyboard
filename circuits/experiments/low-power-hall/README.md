# Separate lower-current Hall candidate

Replace the 100 Hall part values with **DRV5053RAQDBZR**, keeping the original
power domain, nets, 100 nF supply decoupling and **4.7 nF output filters**. Neither
the separate-regulator allocation nor the smaller-capacitor candidate is stacked
here. `keyboard-drv5053ra.kicad_sch` is a separate complete schematic; the original
symbol geometry is renamed for the candidate. Exported connectivity matches the
full candidate netlist. Existing footprint geometry still needs mechanical review.

At the stated datasheet current points, the 100-sensor budget changes:

| Case | Existing MT910X family | DRV5053 | Sensor heat reduction |
|---|---:|---:|---:|
| Typical current, 25°C | 0.60 A / 1.98 W | 0.27 A / 0.891 W | 55% |
| Stated hot maximum | 0.70 A / 2.31 W | 0.36 A / 1.188 W | 48.6% |

These are datasheet-based scenario calculations at nominal 3.3 V, not measured
board savings. TI's current maximum is specified at 125°C; this does not extend
the part to the original family's 150°C range. `hall-thermal.csv` preserves all
other power/thermal assumptions while comparing the electrical change. Some
whole-device scenarios still exceed the project budget: a sensor swap cannot
guarantee adequate Seed regulation or cooling under arbitrary loads.

## Functional conditions

The DBZ pin functions match the modeled original: VCC1, OUT2, GND3. The signal
characteristics do **not** match: the candidate has a different zero-field output,
sensitivity, saturation, output drive and noise behavior. This is not an approved
physical drop-in replacement. The original schematic says MT9102ET while hardware
documentation says MT9105ET; confirm the populated part from order/BOM/markings.

The unchanged production filter/state-machine kernels are tested at three Vq
corners, three sensitivity corners, both magnetic polarities, four field strokes
and 0/25 µs declared delay. **All 36 of the 20 mT cases pass**; **60 weak-field
cases do not produce a balanced note pair**. That is a useful negative control,
not evidence that the existing magnets provide 20 mT. Endpoints, noise, thermal
rest drift, output loading and velocity response must be measured/calibrated.
The corner model holds ADC reference at 3.3 V and uses a fixed calibrated rest.
It is not a vendor transistor/noise or complete thermal-drift model.

The separate ARM regression uses the original immutable ELF with a declared
1.02 V rest and nominal −45 mV/mT response to a **test-defined +17.4603 mT stroke**,
clipped conservatively to 0.2–1.8 V. The fixture is not a measured equivalent of
the original magnets. It retains the lab's provisional RC source model; vendor
current limits, delay and noise are not integrated into that ARM carrier. The
native corner study covers different, explicitly stated assumptions separately.

**Decision: conditional virtual candidate for magnetic, analog and mechanical
qualification.** No production firmware, PCB or original schematic is modified.
See device evidence for the actual ARM result and its exact input fingerprints.

Sources: [TI DRV5053 datasheet, electrical/magnetic and pin tables](https://www.ti.com/lit/ds/symlink/drv5053.pdf),
[MagnTek MT910X manufacturer datasheet](https://xonstorage.z8.web.core.windows.net/pdf/magntek_mt9105et_apr22_xonlink.pdf).
[Independent verification commands](../../verify-power-noise.md) include both
successful strong-field cases and deliberately insufficient-field cases.
