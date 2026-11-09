use crate::constants::*;
use crate::velocity::SwitchCalibration;
use log::info;

// Fixed-size averaging: four samples for triggers, two for velocity timing.

#[derive(Clone, Copy)]
pub struct ChannelFilter<const N: usize = FILTER_SIZE> {
    ring: [u16; N],
    index: usize,
    sum: u32,
}

impl<const N: usize> ChannelFilter<N> {
    pub const fn new() -> Self {
        Self {
            ring: [0; N],
            index: 0,
            sum: 0,
        }
    }

    pub fn feed(&mut self, raw: u16) -> u16 {
        self.sum -= self.ring[self.index] as u32;
        self.sum += raw as u32;
        self.ring[self.index] = raw;
        self.index = (self.index + 1) % N;
        (self.sum / N as u32) as u16
    }

    pub fn prime(&mut self, value: u16) {
        for slot in self.ring.iter_mut() {
            *slot = value;
        }
        self.sum = value as u32 * N as u32;
        self.index = 0;
    }

    /// Returns the current averaged output without feeding a new sample.
    pub fn last_output(&self) -> u16 {
        (self.sum / N as u32) as u16
    }
}

// ── Switch state machine ──────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub enum SwitchPhase {
    Idle,
    FirstActuated,
    FullyActuated { velocity: u8 },
}

#[derive(Clone, Copy)]
pub struct SwitchState {
    phase: SwitchPhase,
    pub last_adc: u16,
    debounce_count: u8,
    release_count: u8,
    previous_delta: u16,
    previous_tick: Option<u32>,
    first_crossing: Option<Crossing>,
    transit_us: Option<u32>,
    calibration: SwitchCalibration,
}

#[derive(Clone, Copy)]
struct Crossing {
    tick: u32,
    before_tick_us: u32,
}

impl SwitchState {
    pub const fn new() -> Self {
        Self {
            phase: SwitchPhase::Idle,
            last_adc: 0,
            debounce_count: 0,
            release_count: 0,
            previous_delta: 0,
            previous_tick: None,
            first_crossing: None,
            transit_us: None,
            calibration: SwitchCalibration::DEFAULT,
        }
    }

    /// Invalid calibration leaves the previous settings intact.
    pub fn set_calibration(&mut self, calibration: SwitchCalibration) -> bool {
        if !self.is_idle() || !calibration.is_valid() {
            return false;
        }
        *self = Self {
            calibration,
            ..Self::new()
        };
        true
    }

    fn crossing(&self, delta: u16, threshold: u16, tick: u32) -> Crossing {
        let before_tick_us = self.previous_tick.map_or(0, |previous_tick| {
            let interval = u64::from(tick.wrapping_sub(previous_tick)) * 1000;
            let rise = u64::from(delta.saturating_sub(self.previous_delta));
            if rise == 0 || self.previous_delta >= threshold {
                0
            } else {
                (interval * u64::from(delta - threshold) / rise).min(u64::from(u32::MAX)) as u32
            }
        });
        Crossing {
            tick,
            before_tick_us,
        }
    }

    fn measure(&mut self, delta: u16, tick: u32) {
        if delta < self.calibration.release_delta {
            self.first_crossing = None;
            self.transit_us = None;
        } else {
            if self.is_idle() && delta < self.calibration.first_delta {
                self.first_crossing = None;
            }
            if delta < self.calibration.second_delta {
                self.transit_us = None;
            }
            if self.first_crossing.is_none() && delta >= self.calibration.first_delta {
                self.first_crossing =
                    Some(self.crossing(delta, self.calibration.first_delta, tick));
            }
            if self.transit_us.is_none() && delta >= self.calibration.second_delta {
                if let Some(first) = self.first_crossing {
                    let second = self.crossing(delta, self.calibration.second_delta, tick);
                    let elapsed = (u64::from(second.tick.wrapping_sub(first.tick)) * 1000
                        + u64::from(first.before_tick_us))
                    .saturating_sub(u64::from(second.before_tick_us));
                    self.transit_us = Some(elapsed.min(u64::from(u32::MAX)) as u32);
                }
            }
        }
        self.previous_delta = delta;
        self.previous_tick = Some(tick);
    }

    pub fn transit_us(&self) -> Option<u32> {
        self.transit_us
    }

    pub fn is_idle(&self) -> bool {
        matches!(self.phase, SwitchPhase::Idle)
    }

    pub fn update(
        &mut self,
        adc_value: u16,
        velocity_adc_value: u16,
        baseline: u16,
        tick: u32,
        switch_idx: usize,
    ) -> Option<SwitchEvent> {
        self.last_adc = adc_value;
        // Absolute delta — detects press in either direction (north or south pole).
        let delta = adc_value.abs_diff(baseline);
        // Trigger on smoothed/confirmed travel, measure faster crossings before
        // filtering and debounce can flatten a fast stroke into fixed delays.
        self.measure(velocity_adc_value.abs_diff(baseline), tick);

        match self.phase {
            SwitchPhase::Idle => {
                if delta >= self.calibration.first_delta {
                    self.debounce_count = self.debounce_count.saturating_add(1);
                    if self.debounce_count >= DEBOUNCE_TICKS {
                        if DIAG_LOGGING {
                            info!(
                                "switch={} FirstActuated delta={} adc={} base={}",
                                switch_idx, delta, adc_value, baseline
                            );
                        }
                        self.phase = SwitchPhase::FirstActuated;
                        self.debounce_count = 0;
                    }
                } else {
                    self.debounce_count = 0;
                }
                None
            }
            SwitchPhase::FirstActuated => {
                if delta >= self.calibration.second_delta && self.transit_us.is_some() {
                    self.release_count = 0;
                    self.debounce_count = self.debounce_count.saturating_add(1);
                    if self.debounce_count >= DEBOUNCE_TICKS {
                        let elapsed = self.transit_us().unwrap_or(0);
                        let velocity = self.calibration.curve.map(elapsed);
                        if DIAG_LOGGING {
                            info!(
                                "switch={} FullyActuated elapsed={}us vel={}",
                                switch_idx, elapsed, velocity
                            );
                        }
                        self.phase = SwitchPhase::FullyActuated { velocity };
                        self.debounce_count = 0;
                        Some(SwitchEvent::NoteOn { velocity })
                    } else {
                        None
                    }
                } else if delta < self.calibration.release_delta {
                    self.debounce_count = 0;
                    self.release_count = self.release_count.saturating_add(1);
                    if self.release_count >= DEBOUNCE_TICKS {
                        self.phase = SwitchPhase::Idle;
                        self.release_count = 0;
                    }
                    None
                } else {
                    self.debounce_count = 0;
                    self.release_count = 0;
                    None
                }
            }
            SwitchPhase::FullyActuated { .. } => {
                if delta < self.calibration.release_delta {
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

// ── Events ────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum SwitchEvent {
    NoteOn { velocity: u8 },
    NoteOff,
    PotChange { cc: u8, value: u8 },
    PitchBend { value: u16 },
}
