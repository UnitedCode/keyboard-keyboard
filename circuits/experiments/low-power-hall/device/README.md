# Complete firmware against the declared Hall candidate

This uses the original immutable production ARM ELF, SHA256
`e1416750d97d39086ea4f6fa9cb9987d1fda0965b09c844c19ca9b3d645ab3f1`,
with the separate DRV5053RA graph and original 4.7 nF filters. It executes the
full 3.4 s sequence, including the real splash delay and settings interactions;
no firmware timers, PC or private application state are patched.

The carrier applies declared 1.02 V rest and nominal −45 mV/mT response to the
test-defined +17.4603 mT stroke, clipped to 0.2–1.8 V. This is a reproducible
electrical fixture, not an observation of the existing magnets. It retains the
reference fixture's provisional 1 kΩ source / 100 Ω mux / 50 pF ADC loading.
Vendor current limit, propagation delay, intrinsic noise and thermal drift are
not incorporated into this complete ARM carrier. The separate native corner
study has different, explicit assumptions; its results are not stacked here.

`report.txt` records the original acceptance, fingerprints, GPIO/scan/clock
observations and any failures. `analysis.txt` independently summarizes MIDI/PCM;
`midi.csv` allows another decoder to check note pairing. Expected successful
behavior covers 70 melodic keys, 20 drum pads, twelve pots, expressive controls,
program changes, recalibration, settings, display and bounded scan gaps.

The shared engine's source fingerprint matches the merged engine used by this
repository's pinned dependency. Rev4/AK4556 is the virtual target; the physically
fitted Seed revision remains unknown. Passing this test does not promote the
candidate without magnetic, analog, thermal and mechanical qualification.

Full ELF, ADC/GPIO, PCM, OLED RAM, flash and progress captures are preserved locally
under the Synthphone repository:
`audio-lab-results/power-thermal-noise-2026-11-22/keyboard-raw/hall-complete/`.
Compact observations and exact compiled-source hashes are retained here.
Result: **ARM_DEVICE=PASS**. The recorded MIDI has **90 note-on/off pairs**,
zero unmatched releases and zero held notes. All twelve pot extremes and the
remaining firmware gates pass. The run records 3,257 scans and 349,284 ADC
observations, with maximum modeled scan gap **1,004,913 ns**, below its 2 ms gate.
[Reproduction commands](../../../verify-power-noise.md) also describe how to build
and identify a new ELF rather than mislabel it as this immutable artifact.
