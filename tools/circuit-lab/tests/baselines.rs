use circuit_workbench::{carrier::KeyboardCarrier, fixture::Fixture, netlist::Graph, profiles};
use std::{fs, path::PathBuf};
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn graph(name: &str) -> Graph {
    Graph::parse(
        &fs::read_to_string(root().join(format!("circuits/baselines/{name}.net"))).unwrap(),
    )
    .unwrap()
}
#[test]
fn preserves_both_board_revisions_and_detects_faults() {
    let fixture = Fixture::parse(
        &fs::read_to_string(root().join("circuits/fixtures/reference.fixture")).unwrap(),
    )
    .unwrap();
    for name in ["keyboard-current", "keyboard-pre-repair"] {
        let out =
            std::env::temp_dir().join(format!("circuit-regression-{}-{name}", std::process::id()));
        fs::create_dir(&out).unwrap();
        let mut g = graph(name);
        let report = profiles::verify(name, &g, &fixture, &out).unwrap();
        assert!(report.contains("controls_routed=112"));
        #[cfg(feature = "spice")]
        {
            assert!(report.contains("controls_sampled=112"));
            assert!(report.contains(if name.ends_with("pre-repair") {
                "unused_outputs_grounded=[true, true]"
            } else {
                "unused_outputs_grounded=[false, false]"
            }));
        }
        g.disconnect("A1.35");
        assert!(profiles::verify(name, &g, &fixture, &out).is_err());
        fs::remove_dir_all(out).unwrap();
    }
}
#[test]
fn carrier_routes_all_controls_and_refuses_shorted_board() {
    let g = graph("keyboard-current");
    let mut c = KeyboardCarrier::new(&g).unwrap();
    for (index, (r, m, ch, pad)) in profiles::keyboard_routes(&g).unwrap().iter().enumerate() {
        let mux: usize = m[2..].parse().unwrap();
        let decoder = mux.saturating_sub(10);
        let mut outputs = [0; 11];
        outputs[6] = ((*ch as u16 & 1) << 10) | (((*ch as u16 >> 1) & 1) << 11);
        outputs[1] = ((*ch as u16 >> 2) & 1) << 4;
        outputs[2] = ((decoder as u16 & 1) << 8)
            | (((decoder as u16 >> 1) & 1) << 9)
            | (((decoder as u16 >> 2) & 1) << 10);
        let channel = match pad.as_str() {
            "A1.22" => 10,
            "A1.23" => 15,
            "A1.24" => 5,
            "A1.25" => 7,
            "A1.26" => 3,
            "A1.27" => 11,
            "A1.28" => 4,
            "A1.29" => 19,
            "A1.30" => 18,
            "A1.31" => 17,
            "A1.32" => 16,
            "A1.35" => 14,
            _ => panic!("unknown pad"),
        };
        c.set_microvolts(r, 300_000 + index as u32 * 20_000)
            .unwrap();
        assert_eq!(
            c.sample(channel, &outputs).unwrap(),
            300_000 + index as u32 * 20_000
        );
    }
    assert!(c.set_microvolts("HE1", 3_300_001).is_err());
    assert!(c.set_microvolts("HE101", 1).is_err());
    assert!(KeyboardCarrier::new(&graph("keyboard-pre-repair")).is_err());
}

#[test]
fn frozen_current_source_still_matches_schematic() {
    circuit_workbench::cli(vec![
        "check-source".into(),
        root()
            .join("circuits/baselines/keyboard-current.net")
            .display()
            .to_string(),
        root()
            .join("keyboard_keyboard/kicad/keyboard_keyboard.kicad_sch")
            .display()
            .to_string(),
    ])
    .unwrap();
}

#[test]
fn evolving_circuit_preserves_filter_lag_and_an_open_sensor_cannot_actuate() {
    let g = graph("keyboard-current");
    let f = Fixture::parse(
        &fs::read_to_string(root().join("circuits/fixtures/reference.fixture")).unwrap(),
    )
    .unwrap();
    let mut circuit = circuit_workbench::electrical::KeyboardCircuit::new(&g, &f).unwrap();
    let mut open = circuit_workbench::electrical::KeyboardCircuit::new(&g, &f).unwrap();
    open.open_control = Some("HE7".into());
    for c in [&mut circuit, &mut open] {
        c.observe(0, &[0; 11]).unwrap();
        c.sample(100_000, 10, &[0; 11]).unwrap();
        c.set_microvolts(100_000, "HE7", 2_200_000).unwrap();
    }
    let before = circuit.sample(100_000, 10, &[0; 11]).unwrap();
    let after = circuit.sample(150_000, 10, &[0; 11]).unwrap();
    assert!(before < 1_700_000);
    assert!(after > 2_190_000 && after <= 2_200_000);
    assert!(open.sample(150_000, 10, &[0; 11]).unwrap() < 1_700_000);
    assert!(circuit.sample(140_000, 10, &[0; 11]).is_err());
}

#[cfg(feature = "daisy")]
#[test]
fn sdk_adapter_observes_changed_sensor_and_rejects_wrong_adc() {
    let (handle, mut adapter) = circuit_workbench::carrier::shared(
        KeyboardCarrier::new(&graph("keyboard-current")).unwrap(),
    );
    // HE7 is AM1/X0 in the actual source, not HE1.
    handle
        .borrow_mut()
        .set_microvolts("HE7", 2_000_000)
        .unwrap();
    assert_eq!(adapter.adc_input(1, 10, &[0; 11], 0).unwrap(), 2_000_000);
    assert!(adapter.adc_input(2, 10, &[0; 11], 0).is_err());
    adapter.i2c_write(1, 0x3c, &[0]).unwrap();
    assert_eq!(handle.borrow().oled_writes, 1);
}

#[test]
fn independent_filter_candidates_change_only_the_output_capacitors() {
    use circuit_workbench::improvements::{self, FILTER, FILTER470};
    let base = graph("keyboard-current");
    for (name, dir) in [(FILTER, "filter-1n"), (FILTER470, "filter-470p")] {
        let changed = Graph::parse(
            &fs::read_to_string(root().join(format!("circuits/experiments/{dir}/candidate.net")))
                .unwrap(),
        )
        .unwrap();
        improvements::validate(name, &base, &changed).unwrap();
        assert_eq!(base.nets, changed.nets);
        assert_eq!(changed.components["C1"], "100n");
        let mut wrong = changed.clone();
        wrong.components.insert("C1".into(), "1n".into());
        assert!(improvements::validate(name, &base, &wrong).is_err());
        wrong = changed.clone();
        wrong.disconnect("HE1.2");
        assert!(improvements::validate(name, &base, &wrong).is_err());
        let out =
            std::env::temp_dir().join(format!("keyboard-candidate-{}-{dir}", std::process::id()));
        circuit_workbench::cli(vec![
            "compare".into(),
            name.into(),
            root()
                .join("circuits/baselines/keyboard-current.net")
                .display()
                .to_string(),
            root()
                .join(format!("circuits/experiments/{dir}/candidate.net"))
                .display()
                .to_string(),
            root()
                .join("circuits/fixtures/reference.fixture")
                .display()
                .to_string(),
            out.display().to_string(),
        ])
        .unwrap();
        let report = fs::read_to_string(out.join("report.txt")).unwrap();
        assert!(report.contains("ISOLATION=PASS"));
        #[cfg(feature = "spice")]
        assert!(report.contains(if name == FILTER {
            "full_scan_failures_after=4"
        } else {
            "full_scan_failures_after=0"
        }));
        fs::remove_dir_all(out).unwrap();
    }
}

#[test]
fn power_candidate_preserves_filters_and_rejects_bypasses() {
    let base = graph("keyboard-current");
    let c = circuit_workbench::qualification::offload(&base).unwrap();
    for (r, v) in &base.components {
        assert_eq!(c.components.get(r), Some(v));
    }
    assert_ne!(c.net("A1.38").unwrap(), c.net("HE1.1").unwrap());
    assert_eq!(c.components["PG1"], "DUAL_RAIL_GUARD_UNSELECTED");
    assert_eq!(c.components["R5"], "220");
    assert_eq!(
        base.components
            .iter()
            .filter(|(r, v)| r.starts_with('C') && *v == "4.7n")
            .count(),
        100
    );
    circuit_workbench::qualification::validate_offload(&base, &c).unwrap();
    let mut bad = c.clone();
    bad.disconnect("PU2.3");
    assert!(circuit_workbench::qualification::validate_offload(&base, &bad).is_err());
}

#[test]
fn lower_current_sensor_swap_changes_only_part_values() {
    let base = graph("keyboard-current");
    let candidate = circuit_workbench::qualification::hall_candidate(&base).unwrap();
    assert_eq!(base.nets, candidate.nets);
    for (r, v) in &candidate.components {
        if r.starts_with("HE") {
            assert_eq!(v, circuit_workbench::qualification::HALL_PART);
        } else {
            assert_eq!(Some(v), base.components.get(r));
        }
    }
    assert_eq!(
        candidate
            .components
            .iter()
            .filter(|(r, v)| r.starts_with('C') && *v == "4.7n")
            .count(),
        100
    );
}
