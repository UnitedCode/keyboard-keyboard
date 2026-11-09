use keyboard_seed_tests::simulation::Press;
use std::{fs, path::PathBuf, process::Command, time::SystemTime};

struct Trace(PathBuf);
impl Trace {
    fn new(text: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "keyboard-velocity-{}-{stamp}.csv",
            std::process::id()
        ));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        std::io::Write::write_all(&mut file, text.as_bytes()).unwrap();
        Self(path)
    }
}
impl Drop for Trace {
    fn drop(&mut self) {
        fs::remove_file(&self.0).unwrap();
    }
}

#[test]
fn replay_uses_the_recorded_baseline_and_rejects_missing_samples() {
    let mut csv = String::from("tick_ms,adc\n");
    for (tick, adc) in Press::linear(32.).samples() {
        csv.push_str(&format!("{tick},{}\n", adc - 200));
    }
    let trace = Trace::new(&csv);
    let result = Command::new(env!("CARGO_BIN_EXE_velocity-sim"))
        .arg("replay")
        .arg(&trace.0)
        .arg("1800")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report = String::from_utf8(result.stdout).unwrap();
    assert_eq!(report.lines().count(), 3, "{report}");
    assert!(report.contains("release_ticks=["));
    let velocity: u8 = report
        .lines()
        .nth(1)
        .unwrap()
        .split(',')
        .nth(2)
        .unwrap()
        .parse()
        .unwrap();
    assert!((57..=58).contains(&velocity));

    let missing = Trace::new("tick_ms,adc\n0,1800\n2,1800\n");
    let result = Command::new(env!("CARGO_BIN_EXE_velocity-sim"))
        .arg("replay")
        .arg(&missing.0)
        .arg("1800")
        .output()
        .unwrap();
    assert!(!result.status.success());
}

#[test]
fn calibration_command_checks_measured_noise_margin() {
    for (bottom, success) in [("2600", true), ("2100", false), ("5000", false)] {
        let result = Command::new(env!("CARGO_BIN_EXE_velocity-sim"))
            .args(["calibrate", "2000", bottom, "12"])
            .output()
            .unwrap();
        assert_eq!(result.status.success(), success);
    }
}
