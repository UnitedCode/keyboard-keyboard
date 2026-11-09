use keyboard_seed_tests::{
    simulation::{experiment, run, run_configured, run_legacy, run_samples, Algorithm, Press},
    switch::{SwitchEvent, SwitchState},
    velocity::{SwitchCalibration, VelocityCurve},
};

#[test]
fn historical_compression_is_reproduced_and_candidate_separates_speeds() {
    let durations = [4., 8., 16., 32., 64., 128.];
    for phase in 0..16 {
        let outcomes: Vec<_> = durations
            .iter()
            .map(|&duration| {
                let mut press = Press::linear(duration);
                press.phase_ms = f64::from(phase) / 16.;
                let old = run_legacy(press);
                let new = run(press);
                assert_eq!(new.on_ticks, old.on_ticks);
                assert_eq!(new.off_ticks, old.off_ticks);
                (old, new)
            })
            .collect();
        assert!(outcomes[..3].iter().all(|(old, _)| old.velocities == [124]));
        let velocities: Vec<_> = outcomes
            .iter()
            .map(|(_, new)| {
                assert_eq!(new.velocities.len(), 1);
                assert_eq!(new.off_ticks.len(), 1);
                new.velocities[0]
            })
            .collect();
        assert!(velocities.windows(2).all(|v| v[0] > v[1]), "{velocities:?}");
        assert!(velocities[0] - velocities[5] > 100);
    }
}

#[test]
fn clean_linear_crossings_match_the_known_input_slope() {
    for duration in [8., 16., 32., 64., 128.] {
        for phase in 0..16 {
            let mut press = Press::linear(duration);
            press.phase_ms = f64::from(phase) / 16.;
            let result = run(press);
            let truth_us = duration * 1000. * 100. / press.span;
            assert!((f64::from(result.transit_us[0]) - truth_us).abs() < 300.);
        }
    }
}

#[test]
fn synthetic_noise_polarity_magnetic_response_and_jitter_preserve_note_lifecycle() {
    for duration in [2., 4., 8., 16., 32., 64., 128., 256.] {
        for span in [350., 600., 1000., 1500.] {
            for exponent in [1., 2., 3.] {
                for noise in [0., 4., 12.] {
                    for polarity in [-1., 1.] {
                        for jitter in [0., 0.2, 0.4] {
                            for phase in 0..8 {
                                let mut press = Press::linear(duration);
                                press.span = span;
                                press.magnetic_exponent = exponent;
                                press.noise = noise;
                                press.polarity = polarity;
                                press.jitter_ms = jitter;
                                press.phase_ms = f64::from(phase) / 8.;
                                for algorithm in [
                                    Algorithm::RawTiming,
                                    Algorithm::TwoSampleTiming,
                                    Algorithm::CalibratedTwoSample,
                                ] {
                                    let result = experiment(press, algorithm);
                                    assert_eq!(
                                        result.velocities.len(),
                                        1,
                                        "{press:?} {algorithm:?}: {result:?}"
                                    );
                                    assert_eq!(
                                        result.off_ticks.len(),
                                        1,
                                        "{press:?} {algorithm:?}: {result:?}"
                                    );
                                    assert!(result.on_ticks[0] < result.off_ticks[0]);
                                    assert!((1..=127).contains(&result.velocities[0]));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn measured_endpoints_reduce_linear_key_to_key_electrical_span_bias() {
    let mut velocities = Vec::new();
    for span in [350., 600., 1000., 1500.] {
        let mut press = Press::linear(32.);
        press.span = span;
        let calibration = SwitchCalibration::from_endpoints(2000, 2000 + span as u16, 12).unwrap();
        velocities.push(run_configured(press.samples(), calibration, false).velocities[0]);
    }
    assert!(
        velocities.iter().max().unwrap() - velocities.iter().min().unwrap() <= 2,
        "{velocities:?}"
    );
}

#[test]
fn polarity_stays_within_one_velocity_step_of_integer_filter_quantization() {
    for duration in [4., 16., 64., 128.] {
        let positive = Press::linear(duration);
        let mut negative = positive;
        negative.polarity = -1.;
        let positive = run(positive).velocities[0];
        let negative = run(negative).velocities[0];
        assert!(positive.abs_diff(negative) <= 1);
    }
}

#[test]
fn timer_rollover_does_not_turn_a_soft_note_into_a_hard_note() {
    let press = Press::linear(64.);
    let normal = run(press);
    let wrapped = run_samples(
        press
            .samples()
            .map(|(t, adc)| (t.wrapping_add(u32::MAX - 40), adc)),
    );
    assert_eq!(normal.velocities, wrapped.velocities);
    assert_eq!(normal.transit_us, wrapped.transit_us);
}

#[test]
fn resting_noise_and_a_single_full_scale_spike_do_not_sound_notes() {
    let samples = (0..500).map(|tick| {
        let raw = if tick == 100 {
            4095
        } else {
            2000 + ((tick * 17) % 81) as u16 - 40
        };
        (tick, raw)
    });
    let result = run_samples(samples);
    assert!(result.velocities.is_empty(), "{result:?}");
    assert!(result.off_ticks.is_empty());
}

#[test]
fn aborted_partial_press_does_not_poison_the_next_velocity() {
    let partial = (0..50).map(|tick| (tick, if (20..30).contains(&tick) { 2190 } else { 2000 }));
    let press = Press::linear(32.);
    let result = run_samples(partial.chain(press.samples().map(|(t, adc)| (t + 50, adc))));
    assert_eq!(result.velocities, run(press).velocities);
    assert_eq!(result.off_ticks.len(), 1);
}

#[test]
fn consecutive_presses_each_have_one_release() {
    let result = run_samples((0..20).flat_map(|i| {
        Press::linear(if i % 2 == 0 { 8. } else { 64. })
            .samples()
            .map(move |(t, adc)| (t + i * 300, adc))
    }));
    assert_eq!(result.velocities.len(), 20);
    assert_eq!(result.off_ticks.len(), 20);
    assert!(result.velocities.chunks_exact(2).all(|v| v[0] > v[1]));
}

#[test]
fn opposite_conditions_cannot_share_a_debounce_counter() {
    let mut state = SwitchState::new();
    for tick in 0..3 {
        assert!(state.update(2200, 2200, 2000, tick, 0).is_none());
    }
    for tick in 3..5 {
        assert!(state.update(2000, 2000, 2000, tick, 0).is_none());
    }
    assert!(state.update(2300, 2300, 2000, 5, 0).is_none());
    assert!(state.update(2300, 2300, 2000, 6, 0).is_none());
    assert!(matches!(
        state.update(2300, 2300, 2000, 7, 0),
        Some(SwitchEvent::NoteOn { .. })
    ));
}

#[test]
fn velocity_mapping_is_monotonic_bounded_and_has_no_note_off_zero() {
    let mut previous = 127;
    for time in 0..=100_000 {
        let value = VelocityCurve::DEFAULT.map(time);
        assert!((1..=previous).contains(&value));
        previous = value;
    }
    assert_eq!(VelocityCurve::DEFAULT.map(u32::MAX), 1);
}

#[test]
fn unsafe_calibration_is_rejected() {
    assert!(SwitchCalibration::from_endpoints(2000, 2100, 12).is_none());
    assert!(SwitchCalibration::from_endpoints(2000, 2000, 0).is_none());
    assert!(SwitchCalibration::from_endpoints(2000, 5000, 0).is_none());
    let mut state = SwitchState::new();
    assert!(!state.set_calibration(SwitchCalibration {
        first_delta: 500,
        ..SwitchCalibration::DEFAULT
    }));
    assert!(!state.set_calibration(SwitchCalibration {
        curve: VelocityCurve {
            fast_us: 10,
            medium_us: 5,
            slow_us: 1
        },
        ..SwitchCalibration::DEFAULT
    }));
}
