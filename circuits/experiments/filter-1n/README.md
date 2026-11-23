# Isolated trial: 4.7 nF to 1 nF

This changes only the 100 Hall output filter capacitors. It starts directly from
`keyboard-current.net`, with the original fixture copied unchanged. The full
15-mux, 112-control SPICE scan's errors above 1 mV fall from **17 to 4**.
It therefore **does not meet that complete scan gate**. Keep this trial as evidence,
not as the selected prototype. Its short actual-firmware HE1 press/release run is
retained separately; a functional note test cannot waive an analog scan failure.

The independent [470 pF trial](../filter-470p/README.md) also starts from the
original, not from this candidate. Production schematic/PCB/firmware stay intact.
