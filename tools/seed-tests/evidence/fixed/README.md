# Repaired-firmware evidence

The actual firmware passes the All Notes Off/key/pot experiment on both profiles:
quiet gap 1.000 ms, paced/control worst gap 1.00005 ms, complete 48-byte burst and
ordinary note-on/off plus pot CC output. Compare the pre-fix 10.209 ms gap in
`../rev5` and `../rev7`. No deadline threshold or baud rate was weakened.

Final release ELF SHA256:
`51f38cd9e4c84f209b4d3335369a04b41b6573874ad01514f1055d694558b33b`.

`../chord-before` runs the short-lock repair with the old 32-event temporary
buffer: forty sensor inputs yield only 32 on/off pairs (192 bytes), so the chord
gate fails. `../chord-after` runs the full-scan buffer and bounded queue repair:
40 on/off pairs (240 bytes), no stuck notes and max scan gap 1.0001 ms in both
UART controls. `../overload-after` deliberately exceeds MIDI throughput; overflow
is logged, all-channel recovery is emitted, scans stay within 1.00005 ms and the
ending MIDI state has no sounding notes.

The chord/overload captures preceded a comment-only source clarification. Every
ELF PT_LOAD segment was compared against the final release: all loaded bytes,
addresses, memory sizes, flags and alignment are identical. Exact captured ELF
hashes and model/core provenance are retained with each run. The final All Notes
Off captures use the final release ELF directly.

These are short virtual-system checks. They do not establish physical timing,
thermal margin, the cause of the OpenSauce incident, or a continuous one-hour
soak. The room was reported hot; an elapsed-time failure threshold is unknown.
