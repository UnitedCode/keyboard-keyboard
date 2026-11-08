//! Offline grading of recorded traces is distinct from executing the firmware.
use std::{path::Path, process::Command};

#[test]
fn recorded_uart_controls_are_graded_through_the_public_cli() {
    for profile in ["rev5", "rev7"] {
        let output = Command::new(env!("CARGO_BIN_EXE_keyboard-seed-tests"))
            .arg("grade")
            .arg(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("evidence")
                    .join(profile),
            )
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report = String::from_utf8(output.stdout).unwrap();
        assert!(report.contains("SOFTWARE_STALL_REPRODUCED"));
        assert!(report.contains("physical_hardware_cause=UNDETERMINED"));
    }
}

#[test]
fn repaired_firmware_and_burst_recovery_traces_pass_their_public_gates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("evidence");
    for (command, case) in [
        ("grade-fixed", "fixed/rev5"),
        ("grade-fixed", "fixed/rev7"),
        ("grade-chord", "chord-after"),
        ("grade-overload", "overload-after"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_keyboard-seed-tests"))
            .arg(command)
            .arg(root.join(case))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{case}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let output = Command::new(env!("CARGO_BIN_EXE_keyboard-seed-tests"))
        .arg("grade-chord")
        .arg(root.join("chord-before"))
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "The old 32-key limit must fail the chord gate"
    );
}
