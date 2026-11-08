use keyboard_seed_tests::{midi_csv, verify_fixed, Evidence};

#[test]
fn the_fix_gate_rejects_the_measured_firmware_stall() {
    let evidence = |text| Evidence::parse(text).unwrap();
    let quiet = evidence(include_str!(
        "../evidence/rev7/paced/keyboard.baseline.keyboard.txt"
    ));
    let paced = evidence(include_str!(
        "../evidence/rev7/paced/keyboard.early.keyboard.txt"
    ));
    let instant = evidence(include_str!(
        "../evidence/rev7/instant/keyboard.early.keyboard.txt"
    ));
    let bytes = |text| {
        midi_csv(text)
            .unwrap()
            .into_iter()
            .map(|(_, b)| b)
            .collect::<Vec<_>>()
    };
    let midi = bytes(include_str!(
        "../evidence/rev7/paced/keyboard.early.midi.csv"
    ));
    let control = bytes(include_str!(
        "../evidence/rev7/instant/keyboard.early.midi.csv"
    ));
    assert!(verify_fixed(&quiet, &paced, &instant, &midi, &control).is_err());
}
