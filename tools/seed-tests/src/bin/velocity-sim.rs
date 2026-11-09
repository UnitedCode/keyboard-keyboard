use keyboard_seed_tests::{
    simulation::{experiment, Algorithm, Press, ALGORITHMS},
    velocity::SwitchCalibration,
    Result,
};
use std::{collections::BTreeSet, env, fmt::Write, fs, path::Path};

const DURATIONS: [f64; 8] = [2., 4., 8., 16., 32., 64., 128., 256.];

fn response() -> String {
    let mut report = String::from("| Full travel (ms) | Legacy | Curve only | Four-sample crossings | Raw crossings | Two-sample crossings |\n|---:|---:|---:|---:|---:|---:|\n");
    for duration in DURATIONS {
        write!(report, "| {duration} ").unwrap();
        for algorithm in &ALGORITHMS[..5] {
            let mut values = BTreeSet::new();
            for phase in 0..16 {
                let mut press = Press::linear(duration);
                press.phase_ms = f64::from(phase) / 16.;
                let result = experiment(press, *algorithm);
                assert_eq!(result.velocities.len(), 1);
                assert_eq!(result.off_ticks.len(), 1);
                values.extend(result.velocities);
            }
            write!(
                report,
                "| {}–{} ",
                values.first().unwrap(),
                values.last().unwrap()
            )
            .unwrap();
        }
        report.push_str("|\n");
    }
    report
}

fn sweep(directory: &Path) -> Result<()> {
    fs::create_dir(directory)?;
    let mut matrix = String::from("algorithm\ttravel_ms\tspan\texponent\tnoise\tpolarity\tjitter_ms\tvelocity_min\tvelocity_max\tmax_transit_error_us\tlifecycle_failures\n");
    let mut counts = [0u32; 6];
    let mut failures = [0u32; 6];
    let mut errors = [0f64; 6];
    let mut spreads = [0u8; 6];
    let mut clean_errors = [0f64; 6];
    for duration in DURATIONS {
        for span in [350., 600., 1000., 1500.] {
            for exponent in [1., 2., 3.] {
                for noise in [0., 4., 12.] {
                    for polarity in [-1., 1.] {
                        for jitter in [0., 0.2, 0.4] {
                            for (index, algorithm) in ALGORITHMS.iter().enumerate() {
                                let mut values = BTreeSet::new();
                                let mut bad = 0;
                                let mut error = 0f64;
                                for phase in 0..8 {
                                    let mut press = Press::linear(duration);
                                    press.span = span;
                                    press.magnetic_exponent = exponent;
                                    press.noise = noise;
                                    press.polarity = polarity;
                                    press.jitter_ms = jitter;
                                    press.phase_ms = f64::from(phase) / 8.;
                                    let result = experiment(press, *algorithm);
                                    counts[index] += 1;
                                    if result.velocities.len() != 1 || result.off_ticks.len() != 1 {
                                        bad += 1;
                                    }
                                    let calibration =
                                        if matches!(algorithm, Algorithm::CalibratedTwoSample) {
                                            SwitchCalibration::from_endpoints(
                                                2000,
                                                (2000. + polarity * span) as u16,
                                                noise as u16,
                                            )
                                            .unwrap()
                                        } else {
                                            SwitchCalibration::DEFAULT
                                        };
                                    let truth = duration
                                        * 1000.
                                        * ((f64::from(calibration.second_delta) / span)
                                            .powf(1. / exponent)
                                            - (f64::from(calibration.first_delta) / span)
                                                .powf(1. / exponent));
                                    for &measured in &result.transit_us {
                                        error = error.max((f64::from(measured) - truth).abs());
                                    }
                                    values.extend(result.velocities);
                                }
                                let lo = *values.first().unwrap_or(&0);
                                let hi = *values.last().unwrap_or(&0);
                                failures[index] += bad;
                                errors[index] = errors[index].max(error);
                                spreads[index] = spreads[index].max(hi - lo);
                                if noise == 0. && jitter == 0. {
                                    clean_errors[index] = clean_errors[index].max(error);
                                }
                                writeln!(matrix, "{}\t{duration}\t{span}\t{exponent}\t{noise}\t{polarity}\t{jitter}\t{lo}\t{hi}\t{error:.1}\t{bad}", algorithm.name()).unwrap();
                            }
                        }
                    }
                }
            }
        }
    }
    let mut report = String::from("# Velocity sensing simulation\n\nSynthetic inputs; shared production Rust filter/state machine. No measured keyboard traces or player acceptance. Sampling is 1 kHz; jitter changes physical sample times while firmware still counts 1 ms ticks. CPU scheduling, mux settling, electrical crosstalk and temperature are not emulated.\n\n## Controlled response\n\n600 ADC counts full travel; linear magnetic transfer; no noise/jitter; sixteen scan alignments per duration. Values are MIDI velocity ranges. Curve-only changes the historical mapping; filtered/raw crossings interpolate before debounce. All variants retain the four-sample trigger filter and three-scan confirmation.\n\n");
    report.push_str(&response());
    report.push_str("\n## Stress envelope\n\nEight travel durations × four spans (350/600/1000/1500 ADC counts) × three magnetic exponents (1/2/3) × three peak noise amplitudes (0/4/12 counts) × both polarities × three alternating timing jitters (0/±0.2/±0.4 ms) × eight scan alignments, for each algorithm. The deterministic noise sequence is shared across comparisons; these are specified stress cases, not a statistical model of real keyboards.\n\n| Algorithm | Presses | Lifecycle failures | Worst transit error (µs) | Worst clean error (µs) | Worst alignment velocity spread |\n|---|---:|---:|---:|---:|---:|\n");
    for (i, algorithm) in ALGORITHMS.iter().enumerate() {
        writeln!(
            report,
            "| {} | {} | {} | {:.1} | {:.1} | {} |",
            algorithm.name(),
            counts[i],
            failures[i],
            errors[i],
            clean_errors[i],
            spreads[i]
        )
        .unwrap();
    }
    report.push_str("\nTransit error is against exact noiseless crossings of the specified ADC thresholds, not an assumed MIDI velocity. Velocity spread is across scan alignments for a fixed condition. Endpoint scaling cannot linearize the magnetic response.\n\nProvisional response anchors: 250 µs → 127, 3000 µs → 64, 24000 µs → 1, piecewise linear interpolation. They spread the controlled envelope; simulations cannot establish the player's soft/medium/hard timing distribution. Existing trigger thresholds stay 100/150/250 counts. Endpoint calibration is opt-in and tested separately.\n");
    fs::write(directory.join("matrix.tsv"), matrix)?;
    fs::write(directory.join("summary.md"), &report)?;
    print!("{report}");
    if failures.iter().any(|&n| n != 0) {
        return Err(
            "Some synthetic presses did not produce exactly one Note On and one Note Off".into(),
        );
    }
    Ok(())
}

fn replay(file: &Path, baseline: u16) -> Result<()> {
    let text = fs::read_to_string(file)?;
    let mut lines = text.lines();
    if lines.next() != Some("tick_ms,adc") || baseline > 4095 {
        return Err("Expected tick_ms,adc header and a 12-bit resting baseline".into());
    }
    let mut samples: Vec<(u32, u16)> = Vec::new();
    for line in lines {
        let (t, adc) = line.split_once(',').ok_or("Expected tick_ms,adc row")?;
        let tick: u32 = t.parse()?;
        let adc: u16 = adc.parse()?;
        if adc > 4095 || samples.last().is_some_and(|p| tick.wrapping_sub(p.0) != 1) {
            return Err(
                "Expected 12-bit ADC samples at consecutive 1 ms ticks (wrapping allowed)".into(),
            );
        }
        samples.push((tick, adc));
    }
    if samples.is_empty() {
        return Err("Empty recording".into());
    }
    // Replay accepts the recorded rest value directly; no clipping or invented samples.
    let result = keyboard_seed_tests::simulation::replay_samples(samples, baseline);
    println!("on_tick,transit_us,velocity");
    for ((tick, time), velocity) in result
        .on_ticks
        .iter()
        .zip(&result.transit_us)
        .zip(&result.velocities)
    {
        println!("{tick},{time},{velocity}");
    }
    println!("release_ticks={:?}", result.off_ticks);
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = env::args().skip(1).collect();
    match args.as_slice() {
        [] => print!("{}", response()),
        [command, directory] if command == "sweep" => sweep(Path::new(directory))?,
        [command, file, baseline] if command == "replay" => replay(Path::new(file), baseline.parse()?)?,
        [command, rest, bottom, noise] if command == "calibrate" => {
            let calibration = SwitchCalibration::from_endpoints(rest.parse()?, bottom.parse()?, noise.parse()?)
                .ok_or("Insufficient travel/noise margin or invalid endpoints")?;
            println!("{calibration:#?}");
        }
        _ => return Err("Usage: velocity-sim [sweep FRESH_DIRECTORY | replay TRACE.csv BASELINE | calibrate REST BOTTOM PEAK_NOISE]".into()),
    }
    Ok(())
}
