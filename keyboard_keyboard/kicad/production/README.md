# Historical manufacturing exports

The Gerbers, ZIP, IPC netlist, BOM and position files in this directory are the
historical March/April 2026 package. They predate the U1 output isolation and
copper-fill corrections in the current source. **Do not use this old ZIP to
fabricate the repaired revision.**

Generate a new reviewed package from `../keyboard_keyboard.kicad_pcb`, after
reviewing the remaining ERC/DRC warnings documented in `../validation/README.md`
and checking the fitted parts and power budget. The historical files are retained
for identifying existing boards and preserving manufacturing history.
