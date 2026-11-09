//! Synthetic travel/ADC generator. These are stress envelopes, not measured keys.
use crate::switch::{ChannelFilter, SwitchEvent, SwitchState};
use crate::velocity::{SwitchCalibration, VelocityCurve};

#[derive(Clone, Copy, Debug)]
pub struct Press {
    pub travel_ms: f64,
    pub span: f64,
    pub magnetic_exponent: f64,
    pub phase_ms: f64,
    pub noise: f64,
    pub polarity: f64,
    pub jitter_ms: f64,
}

impl Press {
    pub fn linear(travel_ms: f64) -> Self {
        Self {
            travel_ms,
            span: 600.0,
            magnetic_exponent: 1.0,
            phase_ms: 0.0,
            noise: 0.0,
            polarity: 1.0,
            jitter_ms: 0.0,
        }
    }

    pub fn samples(self) -> impl Iterator<Item = (u32, u16)> {
        let end = (self.travel_ms + 110.0).ceil() as u32;
        (0..end).map(move |tick| {
            let jitter = if tick % 2 == 0 {
                self.jitter_ms
            } else {
                -self.jitter_ms
            };
            let t = f64::from(tick) + jitter - 20.0 - self.phase_ms;
            let position = if t < 0.0 {
                0.0
            } else if t < self.travel_ms {
                t / self.travel_ms
            } else if t < self.travel_ms + 30.0 {
                1.0
            } else {
                (1.0 - (t - self.travel_ms - 30.0) / 8.0).max(0.0)
            };
            // Deterministic pseudo-random, bounded noise; every run is repeatable.
            let bits = tick.wrapping_mul(747796405).wrapping_add(2891336453);
            let bits = ((bits >> ((bits >> 28) + 4)) ^ bits).wrapping_mul(277803737);
            let noise = (f64::from((bits >> 22) & 1023) / 511.5 - 1.0) * self.noise;
            let raw =
                2000.0 + self.polarity * self.span * position.powf(self.magnetic_exponent) + noise;
            (tick, raw.round().clamp(0.0, 4095.0) as u16)
        })
    }
}

#[derive(Debug, Default)]
pub struct Outcome {
    pub velocities: Vec<u8>,
    pub on_ticks: Vec<u32>,
    pub off_ticks: Vec<u32>,
    pub transit_us: Vec<u32>,
}

#[derive(Clone, Copy, Debug)]
pub enum Algorithm {
    Legacy,
    CurveOnly,
    FilteredTiming,
    RawTiming,
    TwoSampleTiming,
    CalibratedTwoSample,
}

pub const ALGORITHMS: [Algorithm; 6] = [
    Algorithm::Legacy,
    Algorithm::CurveOnly,
    Algorithm::FilteredTiming,
    Algorithm::RawTiming,
    Algorithm::TwoSampleTiming,
    Algorithm::CalibratedTwoSample,
];

impl Algorithm {
    pub fn name(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::CurveOnly => "curve_only",
            Self::FilteredTiming => "filtered_timing",
            Self::RawTiming => "raw_timing",
            Self::TwoSampleTiming => "two_sample_timing",
            Self::CalibratedTwoSample => "endpoint_calibrated",
        }
    }
}

pub fn experiment(press: Press, algorithm: Algorithm) -> Outcome {
    match algorithm {
        Algorithm::Legacy => run_old(press, false),
        Algorithm::CurveOnly => run_old(press, true),
        _ => {
            let calibration = if matches!(algorithm, Algorithm::CalibratedTwoSample) {
                SwitchCalibration::from_endpoints(
                    2000,
                    (2000. + press.polarity * press.span) as u16,
                    press.noise.ceil() as u16,
                )
                .expect("synthetic span must exceed noise")
            } else {
                SwitchCalibration::DEFAULT
            };
            run_at_baseline(press.samples(), calibration, algorithm, 2000)
        }
    }
}

pub fn run(press: Press) -> Outcome {
    run_samples(press.samples())
}

pub fn run_samples(samples: impl IntoIterator<Item = (u32, u16)>) -> Outcome {
    run_configured(samples, SwitchCalibration::DEFAULT, false)
}

pub fn run_configured(
    samples: impl IntoIterator<Item = (u32, u16)>,
    calibration: SwitchCalibration,
    filtered_measurement: bool,
) -> Outcome {
    run_at_baseline(
        samples,
        calibration,
        if filtered_measurement {
            Algorithm::FilteredTiming
        } else {
            Algorithm::TwoSampleTiming
        },
        2000,
    )
}

pub fn replay_samples(samples: impl IntoIterator<Item = (u32, u16)>, baseline: u16) -> Outcome {
    run_at_baseline(
        samples,
        SwitchCalibration::DEFAULT,
        Algorithm::TwoSampleTiming,
        baseline,
    )
}

fn run_at_baseline(
    samples: impl IntoIterator<Item = (u32, u16)>,
    calibration: SwitchCalibration,
    algorithm: Algorithm,
    baseline: u16,
) -> Outcome {
    let mut filter = ChannelFilter::<4>::new();
    let mut velocity_filter = ChannelFilter::<2>::new();
    filter.prime(baseline);
    velocity_filter.prime(baseline);
    let mut state = SwitchState::new();
    assert!(state.set_calibration(calibration));
    let mut result = Outcome::default();
    for (tick, raw) in samples {
        let filtered = filter.feed(raw);
        let velocity_filtered = velocity_filter.feed(raw);
        let measurement = match algorithm {
            Algorithm::FilteredTiming => filtered,
            Algorithm::RawTiming => raw,
            _ => velocity_filtered,
        };
        match state.update(filtered, measurement, baseline, tick, 0) {
            Some(SwitchEvent::NoteOn { velocity }) => {
                result.velocities.push(velocity);
                result.on_ticks.push(tick);
                result
                    .transit_us
                    .push(state.transit_us().expect("note requires a crossing"));
            }
            Some(SwitchEvent::NoteOff) => result.off_ticks.push(tick),
            _ => {}
        }
    }
    result
}

pub fn run_legacy(press: Press) -> Outcome {
    run_old(press, false)
}

fn run_old(press: Press, curve_only: bool) -> Outcome {
    let mut filter = crate::legacy_switch::ChannelFilter::new();
    filter.prime(2000);
    let mut state = crate::legacy_switch::SwitchState::new();
    let mut result = Outcome::default();
    for (tick, raw) in press.samples() {
        let elapsed = crate::legacy_switch::pending_elapsed(&state, tick);
        match state.update(filter.feed(raw), 2000, tick, 0) {
            Some(crate::legacy_switch::SwitchEvent::NoteOn { velocity }) => {
                let transit_us = elapsed.expect("legacy note requires first phase") * 1000;
                result.velocities.push(if curve_only {
                    VelocityCurve::DEFAULT.map(transit_us)
                } else {
                    velocity
                });
                result.on_ticks.push(tick);
                result.transit_us.push(transit_us);
            }
            Some(crate::legacy_switch::SwitchEvent::NoteOff) => result.off_ticks.push(tick),
            _ => {}
        }
    }
    result
}
