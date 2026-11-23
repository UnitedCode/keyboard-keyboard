//! Sine interference + RC + recorded scan times + production firmware kernels.
//! Host kernel execution, not a new ARM/full-device/noise measurement claim.
use crate::{
    constants::*,
    switch::{ChannelFilter, SwitchEvent, SwitchState},
};
use circuit_workbench::{
    electrical::rc_pair, fixture::Fixture, improvements, netlist::Graph, qualification::response,
    sha256, Result,
};
use std::{fs, path::Path};
const ON: f64 = 0.250;
const OFF: f64 = 1.250;
fn numbers(line: &str) -> Result<Vec<u64>> {
    line.split(',')
        .map(|x| {
            if let Some(s) = x.strip_prefix("0x") {
                Ok(u64::from_str_radix(s, 16)?)
            } else {
                Ok(x.parse()?)
            }
        })
        .collect()
}
pub fn clock_cli(args: &[String]) -> Result<()> {
    if args.len() != 2 {
        return Err("noise-clock CAPTURE_DIR NEW_CLOCK_CSV".into());
    }
    let capture = Path::new(&args[0]);
    let report = fs::read_to_string(capture.join("report.txt"))?;
    if !report.contains("ARM_DEVICE=PASS") {
        return Err("clock capture must have a passing complete ARM report".into());
    }
    let gpio = fs::read_to_string(capture.join("gpio.csv"))?
        .lines()
        .skip(1)
        .map(numbers)
        .collect::<Result<Vec<_>>>()?;
    let adc = fs::read_to_string(capture.join("adc.csv"))?;
    let mut at = 0;
    let mut times = Vec::new();
    for l in adc.lines().skip(1) {
        let a = numbers(l)?;
        if a.len() != 3 {
            return Err("invalid ADC trace".into());
        }
        // Explicit crop removes the boot calibration reads, not runtime jitter.
        if a[1] != 10 || a[0] < 200_000_000 {
            continue;
        }
        while at + 1 < gpio.len() && gpio[at + 1][0] <= a[0] {
            at += 1;
        }
        let g = &gpio[at];
        if g.len() != 5 {
            return Err("invalid GPIO trace".into());
        }
        let ch = ((g[4] >> 10) & 1) | (((g[4] >> 11) & 1) << 1) | (((g[2] >> 4) & 1) << 2);
        if ch == 4 {
            times.push(a[0]);
        }
    }
    validate_clock(&times)?;
    let start = times[0];
    let mut csv = String::from("time_ns\n");
    for t in times {
        csv.push_str(&format!("{}\n", t - start));
    }
    use std::io::Write;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[1])?
        .write_all(csv.as_bytes())?;
    fs::write(Path::new(&args[1]).with_extension("sources.txt"),format!("report_sha256={}\nadc_sha256={}\ngpio_sha256={}\nclock_sha256={}\nroute=HE1,AM1,X4,ADC1_INP10\n",sha256(&capture.join("report.txt"))?,sha256(&capture.join("adc.csv"))?,sha256(&capture.join("gpio.csv"))?,sha256(Path::new(&args[1]))?))?;
    Ok(())
}
fn validate_clock(t: &[u64]) -> Result<()> {
    if t.len() < 1500
        || t.windows(2)
            .any(|w| w[1] <= w[0] || !(500_000..=2_000_000).contains(&(w[1] - w[0])))
        || t[t.len() - 1] - t[0] < 1_500_000_000
    {
        return Err(
            "need >=1.5s HE1 runtime clock, >=1500 scans, gaps 0.5-2ms; exclude boot calibration"
                .into(),
        );
    }
    Ok(())
}
#[derive(Clone, Copy)]
struct Circuit {
    rs: f64,
    ron: f64,
    cf: f64,
    ca: f64,
}
impl Circuit {
    fn sine(self, t: f64, hz: f64, phase: f64) -> Result<f64> {
        let (a, b) = response(self.rs, self.ron, self.cf, self.ca, hz)?;
        let w = std::f64::consts::TAU * hz;
        let hall_a = a * (1. + self.ron / 1e9) - b * w * self.ron * self.ca;
        let hall_b = b * (1. + self.ron / 1e9) + a * w * self.ron * self.ca;
        let steady = |time: f64| {
            let p = w * time + phase;
            (
                hall_a * p.sin() + hall_b * p.cos(),
                a * p.sin() + b * p.cos(),
            )
        };
        if t < ON {
            return Ok(0.);
        }
        if t < OFF {
            let (hi, ha) = steady(ON);
            let transient = if t - ON < 30. * self.rs * (self.cf + self.ca) {
                rc_pair(-hi, -ha, 0., self.rs, self.ron, self.cf, self.ca, t - ON).1
            } else {
                0.
            };
            Ok(steady(t).1 + transient)
        } else if t - OFF < 30. * self.rs * (self.cf + self.ca) {
            let (hi, ha) = steady(OFF);
            Ok(rc_pair(hi, ha, 0., self.rs, self.ron, self.cf, self.ca, t - OFF).1)
        } else {
            Ok(0.)
        }
    }
}
#[derive(Default, Debug)]
struct Observation {
    on: usize,
    off: usize,
    false_on: usize,
    stroke_on: usize,
    held: bool,
    rms: f64,
    filtered_rms: f64,
    latency: f64,
    cc1_changes: usize,
    bend_changes: usize,
    max_cc1: u8,
}
fn observe(
    c: Circuit,
    times: &[u64],
    hz: f64,
    phase: f64,
    amp: f64,
    correlated: bool,
    stroke: bool,
) -> Result<Observation> {
    let anchor = 2048;
    let mut baseline = anchor;
    let mut analog_baseline = anchor;
    let mut last_cc = 0u8;
    let mut last_bend = 0x2000u16;
    let mut filter = ChannelFilter::<FILTER_SIZE>::new();
    filter.prime(anchor);
    let mut vf = ChannelFilter::<VELOCITY_FILTER_SIZE>::new();
    vf.prime(anchor);
    let mut state = SwitchState::new();
    if !state.set_calibration(crate::velocity::SWITCH_CALIBRATIONS[0]) {
        return Err("invalid production HE1 calibration".into());
    }
    let mut o = Observation::default();
    let mut energy = 0.;
    let mut filtered_energy = 0.;
    let mut n = 0;
    for (scan, &ns) in times.iter().enumerate() {
        let t = ns as f64 * 1e-9;
        if t > 1.6 {
            break;
        }
        let travel = if !stroke || t < 0.5 {
            0.
        } else if t < 0.505 {
            0.55 * (t - 0.5) / 0.005
        } else if t < 1.0 {
            0.55
        } else if t < 1.005 {
            0.55 * (1.005 - t) / 0.005
        } else {
            0.
        };
        let noise = amp * c.sine(t, hz, phase)?;
        let reference = if correlated && (ON..OFF).contains(&t) {
            3.3 + 2. * amp * (std::f64::consts::TAU * hz * t + phase).sin()
        } else {
            3.3
        };
        // Quasi-static millisecond travel envelope. Continuous RC response is exact for the sine.
        let voltage = (1.65 + travel) * 1e9 / (1e9 + c.rs + c.ron) + noise;
        let raw = (voltage / reference * 4095.).round().clamp(0., 4095.) as u16;
        let f = filter.feed(raw);
        let v = vf.feed(raw);
        // Production tick_ms counts TIM2 invocations; interference uses physical sample time.
        if let Some(event) = state.update(f, v, baseline, scan as u32 + 1, 0) {
            match event {
                SwitchEvent::NoteOn { .. } => {
                    o.on += 1;
                    o.held = true;
                    if stroke && (0.5..1.0).contains(&t) {
                        o.stroke_on += 1;
                        if o.stroke_on == 1 {
                            o.latency = (t - 0.5) * 1000.;
                        }
                    } else {
                        o.false_on += 1;
                    }
                }
                SwitchEvent::NoteOff => {
                    o.off += 1;
                    o.held = false;
                }
                SwitchEvent::PotChange { .. } | SwitchEvent::PitchBend { .. } => {
                    return Err("unexpected non-note event from HE1 switch kernel".into())
                }
            }
        }
        // This is the inline production main.rs idle-baseline rule, copied without changing firmware.
        if state.is_idle() && f.abs_diff(baseline) < RELEASE_DELTA {
            let tracked = ((baseline as u32 * (BASELINE_TRACKING_ALPHA - 1) + f as u32)
                / BASELINE_TRACKING_ALPHA) as u16;
            baseline = tracked.clamp(
                anchor.saturating_sub(BASELINE_DRIFT_MAX),
                anchor.saturating_add(BASELINE_DRIFT_MAX),
            );
        }
        if (ON..OFF).contains(&t) && !stroke {
            energy += (raw as f64 - anchor as f64).powi(2);
            filtered_energy += (f as f64 - anchor as f64).powi(2);
            n += 1;
        }
        if !stroke {
            // Host mirror of main.rs inline rules. Other control is quiet for
            // localized output noise, identical for ideal simultaneous common ripple.
            // These are not observed UART messages or full ARM execution.
            if f.abs_diff(analog_baseline) < RELEASE_DELTA {
                let tracked = ((analog_baseline as u32 * (BASELINE_TRACKING_ALPHA - 1) + f as u32)
                    / BASELINE_TRACKING_ALPHA) as u16;
                analog_baseline = tracked.clamp(
                    anchor.saturating_sub(BASELINE_DRIFT_MAX),
                    anchor.saturating_add(BASELINE_DRIFT_MAX),
                );
            }
            let tick = scan as u32 + 1;
            let delta = f.abs_diff(analog_baseline);
            if tick.is_multiple_of(VIBRATO_INTERVAL_MS) {
                let d = delta.saturating_sub(VIBRATO_DEAD_ZONE);
                let cc = ((d.min(VIBRATO_MAX_DELTA) as u32 * 127 / VIBRATO_MAX_DELTA as u32) as u8)
                    .min(127);
                if cc.abs_diff(last_cc) >= VIBRATO_HYSTERESIS {
                    o.cc1_changes += 1;
                    last_cc = cc;
                    o.max_cc1 = o.max_cc1.max(cc);
                }
            }
            if tick.is_multiple_of(PITCH_BEND_INTERVAL_MS) {
                let other_delta = if correlated { delta } else { 0 };
                let bend = if delta < RELEASE_DELTA && other_delta < RELEASE_DELTA {
                    0x2000
                } else {
                    0x2000u16
                        .saturating_sub(
                            (delta.min(PITCH_BEND_MAX_DELTA) as u32 * 0x2000
                                / PITCH_BEND_MAX_DELTA as u32) as u16,
                        )
                        .saturating_add(
                            (other_delta.min(PITCH_BEND_MAX_DELTA) as u32 * 0x1FFF
                                / PITCH_BEND_MAX_DELTA as u32) as u16,
                        )
                        .min(0x3FFF)
                };
                if bend.abs_diff(last_bend) >= PITCH_BEND_HYSTERESIS {
                    o.bend_changes += 1;
                    last_bend = bend;
                }
            }
        }
    }
    if n > 0 {
        o.rms = (energy / n as f64).sqrt();
        o.filtered_rms = (filtered_energy / n as f64).sqrt();
    }
    Ok(o)
}
fn filter_cap(g: &Graph) -> Result<f64> {
    let n = g.net("HE1.2")?;
    let ground = g.net("A1.40")?;
    for r in g.components.keys().filter(|r| r.starts_with('C')) {
        let a = g.net(&format!("{r}.1"))?;
        let b = g.net(&format!("{r}.2"))?;
        if (a == n && b == ground) || (b == n && a == ground) {
            return g.value(r);
        }
    }
    Err("HE1 filter absent".into())
}
pub fn cli(args: &[String]) -> Result<()> {
    if args.len() != 6 {
        return Err("noise-study ORIGINAL_NET NET_1N NET_470P FIXTURE CLOCK_CSV FRESH_DIR".into());
    }
    let out = Path::new(&args[5]);
    fs::create_dir(out)?;
    let r = study(args, out).map(|mut report| {
        report.push_str("ADC_APERTURE=NOT_MODELED\nSOURCE_NOTE=4ohm slice is conditioned on the datasheet DC test; 100/1000/3000ohm are stress assumptions\nANALOG_CONTROLS=read analog-summary.txt; no false HE1 notes does not mean quiet CC1/pitch-bend\n");
        report
    });
    let report = match &r {
        Ok(s) => s.clone(),
        Err(e) => format!("NOISE_STUDY=FAIL\nerror={e}\n"),
    };
    fs::write(out.join("report.txt"), &report)?;
    println!("{report}");
    r?;
    Ok(())
}
fn study(args: &[String], out: &Path) -> Result<String> {
    let graphs = args[..3]
        .iter()
        .map(|p| Graph::parse(&fs::read_to_string(p)?))
        .collect::<Result<Vec<_>>>()?;
    improvements::validate(improvements::FILTER, &graphs[0], &graphs[1])?;
    improvements::validate(improvements::FILTER470, &graphs[0], &graphs[2])?;
    let fixture = Fixture::parse(&fs::read_to_string(&args[3])?)?;
    let clock = fs::read_to_string(&args[4])?
        .lines()
        .skip(1)
        .map(|s| s.parse::<u64>().map_err(Into::into))
        .collect::<Result<Vec<_>>>()?;
    validate_clock(&clock)?;
    for (i, name) in [
        "original.net",
        "1n.net",
        "470p.net",
        "assumptions.fixture",
        "clock.csv",
    ]
    .iter()
    .enumerate()
    {
        fs::copy(&args[i], out.join(name))?;
    }
    fs::write(
        out.join("firmware-kernel-sources.sha256"),
        env!("KEYBOARD_NOISE_SOURCES").replace(';', "\n"),
    )?;
    fs::write(
        out.join("shared-runner-sources.sha256"),
        circuit_workbench::RUNNER_SOURCES.replace(';', "\n"),
    )?;
    #[cfg(feature = "spice")]
    circuit_workbench::qualification::spice_noise(out)?;
    let ideal: Vec<_> = (0..=1600).map(|i| i * 1_000_000).collect();
    let mut csv=String::from("design,clock,rs_ohm,c_tolerance,hz,phase_rad,amplitude_v,reference_correlated,stroke,note_on,note_off,false_on,stroke_on,held_end,raw_rms_counts,filtered_rms_counts,latency_ms\n");
    let mut cases = 0;
    let mut bad = [0usize; 3];
    let mut clean_pass = 0;
    let mut small_bad = [0usize; 3];
    let mut counterexample = None;
    let mut baseline_failed = Vec::new();
    let mut analog=String::from("design,clock,rs_ohm,c_tolerance,hz,phase_rad,amplitude_v,reference_correlated,mirrored_cc1_changes,mirrored_bend_changes,max_cc1\n");
    let mut low_cc_cases = [0usize; 3];
    let mut low_bend_cases = [0usize; 3];
    for (i, (g, label)) in graphs.iter().zip(["4n7", "1n", "470p"]).enumerate() {
        let cap = filter_cap(g)?;
        let mut local_case = 0;
        for (clock_name, times) in [("ideal_1ms", &ideal), ("captured_HE1", &clock)] {
            let fsamp =
                1e9 / ((times[times.len() - 1] - times[0]) as f64 / (times.len() - 1) as f64);
            // 4 ohm slice reflects the datasheet DC bound at VOUT=2.5V;
            // the larger values remain independent dynamic/wiring stress assumptions.
            for rs in [4., 100., fixture.get("sensor_source_ohm"), 3000.] {
                for tol in [0.9, 1.1] {
                    let c = Circuit {
                        rs,
                        ron: fixture.get("mux_ron_ohm"),
                        cf: cap * tol,
                        ca: fixture.get("adc_cap_f"),
                    };
                    for stroke in [false, true] {
                        let o = observe(c, times, 100000., 0., 0., false, stroke)?;
                        if o.on != usize::from(stroke) || o.off != usize::from(stroke) || o.held {
                            return Err(
                                "clean-signal firmware kernel negative control failed".into()
                            );
                        }
                        clean_pass += 1;
                    }
                    for hz in [
                        50.,
                        1000.,
                        10000.,
                        50000.,
                        100000.,
                        400000.,
                        fsamp * 100. + 5.,
                    ] {
                        for phase in [
                            0.,
                            std::f64::consts::FRAC_PI_2,
                            std::f64::consts::PI,
                            3. * std::f64::consts::FRAC_PI_2,
                        ] {
                            for correlated in [false, true] {
                                let amplitudes = if correlated {
                                    [0.005, 0.025, 0.05, 0.1]
                                } else {
                                    [0.005, 0.05, 0.2, 0.4]
                                };
                                for amp in amplitudes {
                                    for stroke in [false, true] {
                                        let o =
                                            observe(c, times, hz, phase, amp, correlated, stroke)?;
                                        cases += 1;
                                        let failed = if stroke {
                                            o.on != 1
                                                || o.off != 1
                                                || o.stroke_on != 1
                                                || o.held
                                                || o.false_on > 0
                                        } else {
                                            o.on != 0 || o.off != 0 || o.held
                                        };
                                        bad[i] += usize::from(failed);
                                        if !stroke {
                                            analog.push_str(&format!("{label},{clock_name},{rs},{tol},{hz},{phase},{amp},{correlated},{},{},{}\n",o.cc1_changes,o.bend_changes,o.max_cc1));
                                            if amp <= 0.05 {
                                                low_cc_cases[i] += usize::from(o.cc1_changes > 0);
                                                low_bend_cases[i] +=
                                                    usize::from(o.bend_changes > 0);
                                            }
                                        }
                                        if i == 0 {
                                            baseline_failed.push(failed);
                                        }
                                        if amp <= 0.05 {
                                            small_bad[i] += usize::from(failed);
                                        }
                                        if i == 2
                                            && !correlated
                                            && !stroke
                                            && o.false_on > 0
                                            && hz >= 10000.
                                            && !baseline_failed[local_case]
                                            && counterexample.is_none()
                                        {
                                            counterexample=Some(format!("{clock_name},Rs={rs},Ctol={tol},Hz={hz},phase={phase},amplitude={amp},false_on={}",o.false_on));
                                        }
                                        csv.push_str(&format!("{label},{clock_name},{rs},{tol},{hz},{phase},{amp},{correlated},{stroke},{},{},{},{},{},{},{},{}\n",o.on,o.off,o.false_on,o.stroke_on,o.held,o.rms,o.filtered_rms,o.latency));
                                        local_case += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    if counterexample.is_none() {
        return Err(
            "strong-noise false-trigger control did not exercise the firmware kernel".into(),
        );
    }
    fs::write(out.join("noise-cases.csv"), csv)?;
    fs::write(out.join("analog-controls.csv"), analog)?;
    fs::write(out.join("analog-summary.txt"),format!("SCOPE=host mirror of inline main.rs CC1/pitch-bend; localized output noise with other quiet, correlated ripple with identical simultaneous controls; HE1 timing used for phase stress\ncc1_cases_at_or_below_50mv_4n7_1n_470p={low_cc_cases:?}\npitchbend_cases_at_or_below_50mv_4n7_1n_470p={low_bend_cases:?}\nNOTE=actual relative analog-channel phase not modeled; no-note pass does not imply quiet controls; not ARM/UART output\n"))?;
    Ok(format!("NOISE_STUDY=PASS (execution and negative controls, not every stress case)\nISOLATION=PASS\nKERNEL_SOURCE=unchanged production switch.rs,velocity.rs,constants.rs\nCLOCK_SHA256={}\ncase_count={cases}\nclean_signal_checks_passed={clean_pass}\nfailed_stress_cases_4n7_1n_470p={bad:?}\nfailed_cases_at_or_below_50mv_4n7_1n_470p={small_bad:?}\nstrong_noise_counterexample={}\nANALOG_MODEL=continuous selected-channel two-node RC plus 1Gohm ADC leakage; burst transients included\nSPICE_CROSSCHECK={}\nDECISION=470p is not a universal upgrade; retain 4.7n baseline pending measured noise and source impedance\nPOWER_REFERENCE_NOTE=correlated Vref ripple can change the ranking; input filtering and reference filtering are distinct\nNOT_RUN=full ARM under noise; intrinsic Hall noise; switched-mux noise propagation; reference decoupling; physical EMI\n",sha256(Path::new(&args[4]))?,counterexample.unwrap(),if cfg!(feature="spice"){ "PASS" }else{"NOT_RUN"}))
}
pub fn power_fault_cli(args: &[String]) -> Result<()> {
    if args.len() != 1 {
        return Err("power-kernel FRESH_DIR".into());
    }
    let out = Path::new(&args[0]);
    fs::create_dir(out)?;
    let mut csv = String::from(
        "calibrated_rest_v,held_before_fault,note_on,note_off,held_after_fault,failed\n",
    );
    let mut failures = 0;
    for rest in [1.0_f64, 1.3, 1.65, 2.0, 2.3] {
        for held in [false, true] {
            let anchor = (rest / 3.3 * 4095.).round() as u16;
            let mut filter = ChannelFilter::<FILTER_SIZE>::new();
            let mut vf = ChannelFilter::<VELOCITY_FILTER_SIZE>::new();
            filter.prime(anchor);
            vf.prime(anchor);
            let mut state = SwitchState::new();
            if !state.set_calibration(crate::velocity::SWITCH_CALIBRATIONS[0]) {
                return Err("invalid production HE1 calibration".into());
            }
            let mut on = 0;
            let mut off = 0;
            let mut active = false;
            for ms in 0..1600 {
                let raw = if ms >= 750 {
                    2048
                } else if held && ms >= 500 {
                    anchor + 683
                } else {
                    anchor
                };
                if let Some(event) = state.update(filter.feed(raw), vf.feed(raw), anchor, ms, 0) {
                    match event {
                        SwitchEvent::NoteOn { .. } => {
                            on += 1;
                            active = true;
                        }
                        SwitchEvent::NoteOff => {
                            off += 1;
                            active = false;
                        }
                        _ => return Err("unexpected event in power fault kernel".into()),
                    }
                }
            }
            let failed = active || on != usize::from(held) || off != usize::from(held);
            failures += usize::from(failed);
            csv.push_str(&format!("{rest},{held},{on},{off},{active},{failed}\n"));
        }
    }
    fs::write(out.join("power-loss-kernel.csv"), csv)?;
    fs::write(
        out.join("firmware-kernel-sources.sha256"),
        env!("KEYBOARD_NOISE_SOURCES").replace(';', "\n"),
    )?;
    if failures == 0 {
        return Err("power-loss fault was not exercised".into());
    }
    let report=format!("KERNEL_EXECUTION=PASS\nFAULT_CASES_FAILED={failures}\nDESIGN_ACCEPTANCE=FAIL_FOR_FIXED_MID_BIAS_ALONE\nMODEL=ideal guardian opens signal switches at 750ms; ADC biases to 1.65V; fixed calibrated anchors; unchanged ChannelFilter and SwitchState\nREQUIRED=power-valid must suppress new notes and release/reset active notes independently of analog rest voltage\nNOT_RUN=full ARM firmware; actual selected guardian/reset wiring\n");
    fs::write(out.join("report.txt"), &report)?;
    println!("{report}");
    Ok(())
}

pub fn hall_fault_cli(args: &[String]) -> Result<()> {
    if args.len() != 1 {
        return Err("hall-kernel FRESH_DIR".into());
    }
    let out = Path::new(&args[0]);
    fs::create_dir(out)?;
    let mut csv = String::from(
        "vq_v,sensitivity_v_mt,stroke_mt,polarity,delay_us,note_on,note_off,held,one_pair\n",
    );
    let mut cases = 0;
    let mut weak = 0;
    let mut strong = 0;
    for q in [0.9_f64, 1.02, 1.15] {
        for gain in [-0.02_f64, -0.045, -0.07] {
            for field in [2., 5., 10., 20.] {
                for polarity in [-1., 1.] {
                    for delay in [0., 25.] {
                        let anchor = (q / 3.3 * 4095.).round() as u16;
                        let mut f = ChannelFilter::<FILTER_SIZE>::new();
                        let mut vf = ChannelFilter::<VELOCITY_FILTER_SIZE>::new();
                        f.prime(anchor);
                        vf.prime(anchor);
                        let mut state = SwitchState::new();
                        if !state.set_calibration(crate::velocity::SWITCH_CALIBRATIONS[0]) {
                            return Err("invalid HE1 calibration".into());
                        }
                        let mut on = 0;
                        let mut off = 0;
                        let mut active = false;
                        for ms in 0..1600 {
                            let t = ms as f64 * 0.001 - delay * 1e-6;
                            let ramp = if t < 0.5 {
                                0.
                            } else if t < 0.505 {
                                (t - 0.5) / 0.005
                            } else if t < 1.0 {
                                1.
                            } else if t < 1.005 {
                                (1.005 - t) / 0.005
                            } else {
                                0.
                            };
                            let volts = (q + gain * field * polarity * ramp).clamp(0.2, 1.8);
                            let raw = (volts / 3.3 * 4095.).round() as u16;
                            if let Some(event) =
                                state.update(f.feed(raw), vf.feed(raw), anchor, ms + 1, 0)
                            {
                                match event {
                                    SwitchEvent::NoteOn { .. } => {
                                        on += 1;
                                        active = true;
                                    }
                                    SwitchEvent::NoteOff => {
                                        off += 1;
                                        active = false;
                                    }
                                    _ => return Err("unexpected Hall kernel event".into()),
                                }
                            }
                        }
                        let pass = on == 1 && off == 1 && !active;
                        cases += 1;
                        if field == 20. {
                            if !pass {
                                return Err("20mT corner failed".into());
                            }
                            strong += 1;
                        } else if !pass {
                            weak += 1;
                        }
                        csv.push_str(&format!(
                            "{q},{gain},{field},{polarity},{delay},{on},{off},{active},{pass}\n"
                        ));
                    }
                }
            }
        }
    }
    if weak == 0 {
        return Err("weak-field negative control not exercised".into());
    }
    fs::write(out.join("hall-kernel.csv"), csv)?;
    fs::write(
        out.join("firmware-kernel-sources.sha256"),
        env!("KEYBOARD_NOISE_SOURCES").replace(';', "\n"),
    )?;
    let report=format!("HALL_KERNEL=PASS (execution, strong-field gates and weak-field negatives)\ncase_count={cases}\n20mt_cases_passed={strong}\nweak_field_cases_without_one_pair={weak}\nMODEL=declared Vq/sensitivity corners, conservative0.2-1.8V clipping,0/25us delay and5ms magnetic ramp; unchanged production ChannelFilter/SwitchState\nDESIGN_ACCEPTANCE=CONDITIONAL_MEASURE_MAGNETS_AND_DYNAMIC_OUTPUT\nNOT_RUN=real magnet geometry; intrinsic noise; vendor output-driver model; mechanical fit\n");
    fs::write(out.join("report.txt"), &report)?;
    println!("{report}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_boot_calibration_in_runtime_clock() {
        let mut t: Vec<_> = (0..=1600).map(|i| i * 1_000_000).collect();
        validate_clock(&t).unwrap();
        t[10] = t[9] + 65_964;
        assert!(validate_clock(&t).is_err());
    }
    #[test]
    fn clean_and_large_interference_exercise_real_kernel() {
        let t: Vec<_> = (0..=1600).map(|i| i * 1_000_000).collect();
        let c = Circuit {
            rs: 1000.,
            ron: 100.,
            cf: 470e-12,
            ca: 50e-12,
        };
        let clean = observe(c, &t, 100000., 0., 0., false, true).unwrap();
        assert_eq!((clean.on, clean.off), (1, 1));
        let bad = observe(
            c,
            &t,
            100000.,
            std::f64::consts::FRAC_PI_2,
            0.4,
            false,
            false,
        )
        .unwrap();
        assert!(bad.false_on > 0);
        assert!(!bad.held);
        let small = observe(c, &t, 50., 0., 0.05, false, false).unwrap();
        assert_eq!(small.on, 0);
        assert!(small.cc1_changes > 0);
    }
    #[test]
    fn loss_of_carrier_power_reproduces_stuck_or_false_notes() {
        let out = std::env::temp_dir().join(format!("power-kernel-{}", std::process::id()));
        power_fault_cli(&[out.display().to_string()]).unwrap();
        assert!(fs::read_to_string(out.join("report.txt"))
            .unwrap()
            .contains("FAULT_CASES_FAILED=8"));
        fs::remove_dir_all(out).unwrap();
    }
}
