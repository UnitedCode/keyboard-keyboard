//! hall_effect_keyboard — Daisy Seed + SN74LV4051A × 5 + MT9102ET × 40
//! MIDI output over USART1 at 31250 baud
//!
//! ## Pin mapping (see HARDWARE.md for full reference):
//!
//! Select lines (shared by all muxes):
//!   Daisy7  (SPI1_CS)   → MUX_SELECT_0 (A / bit 0)
//!   Daisy8  (SPI1_SCK)  → MUX_SELECT_1 (B / bit 1)
//!   Daisy9  (SPI1_POCI) → MUX_SELECT_2 (C / bit 2)
//!
//! Mux outputs (Daisy15–19 = ADC_0–ADC_4):
//!   Daisy15 (ADC_0 / PC0) → AM1  (HE1–HE8)
//!   Daisy16 (ADC_1 / PA3) → AM2  (HE9–HE14, HE28–HE29)
//!   Daisy17 (ADC_2 / PB1) → AM3  (HE15–HE22)
//!   Daisy18 (ADC_3 / PA7) → AM4  (HE23–HE27, HE41–HE43)
//!   Daisy19 (ADC_4 / PA6) → AM5  (HE30–HE37)
//!
//! To expand: add Daisy20 (AM6), Daisy21 (AM7), then confirm those ADC channels
//! work before adding Daisy22/23 (ADC channels 18/19 — verify HAL support first).

#![no_main]
#![no_std]

use panic_rtt_target as _;

mod midi_sender;

#[rtic::app(
    device = stm32h7xx_hal::stm32,
    peripherals = true,
    dispatchers = [DMA1_STR2, DMA1_STR3, DMA1_STR4, DMA1_STR5, DMA1_STR6]
)]
mod app {
    const BLOCK_SIZE: usize = 128;

    const NUM_SWITCHES: usize = 100;
    const NUM_MUXES: usize = 13;
    const NUM_ADC_PINS: usize = 9;
    const MUX_CHANNELS: usize = 8;

    // ── Thresholds ────────────────────────────────────────────────────────────
    const FIRST_DELTA: u16 = 150;
    const SECOND_DELTA: u16 = 250;
    const RELEASE_DELTA: u16 = 100;
    const DEBOUNCE_TICKS: u8 = 3;

    const FILTER_SIZE: usize = 4;
    const FILTER_SHIFT: u32 = 2;

    const VELOCITY_WINDOW_MS: u32 = 80;
    const CALIBRATION_SAMPLES: usize = 64;

    const DIAG_LOGGING: bool = false; // set true to see raw ADC / calibration logs
    const LOG_INTERVAL_MS: u32 = 500;
    const LOG_SWITCH: usize = 0; // HE1 (AM1 X4) — first switch

    // ── Drum pad (switches 81–100, MIDI ch 3) ────────────────────────────────────
    const DRUM_CHANNEL: u8 = 9; // MIDI channel 10 (0-indexed)
    const DRUM_SWITCH_START: usize = 80; // switch index of first drum pad (HE81)
    #[rustfmt::skip]
    const DRUM_NOTE: [u8; 20] = [
        36, 37, 38, 39, 40, 41, 42, 43, 44, 45, // GM Bass Drum → Pedal Hi-Hat
        46, 47, 48, 49, 50, 51, 52, 53, 54, 55, // GM Open Hi-Hat → Splash Cymbal
    ];

    // ── Pitch bend sensors (left/right arrow keys) ───────────────────────────────
    const PITCH_BEND_DOWN: usize = 77; // HE78 (left arrow)  → bends pitch down
    const PITCH_BEND_UP: usize = 79; // HE80 (right arrow) → bends pitch up
    const PITCH_BEND_MAX_DELTA: u16 = 400; // ADC counts for full bend travel
    const PITCH_BEND_HYSTERESIS: u16 = 32; // min 14-bit change to send a message
    const PITCH_BEND_INTERVAL_MS: u32 = 5; // send at most every 5 ms (200 Hz)

    // ── Vibrato sensors (up/down arrow keys) → CC1 (mod wheel) ──────────────────
    const VIBRATO_A: usize = 76; // HE77 (up arrow)   → vibrato depth
    const VIBRATO_B: usize = 78; // HE79 (down arrow) → vibrato depth
    const VIBRATO_MAX_DELTA: u16 = 300; // ADC counts for full vibrato depth
    const VIBRATO_HYSTERESIS: u8 = 2; // min CC change to send
    const VIBRATO_INTERVAL_MS: u32 = 10; // send at most every 10 ms (100 Hz)

    // ── Potentiometers ───────────────────────────────────────────────────────────
    const NUM_POTS: usize = 12;
    const POT_SCAN_MS: u32 = 10; // scan pots at 100 Hz
    const POT_CC_HYSTERESIS: u8 = 2; // min CC change to send
                                     // (decoder_idx, mux_channel, CC_number)
                                     // decoder_idx 4 = AM14 (Y4), 5 = AM15 (Y5) — both read via Daisy26 / A11
    #[rustfmt::skip]
    const POT_MAP: [(u8, u8, u8); NUM_POTS] = [
        (4, 4, 20), // RV1  AM14 X4 → CC20
        (4, 6, 21), // RV2  AM14 X6 → CC21
        (4, 7, 22), // RV3  AM14 X7 → CC22
        (4, 5, 23), // RV4  AM14 X5 → CC23
        (4, 2, 24), // RV5  AM14 X2 → CC24
        (4, 1, 25), // RV6  AM14 X1 → CC25
        (4, 0, 26), // RV7  AM14 X0 → CC26
        (4, 3, 27), // RV8  AM14 X3 → CC27
        (5, 4, 28), // RV9  AM15 X4 → CC28
        (5, 6, 29), // RV10 AM15 X6 → CC29
        (5, 7, 30), // RV11 AM15 X7 → CC30
        (5, 5, 31), // RV12 AM15 X5 → CC31
    ];

    // ── Switch map ───────────────────────────────────────────────────────────────
    // (mux_index, channel) in HE sensor order.
    // mux_index 0=AM1 1=AM2 2=AM3 3=AM4 4=AM5 5=AM6 6=AM7  (Daisy15–21 / A0–A6)
    #[rustfmt::skip]
    const SWITCH_MAP: [(u8, u8); NUM_SWITCHES] = [
        // AM1 (mux 0, Daisy15/A0) — HE1–HE8
        (0, 4),  // HE1  → AM1 X4
        (0, 6),  // HE2  → AM1 X6
        (0, 7),  // HE3  → AM1 X7
        (0, 5),  // HE4  → AM1 X5
        (0, 2),  // HE5  → AM1 X2
        (0, 1),  // HE6  → AM1 X1
        (0, 0),  // HE7  → AM1 X0
        (0, 3),  // HE8  → AM1 X3
        // AM2 (mux 1, Daisy16/A1) — HE9–HE14
        (1, 4),  // HE9  → AM2 X4
        (1, 6),  // HE10 → AM2 X6
        (1, 7),  // HE11 → AM2 X7
        (1, 5),  // HE12 → AM2 X5
        (1, 2),  // HE13 → AM2 X2
        (1, 1),  // HE14 → AM2 X1
        // AM3 (mux 2, Daisy17/A2) — HE15–HE22
        (2, 4),  // HE15 → AM3 X4
        (2, 6),  // HE16 → AM3 X6
        (2, 7),  // HE17 → AM3 X7
        (2, 5),  // HE18 → AM3 X5
        (2, 2),  // HE19 → AM3 X2
        (2, 1),  // HE20 → AM3 X1
        (2, 0),  // HE21 → AM3 X0
        (2, 3),  // HE22 → AM3 X3
        // AM4 (mux 3, Daisy18/A3) — HE23–HE27
        (3, 4),  // HE23 → AM4 X4
        (3, 6),  // HE24 → AM4 X6
        (3, 7),  // HE25 → AM4 X7
        (3, 5),  // HE26 → AM4 X5
        (3, 2),  // HE27 → AM4 X2
        // AM2 continued — HE28–HE29
        (1, 3),  // HE28 → AM2 X3
        (1, 0),  // HE29 → AM2 X0
        // AM5 (mux 4, Daisy19/A4) — HE30–HE37
        (4, 4),  // HE30 → AM5 X4
        (4, 6),  // HE31 → AM5 X6
        (4, 7),  // HE32 → AM5 X7
        (4, 5),  // HE33 → AM5 X5
        (4, 2),  // HE34 → AM5 X2
        (4, 1),  // HE35 → AM5 X1
        (4, 0),  // HE36 → AM5 X0
        (4, 3),  // HE37 → AM5 X3
        // AM6 (mux 5, Daisy20/A5) — HE38–HE40
        (5, 4),  // HE38 → AM6 X4
        (5, 6),  // HE39 → AM6 X6
        (5, 7),  // HE40 → AM6 X7
        // AM4 continued — HE41–HE43
        (3, 1),  // HE41 → AM4 X1
        (3, 0),  // HE42 → AM4 X0
        (3, 3),  // HE43 → AM4 X3
        // AM7 (mux 6, Daisy21/A6) — HE44–HE51
        (6, 4),  // HE44 → AM7 X4
        (6, 6),  // HE45 → AM7 X6
        (6, 7),  // HE46 → AM7 X7
        (6, 5),  // HE47 → AM7 X5
        (6, 2),  // HE48 → AM7 X2
        (6, 1),  // HE49 → AM7 X1
        (6, 0),  // HE50 → AM7 X0
        (6, 3),  // HE51 → AM7 X3
        // AM8 (mux 7, Daisy22) — HE52–HE53, HE67–HE70
        (7, 4),  // HE52
        (7, 6),  // HE53
        // AM6 continued — HE54–HE58
        (5, 5),  // HE54 → AM6 X5
        (5, 2),  // HE55 → AM6 X2
        (5, 1),  // HE56 → AM6 X1
        (5, 0),  // HE57 → AM6 X0
        (5, 3),  // HE58 → AM6 X3
        // AM9 (mux 8, Daisy23) — HE59–HE66
        (8, 4), (8, 6), (8, 7), (8, 5), (8, 2), (8, 1), (8, 0), (8, 3),
        // AM8 continued — HE67–HE70
        (7, 7), (7, 5), (7, 2), (7, 1),
        // AM10 (mux 9, Daisy24) — HE71–HE73, HE81–HE84 (X3=GND)
        (9, 4),  // HE71 → AM10 X4
        (9, 6),  // HE72 → AM10 X6
        (9, 7),  // HE73 → AM10 X7
        // AM11 (mux 10, Daisy24) — HE74–HE80 (X3=GND)
        (10, 4), // HE74 → AM11 X4
        (10, 6), // HE75 → AM11 X6
        (10, 7), // HE76 → AM11 X7
        (10, 5), // HE77 → AM11 X5
        (10, 2), // HE78 → AM11 X2
        (10, 1), // HE79 → AM11 X1
        (10, 0), // HE80 → AM11 X0
        // AM10 continued — HE81–HE84
        (9, 5),  // HE81 → AM10 X5
        (9, 2),  // HE82 → AM10 X2
        (9, 1),  // HE83 → AM10 X1
        (9, 0),  // HE84 → AM10 X0
        // AM12 (mux 11, Daisy25) — HE85–HE92
        (11, 4), (11, 6), (11, 7), (11, 5), (11, 2), (11, 1), (11, 0), (11, 3),
        // AM13 (mux 12, Daisy25) — HE93–HE100
        (12, 4), (12, 6), (12, 7), (12, 5), (12, 2), (12, 1), (12, 0), (12, 3),
    ];

    // Wicki-Hayden layout, derived from HE sensor number at each switch index.
    // Rows alternate between two whole-tone scales a perfect fourth apart:
    //   Odd rows  (1,3,5): Gb Ab Bb C D E F# G# A# … (MIDI base 66/78/90)
    //   Even rows (2,4):   B  C# D# F  G  A  B  C# … (MIDI base 71/83)
    // Within each row every step right = +2 semitones (whole step).
    // Row 1 (top/highest) → HE1–14 base 90, row 2 → HE15–29 base 83,
    // row 3 → HE30–43 base 78, row 4 → HE44–58 base 71,
    // row 5 (bottom/lowest) → HE59–70 base 66.  Switches 70–99 unused (note 0).
    const SWITCH_TO_NOTE: [u8; NUM_SWITCHES] = [
        // Row 1 — HE 1–14  (base 78, whole-tone steps)
        78, 80, 82, 84, 86, 88, 90, 92, 94, 96, 98, 100, 102, 104,
        // Row 2 — HE 15–29 (base 71)
        71, 73, 75, 77, 79, 81, 83, 85, 87, 89, 91, 93, 95, 97, 99,
        // Row 3 — HE 30–43 (base 66)
        66, 68, 70, 72, 74, 76, 78, 80, 82, 84, 86, 88, 90, 92,
        // Row 4 — HE 44–58 (base 59)
        59, 61, 63, 65, 67, 69, 71, 73, 75, 77, 79, 81, 83, 85, 87,
        // Row 5 — HE 59–70 (base 54)
        54, 58, 60, 62, 64, 66, 68, 70, 72, 74, 76,
        80, // Skip 56 and 78 becuase those keys don't exist on the board
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];

    // HE sensor number for each switch index — used in log messages.
    #[rustfmt::skip]
    const HE_NUM: [u8; NUM_SWITCHES] = [
         1,  2,  3,  4,  5,  6,  7,  8, // switch  0–7  : HE1–HE8   (AM1)
         9, 10, 11, 12, 13, 14,          // switch  8–13 : HE9–HE14  (AM2)
        15, 16, 17, 18, 19, 20, 21, 22,  // switch 14–21 : HE15–HE22 (AM3)
        23, 24, 25, 26, 27,              // switch 22–26 : HE23–HE27 (AM4)
        28, 29,                          // switch 27–28 : HE28–HE29 (AM2)
        30, 31, 32, 33, 34, 35, 36, 37, // switch 29–36 : HE30–HE37 (AM5)
        38, 39, 40,                      // switch 37–39 : HE38–HE40 (AM6)
        41, 42, 43,                      // switch 40–42 : HE41–HE43 (AM4)
        44, 45, 46, 47, 48, 49, 50, 51,  // switch 43–50 : HE44–HE51 (AM7)
        52, 53,                           // switch 51–52 : HE52–HE53 (AM8)
        54, 55, 56, 57, 58,               // switch 53–57 : HE54–HE58 (AM6)
        59, 60, 61, 62, 63, 64, 65, 66,  // switch 58–65 : HE59–HE66 (AM9)
        67, 68, 69, 70,                   // switch 66–69 : HE67–HE70 (AM8)
        71, 72, 73,                       // switch 70–72 : HE71–HE73  (AM10 part 1)
        74, 75, 76, 77, 78, 79, 80,       // switch 73–79 : HE74–HE80  (AM11)
        81, 82, 83, 84,                   // switch 80–83 : HE81–HE84  (AM10 part 2)
        85, 86, 87, 88, 89, 90, 91, 92,   // switch 84–91 : HE85–HE92  (AM12)
        93, 94, 95, 96, 97, 98, 99, 100,  // switch 92–99 : HE93–HE100 (AM13)
    ];

    use crate::midi_sender::MidiSender;
    use libdaisy::gpio::*;
    use libdaisy::logger;
    use libdaisy::{audio, system};
    use stm32h7xx_hal::time::MilliSeconds;

    use libdaisy::hal::{
        adc::{self, Adc, AdcSampleTime, Resolution},
        gpio::{Analog, Output, PushPull},
        prelude::*,
        serial::{config::Config as SerialConfig, SerialExt},
        stm32,
        time::U32Ext,
        timer,
    };
    use log::{info, warn};

    use embedded_graphics::{
        mono_font::{ascii::FONT_10X20, MonoTextStyleBuilder},
        pixelcolor::BinaryColor,
        prelude::*,
        text::{Baseline, Text},
    };
    use fugit::RateExtU32;
    use ssd1306::{mode::BufferedGraphicsMode, prelude::*, I2CDisplayInterface, Ssd1306};
    use stm32h7xx_hal::i2c::{I2c, I2cExt};

    type LcdDisplay = Ssd1306<
        ssd1306::prelude::I2CInterface<I2c<stm32::I2C1>>,
        ssd1306::prelude::DisplaySize128x32,
        BufferedGraphicsMode<ssd1306::prelude::DisplaySize128x32>,
    >;

    // T_1: 1+7.5=8.5 ADC cycles — sufficient for HE sensor push-pull outputs
    // (<<1ns RC constant through 60Ω mux + 5pF ADC cap).
    // T_16 (23.5 cycles) was too slow for 9 muxes within a 1ms TIM2 period.
    const ADC_SAMPLE_TIME: AdcSampleTime = AdcSampleTime::T_1;
    const ADC_RESOLUTION: Resolution = Resolution::TwelveBit;

    type AdcPins = (
        Daisy15<Analog>,
        Daisy16<Analog>,
        Daisy17<Analog>,
        Daisy18<Analog>,
        Daisy19<Analog>,
        Daisy20<Analog>,
        Daisy21<Analog>,
        Daisy22<Analog>,
        Daisy23<Analog>,
    );

    // ── Filter ────────────────────────────────────────────────────────────────
    #[derive(Clone, Copy)]
    pub struct ChannelFilter {
        ring: [u16; FILTER_SIZE],
        index: usize,
        sum: u32,
    }

    impl ChannelFilter {
        const fn new() -> Self {
            Self {
                ring: [0; FILTER_SIZE],
                index: 0,
                sum: 0,
            }
        }
        fn feed(&mut self, raw: u16) -> u16 {
            self.sum -= self.ring[self.index] as u32;
            self.sum += raw as u32;
            self.ring[self.index] = raw;
            self.index = (self.index + 1) % FILTER_SIZE;
            (self.sum >> FILTER_SHIFT) as u16
        }
        fn prime(&mut self, value: u16) {
            for slot in self.ring.iter_mut() {
                *slot = value;
            }
            self.sum = value as u32 * FILTER_SIZE as u32;
            self.index = 0;
        }
    }

    // ── State machine ─────────────────────────────────────────────────────────
    #[derive(Clone, Copy, Debug)]
    pub enum SwitchPhase {
        Idle,
        FirstActuated { tick: u32 },
        FullyActuated { velocity: u8 },
    }

    #[derive(Clone, Copy)]
    pub struct SwitchState {
        phase: SwitchPhase,
        pub last_adc: u16,
        debounce_count: u8,
    }

    impl SwitchState {
        const fn new() -> Self {
            Self {
                phase: SwitchPhase::Idle,
                last_adc: 0,
                debounce_count: 0,
            }
        }

        fn update(
            &mut self,
            adc_value: u16,
            baseline: u16,
            tick: u32,
            switch_idx: usize,
        ) -> Option<SwitchEvent> {
            self.last_adc = adc_value;
            // Absolute delta — detects press in either direction (north or south pole).
            let delta = adc_value.abs_diff(baseline);

            match self.phase {
                SwitchPhase::Idle => {
                    if delta >= FIRST_DELTA {
                        self.debounce_count = self.debounce_count.saturating_add(1);
                        if self.debounce_count >= DEBOUNCE_TICKS {
                            if DIAG_LOGGING {
                                info!(
                                    "switch={} FirstActuated delta={} adc={} base={}",
                                    switch_idx, delta, adc_value, baseline
                                );
                            }
                            self.phase = SwitchPhase::FirstActuated { tick };
                            self.debounce_count = 0;
                        }
                    } else {
                        self.debounce_count = 0;
                    }
                    None
                }
                SwitchPhase::FirstActuated { tick: t1 } => {
                    if delta >= SECOND_DELTA {
                        self.debounce_count = self.debounce_count.saturating_add(1);
                        if self.debounce_count >= DEBOUNCE_TICKS {
                            let elapsed = tick.saturating_sub(t1);
                            let velocity = if elapsed == 0 {
                                127u8
                            } else if elapsed >= VELOCITY_WINDOW_MS {
                                1u8
                            } else {
                                let v = 127u32.saturating_sub((elapsed * 126) / VELOCITY_WINDOW_MS);
                                (v + 1).min(127) as u8
                            };
                            if DIAG_LOGGING {
                                info!(
                                    "switch={} FullyActuated elapsed={}ms vel={}",
                                    switch_idx, elapsed, velocity
                                );
                            }
                            self.phase = SwitchPhase::FullyActuated { velocity };
                            self.debounce_count = 0;
                            Some(SwitchEvent::NoteOn { velocity })
                        } else {
                            None
                        }
                    } else if delta < RELEASE_DELTA {
                        self.debounce_count = self.debounce_count.saturating_add(1);
                        if self.debounce_count >= DEBOUNCE_TICKS {
                            self.phase = SwitchPhase::Idle;
                            self.debounce_count = 0;
                        }
                        None
                    } else {
                        self.debounce_count = 0;
                        None
                    }
                }
                SwitchPhase::FullyActuated { .. } => {
                    if delta < RELEASE_DELTA {
                        self.debounce_count = self.debounce_count.saturating_add(1);
                        if self.debounce_count >= DEBOUNCE_TICKS {
                            self.phase = SwitchPhase::Idle;
                            self.debounce_count = 0;
                            Some(SwitchEvent::NoteOff)
                        } else {
                            None
                        }
                    } else {
                        self.debounce_count = 0;
                        None
                    }
                }
            }
        }
    }

    #[derive(Debug)]
    pub enum SwitchEvent {
        NoteOn { velocity: u8 },
        NoteOff,
        PotChange { cc: u8, value: u8 },
        PitchBend { value: u16 },
    }

    type MuxRaw = [[u16; MUX_CHANNELS]; NUM_MUXES];

    // ── Resources ─────────────────────────────────────────────────────────────
    #[shared]
    struct Shared {
        tick_ms: u32,
        switch_states: [SwitchState; NUM_SWITCHES],
        baselines: [u16; NUM_SWITCHES],
        event_queue: heapless::spsc::Queue<(usize, SwitchEvent), 64>,
    }

    #[local]
    struct Local {
        audio: audio::Audio,
        adc: Adc<stm32::ADC1, adc::Enabled>,
        adc_pins: AdcPins,
        enb_a: Daisy4<Output<PushPull>>, // U1 A0 (ENB_A, D4)
        enb_b: Daisy3<Output<PushPull>>, // U1 A1 (ENB_B, D3)
        enb_c: Daisy2<Output<PushPull>>, // U1 A2 (ENB_C, D2)
        adc_pin_a9: Daisy24<Analog>,     // AM10+AM11 shared (PA1 ch17)
        adc_pin_a10: Daisy25<Analog>,    // AM12+AM13 shared (PA0 ch16)
        // Pad 33 (D26/PD11) has no ADC. Fix: solder a wire pad 33 → pad 35,
        // then leave Daisy26 unconfigured (floating/high-Z, no firmware code).
        // Pad 35 = A11 / D28 / ADC11 — readable via ADC1.
        adc_pin_a11: Daisy28<Analog>, // AM14+AM15 pots via pad 35 (A11 / ADC11)
        pot_last_cc: [u8; NUM_POTS],  // last transmitted CC value per pot
        s0: Daisy7<Output<PushPull>>, // MUX_SELECT_0
        s1: Daisy8<Output<PushPull>>, // MUX_SELECT_1
        s2: Daisy9<Output<PushPull>>, // MUX_SELECT_2
        led1: Daisy6<Output<PushPull>>, // active-low
        led2: Daisy5<Output<PushPull>>, // active-low
        led3: Daisy1<Output<PushPull>>, // active-low
        timer2: timer::Timer<stm32::TIM2>,
        mux_raw: MuxRaw,
        midi_sender: MidiSender,
        filters: [ChannelFilter; NUM_SWITCHES],
        last_pitch_bend: u16,
        last_vibrato_cc: u8,
        display: Option<LcdDisplay>,
    }

    // ── init ──────────────────────────────────────────────────────────────────
    #[init]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        logger::init();

        let mut core = ctx.core;
        let device = ctx.device;

        let ccdr = system::System::init_clocks(device.PWR, device.RCC, &device.SYSCFG);
        let mut system = libdaisy::system_init!(core, device, ccdr, BLOCK_SIZE);

        let mut s0 = system
            .gpio
            .daisy7
            .take()
            .expect("daisy7")
            .into_push_pull_output();
        let mut s1 = system
            .gpio
            .daisy8
            .take()
            .expect("daisy8")
            .into_push_pull_output();
        let mut s2 = system
            .gpio
            .daisy9
            .take()
            .expect("daisy9")
            .into_push_pull_output();

        let mut adc_pins = (
            system.gpio.daisy15.take().expect("daisy15").into_analog(), // AM1 A0
            system.gpio.daisy16.take().expect("daisy16").into_analog(), // AM2 A1
            system.gpio.daisy17.take().expect("daisy17").into_analog(), // AM3 A2
            system.gpio.daisy18.take().expect("daisy18").into_analog(), // AM4 A3
            system.gpio.daisy19.take().expect("daisy19").into_analog(), // AM5 A4
            system.gpio.daisy20.take().expect("daisy20").into_analog(), // AM6 A5
            system.gpio.daisy21.take().expect("daisy21").into_analog(), // AM7 A6
            system.gpio.daisy22.take().expect("daisy22").into_analog(), // AM8 A7
            system.gpio.daisy23.take().expect("daisy23").into_analog(), // AM9 A8
        );

        let mut enb_a = system
            .gpio
            .daisy4
            .take()
            .expect("daisy4")
            .into_push_pull_output();
        let mut enb_b = system
            .gpio
            .daisy3
            .take()
            .expect("daisy3")
            .into_push_pull_output();
        let mut enb_c = system
            .gpio
            .daisy2
            .take()
            .expect("daisy2")
            .into_push_pull_output();
        let mut adc_pin_a9 = system.gpio.daisy24.take().expect("daisy24").into_analog();
        let mut adc_pin_a10 = system.gpio.daisy25.take().expect("daisy25").into_analog();
        // Pad 35 (D28 / A11 / ADC11) reads AM14+AM15 pots.
        // Requires a wire from pad 33 → pad 35 on the Daisy Seed; leave D26 unconfigured.
        let adc_pin_a11 = system.gpio.daisy28.take().expect("daisy28").into_analog();
        enb_a.set_low();
        enb_b.set_low();
        enb_c.set_low(); // select AM10

        let mut adc = system.adc1.enable();
        adc.set_resolution(ADC_RESOLUTION);
        adc.set_sample_time(ADC_SAMPLE_TIME);

        cortex_m::asm::delay(480 * 50_000);

        // ── Boot calibration ──────────────────────────────────────────────────
        let mut slot_sum = [[0u32; MUX_CHANNELS]; NUM_MUXES];
        let mut slot_count = [[0u32; MUX_CHANNELS]; NUM_MUXES];

        for ch in 0..MUX_CHANNELS {
            set_mux_channel(ch, &mut s0, &mut s1, &mut s2);
            cortex_m::asm::delay(480 * 200);

            for _ in 0..CALIBRATION_SAMPLES {
                let readings = read_all_adcs(&mut adc, &mut adc_pins);
                for mux in 0..NUM_ADC_PINS {
                    slot_sum[mux][ch] += readings[mux] as u32;
                    slot_count[mux][ch] += 1;
                }
                cortex_m::asm::delay(480 * 10);
                // AM10–AM13 via decoder
                for decoder_idx in 0..4u8 {
                    set_decoder(decoder_idx, &mut enb_a, &mut enb_b, &mut enb_c);
                    cortex_m::asm::delay(480 * 5);
                    let mux_idx = 9 + decoder_idx as usize;
                    let r = if decoder_idx < 2 {
                        adc.read(&mut adc_pin_a9).unwrap_or(0u32) as u16
                    } else {
                        adc.read(&mut adc_pin_a10).unwrap_or(0u32) as u16
                    };
                    slot_sum[mux_idx][ch] += r as u32;
                    slot_count[mux_idx][ch] += 1;
                }
            }
        }

        let mut baselines = [0u16; NUM_SWITCHES];
        let mut filters = [ChannelFilter::new(); NUM_SWITCHES];

        for (switch_idx, &(mux, ch)) in SWITCH_MAP.iter().enumerate() {
            let avg = slot_sum[mux as usize][ch as usize]
                .checked_div(slot_count[mux as usize][ch as usize])
                .unwrap_or(0) as u16;
            baselines[switch_idx] = avg;
            filters[switch_idx].prime(avg);
            if DIAG_LOGGING {
                info!(
                    "baseline switch={} mux={} ch={} val={}",
                    switch_idx, mux, ch, avg
                );
            }
        }

        // ── MIDI UART ─────────────────────────────────────────────────────────
        let midi_tx_pin = system
            .gpio
            .daisy13
            .take()
            .expect("daisy13")
            .into_alternate::<7>();
        let midi_rx_pin = system
            .gpio
            .daisy14
            .take()
            .expect("daisy14")
            .into_alternate::<7>();
        let midi_config = SerialConfig {
            baudrate: 31_250_u32.bps(),
            ..SerialConfig::default()
        };
        let midi_serial = device
            .USART1
            .serial(
                (midi_tx_pin, midi_rx_pin),
                midi_config,
                ccdr.peripheral.USART1,
                &ccdr.clocks,
            )
            .unwrap();
        let (midi_tx, _) = midi_serial.split();
        let midi_sender = MidiSender::new(midi_tx, 0);

        // ── Timer2 @ 1 kHz ────────────────────────────────────────────────────
        let mut timer2 = stm32h7xx_hal::timer::TimerExt::timer(
            device.TIM2,
            MilliSeconds::from_ticks(1).into_rate(),
            ccdr.peripheral.TIM2,
            &ccdr.clocks,
        );
        timer2.listen(timer::Event::TimeOut);

        let mut led1 = system
            .gpio
            .daisy6
            .take()
            .expect("daisy6")
            .into_push_pull_output();
        let mut led2 = system
            .gpio
            .daisy5
            .take()
            .expect("daisy5")
            .into_push_pull_output();
        let mut led3 = system
            .gpio
            .daisy1
            .take()
            .expect("daisy1")
            .into_push_pull_output();

        // All LEDs off initially (active-low)
        led1.set_high();
        led2.set_high();
        led3.set_high();

        // ── I2C bus recovery ───────────────────────────────────────────────────
        // Pulse SCL 9× as GPIO to release any device holding SDA low after reset.
        let mut scl = system
            .gpio
            .daisy11
            .take()
            .expect("daisy11")
            .into_push_pull_output();
        let mut sda = system
            .gpio
            .daisy12
            .take()
            .expect("daisy12")
            .into_push_pull_output();
        scl.set_high();
        sda.set_high();
        cortex_m::asm::delay(480 * 10);
        for _ in 0..9 {
            scl.set_low();
            cortex_m::asm::delay(480 * 5);
            scl.set_high();
            cortex_m::asm::delay(480 * 5);
        }
        // STOP condition: SDA rises while SCL high
        scl.set_low();
        cortex_m::asm::delay(480 * 5);
        sda.set_low();
        cortex_m::asm::delay(480 * 5);
        scl.set_high();
        cortex_m::asm::delay(480 * 5);
        sda.set_high();
        cortex_m::asm::delay(480 * 10);

        // ── I2C + SSD1306 display ─────────────────────────────────────────────
        let scl = scl
            .into_alternate::<4>()
            .internal_pull_up(true)
            .set_open_drain();
        let sda = sda
            .into_alternate::<4>()
            .internal_pull_up(true)
            .set_open_drain();
        let i2c = device.I2C1.i2c(
            (scl, sda),
            100_u32.kHz(),
            ccdr.peripheral.I2C1,
            &ccdr.clocks,
        );
        let i2c_interface = I2CDisplayInterface::new_custom_address(i2c, 0x3C);
        // Only construct the driver here — no I2C bus traffic until display_init task runs.
        let display = Some(
            Ssd1306::new(i2c_interface, DisplaySize128x32, DisplayRotation::Rotate0)
                .into_buffered_graphics_mode(),
        );
        display_init::spawn().ok();

        set_mux_channel(0, &mut s0, &mut s1, &mut s2);
        info!(
            "keyboard_keyboard ready: {} switches on {} muxes (DIAG_LOGGING={})",
            NUM_SWITCHES, NUM_MUXES, DIAG_LOGGING
        );

        (
            Shared {
                tick_ms: 0,
                switch_states: [SwitchState::new(); NUM_SWITCHES],
                baselines,
                event_queue: heapless::spsc::Queue::new(),
            },
            Local {
                audio: system.audio,
                adc,
                adc_pins,
                enb_a,
                enb_b,
                enb_c,
                adc_pin_a9,
                adc_pin_a10,
                adc_pin_a11,
                pot_last_cc: [255u8; NUM_POTS], // 255 forces CC send on first scan
                s0,
                s1,
                s2,
                led1,
                led2,
                led3,
                timer2,
                mux_raw: [[0u16; MUX_CHANNELS]; NUM_MUXES],
                midi_sender,
                filters,
                last_pitch_bend: 0x2000,
                last_vibrato_cc: 0,
                display,
            },
            init::Monotonics(),
        )
    }

    #[idle]
    fn idle(_ctx: idle::Context) -> ! {
        loop {
            cortex_m::asm::nop();
        }
    }

    // Runs once after boot at priority 1. display_init is intentionally below
    // process_events (priority 2) so a slow/absent display never blocks key events.
    #[task(local = [display], priority = 1)]
    fn display_init(ctx: display_init::Context) {
        info!("display_init start");
        let Some(disp) = ctx.local.display.as_mut() else {
            warn!("display resource is None");
            return;
        };
        info!("display calling init");
        match disp.init() {
            Ok(()) => {
                disp.clear();
                let style = MonoTextStyleBuilder::new()
                    .font(&FONT_10X20)
                    .text_color(BinaryColor::On)
                    .build();
                // "key*key" @ FONT_10X20 = 70 px wide, 20 px tall — centered on 128x32
                Text::with_baseline("key*key", Point::new(29, 6), style, Baseline::Top)
                    .draw(disp)
                    .ok();
                disp.flush().ok();
                info!("display ok");
            }
            Err(_) => warn!("display not found"),
        }
    }

    #[task(binds = DMA1_STR1, priority = 8, local = [audio])]
    fn audio_handler(ctx: audio_handler::Context) {
        ctx.local.audio.for_each(|left, right| (left, right));
    }

    #[task(
        binds = TIM2,
        local  = [timer2, adc, adc_pins, enb_a, enb_b, enb_c, adc_pin_a9, adc_pin_a10, adc_pin_a11, pot_last_cc, s0, s1, s2, mux_raw, filters, last_pitch_bend, last_vibrato_cc, led1, led2, led3],
        shared = [tick_ms, switch_states, baselines, event_queue],
        priority = 15
    )]
    fn timer_handler(mut ctx: timer_handler::Context) {
        ctx.local.timer2.clear_irq();

        let now = ctx.shared.tick_ms.lock(|t| {
            *t = t.wrapping_add(1);
            *t
        });

        // Heartbeat BEFORE scan — confirms timer is alive even if ADC scan hangs.
        if now % 2000 == 0 {
            info!("tick={}", now);
        }

        let baselines = ctx.shared.baselines.lock(|b| *b);
        let mut pending: heapless::Vec<(usize, SwitchEvent), 32> = heapless::Vec::new();

        for ch in 0..MUX_CHANNELS {
            set_mux_channel(ch, ctx.local.s0, ctx.local.s1, ctx.local.s2);
            cortex_m::asm::delay(480 * 10);
            let readings = read_all_adcs(ctx.local.adc, ctx.local.adc_pins);
            for (mux, &val) in readings.iter().enumerate() {
                ctx.local.mux_raw[mux][ch] = val;
            }
            // AM10–AM13 via U1 decoder
            for decoder_idx in 0..4u8 {
                set_decoder(
                    decoder_idx,
                    ctx.local.enb_a,
                    ctx.local.enb_b,
                    ctx.local.enb_c,
                );
                cortex_m::asm::delay(480 * 5);
                let mux_idx = 9 + decoder_idx as usize;
                ctx.local.mux_raw[mux_idx][ch] = if decoder_idx < 2 {
                    ctx.local.adc.read(ctx.local.adc_pin_a9).unwrap_or(0u32) as u16
                } else {
                    ctx.local.adc.read(ctx.local.adc_pin_a10).unwrap_or(0u32) as u16
                };
            }
        }

        let mut pb_filt_down = baselines[PITCH_BEND_DOWN];
        let mut pb_filt_up = baselines[PITCH_BEND_UP];
        let mut vib_filt_a = baselines[VIBRATO_A];
        let mut vib_filt_b = baselines[VIBRATO_B];

        ctx.shared.switch_states.lock(|states| {
            for (switch_idx, &(mux, ch)) in SWITCH_MAP.iter().enumerate() {
                let raw = ctx.local.mux_raw[mux as usize][ch as usize];
                let filtered = ctx.local.filters[switch_idx].feed(raw);

                if switch_idx == PITCH_BEND_DOWN {
                    pb_filt_down = filtered;
                } else if switch_idx == PITCH_BEND_UP {
                    pb_filt_up = filtered;
                } else if switch_idx == VIBRATO_A {
                    vib_filt_a = filtered;
                } else if switch_idx == VIBRATO_B {
                    vib_filt_b = filtered;
                } else if let Some(event) =
                    states[switch_idx].update(filtered, baselines[switch_idx], now, switch_idx)
                {
                    pending.push((switch_idx, event)).ok();
                }
            }
        });

        // Pitch bend from HE71 (bend down) and HE73 (bend up), rate-limited.
        if now % PITCH_BEND_INTERVAL_MS == 0 {
            let delta_down = pb_filt_down.abs_diff(baselines[PITCH_BEND_DOWN]);
            let delta_up = pb_filt_up.abs_diff(baselines[PITCH_BEND_UP]);
            let pb_value = if delta_down < RELEASE_DELTA && delta_up < RELEASE_DELTA {
                0x2000u16 // snap to center when both sensors at rest
            } else {
                let d_down = delta_down.min(PITCH_BEND_MAX_DELTA);
                let d_up = delta_up.min(PITCH_BEND_MAX_DELTA);
                let bend_down = (d_down as u32 * 0x2000 / PITCH_BEND_MAX_DELTA as u32) as u16;
                let bend_up = (d_up as u32 * 0x1FFF / PITCH_BEND_MAX_DELTA as u32) as u16;
                (0x2000u16.saturating_sub(bend_down))
                    .saturating_add(bend_up)
                    .min(0x3FFF)
            };
            let prev_pb = *ctx.local.last_pitch_bend;
            if pb_value.abs_diff(prev_pb) >= PITCH_BEND_HYSTERESIS {
                *ctx.local.last_pitch_bend = pb_value;
                pending
                    .push((0, SwitchEvent::PitchBend { value: pb_value }))
                    .ok();
            }
        }

        if DIAG_LOGGING && now % LOG_INTERVAL_MS == 0 {
            // Print raw ADC for all 5 mux outputs at the current mux channel state.
            // Row format: AM1..AM5 raw values + signed delta from baseline for LOG_SWITCH.
            let (lk_mux, lk_ch) = SWITCH_MAP[LOG_SWITCH];
            let lk_raw = ctx.local.mux_raw[lk_mux as usize][lk_ch as usize];
            let lk_filt = (ctx.local.filters[LOG_SWITCH].sum >> FILTER_SHIFT) as u16;
            let lk_base = baselines[LOG_SWITCH];
            let lk_delta: i32 = lk_filt as i32 - lk_base as i32;
            info!(
                "DIAG HE{} raw={} filt={} base={} delta={:+}",
                HE_NUM[LOG_SWITCH], lk_raw, lk_filt, lk_base, lk_delta
            );
            // Also show a snapshot of all 5 mux outputs for the X4 channel
            // so you can see which mux is responding to presses.
            info!(
                "ADC_X4: AM1={} AM2={} AM3={} AM4={} AM5={}",
                ctx.local.mux_raw[0][4],
                ctx.local.mux_raw[1][4],
                ctx.local.mux_raw[2][4],
                ctx.local.mux_raw[3][4],
                ctx.local.mux_raw[4][4],
            );
        }

        // Vibrato depth from HE77/HE79 (up/down arrows) → CC1, rate-limited.
        if now % VIBRATO_INTERVAL_MS == 0 {
            let delta_a = vib_filt_a.abs_diff(baselines[VIBRATO_A]);
            let delta_b = vib_filt_b.abs_diff(baselines[VIBRATO_B]);
            let max_delta = delta_a.max(delta_b);
            let cc_val =
                ((max_delta.min(VIBRATO_MAX_DELTA) as u32 * 127 / VIBRATO_MAX_DELTA as u32) as u8)
                    .min(127);
            let prev_vib = *ctx.local.last_vibrato_cc;
            if cc_val.abs_diff(prev_vib) >= VIBRATO_HYSTERESIS {
                *ctx.local.last_vibrato_cc = cc_val;
                pending
                    .push((
                        0,
                        SwitchEvent::PotChange {
                            cc: 1,
                            value: cc_val,
                        },
                    ))
                    .ok();
            }
        }

        // ── Pot scan (100 Hz) ─────────────────────────────────────────────────────────
        // Reads via Daisy28 (pad 35 / A11 / ADC11). Needs wire: pad 33 → pad 35.
        if now % POT_SCAN_MS == 0 {
            for (pot_idx, &(dec_idx, mux_ch, cc)) in POT_MAP.iter().enumerate() {
                set_mux_channel(mux_ch as usize, ctx.local.s0, ctx.local.s1, ctx.local.s2);
                set_decoder(dec_idx, ctx.local.enb_a, ctx.local.enb_b, ctx.local.enb_c);
                cortex_m::asm::delay(480 * 5);
                let raw = ctx.local.adc.read(ctx.local.adc_pin_a11).unwrap_or(0u32) as u16;
                let cc_val = (raw >> 5) as u8; // 12-bit → 7-bit CC
                let prev = ctx.local.pot_last_cc[pot_idx];
                let delta = cc_val.abs_diff(prev);
                if delta >= POT_CC_HYSTERESIS {
                    ctx.local.pot_last_cc[pot_idx] = cc_val;
                    pending
                        .push((0, SwitchEvent::PotChange { cc, value: cc_val }))
                        .ok();
                }
            }
            // Restore decoder to switch-scan state (AM10 = idx 0, A2 = 0)
            set_decoder(0, ctx.local.enb_a, ctx.local.enb_b, ctx.local.enb_c);
        }

        // ── Sequential LED chase: 500 ms per LED (active-low)
        let phase = now % 1500;
        if phase < 500 {
            ctx.local.led1.set_low();
            ctx.local.led2.set_high();
            ctx.local.led3.set_high();
        } else if phase < 1000 {
            ctx.local.led1.set_high();
            ctx.local.led2.set_low();
            ctx.local.led3.set_high();
        } else {
            ctx.local.led1.set_high();
            ctx.local.led2.set_high();
            ctx.local.led3.set_low();
        }

        if !pending.is_empty() {
            ctx.shared.event_queue.lock(|queue| {
                for item in pending {
                    queue.enqueue(item).ok();
                }
            });
            process_events::spawn().ok();
        }
    }

    #[task(shared = [event_queue], local = [midi_sender], priority = 2, capacity = 32)]
    fn process_events(mut ctx: process_events::Context) {
        ctx.shared.event_queue.lock(|queue| {
            while let Some((switch_idx, event)) = queue.dequeue() {
                let he = HE_NUM[switch_idx];
                let is_drum = switch_idx >= DRUM_SWITCH_START
                    && switch_idx < DRUM_SWITCH_START + DRUM_NOTE.len();
                let (note, channel) = if is_drum {
                    (DRUM_NOTE[switch_idx - DRUM_SWITCH_START], DRUM_CHANNEL)
                } else {
                    (SWITCH_TO_NOTE[switch_idx], 0u8)
                };
                if note == 0 {
                    continue;
                }
                ctx.local.midi_sender.set_channel(channel);
                match event {
                    SwitchEvent::NoteOn { velocity } => {
                        info!(
                            "NoteOn  HE{} switch={} ch={} note={} vel={}",
                            he,
                            switch_idx,
                            channel + 1,
                            note,
                            velocity
                        );
                        ctx.local.midi_sender.note_on(note, velocity);
                    }
                    SwitchEvent::NoteOff => {
                        info!(
                            "NoteOff HE{} switch={} ch={} note={}",
                            he,
                            switch_idx,
                            channel + 1,
                            note
                        );
                        ctx.local.midi_sender.note_off(note, 0);
                    }
                    SwitchEvent::PotChange { cc, value } => {
                        info!("CC{} = {}", cc, value);
                        ctx.local.midi_sender.control_change(cc, value);
                    }
                    SwitchEvent::PitchBend { value } => {
                        info!("PitchBend value={}", value);
                        ctx.local.midi_sender.pitch_bend(value);
                    }
                }
            }
        });
    }

    #[inline(always)]
    fn set_mux_channel(
        ch: usize,
        s0: &mut Daisy7<Output<PushPull>>,
        s1: &mut Daisy8<Output<PushPull>>,
        s2: &mut Daisy9<Output<PushPull>>,
    ) {
        if ch & 0b001 != 0 {
            s0.set_high()
        } else {
            s0.set_low()
        }
        if ch & 0b010 != 0 {
            s1.set_high()
        } else {
            s1.set_low()
        }
        if ch & 0b100 != 0 {
            s2.set_high()
        } else {
            s2.set_low()
        }
    }

    #[inline(always)]
    fn set_decoder(
        idx: u8, // 0=AM10, 1=AM11, 2=AM12, 3=AM13
        enb_a: &mut Daisy4<Output<PushPull>>,
        enb_b: &mut Daisy3<Output<PushPull>>,
        enb_c: &mut Daisy2<Output<PushPull>>,
    ) {
        if idx & 0b001 != 0 {
            enb_a.set_high()
        } else {
            enb_a.set_low()
        }
        if idx & 0b010 != 0 {
            enb_b.set_high()
        } else {
            enb_b.set_low()
        }
        // A2 bit: 0 = AM10–13 (switches), 1 = AM14–15 (pots)
        if idx & 0b100 != 0 {
            enb_c.set_high()
        } else {
            enb_c.set_low()
        }
    }

    #[inline(always)]
    fn read_all_adcs(
        adc: &mut Adc<stm32::ADC1, adc::Enabled>,
        pins: &mut AdcPins,
    ) -> [u16; NUM_ADC_PINS] {
        let r = |res: Result<u32, _>| res.unwrap_or(0) as u16;
        [
            r(adc.read(&mut pins.0)),
            r(adc.read(&mut pins.1)),
            r(adc.read(&mut pins.2)),
            r(adc.read(&mut pins.3)),
            r(adc.read(&mut pins.4)),
            r(adc.read(&mut pins.5)),
            r(adc.read(&mut pins.6)),
            r(adc.read(&mut pins.7)),
            r(adc.read(&mut pins.8)),
        ]
    }
}
