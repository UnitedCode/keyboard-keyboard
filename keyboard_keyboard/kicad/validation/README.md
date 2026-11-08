# Decoder/copper repair checks — 2026-11-08

KiCad 9.0.8 reports are retained before and after this repair. The PCB zones
were refilled in the native PCB editor using the original project's unchanged
design rules. No DRC severity or clearance rule was relaxed.

| Check | Before | After |
| --- | ---: | ---: |
| PCB clearance errors | 3 | 0 |
| PCB unconnected items | 1 | 0 |
| Other PCB warnings | 324 | 324 |
| PCB schematic-parity warnings | 56 | 56 |
| Schematic output-to-output conflict | 1 | 0 |
| Other schematic errors | 6 | 6 |

The three clearance errors were stale ground copper overlapping MUX_OUT_14.
The unconnected item was U1 enable pad 5; it now has a direct 0.3 mm track to
grounded pad 4. U1 output pads 7/Y7 and 9/Y6 each have their own unconnected net;
the schematic has explicit no-connect markers. They are no longer grounded or
connected to each other. The unused-output fix removes a real drive-to-ground
fault even though DRC alone cannot detect a wrongly assigned ground net.

The before netlist placed U1 pads 4, 5, 7, 8 and 9 on GND. The after netlist
places only 4, 5 and 8 there, and assigns 7/9 independent `output+no_connect`
nets. The six used outputs remain ENABLE_10 through ENABLE_15. A1 pad 35 remains
the pot ADC input and pad 33 remains unconnected.

Remaining PCB warnings: 199 footprint-library differences, 28 narrow copper
connections, 94 silkscreen warnings, two dangling tracks and one dangling via.
The 56 existing parity warnings concern mechanical footprints, connector pads
and footprint data; none are parity errors. These warnings were not suppressed.

Remaining schematic errors: external VIN and ground are not marked as driven,
and unused connector pins J4/J5 pins 1/5 lack no-connect markers. Three shared
mux-output label-name warnings remain. Library differences include the existing
6N138 symbol and the 74HC138's corrected manufacturer datasheet link; the old
link pointed at the different, non-inverting HC238 part. Metadata correction adds
one library warning while the actual output-conflict error is removed.

This check does not certify the complete design for manufacture, establish
thermal margin, or repair previously manufactured boards. The historical
`production/` exports predate these changes and are explicitly labelled as such.
See `../../code/HARDWARE.md` for revision checks and physical power/temperature
validation. No debug probe was connected, so firmware was not flashed here.

Reproduce from the repository root (output paths may be changed):

```sh
kicad-cli sch erc --format json -o after-erc.json keyboard_keyboard/kicad/keyboard_keyboard.kicad_sch
kicad-cli pcb drc --format json --schematic-parity -o after-drc.json keyboard_keyboard/kicad/keyboard_keyboard.kicad_pcb
kicad-cli sch export netlist -o after.net keyboard_keyboard/kicad/keyboard_keyboard.kicad_sch
```
