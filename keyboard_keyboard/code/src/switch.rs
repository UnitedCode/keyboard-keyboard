use crate::constants::*;
use log::info;

// ── 4-sample ring-buffer averaging filter ────────────────────────────────────

#[derive(Clone, Copy)]
pub struct ChannelFilter {
    ring: [u16; FILTER_SIZE],
    index: usize,
    sum: u32,
}

impl ChannelFilter {
    pub const fn new() -> Self {
        Self {
            ring: [0; FILTER_SIZE],
            index: 0,
            sum: 0,
        }
    }

    pub fn feed(&mut self, raw: u16) -> u16 {
        self.sum -= self.ring[self.index] as u32;
        self.sum += raw as u32;
        self.ring[self.index] = raw;
        self.index = (self.index + 1) % FILTER_SIZE;
        (self.sum >> FILTER_SHIFT) as u16
    }

    pub fn prime(&mut self, value: u16) {
        for slot in self.ring.iter_mut() {
            *slot = value;
        }
        self.sum = value as u32 * FILTER_SIZE as u32;
        self.index = 0;
    }

    /// Returns the current averaged output without feeding a new sample.
    pub fn last_output(&self) -> u16 {
        (self.sum >> FILTER_SHIFT) as u16
    }
}

// ── Switch state machine ──────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
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
    pub const fn new() -> Self {
        Self {
            phase: SwitchPhase::Idle,
            last_adc: 0,
            debounce_count: 0,
        }
    }

    pub fn update(
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

// ── Events ────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum SwitchEvent {
    NoteOn { velocity: u8 },
    NoteOff,
    PotChange { cc: u8, value: u8 },
    PitchBend { value: u16 },
}
