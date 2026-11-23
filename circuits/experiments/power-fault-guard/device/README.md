# Final ARM power-fault captures

All seven positive cases pass with candidate ELF SHA256
`5413e5e8b759b028fa4aad3825df9a581f1f5fc4ac97a567e561670cf7729f21`.
The executed source revision is `14ebec4ac649091eaf63e2e949db6d81088aaff0`;
source/lockfile fingerprints accompany each capture. The immutable circuit hash
is `9ce718ce53d1118f81376a1190c64064598f2e0274ab5463b580950c68fd2fb3`.
The shared engine/emulator revisions remain pinned in the lab's Cargo files.

| Case | Observed note-on / explicit off | Result |
|---|---:|---|
| Healthy cold start | 28 / 28 | Both 14-key strokes and all twelve initial pots pass |
| Sensor rail loss / restoration | 14 / 14 | Panic, sticky lockout, later stroke suppressed |
| 100 µs brownout during a MIDI burst | 7 / 7 | Pending chord tail canceled; no later starts |
| Sensor rail arrives late | 0 / 0 | Invalid boot stays disarmed after restoration |
| Slow sensor rail ramp | 0 / 0 | Invalid boot stays disarmed |
| Seed rail 2.9 V dip, running-CPU assumption | 14 / 14 | Qualified fault locks out |
| Repeated sensor-rail chatter | 14 / 14 | One lockout; restored rail does not retrigger |

Every case ends with zero held notes and zero forbidden late NoteOn. No NoteOn
status byte completes after fault assertion in these particular runtime captures;
a message already on the wire can still finish. The gate's 3 ms response window
is explicit rather than an instantaneous-silence claim. Panic finishes about
83.91 ms after the short-dip assertion and 90.40 ms in the other runtime cases,
inside the **14-key** 170 ms test budget. The separate 90-pitch native test gives
wire time only, not a measured ARM scheduling bound for a 90-key panic.

Maximum observed scan gap during active scan windows is **1,000,159 ns**, below
the 2 ms gate. PB5 is actually configured as input with no internal pull. UART
frames are paced at 31,250 baud. The CPU executes the full experimental firmware
for 600 ms per positive case; there are no timer/PC/private-state shortcuts.
The three-second splash has **not** elapsed in these fault captures. The earlier
3.4 s original-device regression remains a separate frozen control; this suite
does not claim a new complete settings/all-90-key acceptance run.

The analog model is the original RC network used as a signal projection, plus
declared ideal power-valid/isolation behavior. It is not a transistor simulation
of the added supervisor, switches, regulator or ADC aperture. The Seed's physical
revision is unknown; the virtual target is Rev4/AK4556. Source voltages and rail
waveforms are test fixtures, not hardware measurements.

The final warm-reset case is preserved under `../retained-failures/warm-reset-final`:
it silences all 14 notes first, then fails in flash reinitialization after
SYSRESETREQ. It is **not** counted among the seven passes or treated as a successful
negative control. Cold startup is tested independently; continuous physical
power-cycle recovery is not claimed.

The original-firmware control under `../retained-failures/original-firmware`
produces 140 note-ons under the loss fixture, versus 28 in the healthy sequence.
Its 126 post-fault starts include the later deliberately played stroke; do not
call every one of those 126 a phantom. The earlier exact ten-case native test
separately reproduces eight false/stuck-note failures.

Full ELF, ADC/GPIO traces and intermediate runs are retained locally in the
Synthphone repository's ignored
`audio-lab-results/sensor-power-fault-guard-2026-11-23/keyboard-raw/` archive.
The compact reports, MIDI, supervisor edges, RTT, OLED and hashes here are the
reviewable evidence. [Independent verification](../verify.md) reruns each case.
