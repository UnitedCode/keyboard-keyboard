//! hall_effect_keyboard — Daisy Seed MIDI keyboard firmware
//! MIDI out over USART1 at 31250 baud. See HARDWARE.md for full pin mapping.

#![no_main]
#![no_std]

use panic_rtt_target as _;

mod constants;
mod display;
mod hardware;
mod midi;
mod switch;
mod types;

#[rtic::app(
    device = stm32h7xx_hal::stm32,
    peripherals = true,
    dispatchers = [DMA1_STR2, DMA1_STR3, DMA1_STR4, DMA1_STR5, DMA1_STR6]
)]
mod app {
    use crate::constants::*;
    use crate::hardware::{
        i2c_bus_recovery, read_all_adcs, set_decoder, set_mux_channel, AdcPins, MuxRaw,
    };
    use crate::midi::MidiSender;
    use crate::switch::{ChannelFilter, SwitchEvent, SwitchState};
    use crate::types::{DisplayState, LastEvent, LcdDisplay};

    use libdaisy::gpio::*;
    use libdaisy::logger;
    use libdaisy::{audio, system};
    use stm32h7xx_hal::time::MilliSeconds;

    use libdaisy::hal::{
        adc::{self, Adc, AdcSampleTime, Resolution},
        gpio::{Analog, Output, PushPull},
        prelude::*,
        serial::{config::Config as SerialConfig, Rx, SerialExt},
        stm32,
        time::U32Ext,
        timer,
    };
    use log::{info, warn};

    use fugit::RateExtU32;
    use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306};
    use stm32h7xx_hal::i2c::I2cExt;

    // T_1 (8.5 ADC cycles) — sufficient for HE sensor push-pull outputs.
    // T_16 was too slow for 9 muxes within a 1 ms TIM2 period.
    const ADC_SAMPLE_TIME: AdcSampleTime = AdcSampleTime::T_1;
    const ADC_RESOLUTION: Resolution = Resolution::TwelveBit;

    // ── Shared resources ──────────────────────────────────────────────────────
    #[shared]
    struct Shared {
        tick_ms: u32,
        switch_states: [SwitchState; NUM_SWITCHES],
        baselines: [u16; NUM_SWITCHES],
        event_queue: heapless::spsc::Queue<(usize, SwitchEvent), 64>,
        midi_tx_flag: bool,
        display_state: DisplayState,
    }

    // ── Local resources ───────────────────────────────────────────────────────
    #[local]
    struct Local {
        audio: audio::Audio,
        adc: Adc<stm32::ADC1, adc::Enabled>,
        adc_pins: AdcPins,
        enb_a: Daisy4<Output<PushPull>>, // U1 A0 (ENB_A)
        enb_b: Daisy3<Output<PushPull>>, // U1 A1 (ENB_B)
        enb_c: Daisy2<Output<PushPull>>, // U1 A2 (ENB_C)
        adc_pin_a9: Daisy24<Analog>,     // AM10+AM11 shared
        adc_pin_a10: Daisy25<Analog>,    // AM12+AM13 shared
        // Pad 33 (D26) has no ADC — wire pad 33 → pad 35 on Daisy Seed.
        adc_pin_a11: Daisy28<Analog>, // AM14+AM15 pots via pad 35 (A11)
        pot_last_cc: [u8; NUM_POTS],
        s0: Daisy7<Output<PushPull>>,   // MUX_SELECT_0
        s1: Daisy8<Output<PushPull>>,   // MUX_SELECT_1
        s2: Daisy9<Output<PushPull>>,   // MUX_SELECT_2
        led1: Daisy6<Output<PushPull>>, // active-low — MIDI TX flash
        led2: Daisy5<Output<PushPull>>, // active-low — MIDI RX flash
        led3: Daisy1<Output<PushPull>>, // active-low — heartbeat
        timer2: timer::Timer<stm32::TIM2>,
        mux_raw: MuxRaw,
        midi_sender: MidiSender,
        midi_rx: Rx<stm32::USART1>,
        filters: [ChannelFilter; NUM_SWITCHES],
        last_pitch_bend: u16,
        last_vibrato_cc: u8,
        melody_channel: u8,
        led1_off_at: u32,
        led2_off_at: u32,
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
            system.gpio.daisy15.take().expect("daisy15").into_analog(),
            system.gpio.daisy16.take().expect("daisy16").into_analog(),
            system.gpio.daisy17.take().expect("daisy17").into_analog(),
            system.gpio.daisy18.take().expect("daisy18").into_analog(),
            system.gpio.daisy19.take().expect("daisy19").into_analog(),
            system.gpio.daisy20.take().expect("daisy20").into_analog(),
            system.gpio.daisy21.take().expect("daisy21").into_analog(),
            system.gpio.daisy22.take().expect("daisy22").into_analog(),
            system.gpio.daisy23.take().expect("daisy23").into_analog(),
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
        // Pad 35 (D28/A11) reads AM14+AM15. Requires bodge wire pad 33 → pad 35.
        let adc_pin_a11 = system.gpio.daisy28.take().expect("daisy28").into_analog();

        enb_a.set_low();
        enb_b.set_low();
        enb_c.set_low();

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
        let midi_serial = device
            .USART1
            .serial(
                (midi_tx_pin, midi_rx_pin),
                SerialConfig {
                    baudrate: 31_250_u32.bps(),
                    ..SerialConfig::default()
                },
                ccdr.peripheral.USART1,
                &ccdr.clocks,
            )
            .unwrap();
        let (midi_tx, midi_rx) = midi_serial.split();
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
        led1.set_high();
        led2.set_high();
        led3.set_high();

        // ── I2C + SSD1306 ─────────────────────────────────────────────────────
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
        i2c_bus_recovery(&mut scl, &mut sda);
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
        let display = Some(
            Ssd1306::new(
                I2CDisplayInterface::new_custom_address(i2c, 0x3C),
                DisplaySize128x32,
                DisplayRotation::Rotate0,
            )
            .into_buffered_graphics_mode(),
        );
        display_update::spawn().ok();

        set_mux_channel(0, &mut s0, &mut s1, &mut s2);
        info!(
            "keyboard_keyboard ready: {} switches, {} muxes",
            NUM_SWITCHES, NUM_MUXES
        );

        (
            Shared {
                tick_ms: 0,
                switch_states: [SwitchState::new(); NUM_SWITCHES],
                baselines,
                event_queue: heapless::spsc::Queue::new(),
                midi_tx_flag: false,
                display_state: DisplayState::new(),
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
                pot_last_cc: [255u8; NUM_POTS],
                s0,
                s1,
                s2,
                led1,
                led2,
                led3,
                timer2,
                mux_raw: [[0u16; MUX_CHANNELS]; NUM_MUXES],
                midi_sender,
                midi_rx,
                filters,
                last_pitch_bend: 0x2000,
                last_vibrato_cc: 0,
                melody_channel: 0,
                led1_off_at: 0,
                led2_off_at: 0,
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

    // Priority 1 — below process_events so a slow display never blocks key events.
    // First spawn: init hardware + show splash. Subsequent spawns: redraw main screen.
    #[task(local = [display, initialized: bool = false], shared = [display_state], priority = 1, capacity = 2)]
    fn display_update(mut ctx: display_update::Context) {
        let Some(disp) = ctx.local.display.as_mut() else {
            return;
        };
        if !*ctx.local.initialized {
            match disp.init() {
                Ok(()) => {
                    *ctx.local.initialized = true;
                    crate::display::draw_splash(disp);
                    info!("display ok");
                }
                Err(_) => warn!("display not found"),
            }
            return;
        }
        let state = ctx.shared.display_state.lock(|s| *s);
        crate::display::draw_main(disp, &state);
    }

    #[task(binds = DMA1_STR1, priority = 8, local = [audio])]
    fn audio_handler(ctx: audio_handler::Context) {
        ctx.local.audio.for_each(|left, right| (left, right));
    }

    // ── 1 kHz scan loop ───────────────────────────────────────────────────────
    #[task(
        binds = TIM2,
        local  = [timer2, adc, adc_pins, enb_a, enb_b, enb_c, adc_pin_a9, adc_pin_a10,
                  adc_pin_a11, pot_last_cc, s0, s1, s2, mux_raw, filters, last_pitch_bend,
                  last_vibrato_cc, led1, led2, led3, midi_rx, led1_off_at, led2_off_at],
        shared = [tick_ms, switch_states, baselines, event_queue, midi_tx_flag],
        priority = 15
    )]
    fn timer_handler(mut ctx: timer_handler::Context) {
        ctx.local.timer2.clear_irq();

        let now = ctx.shared.tick_ms.lock(|t| {
            *t = t.wrapping_add(1);
            *t
        });

        if now % 2000 == 0 {
            info!("tick={}", now);
        }

        let baselines = ctx.shared.baselines.lock(|b| *b);
        let mut pending: heapless::Vec<(usize, SwitchEvent), 32> = heapless::Vec::new();

        // ── ADC scan ──────────────────────────────────────────────────────────
        for ch in 0..MUX_CHANNELS {
            set_mux_channel(ch, ctx.local.s0, ctx.local.s1, ctx.local.s2);
            cortex_m::asm::delay(480 * 10);
            let readings = read_all_adcs(ctx.local.adc, ctx.local.adc_pins);
            for (mux, &val) in readings.iter().enumerate() {
                ctx.local.mux_raw[mux][ch] = val;
            }
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

        // ── Switch state machine ───────────────────────────────────────────────
        let mut pb_filt_down = baselines[PITCH_BEND_DOWN];
        let mut pb_filt_up = baselines[PITCH_BEND_UP];
        let mut vib_filt_a = baselines[VIBRATO_A];
        let mut vib_filt_b = baselines[VIBRATO_B];

        ctx.shared.switch_states.lock(|states| {
            for (switch_idx, &(mux, ch)) in SWITCH_MAP.iter().enumerate() {
                if DISABLED_SWITCHES.contains(&switch_idx) {
                    continue;
                }
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

        // ── Pitch bend (rate-limited) ─────────────────────────────────────────
        if now % PITCH_BEND_INTERVAL_MS == 0 {
            let delta_down = pb_filt_down.abs_diff(baselines[PITCH_BEND_DOWN]);
            let delta_up = pb_filt_up.abs_diff(baselines[PITCH_BEND_UP]);
            let pb_value = if delta_down < RELEASE_DELTA && delta_up < RELEASE_DELTA {
                0x2000u16
            } else {
                let bend_down = (delta_down.min(PITCH_BEND_MAX_DELTA) as u32 * 0x2000
                    / PITCH_BEND_MAX_DELTA as u32) as u16;
                let bend_up = (delta_up.min(PITCH_BEND_MAX_DELTA) as u32 * 0x1FFF
                    / PITCH_BEND_MAX_DELTA as u32) as u16;
                (0x2000u16.saturating_sub(bend_down))
                    .saturating_add(bend_up)
                    .min(0x3FFF)
            };
            if pb_value.abs_diff(*ctx.local.last_pitch_bend) >= PITCH_BEND_HYSTERESIS {
                *ctx.local.last_pitch_bend = pb_value;
                pending
                    .push((0, SwitchEvent::PitchBend { value: pb_value }))
                    .ok();
            }
        }

        // ── DIAG logging ──────────────────────────────────────────────────────
        if DIAG_LOGGING && now % LOG_INTERVAL_MS == 0 {
            let (lk_mux, lk_ch) = SWITCH_MAP[LOG_SWITCH];
            let lk_filt = ctx.local.filters[LOG_SWITCH].last_output();
            let lk_base = baselines[LOG_SWITCH];
            info!(
                "DIAG HE{} raw={} filt={} base={} delta={:+}",
                HE_NUM[LOG_SWITCH],
                ctx.local.mux_raw[lk_mux as usize][lk_ch as usize],
                lk_filt,
                lk_base,
                lk_filt as i32 - lk_base as i32,
            );
            info!(
                "ADC_X4: AM1={} AM2={} AM3={} AM4={} AM5={}",
                ctx.local.mux_raw[0][4],
                ctx.local.mux_raw[1][4],
                ctx.local.mux_raw[2][4],
                ctx.local.mux_raw[3][4],
                ctx.local.mux_raw[4][4],
            );
        }

        // ── Vibrato → CC1 (dead zone + rate-limited) ──────────────────────────
        if now % VIBRATO_INTERVAL_MS == 0 {
            let max_delta = vib_filt_a
                .abs_diff(baselines[VIBRATO_A])
                .max(vib_filt_b.abs_diff(baselines[VIBRATO_B]))
                .saturating_sub(VIBRATO_DEAD_ZONE);
            let cc_val =
                ((max_delta.min(VIBRATO_MAX_DELTA) as u32 * 127 / VIBRATO_MAX_DELTA as u32) as u8)
                    .min(127);
            if cc_val.abs_diff(*ctx.local.last_vibrato_cc) >= VIBRATO_HYSTERESIS {
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

        // ── Pot scan (100 Hz) ─────────────────────────────────────────────────
        if now % POT_SCAN_MS == 0 {
            for (pot_idx, &(dec_idx, mux_ch, cc)) in POT_MAP.iter().enumerate() {
                set_mux_channel(mux_ch as usize, ctx.local.s0, ctx.local.s1, ctx.local.s2);
                set_decoder(dec_idx, ctx.local.enb_a, ctx.local.enb_b, ctx.local.enb_c);
                cortex_m::asm::delay(480 * 5);
                let raw = ctx.local.adc.read(ctx.local.adc_pin_a11).unwrap_or(0u32);
                let cc_val = (POT_ADC_MAX.saturating_sub(raw) * 127 / POT_ADC_MAX).min(127) as u8;
                if cc_val.abs_diff(ctx.local.pot_last_cc[pot_idx]) >= POT_CC_HYSTERESIS {
                    ctx.local.pot_last_cc[pot_idx] = cc_val;
                    pending
                        .push((0, SwitchEvent::PotChange { cc, value: cc_val }))
                        .ok();
                }
            }
            set_decoder(0, ctx.local.enb_a, ctx.local.enb_b, ctx.local.enb_c);
        }

        // ── LEDs ──────────────────────────────────────────────────────────────
        let tx_fired = ctx
            .shared
            .midi_tx_flag
            .lock(|f| core::mem::replace(f, false));
        if tx_fired {
            *ctx.local.led1_off_at = now.wrapping_add(50);
        }
        if now.wrapping_sub(*ctx.local.led1_off_at) > 0x8000_0000u32 {
            ctx.local.led1.set_low();
        } else {
            ctx.local.led1.set_high();
        }

        while ctx.local.midi_rx.read().is_ok() {
            *ctx.local.led2_off_at = now.wrapping_add(50);
        }
        if now.wrapping_sub(*ctx.local.led2_off_at) > 0x8000_0000u32 {
            ctx.local.led2.set_low();
        } else {
            ctx.local.led2.set_high();
        }

        if now % 1000 < 500 {
            ctx.local.led3.set_low();
        } else {
            ctx.local.led3.set_high();
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

    // ── MIDI output ───────────────────────────────────────────────────────────
    #[task(
        shared = [event_queue, midi_tx_flag, display_state],
        local  = [midi_sender, melody_channel],
        priority = 2,
        capacity = 32
    )]
    fn process_events(mut ctx: process_events::Context) {
        let mut did_send = false;
        let mut new_display_event: Option<LastEvent> = None;
        let mut melody_changed = false;

        ctx.shared.event_queue.lock(|queue| {
            while let Some((switch_idx, event)) = queue.dequeue() {
                if switch_idx == SETTINGS_CHAN1 || switch_idx == SETTINGS_CHAN2 {
                    if let SwitchEvent::NoteOn { .. } = event {
                        *ctx.local.melody_channel =
                            if switch_idx == SETTINGS_CHAN1 { 0 } else { 1 };
                        info!("melody ch → {}", *ctx.local.melody_channel + 1);
                        melody_changed = true;
                    }
                    continue;
                }

                let he = HE_NUM[switch_idx];
                let is_drum = switch_idx >= DRUM_SWITCH_START
                    && switch_idx < DRUM_SWITCH_START + DRUM_NOTE.len();
                let (note, channel) = if is_drum {
                    (DRUM_NOTE[switch_idx - DRUM_SWITCH_START], DRUM_CHANNEL)
                } else {
                    (SWITCH_TO_NOTE[switch_idx], *ctx.local.melody_channel)
                };
                if note == 0 {
                    continue;
                }

                ctx.local.midi_sender.set_channel(channel);
                match event {
                    SwitchEvent::NoteOn { velocity } => {
                        info!(
                            "NoteOn  HE{} ch={} note={} vel={}",
                            he,
                            channel + 1,
                            note,
                            velocity
                        );
                        ctx.local.midi_sender.note_on(note, velocity);
                        new_display_event = Some(LastEvent::Note { note });
                    }
                    SwitchEvent::NoteOff => {
                        info!("NoteOff HE{} ch={} note={}", he, channel + 1, note);
                        ctx.local.midi_sender.note_off(note, 0);
                        new_display_event = Some(LastEvent::Clear);
                    }
                    SwitchEvent::PotChange { cc, value } => {
                        info!("CC{} = {}", cc, value);
                        ctx.local.midi_sender.control_change(cc, value);
                        new_display_event = Some(LastEvent::Cc { num: cc, value });
                    }
                    SwitchEvent::PitchBend { value } => {
                        info!("PitchBend value={}", value);
                        ctx.local.midi_sender.pitch_bend(value);
                    }
                }
                did_send = true;
            }
        });

        if new_display_event.is_some() || melody_changed {
            ctx.shared.display_state.lock(|s| {
                match new_display_event {
                    Some(LastEvent::Clear) => s.last_event = None,
                    Some(ev) => s.last_event = Some(ev),
                    None => {}
                }
                if melody_changed {
                    s.melody_channel = *ctx.local.melody_channel;
                }
            });
            display_update::spawn().ok();
        }

        if did_send {
            ctx.shared.midi_tx_flag.lock(|f| *f = true);
        }
    }
}
