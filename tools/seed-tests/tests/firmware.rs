//! Explicit integration test: absent runtime/ELF must fail, never silently skip.
use std::{env, process::Command};

#[test]
#[ignore = "requires pinned virtual-daisy, Renode runtime and a release keyboard ELF; see README"]
fn actual_firmware_reproduces_uart_dependent_scan_stall() {
    let required = |name| env::var_os(name).unwrap_or_else(|| panic!("Set {name}; see README"));
    let status = Command::new(env!("CARGO_BIN_EXE_keyboard-seed-tests"))
        .arg("reproduce")
        .arg(required("KEYBOARD_VIRTUAL_DAISY"))
        .arg(required("KEYBOARD_RENODE_RUNTIME"))
        .arg(required("KEYBOARD_FIRMWARE_ELF"))
        .arg(required("KEYBOARD_TEST_OUTPUT"))
        .arg(required("KEYBOARD_SEED_PROFILE"))
        .status()
        .unwrap();
    assert!(status.success(), "Actual firmware diagnostic failed");
}
