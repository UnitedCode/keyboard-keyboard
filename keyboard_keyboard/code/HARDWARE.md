# Hardware Reference — keyboard-keyboard

## Overview

Hall-effect MIDI keyboard controller on Electrosmith Daisy Seed (STM32H750V).
PCB hosts up to 15 analog muxes (AM1–AM15) feeding hall-effect sensors into Daisy ADC pins.
Firmware drives 13 sensor muxes (100 keys, HE1–HE100) and 2 pot muxes (12 pots).

KiCad schematic: `keyboard_keyboard/kicad/keyboard_keyboard.kicad_sch`

---

## Component Part Numbers

| Reference | Part Number | Description |
|-----------|-------------|-------------|
| A1 | Electrosmith Daisy Seed Rev4 | STM32H750V microcontroller module |
| AM1–AM15 | SN74LV4051APWR | 8-channel single-ended analog mux (TI, TSSOP-16) |
| HE1–HE100 | MT9105ET | Hall-effect sensor |
| U1 | 74HC138D | 3-to-8 line decoder / demultiplexer (NXP, SOIC-16) |
| U2 | 6N138 | Optocoupler — MIDI RX input isolation |

---

## Pin Mapping: Netlist → Daisy Seed

> **Important:** KiCad schematic uses physical pad numbers (1–40) matching Electrosmith pinout. Several pins labeled by STM32 alt-function (e.g., `DAC_OUT2`, `SAI2_MCLK`) are actually used as ADC inputs — Daisy exposes these as A7–A11.

| Netlist Pin | KiCad Pinfunction | Daisy Name | Role in Design |
|:-----------:|:-----------------:|:----------:|----------------|
| 2  | `SD_DATA_3`  | D1  | `led_3` |
| 3  | `SD_DATA_2`  | D2  | `ENB_C` → U1 decoder address A2 |
| 4  | `SD_DATA_1`  | D3  | `ENB_B` → U1 decoder address A1 |
| 5  | `SD_DATA_0`  | D4  | `ENB_A` → U1 decoder address A0 |
| 6  | `SD_CMD`     | D5  | `led_2` |
| 7  | `SD_CLK`     | D6  | `led_1` |
| 8  | `SPI1_CS`    | D7  | MUX address A (`MUX_SELECT_0`) |
| 9  | `SPI1_SCK`   | D8  | MUX address B (`MUX_SELECT_1`) |
| 10 | `SPI1_POCI`  | D9  | MUX address C (`MUX_SELECT_2`) |
| 12 | `I2C1_SCL`   | D11 | I2C SCL → J3 |
| 13 | `I2C1_SDA`   | D12 | I2C SDA → J3 |
| 14 | `USART1_TX`  | D13 | MIDI out (31250 baud) |
| 15 | `USART1_RX`  | D14 | MIDI in (31250 baud) |
| 22 | `ADC_0`      | A0  | AM1 mux output |
| 23 | `ADC_1`      | A1  | AM2 mux output |
| 24 | `ADC_2`      | A2  | AM3 mux output |
| 25 | `ADC_3`      | A3  | AM4 mux output |
| 26 | `ADC_4`      | A4  | AM5 mux output |
| 27 | `ADC_5`      | A5  | AM6 mux output |
| 28 | `ADC_6`      | A6  | AM7 mux output |
| 29 | `DAC_OUT2`   | A7  | AM8 mux output ← repurposed as ADC |
| 30 | `DAC_OUT1`   | A8  | AM9 mux output ← repurposed as ADC |
| 31 | `SAI2_MCLK`  | A9  | AM10+AM11 shared ← repurposed as ADC |
| 32 | `SAI2_SD_B`  | A10 | AM12+AM13 shared ← repurposed as ADC |
| 33 | `SAI2_SD_A`  | D26 | intentionally unconnected (no ADC) |
| 35 | `SAI2_SCK`   | A11 / D28 | AM14+AM15 shared ADC input |
| 16 | `AUDIO_IN_1` | —   | Audio in 1 → J1 |
| 17 | `AUDIO_IN_2` | —   | Audio in 2 → J1 |
| 18 | `AUDIO_OUT_1`| —   | Audio out 1 → J2 |
| 19 | `AUDIO_OUT_2`| —   | Audio out 2 → J2 |
| 36 | `USB_D_-`    | D29 | USB D− → J6/J7 |
| 37 | `USB_D_+`    | D30 | USB D+ → J6/J7 |
| 38 | `3V3_D`      | —   | 3.3V digital → all mux VDD |
| 39 | `VIN`        | —   | Power in → J6/J7 |
| 20 | `AGND`       | —   | Analog ground |
| 40 | `DGND`       | —   | Digital ground |

---

## Mux Architecture

### Address lines (shared by ALL 15 muxes)

| Signal | Daisy Pin | Selects |
|--------|:---------:|---------|
| `MUX_SELECT_0` | D7 | A (bit 0) |
| `MUX_SELECT_1` | D8 | B (bit 1) |
| `MUX_SELECT_2` | D9 | C (bit 2) |

3 bits → 8 channels (X0–X7), selected simultaneously across all active muxes.

### Mux groups and ADC pins

AM1–AM9: dedicated ADC pin each. AM10–AM15: 3 pairs sharing an ADC pin — 74HC138 decoder (U1, driven by D2/D3/D4) enables one mux per pair at a time.

| ADC Pin | Daisy | Mux(es) | Notes |
|:-------:|:-----:|---------|-------|
| ADC_0 | A0  | AM1 | dedicated |
| ADC_1 | A1  | AM2 | dedicated |
| ADC_2 | A2  | AM3 | dedicated |
| ADC_3 | A3  | AM4 | dedicated |
| ADC_4 | A4  | AM5 | dedicated |
| ADC_5 | A5  | AM6 | dedicated |
| ADC_6 | A6  | AM7 | dedicated |
| A7    | A7  | AM8 | dedicated (`DAC_OUT2` repurposed) |
| A8    | A8  | AM9 | dedicated (`DAC_OUT1` repurposed) |
| A9    | A9  | AM10, AM11 | pair — U1 enables one at a time |
| A10   | A10 | AM12, AM13 | pair — U1 enables one at a time |
| A11   | A11 | AM14, AM15 | pair — U1 enables one at a time |

### U1 — 74HC138 decoder (mux pair enables)

Driven by D2 (A2), D3 (A1), D4 (A0). Outputs active-low `ENABLE_10`–`ENABLE_15`, controlling which mux in each shared-pin pair is active.

---

## Current firmware and board corrections

Firmware scans AM1–AM13 for all 100 switches and AM14–AM15 for 12 pots.
AM10/11 share A9, AM12/13 share A10, and AM14/15 share A11; U1 outputs
Y0–Y5 select the corresponding active-low mux inhibit inputs.

U1 pins 4/5 are grounded and pin 6 is at 3.3 V. Pins 7/Y7 and 9/Y6 are
unused push-pull outputs: leave them open. The earlier PCB grounded them,
forcing high outputs into ground during normal decoder addresses 0–5.
The corrected PCB has isolated pads and refilled ground copper, and a direct
0.3 mm connection from enable pad 5 to grounded pad 4. The refill also removes
stale ground fill overlapping the MUX_OUT_14 pot ADC trace.

Pot input is **pad 35 / D28 / PA2 / A11**. Pad 33 / D26 / PD11 has no ADC and
is intentionally unconnected on the current PCB. Older documentation describing
an obligatory pad-33 bodge or permanently disabled decoder does not describe
these current design files. Check an assembled board's revision and continuity
before rework; do not blindly bridge pins while their old supply connection remains.

The schematic marks unused U1 outputs as no-connects. KiCad 9.0.8 checks found
zero PCB errors and zero unconnected items after repair/refill; pre-existing
library, silkscreen and connection-width warnings remain. The schematic still
has pre-existing external-power and unused-connector ERC errors plus label/library
warnings. Reports and their limits are in `../kicad/validation/README.md`.

## Power and hot-room validation

The carrier rail named +3.3VA is supplied by A1 **pad 38 (3V3_D)**, not the
Seed's pad-21 analog output. MagnTek's MT910X datasheet specifies 6.0 mA typical,
7.0 mA maximum per sensor at 3.3 V: 100 sensors alone account for 0.60–0.70 A
and 1.98–2.31 W on the carrier. Muxes, LEDs, pots and the Seed add other loads.
This is a load estimate, not measured regulator overload or a prediction of
component temperatures. Headroom depends on the fitted Seed, source and wiring.

After correcting the output shorts, record input voltage/current, the 3.3 V rail
at the Seed and a distant sensor, and component temperatures in cool and warm
ambient conditions while repeating the same key/pot sequence. Separate missed
keys, false/stuck notes, latency and resets in the observation log. The OpenSauce
report is temperature-correlated with an unknown duration; an hour is not an
established failure threshold. Virtual tests do not model electrical/thermal faults.

A dedicated regulated 3.3 V carrier supply remains a design option if measured
headroom is inadequate. Its output must be isolated from the Seed's 3.3 V output,
with a shared ground and a checked power-up/back-powering sequence. Do not
parallel two regulators or move sensor power to 5 V without redesigning ADC levels.
No replacement regulator has been selected or validated in this change.

Sources: [Nexperia 74HC138](https://assets.nexperia.com/documents/data-sheet/74HC_HCT138.pdf),
[MagnTek MT910X manufacturer datasheet, distributor mirror](https://xonstorage.z8.web.core.windows.net/pdf/magntek_mt9105et_apr22_xonlink.pdf),
[Electrosmith Seed pin table, manufacturer datasheet mirror](https://www.electrokit.com/upload/product/41020/41020659/Daisy_Seed_datasheet_v1.0.5.pdf).

---

## Other Peripherals

| Component | Connection | Notes |
|-----------|------------|-------|
| MIDI out | D13 / `USART1_TX` (pin 14) | 31250 baud, `src/midi.rs` |
| MIDI in  | D14 / `USART1_RX` (pin 15) | 31250 baud, U2 opto-isolator |
| I2C      | D11/D12 (pins 12–13) | J3 — display (`ssd1306`) |
| LEDs     | D1, D5, D6 (pins 2, 6, 7) | `led_1`, `led_2`, `led_3` |
| Carrier USB | D29/D30 (pins 36–37) | J6/J7 routing; unused by this firmware experiment |
| Built-in USB | PA11/PA12, USB2/OTG_FS | Seed micro-USB connector; MIDI device to a computer ([guide](USB_MIDI.md)) |
| Audio in | pins 16–17 | J1 — stereo TRS |
| Audio out| pins 18–19 | J2 — stereo TRS |
