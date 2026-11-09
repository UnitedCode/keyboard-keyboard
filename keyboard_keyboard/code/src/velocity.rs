//! Per-key sensing thresholds and three configurable transit-time response anchors.
//! Defaults are provisional; real key traces must establish the useful range.

#[derive(Clone, Copy, Debug)]
pub struct VelocityCurve {
    pub fast_us: u32,
    pub medium_us: u32,
    pub slow_us: u32,
}

impl VelocityCurve {
    pub const DEFAULT: Self = Self {
        fast_us: 250,
        medium_us: 3_000,
        slow_us: 24_000,
    };

    pub const fn is_valid(self) -> bool {
        self.fast_us > 0 && self.fast_us < self.medium_us && self.medium_us < self.slow_us
    }

    pub fn map(self, transit_us: u32) -> u8 {
        let curve = if self.is_valid() { self } else { Self::DEFAULT };
        if transit_us <= curve.fast_us {
            127
        } else if transit_us >= curve.slow_us {
            1
        } else {
            let (lo, hi, top) = if transit_us <= curve.medium_us {
                (curve.fast_us, curve.medium_us, 127)
            } else {
                (curve.medium_us, curve.slow_us, 64)
            };
            (top - ((u64::from(transit_us - lo) * 63) / u64::from(hi - lo)) as u8).max(1)
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SwitchCalibration {
    pub release_delta: u16,
    pub first_delta: u16,
    pub second_delta: u16,
    pub curve: VelocityCurve,
}

impl SwitchCalibration {
    pub const DEFAULT: Self = Self {
        release_delta: crate::constants::RELEASE_DELTA,
        first_delta: crate::constants::FIRST_DELTA,
        second_delta: crate::constants::SECOND_DELTA,
        curve: VelocityCurve::DEFAULT,
    };

    pub const fn is_valid(self) -> bool {
        self.release_delta > 0
            && self.release_delta < self.first_delta
            && self.first_delta < self.second_delta
            && self.second_delta <= 4095
            && self.curve.is_valid()
    }

    /// Opt-in endpoint scaling for keys with measured travel. Endpoint scaling
    /// equalizes electrical span, not nonlinear magnetic distance. Reject travel
    /// too small relative to the measured peak resting noise (ADC counts).
    #[allow(dead_code)] // Also used by the host calibration/simulation tool.
    pub fn from_endpoints(rest: u16, bottom: u16, noise_peak: u16) -> Option<Self> {
        if rest > 4095 || bottom > 4095 {
            return None;
        }
        let span = rest.abs_diff(bottom);
        let result = Self {
            release_delta: span / 6,
            first_delta: span / 4,
            second_delta: (u32::from(span) * 5 / 12) as u16,
            curve: VelocityCurve::DEFAULT,
        };
        (result.is_valid() && u32::from(result.release_delta) > u32::from(noise_peak) * 4)
            .then_some(result)
    }
}

// Replace individual entries only from measured key endpoints and noise. The
// default thresholds retain the existing trigger travel until then.
pub const SWITCH_CALIBRATIONS: [SwitchCalibration; crate::constants::NUM_SWITCHES] =
    [SwitchCalibration::DEFAULT; crate::constants::NUM_SWITCHES];
