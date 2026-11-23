//! Separate supervisor/firmware experiment; never modifies production graphs or firmware.
use crate::{
    constants::*,
    guard::{Notes, PowerGuard},
    switch::{ChannelFilter, SwitchEvent, SwitchState},
};
use circuit_workbench::{netlist::Graph, qualification, sha256, Result};
use std::{fs, path::Path};

#[cfg(feature = "spice")]
fn simulate(
    out: &Path,
    name: &str,
    deck: &str,
    analysis: &str,
    names: &[&str],
) -> Result<circuit_workbench::spice::Trace> {
    fs::write(out.join(format!("{name}.cir")), deck)?;
    let trace = circuit_workbench::spice::solve(deck, analysis, names)?;
    fs::write(out.join(format!("{name}.csv")), trace.csv()?)?;
    fs::write(out.join(format!("{name}.log")), &trace.log)?;
    Ok(trace)
}

fn move_pin(g: &mut Graph, pin: &str, net: &str) {
    for nodes in g.nets.values_mut() {
        nodes.retain(|n| n != pin);
    }
    g.nets.entry(net.into()).or_default().push(pin.into());
}
pub fn candidate(base: &Graph) -> Result<Graph> {
    let mut g = qualification::offload(base)?;
    g.components.remove("PG1");
    for nodes in g.nets.values_mut() {
        nodes.retain(|n| !n.starts_with("PG1."));
    }
    let seed = base.net("A1.38")?.to_string();
    let ground = base.net("A1.40")?.to_string();
    for (r, sense) in [("PGS1", seed.as_str()), ("PGS2", "CARRIER_3V3")] {
        g.components.insert(r.into(), "TPS3808G33DBVR".into());
        // DBV:1 RESET,2 GND,3 MR,4 CT,5 SENSE,6 VDD. CT deliberately OPEN.
        for (pin, net) in [
            (1, "POWER_VALID"),
            (2, ground.as_str()),
            (3, seed.as_str()),
            (
                4,
                if r == "PGS1" {
                    "NC_PGS1_CT"
                } else {
                    "NC_PGS2_CT"
                },
            ),
            (5, sense),
            (6, seed.as_str()),
        ] {
            move_pin(&mut g, &format!("{r}.{pin}"), net);
        }
        let cap = if r == "PGS1" { "PGC1" } else { "PGC2" };
        g.components.insert(cap.into(), "100n".into());
        move_pin(&mut g, &format!("{cap}.1"), &seed);
        move_pin(&mut g, &format!("{cap}.2"), &ground);
    }
    // The parent allocation already has a 10k pullup: pin1=Seed, pin2=POWER_VALID.
    g.connected("PR3.1", "A1.38")?;
    g.connected("PR3.2", "PGS1.1")?;
    g.connected("PGS1.1", "PGS2.1")?;
    g.connected("PGS1.5", "A1.38")?;
    g.connected("PGS2.5", "HE1.1")?;
    if base.net("A1.11")? != "unconnected-(A1-SPI1_PICO-Pad11)" {
        return Err("Daisy D10/PB5/pad11 must be unused in the parent".into());
    }
    move_pin(&mut g, "A1.11", "POWER_VALID");
    g.nets.retain(|_, n| !n.is_empty());
    Graph::parse(&g.canonical())
}
pub fn validate(base: &Graph, g: &Graph) -> Result<()> {
    if candidate(base)? != *g {
        return Err("unexpected guard candidate delta".into());
    }
    Ok(())
}
pub fn candidate_cli(a: &[String]) -> Result<()> {
    if a.len() != 2 {
        return Err("guard-candidate PARENT FRESH_DIR".into());
    }
    let out = Path::new(&a[1]);
    fs::create_dir(out)?;
    let base = Graph::parse(&fs::read_to_string(&a[0])?)?;
    let g = candidate(&base)?;
    fs::write(out.join("candidate.net"), g.canonical())?;
    let drawing = out.join("power-guard-allocation.kicad_sch");
    circuit_workbench::schematics::allocation(&base, &g, &drawing)?;
    let s=fs::read_to_string(&drawing)?.replace(
        "Guarded carrier rail allocation: PG1 UNSELECTED. NOT accepted for promotion or fabrication. See study report.",
        "Supervisor + sticky firmware lockout experiment. Functional allocation only; no PCB/fabrication qualification.");
    fs::write(&drawing, s)?;
    let mut comps = String::from("reference\tbefore\tafter\n");
    let mut wires = String::from("pin\tbefore\tafter\n");
    for (r, v) in &g.components {
        if base.components.get(r) != Some(v) {
            comps.push_str(&format!(
                "{r}\t{}\t{v}\n",
                base.components
                    .get(r)
                    .map(String::as_str)
                    .unwrap_or("ADDED")
            ));
        }
    }
    for (net, nodes) in &g.nets {
        for pin in nodes {
            if base.net(pin).ok() != Some(net.as_str()) {
                wires.push_str(&format!(
                    "{pin}\t{}\t{net}\n",
                    base.net(pin).unwrap_or("ADDED")
                ));
            }
        }
    }
    fs::write(out.join("component-changes.tsv"), comps)?;
    fs::write(out.join("connection-changes.tsv"), wires)?;
    Ok(())
}
pub fn schematic_cli(a: &[String]) -> Result<()> {
    if a.len() != 3 {
        return Err("guard-check-schematic PARENT CANDIDATE EXPORTED_XML".into());
    }
    let b = Graph::parse(&fs::read_to_string(&a[0])?)?;
    let g = Graph::parse(&fs::read_to_string(&a[1])?)?;
    validate(&b, &g)?;
    let e = Graph::xml(&fs::read_to_string(&a[2])?)?;
    let added: Vec<_> = g
        .components
        .keys()
        .filter(|r| !b.components.contains_key(*r))
        .collect();
    if e.components.len() != added.len() {
        return Err("allocation component count differs".into());
    }
    let mut expected_export = Graph::default();
    for r in added {
        expected_export
            .components
            .insert(r.clone(), g.components[r].clone());
        if e.components.get(r) != g.components.get(r) {
            return Err("allocation part mismatch".into());
        }
        for (net, nodes) in &g.nets {
            for p in nodes.iter().filter(|p| p.starts_with(&format!("{r}."))) {
                expected_export
                    .nets
                    .entry(format!("/{net}"))
                    .or_default()
                    .push(p.clone());
                if e.net(p)? != format!("/{net}") {
                    return Err(format!("allocation net mismatch: {p}").into());
                }
            }
        }
    }
    if expected_export != e {
        return Err("allocation contains unexpected pins/nets".into());
    }
    println!("SCHEMATIC_CONNECTIVITY=PASS (added allocation only; original pin changes in TSV)");
    Ok(())
}

/// Behavioral timing envelope. Below-20us pulses and VDD<1.7V are not qualified.
#[derive(Clone, Copy)]
pub struct Supervisor {
    pub threshold: f64,
    pub hysteresis: f64,
    pub delay_ns: u64,
    pub release_ns: u64,
    low: bool,
    bad_since: Option<u64>,
    good_since: Option<u64>,
}
impl Supervisor {
    pub fn new(threshold: f64, delay_ns: u64, release_ns: u64) -> Self {
        Self {
            threshold,
            hysteresis: threshold * 0.025,
            delay_ns,
            release_ns,
            low: true,
            bad_since: None,
            good_since: None,
        }
    }
    pub fn sample(&mut self, t: u64, v: f64) -> bool {
        if v < self.threshold {
            self.good_since = None;
            let since = *self.bad_since.get_or_insert(t);
            if t - since >= self.delay_ns {
                self.low = true;
            }
        } else {
            self.bad_since = None;
            if self.low && v >= self.threshold + self.hysteresis {
                let since = *self.good_since.get_or_insert(t);
                if t - since >= self.release_ns {
                    self.low = false;
                }
            } else if self.low {
                self.good_since = None;
            }
        }
        !self.low
    }
}
pub fn rail(name: &str, t: u64) -> f64 {
    match name {
        "healthy" | "seed-brownout" => 3.3,
        "startup-late" => {
            if t < 100_000_000 {
                0.
            } else {
                3.3
            }
        }
        "startup-ramp" => (t as f64 / 100_000_000. * 3.3).min(3.3),
        "loss" | "reset-after-loss" => {
            if (300_000_000..360_000_000).contains(&t) {
                0.
            } else {
                3.3
            }
        }
        "brownout" => {
            if (282_321_000..282_421_000).contains(&t) {
                2.7
            } else {
                3.3
            }
        }
        "chatter" => {
            if (200_000_000..230_000_000).contains(&t) {
                if (t / 500_000).is_multiple_of(2) {
                    2.9
                } else {
                    3.12
                }
            } else {
                3.3
            }
        }
        _ => f64::NAN,
    }
}
pub fn edges(name: &str) -> Result<Vec<(u64, bool)>> {
    if !rail(name, 0).is_finite() {
        return Err("unknown power scenario".into());
    }
    let mut s = Supervisor::new(3.07, 20_000, 28_000_000);
    let mut seed_monitor = s;
    let mut edges = vec![(0, false)];
    for t in (0..=900_000_000).step_by(1000) {
        let seed = if name == "seed-brownout" && (300_000_000..320_000_000).contains(&t) {
            2.9
        } else {
            3.3
        };
        let carrier_good = s.sample(t, rail(name, t));
        let seed_good = seed_monitor.sample(t, seed);
        let valid = carrier_good && seed_good;
        if edges.last().unwrap().1 != valid {
            edges.push((t, valid));
        }
    }
    Ok(edges)
}

pub fn study_cli(a: &[String]) -> Result<()> {
    if a.len() != 3 {
        return Err("guard-study PARENT CANDIDATE FRESH_DIR".into());
    }
    let out = Path::new(&a[2]);
    fs::create_dir(out)?;
    let result = study(a, out);
    let report = match &result {
        Ok(s) => s.clone(),
        Err(e) => format!("GUARD_STUDY=FAIL\nerror={e}\n"),
    };
    fs::write(out.join("report.txt"), &report)?;
    print!("{report}");
    result?;
    Ok(())
}
fn study(a: &[String], out: &Path) -> Result<String> {
    let b = Graph::parse(&fs::read_to_string(&a[0])?)?;
    let g = Graph::parse(&fs::read_to_string(&a[1])?)?;
    validate(&b, &g)?;
    fs::copy(&a[0], out.join("parent.net"))?;
    fs::copy(&a[1], out.join("candidate.net"))?;
    fs::write(
        out.join("sources.sha256"),
        env!("KEYBOARD_NOISE_SOURCES").replace(';', "\n"),
    )?;
    let mut exact = String::from("rest_v,held,guarded,note_on,note_off,held_end,failed\n");
    let mut old_fail = 0;
    let mut new_fail = 0;
    for rest in [1., 1.3, 1.65, 2., 2.3] {
        for held in [false, true] {
            for guarded in [false, true] {
                let anchor = (rest / 3.3 * 4095f64).round() as u16;
                let mut filter = ChannelFilter::<FILTER_SIZE>::new();
                filter.prime(anchor);
                let mut vf = ChannelFilter::<VELOCITY_FILTER_SIZE>::new();
                vf.prime(anchor);
                let mut state = SwitchState::new();
                assert!(state.set_calibration(crate::velocity::SWITCH_CALIBRATIONS[0]));
                let mut guard = PowerGuard::new(true);
                let mut notes = Notes::default();
                let (mut on, mut off, mut active) = (0, 0, false);
                for ms in 0..1600 {
                    if guarded && guard.observe(ms < 750) {
                        notes.panic(|m| {
                            if m[0] & 0xf0 == 0x80 {
                                off += 1;
                                active = false;
                            }
                        });
                    }
                    if guarded && guard.locked() {
                        continue;
                    }
                    let raw = if ms >= 750 {
                        2048
                    } else if held && ms >= 500 {
                        anchor + 683
                    } else {
                        anchor
                    };
                    if let Some(e) = state.update(filter.feed(raw), vf.feed(raw), anchor, ms, 0) {
                        match e {
                            SwitchEvent::NoteOn { .. } => {
                                on += 1;
                                active = true;
                                notes.set(0, 54, true);
                            }
                            SwitchEvent::NoteOff => {
                                off += 1;
                                active = false;
                                notes.set(0, 54, false);
                            }
                            _ => {}
                        }
                    }
                }
                let bad = active || on != usize::from(held) || off != usize::from(held);
                if guarded {
                    new_fail += usize::from(bad);
                } else {
                    old_fail += usize::from(bad);
                }
                exact.push_str(&format!(
                    "{rest},{held},{guarded},{on},{off},{active},{bad}\n"
                ));
            }
        }
    }
    fs::write(out.join("original-fault-comparison.csv"), exact)?;
    if old_fail != 8 || new_fail != 0 {
        return Err("original eight fault counterexamples no longer reproduced/fixed".into());
    }
    #[cfg(feature = "spice")]
    {
        // Independent DC/pullup check only; RESET is an imposed state, not a vendor macromodel.
        for (i, rail) in [3.1, 3.3, 3.432].into_iter().enumerate() {
            let deck=format!("Guard pullup DC, declared total leakage5uA\nVseed seed 0 {rail}\nRpull seed good {}\nIleak good 0 5u\n.end\n",g.value("PR3")?);
            let t = simulate(out, &format!("pullup-high-{i}"), &deck, "op", &["v(good)"])?;
            if t.last("v(good)")? < rail * 0.7 {
                return Err("power-valid logic high margin failed".into());
            }
            let deck=format!("Guard pullup low, declared sink0.4V\nVseed seed 0 {rail}\nRpull seed good {}\nVsink good 0 0.4\n.end\n",g.value("PR3")?);
            let t = simulate(out, &format!("pullup-low-{i}"), &deck, "op", &["i(vsink)"])?;
            if t.last("i(vsink)")? > 0.001 {
                return Err("power-valid sink load exceeds1mA bound".into());
            }
        }
    }
    let mut csv=String::from("rest_v,held,phase_us,pulse_us,threshold_v,release_ms,fault_adc,guarded,false_on,held_end,panic_bytes\n");
    let mut cases = 0;
    let mut failures = 0;
    let mut negative = 0;
    // Same frozen production kernel as the retained eight-case failure. Independent
    // timing phase sweep includes dips shorter than a scan and stretched by supervisor.
    for rest in [1., 1.3, 1.65, 2., 2.3] {
        for held in [false, true] {
            for phase in [0, 1, 250, 500, 750, 999] {
                for pulse in [25, 100, 500, 1000, 20_000] {
                    for threshold in [3.07 * 0.985, 3.07 * 1.015] {
                        for release in [12, 28] {
                            for fault_adc in [0, 2048, 4095] {
                                for guarded in [false, true] {
                                    let anchor = (rest / 3.3 * 4095f64).round() as u16;
                                    let mut f = ChannelFilter::<FILTER_SIZE>::new();
                                    f.prime(anchor);
                                    let mut vf = ChannelFilter::<VELOCITY_FILTER_SIZE>::new();
                                    vf.prime(anchor);
                                    let mut state = SwitchState::new();
                                    assert!(state
                                        .set_calibration(crate::velocity::SWITCH_CALIBRATIONS[0]));
                                    let mut guard = PowerGuard::new(true);
                                    let mut notes = Notes::default();
                                    let mut s =
                                        Supervisor::new(threshold, 20_000, release * 1_000_000);
                                    let mut active = false;
                                    let mut false_on = 0;
                                    let mut panic_bytes = 0;
                                    let fault = 200_000_000 + phase * 1000;
                                    // 1us circuit grid and 1ms firmware scans. Healthy supervisor is prequalified.
                                    for t in (0..=300_000_000u64).step_by(1000) {
                                        let rail = if t >= fault && t < fault + pulse * 1000 {
                                            2.7
                                        } else {
                                            3.3
                                        };
                                        let valid = s.sample(t, rail);
                                        if t % 1_000_000 != 0 {
                                            continue;
                                        }
                                        let ms = t / 1_000_000;
                                        if ms < 30 {
                                            continue;
                                        }
                                        if guarded && guard.observe(valid) {
                                            notes.panic(|m| {
                                                panic_bytes += 3;
                                                if m[0] & 0xf0 == 0x80
                                                    || (m[0] & 0xf0 == 0xb0
                                                        && [120, 123].contains(&m[1]))
                                                {
                                                    active = false;
                                                }
                                            });
                                        }
                                        if guarded && guard.locked() {
                                            continue;
                                        }
                                        let raw = if rail < 3.0 || !valid {
                                            fault_adc
                                        } else if held && (100..250).contains(&ms) {
                                            anchor + 683
                                        } else {
                                            anchor
                                        };
                                        if let Some(e) = state.update(
                                            f.feed(raw),
                                            vf.feed(raw),
                                            anchor,
                                            ms as u32,
                                            0,
                                        ) {
                                            match e {
                                                SwitchEvent::NoteOn { .. } => {
                                                    active = true;
                                                    notes.set(0, 54, true);
                                                    if ms >= 200 {
                                                        false_on += 1;
                                                    }
                                                }
                                                SwitchEvent::NoteOff => {
                                                    active = false;
                                                    notes.set(0, 54, false);
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                    let failed = active || false_on > 0;
                                    if guarded {
                                        cases += 1;
                                        failures += usize::from(failed);
                                    } else {
                                        negative += usize::from(failed);
                                    }
                                    csv.push_str(&format!("{rest},{held},{phase},{pulse},{threshold},{release},{fault_adc},{guarded},{false_on},{active},{panic_bytes}\n"));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    fs::write(out.join("fault-cases.csv"), csv)?;
    let mut start = String::from("scenario,valid_at_calibration,locked_after_restore\n");
    for name in [
        "healthy",
        "startup-late",
        "startup-ramp",
        "chatter",
        "seed-brownout",
    ] {
        let e = edges(name)?;
        let valid_at = |t| e.iter().rev().find(|(at, _)| *at <= t).unwrap().1;
        let boot = (50..90).all(|ms| valid_at(ms * 1_000_000));
        let mut guard = PowerGuard::new(boot);
        for ms in 90..600 {
            guard.observe(valid_at(ms * 1_000_000));
        }
        let expect = name != "healthy";
        if guard.locked() != expect {
            return Err("startup/restore gate failed".into());
        }
        start.push_str(&format!("{name},{boot},{}\n", guard.locked()));
    }
    fs::write(out.join("startup.csv"), start)?;
    let mut margins = String::from("rail_v,threshold_v,hysteresis_ratio,valid_after32ms\n");
    let mut nominal_low_lockouts = 0;
    for rail in [2.7, 2.99, 3., 3.1, 3.168, 3.2, 3.3, 3.432] {
        for threshold in [3.07 * 0.985, 3.07 * 1.015] {
            for h in [0.01, 0.025] {
                let mut s = Supervisor::new(threshold, 20_000, 28_000_000);
                s.hysteresis = threshold * h;
                s.sample(0, rail);
                let valid = s.sample(32_000_000, rail);
                if (rail < 3.0 && valid) || (rail >= 3.2 && !valid) {
                    return Err("startup voltage margin gate failed".into());
                }
                if rail == 3.168 && !valid {
                    nominal_low_lockouts += 1;
                }
                margins.push_str(&format!("{rail},{threshold},{h},{valid}\n"));
            }
        }
    }
    fs::write(out.join("startup-margins.csv"), margins)?;
    fs::write(out.join("startup-margin-report.txt"),format!("STARTUP_MARGIN_STUDY=PASS (safety checks, not all nominal source corners arm)\nrail_3v168_lockout_corners={nominal_low_lockouts}\nDESIGN_ACCEPTANCE=CONDITIONAL_RAIL_AT_LEAST3V2_FOR_ARMING\nNOTE=3.3V module minus4percent is3.168V; selected supervisor worst rising threshold is about3.194V. Some within-module-tolerance startups intentionally remain disarmed. Rail tolerance/part choice must be resolved before physical promotion. Hall minimum3.0V must not be waived by lowering the trip point.\n"))?;
    if failures != 0 || negative == 0 {
        return Err(format!("fault failures={failures}, negatives={negative}").into());
    }
    // A max-size physical chord plus active sustain must be silenced, not CC123-only.
    let mut ledger = Notes::default();
    for n in 0..90 {
        ledger.set((n % 16) as u8, n as u8, true);
    }
    let mut bytes = Vec::new();
    ledger.panic(|m| bytes.extend(m));
    let max_wire_ns = (bytes.len() as u64) * 320_000;
    fs::write(
        out.join("panic-wire.csv"),
        bytes
            .iter()
            .enumerate()
            .map(|(i, b)| format!("{},{}\n", (i as u64 + 1) * 320_000, b))
            .collect::<String>(),
    )?;
    Ok(format!("GUARD_STUDY=PASS\nISOLATION=PASS\nparent_sha256={}\ncandidate_sha256={}\noriginal_fault_failures={old_fail}\nfixed_original_fault_failures={new_fail}\nguarded_fault_cases={cases}\nguarded_failures={failures}\nunguarded_failure_controls={negative}\nstartup_and_seed_cases=5\n90_note_panic_bytes={}\n90_note_panic_wire_ns={max_wire_ns}\nRECOVERY=sticky lockout; reset only with stable rails and released keys\nSCOPE=1us supervisor timing grid, 1ms production-kernel scans; 20us assertion scenario,12/28ms release corners\nUNQUALIFIED=sub20us glitches; complete Seed power loss; VDD below supervisor operating range; overvoltage; physical receiver behavior; PCB parasitics\nPHYSICAL_MEASURED=NOT_RUN\n",sha256(Path::new(&a[0]))?,sha256(Path::new(&a[1]))?,bytes.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sticky_guard_and_explicit_panic() {
        let mut g = PowerGuard::new(true);
        assert!(!g.observe(true));
        assert!(g.observe(false));
        assert!(!g.observe(true));
        assert!(g.locked());
        let mut n = Notes::default();
        n.set(9, 36, true);
        let mut msgs = Vec::new();
        n.panic(|m| msgs.push(m));
        assert_eq!(msgs[0], [0x89, 36, 0]);
        assert!(msgs.contains(&[0xb9, 64, 0]));
        assert!(msgs.contains(&[0xb9, 120, 0]));
        let mut repeated = Vec::new();
        n.panic(|m| repeated.push(m));
        assert!(!repeated.iter().any(|m| m[0] & 0xf0 == 0x80));
    }
    #[test]
    fn stretched_dip_survives_polling_and_bad_boot_stays_locked() {
        let e = edges("brownout").unwrap();
        assert!(e.iter().any(|(t, v)| *t > 182_321_000 && !*v));
        let mut g = PowerGuard::new(false);
        for _ in 0..100 {
            g.observe(true);
        }
        assert!(g.locked());
    }
    #[test]
    fn reject_wrong_pin_and_stacked_filter_changes() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../circuits/baselines/keyboard-current.net");
        let b = Graph::parse(&fs::read_to_string(path).unwrap()).unwrap();
        let mut g = candidate(&b).unwrap();
        validate(&b, &g).unwrap();
        g.connected("PR3.2", "A1.11").unwrap();
        g.connected("PR3.1", "A1.38").unwrap();
        g.disconnect("A1.11");
        assert!(validate(&b, &g).is_err());
        let mut g = candidate(&b).unwrap();
        g.components.insert("C2".into(), "470p".into());
        assert!(validate(&b, &g).is_err());
    }
}
