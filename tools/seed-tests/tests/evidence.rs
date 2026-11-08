use keyboard_seed_tests::{classify, midi_csv, Evidence};

#[test]
fn a_stall_needs_working_input_and_an_independent_fast_uart_control() {
    let quiet=Evidence::parse("elapsed_ns=100000000\nscans=27\nmax_scan_gap_ns=1000000\nscan_gaps_over_2ms=0\nmidi_bytes=0\ninstant_uart=false\n").unwrap();
    let paced=Evidence::parse("elapsed_ns=250000000\nscans=167\nmax_scan_gap_ns=11000000\nscan_gaps_over_2ms=1\nmidi_bytes=48\ninstant_uart=false\n").unwrap();
    let instant=Evidence::parse("elapsed_ns=250000000\nscans=177\nmax_scan_gap_ns=1000000\nscan_gaps_over_2ms=0\nmidi_bytes=48\ninstant_uart=true\n").unwrap();
    let midi: Vec<_> = (0..16)
        .flat_map(|channel| [0xb0 | channel, 123, 0])
        .collect();
    assert!(classify(&quiet, &paced, &instant, &midi, &midi)
        .unwrap()
        .contains("SOFTWARE_STALL_REPRODUCED"));
    assert!(classify(&quiet, &paced, &instant, &[], &midi).is_err());
    let mut broken = midi.clone();
    broken[47] = 1;
    assert!(classify(&quiet, &paced, &instant, &broken, &midi).is_err());
    let mut quiet_control = instant;
    quiet_control.gaps = 1;
    quiet_control.max_gap_ns = 11_000_000;
    assert!(classify(&quiet, &paced, &quiet_control, &midi, &midi).is_err());
}

#[test]
fn malformed_or_missing_evidence_cannot_pass() {
    assert!(Evidence::parse("scans=100\nscans=101\n").is_err());
    assert!(Evidence::parse("scans=100\n").is_err());
    assert!(midi_csv("completed_ns,byte\n2,144\n1,60\n").is_err());
    assert!(midi_csv("completed_ns,byte\n1,256\n").is_err());
    assert!(midi_csv("1,144\n").is_err());
}
