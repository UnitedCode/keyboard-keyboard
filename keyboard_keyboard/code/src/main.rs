//! hall_effect_keyboard — Daisy Seed + SN74LV4051A × 5 + MT9102ET × 40
//! MIDI output over USART1 at 31250 baud
//!
//! ## Correct pin mapping from schematic:
//!
//! Select lines:
//!   Daisy7  (D6) → MUX_SELECT_0 (A / bit 0)
//!   Daisy8  (D7) → MUX_SELECT_1 (B / bit 1)
//!   Daisy9  (D8) → MUX_SELECT_2 (C / bit 2)
//!
//! Mux outputs:
//!   Daisy15 (PC0) → AM1
//!   Daisy16 (PA3) → AM2
//!   Daisy17 (PB1) → AM3
//!   Daisy18 (PA7) → AM4
//!   Daisy19 (PA6) → AM7

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

    const NUM_KEYS: usize = 40;
    const NUM_MUXES: usize = 5;
    const NUM_ADC_PINS: usize = 5;
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

    const DIAG_LOGGING: bool = true;
    const LOG_INTERVAL_MS: u32 = 200;
    const LOG_KEY: usize = 9; // HE10

    // ── Key map ───────────────────────────────────────────────────────────────
    #[rustfmt::skip]
    const KEY_MAP: [(u8, u8); NUM_KEYS] = [
        (0, 4),  // HE1  → AM1 X4
        (0, 6),  // HE2  → AM1 X6
        (0, 7),  // HE3  → AM1 X7
        (0, 5),  // HE4  → AM1 X5
        (0, 2),  // HE5  → AM1 X2
        (0, 1),  // HE6  → AM1 X1
        (0, 0),  // HE7  → AM1 X0
        (0, 3),  // HE8  → AM1 X3
        (1, 4),  // HE9  → AM2 X4
        (1, 6),  // HE10 → AM2 X6
        (1, 7),  // HE11 → AM2 X7
        (1, 5),  // HE12 → AM2 X5
        (1, 2),  // HE13 → AM2 X2
        (1, 1),  // HE14 → AM2 X1
        (2, 4),  // HE15 → AM3 X4
        (2, 6),  // HE16 → AM3 X6
        (2, 7),  // HE17 → AM3 X7
        (2, 5),  // HE18 → AM3 X5
        (2, 2),  // HE19 → AM3 X2
        (2, 1),  // HE20 → AM3 X1
        (2, 0),  // HE21 → AM3 X0
        (2, 3),  // HE22 → AM3 X3
        (3, 4),  // HE23 → AM4 X4
        (3, 6),  // HE24 → AM4 X6
        (3, 7),  // HE25 → AM4 X7
        (3, 5),  // HE26 → AM4 X5
        (3, 2),  // HE27 → AM4 X2
        (1, 3),  // HE28 → AM2 X3
        (1, 0),  // HE29 → AM2 X0
        (3, 1),  // HE41 → AM4 X1
        (3, 0),  // HE42 → AM4 X0
        (3, 3),  // HE43 → AM4 X3
        (4, 4),  // HE44 → AM7 X4
        (4, 6),  // HE45 → AM7 X6
        (4, 7),  // HE46 → AM7 X7
        (4, 5),  // HE47 → AM7 X5
        (4, 2),  // HE48 → AM7 X2
        (4, 1),  // HE49 → AM7 X1
        (4, 0),  // HE50 → AM7 X0
        (4, 3),  // HE51 → AM7 X3
    ];

    const KEY_TO_NOTE: [u8; NUM_KEYS] = {
        let pattern = [48u8, 50, 52, 55, 57, 60, 62, 64, 67, 69];
        let mut notes = [0u8; NUM_KEYS];
        let mut i = 0;
        while i < NUM_KEYS {
            let octave = (i / pattern.len()) as u8;
            notes[i] = pattern[i % pattern.len()].saturating_add(octave * 12);
            i += 1;
        }
        notes
    };

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
    use log::info;

    const ADC_SAMPLE_TIME: AdcSampleTime = AdcSampleTime::T_16;
    const ADC_RESOLUTION: Resolution = Resolution::TwelveBit;

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
    pub enum KeyPhase {
        Idle,
        FirstActuated { tick: u32 },
        FullyActuated { velocity: u8 },
    }

    #[derive(Clone, Copy)]
    pub struct KeyState {
        phase: KeyPhase,
        pub last_adc: u16,
        debounce_count: u8,
    }

    impl KeyState {
        const fn new() -> Self {
            Self {
                phase: KeyPhase::Idle,
                last_adc: 0,
                debounce_count: 0,
            }
        }

        fn update(
            &mut self,
            adc_value: u16,
            baseline: u16,
            tick: u32,
            key_idx: usize,
        ) -> Option<KeyEvent> {
            self.last_adc = adc_value;
            let delta = adc_value.saturating_sub(baseline);

            match self.phase {
                KeyPhase::Idle => {
                    if delta >= FIRST_DELTA {
                        self.debounce_count = self.debounce_count.saturating_add(1);
                        if self.debounce_count >= DEBOUNCE_TICKS {
                            info!(
                                "key={} FirstActuated delta={} adc={} base={}",
                                key_idx, delta, adc_value, baseline
                            );
                            self.phase = KeyPhase::FirstActuated { tick };
                            self.debounce_count = 0;
                        }
                    } else {
                        self.debounce_count = 0;
                    }
                    None
                }
                KeyPhase::FirstActuated { tick: t1 } => {
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
                            info!(
                                "key={} FullyActuated elapsed={}ms vel={}",
                                key_idx, elapsed, velocity
                            );
                            self.phase = KeyPhase::FullyActuated { velocity };
                            self.debounce_count = 0;
                            Some(KeyEvent::NoteOn { velocity })
                        } else {
                            None
                        }
                    } else if delta < RELEASE_DELTA {
                        self.debounce_count = self.debounce_count.saturating_add(1);
                        if self.debounce_count >= DEBOUNCE_TICKS {
                            self.phase = KeyPhase::Idle;
                            self.debounce_count = 0;
                        }
                        None
                    } else {
                        self.debounce_count = 0;
                        None
                    }
                }
                KeyPhase::FullyActuated { .. } => {
                    if delta < RELEASE_DELTA {
                        self.debounce_count = self.debounce_count.saturating_add(1);
                        if self.debounce_count >= DEBOUNCE_TICKS {
                            self.phase = KeyPhase::Idle;
                            self.debounce_count = 0;
                            Some(KeyEvent::NoteOff)
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

        fn is_idle(&self) -> bool {
            matches!(self.phase, KeyPhase::Idle)
        }
    }

    #[derive(Debug)]
    pub enum KeyEvent {
        NoteOn { velocity: u8 },
        NoteOff,
    }

    type MuxRaw = [[u16; MUX_CHANNELS]; NUM_MUXES];

    // ── Resources ─────────────────────────────────────────────────────────────
    #[shared]
    struct Shared {
        tick_ms: u32,
        key_states: [KeyState; NUM_KEYS],
        baselines: [u16; NUM_KEYS],
        event_queue: heapless::spsc::Queue<(usize, KeyEvent), 32>,
    }

    #[local]
    struct Local {
        audio: audio::Audio,
        adc: Adc<stm32::ADC1, adc::Enabled>,
        adc_pins: (
            Daisy15<Analog>, // AM1
            Daisy16<Analog>, // AM2
            Daisy17<Analog>, // AM3
            Daisy18<Analog>, // AM4
            Daisy19<Analog>, // AM7
        ),
        s0: Daisy7<Output<PushPull>>, // MUX_SELECT_0
        s1: Daisy8<Output<PushPull>>, // MUX_SELECT_1
        s2: Daisy9<Output<PushPull>>, // MUX_SELECT_2
        timer2: timer::Timer<stm32::TIM2>,
        mux_raw: MuxRaw,
        midi_sender: MidiSender,
        filters: [ChannelFilter; NUM_KEYS],
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
            system.gpio.daisy15.take().expect("daisy15").into_analog(),
            system.gpio.daisy16.take().expect("daisy16").into_analog(),
            system.gpio.daisy17.take().expect("daisy17").into_analog(),
            system.gpio.daisy18.take().expect("daisy18").into_analog(),
            system.gpio.daisy19.take().expect("daisy19").into_analog(),
        );

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
                for mux in 0..NUM_MUXES {
                    slot_sum[mux][ch] += readings[mux] as u32;
                    slot_count[mux][ch] += 1;
                }
                cortex_m::asm::delay(480 * 10);
            }
        }

        let mut baselines = [0u16; NUM_KEYS];
        let mut filters = [ChannelFilter::new(); NUM_KEYS];

        for (key_idx, &(mux, ch)) in KEY_MAP.iter().enumerate() {
            let avg = if slot_count[mux as usize][ch as usize] > 0 {
                (slot_sum[mux as usize][ch as usize] / slot_count[mux as usize][ch as usize]) as u16
            } else {
                0
            };
            baselines[key_idx] = avg;
            filters[key_idx].prime(avg);
            info!("baseline key={} mux={} ch={} val={}", key_idx, mux, ch, avg);
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
        let mut midi_config = SerialConfig::default();
        midi_config.baudrate = 31_250_u32.bps();
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

        set_mux_channel(0, &mut s0, &mut s1, &mut s2);
        info!("keyboard_keyboard ready: {} keys", NUM_KEYS);

        (
            Shared {
                tick_ms: 0,
                key_states: [KeyState::new(); NUM_KEYS],
                baselines,
                event_queue: heapless::spsc::Queue::new(),
            },
            Local {
                audio: system.audio,
                adc,
                adc_pins,
                s0,
                s1,
                s2,
                timer2,
                mux_raw: [[0u16; MUX_CHANNELS]; NUM_MUXES],
                midi_sender,
                filters,
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

    #[task(binds = DMA1_STR1, priority = 8, local = [audio])]
    fn audio_handler(ctx: audio_handler::Context) {
        ctx.local.audio.for_each(|left, right| (left, right));
    }

    #[task(
        binds = TIM2,
        local  = [timer2, adc, adc_pins, s0, s1, s2, mux_raw, filters],
        shared = [tick_ms, key_states, baselines, event_queue],
        priority = 15
    )]
    fn timer_handler(mut ctx: timer_handler::Context) {
        ctx.local.timer2.clear_irq();

        let now = ctx.shared.tick_ms.lock(|t| {
            *t = t.wrapping_add(1);
            *t
        });
        let baselines = ctx.shared.baselines.lock(|b| *b);
        let mut pending: heapless::Vec<(usize, KeyEvent), 16> = heapless::Vec::new();

        for ch in 0..MUX_CHANNELS {
            set_mux_channel(ch, ctx.local.s0, ctx.local.s1, ctx.local.s2);
            cortex_m::asm::delay(480 * 10);
            let readings = read_all_adcs(ctx.local.adc, ctx.local.adc_pins);
            for mux in 0..NUM_MUXES {
                ctx.local.mux_raw[mux][ch] = readings[mux];
            }
        }

        ctx.shared.key_states.lock(|states| {
            for (key_idx, &(mux, ch)) in KEY_MAP.iter().enumerate() {
                let raw = ctx.local.mux_raw[mux as usize][ch as usize];
                let filtered = ctx.local.filters[key_idx].feed(raw);

                if let Some(event) =
                    states[key_idx].update(filtered, baselines[key_idx], now, key_idx)
                {
                    pending.push((key_idx, event)).ok();
                }
            }
        });

        if DIAG_LOGGING && now % LOG_INTERVAL_MS == 0 {
            let (mux, ch) = KEY_MAP[LOG_KEY];
            let raw = ctx.local.mux_raw[mux as usize][ch as usize];
            let filtered = (ctx.local.filters[LOG_KEY].sum >> FILTER_SHIFT) as u16;
            let baseline = baselines[LOG_KEY];
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

    #[task(shared = [event_queue], local = [midi_sender], priority = 1, capacity = 32)]
    fn process_events(mut ctx: process_events::Context) {
        ctx.shared.event_queue.lock(|queue| {
            while let Some((key_idx, event)) = queue.dequeue() {
                let note = KEY_TO_NOTE[key_idx];
                match event {
                    KeyEvent::NoteOn { velocity } => {
                        info!("NoteOn  key={} note={} vel={}", key_idx, note, velocity);
                        ctx.local.midi_sender.note_on(note, velocity);
                    }
                    KeyEvent::NoteOff => {
                        info!("NoteOff key={} note={}", key_idx, note);
                        ctx.local.midi_sender.note_off(note, 0);
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
    fn read_all_adcs(
        adc: &mut Adc<stm32::ADC1, adc::Enabled>,
        pins: &mut (
            Daisy15<Analog>,
            Daisy16<Analog>,
            Daisy17<Analog>,
            Daisy18<Analog>,
            Daisy19<Analog>,
        ),
    ) -> [u16; NUM_ADC_PINS] {
        let r = |res: Result<u32, _>| res.unwrap_or(0) as u16;
        [
            r(adc.read(&mut pins.0)),
            r(adc.read(&mut pins.1)),
            r(adc.read(&mut pins.2)),
            r(adc.read(&mut pins.3)),
            r(adc.read(&mut pins.4)),
        ]
    }
}
