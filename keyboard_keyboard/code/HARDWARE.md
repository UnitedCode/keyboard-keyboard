# Hardware Reference — keyboard-keyboard

## Overview

Hall-effect MIDI keyboard controller on Electrosmith Daisy Seed (STM32H750V).
PCB hosts up to 15 analog muxes (AM1–AM15) feeding hall-effect sensors into Daisy ADC pins.
Firmware drives 5 muxes (40 keys). PCB supports 15 muxes (~80–120 sensors depending on population).

KiCad schematic: `keyboard_keyboard/kicad/keyboard_keyboard.kicad_sch`

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
| 33 | `SAI2_SD_A`  | A11 | AM14+AM15 shared ← repurposed as ADC |
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

## Firmware vs. PCB Capability

| | Firmware (`src/main.rs`) | PCB (netlist) |
|-|--------------------------|---------------|
| Muxes | 9 (AM1–AM9) | 15 (AM1–AM15) |
| Keys | 70 (HE1–HE70) | 100 (HE1–HE100) |
| ADC pins | A0–A8 | A0–A11 |
| Pair-enable logic | not implemented | needs U1 + D2/D3/D4 |

### Current pin assignments (AM1–AM9)

| Mux | Daisy Pin | ADC | Sensors |
|-----|-----------|-----|---------|
| AM1 | Daisy15 | A0 | HE1–HE8 |
| AM2 | Daisy16 | A1 | HE9–HE14, HE28–HE29 |
| AM3 | Daisy17 | A2 | HE15–HE22 |
| AM4 | Daisy18 | A3 | HE23–HE27, HE41–HE43 |
| AM5 | Daisy19 | A4 | HE30–HE37 |
| AM6 | Daisy20 | A5 | HE38–HE40, HE54–HE58 |
| AM7 | Daisy21 | A6 | HE44–HE51 |
| AM8 | Daisy22 | A7 | HE52–HE53, HE67–HE70 (X0/X3 unused) |
| AM9 | Daisy23 | A8 | HE59–HE66 |

### To expand to HE71–HE100 (AM10–AM13)

1. Implement U1 decoder enable sequencing with Daisy2 (ENB_C/A2), Daisy3 (ENB_B/A1), Daisy4 (ENB_A/A0)
2. Add ADC pins Daisy24 (A9) for AM10/AM11 and Daisy25 (A10) for AM12/AM13
3. Scan AM10/AM11 alternately per mux-channel by toggling the U1 address
4. Increase `NUM_MUXES`, `NUM_ADC_PINS`, `NUM_KEYS` and update `KEY_MAP`

> **⚠ Schematic note:** Netlist shows U1 pin 5 (~E1/G2B, active-low enable) tied to +3.3VA.
> If this reflects the actual PCB, the 74HC138 decoder is permanently disabled and
> AM10–AM15 will be non-functional. Verify the physical board before implementing.

> Note: AM14/AM15 mux outputs are connected to trim pots (RV1–RV12), not hall-effect
> sensors. AM15 X0–X3 are tied to GND. AM14/AM15 Inh pins are tied to GND via U1 Y6/Y7
> (which are also grounded), so these muxes are always enabled on the shared A11 line.

---

## Other Peripherals

| Component | Connection | Notes |
|-----------|------------|-------|
| MIDI out | D13 / `USART1_TX` (pin 14) | 31250 baud, `src/midi_sender.rs` |
| MIDI in  | D14 / `USART1_RX` (pin 15) | 31250 baud, U2 opto-isolator |
| I2C      | D11/D12 (pins 12–13) | J3 — display (`ssd1306`) |
| LEDs     | D1, D5, D6 (pins 2, 6, 7) | `led_1`, `led_2`, `led_3` |
| USB      | D29/D30 (pins 36–37) | J6 + J7 — device + host |
| Audio in | pins 16–17 | J1 — stereo TRS |
| Audio out| pins 18–19 | J2 — stereo TRS |
