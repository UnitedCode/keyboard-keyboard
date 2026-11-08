use keyboard_seed_tests::{classify, midi_csv, Evidence, Result};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn read(prefix: &Path) -> Result<(Evidence, Vec<(u64, u8)>)> {
    let artifact = |suffix: &str| {
        let mut path = prefix.as_os_str().to_os_string();
        path.push(suffix);
        PathBuf::from(path)
    };
    Ok((
        Evidence::parse(&fs::read_to_string(artifact(".keyboard.txt"))?)?,
        midi_csv(&fs::read_to_string(artifact(".midi.csv"))?)?,
    ))
}
fn scenario(seconds: u64) -> String {
    let mut s=String::from("100000000,save,warmup\n100000000,reset\n125000000,save,baseline\n125000000,reset\n130000000,input,10,7,2200000\n180000000,input,10,7,1650000\n250000000,save,early\n250000000,reset\n260000000,input,0,4,2200000\n260000000,input,13,4,0\n310000000,input,0,4,1650000\n350000000,save,inputs\n");
    if seconds > 1 {
        // This is real elapsed virtual time: no timer, PC or RAM fast-forward.
        // Quiet snapshots every minute make a stopped firmware distinguishable
        // from elapsed time simply advancing while no useful work happens.
        for second in (60..seconds).step_by(60) {
            s += &format!("{},save,minute-{}\n", second * 1_000_000_000, second / 60);
        }
        let late = (seconds - 1) * 1_000_000_000;
        s += &format!(
            "{late},save,before-late\n{late},reset\n{},input,10,7,2200000\n{},input,10,7,1650000\n",
            late + 10_000_000,
            late + 60_000_000
        );
        s += &format!("{},save,late\n", seconds * 1_000_000_000);
    }
    s
}
fn grade(out: &Path) -> Result<String> {
    let paced_hash = fs::read_to_string(out.join("paced/firmware.sha256"))?;
    let instant_hash = fs::read_to_string(out.join("instant/firmware.sha256"))?;
    if paced_hash.lines().next() != instant_hash.lines().next() || paced_hash.is_empty() {
        return Err("UART controls used different firmware ELFs".into());
    }
    let (quiet, _) = read(&out.join("paced/keyboard.baseline"))?;
    let (paced, bytes) = read(&out.join("paced/keyboard.early"))?;
    let (instant, control) = read(&out.join("instant/keyboard.early"))?;
    for trace in [&bytes, &control] {
        if trace.iter().any(|&(t, _)| t > paced.elapsed_ns) {
            return Err("MIDI evidence exceeds observation time".into());
        }
    }
    // The paced trace must actually represent 31,250-baud 8N1 transmission.
    if bytes.windows(2).any(|w| w[1].0 - w[0].0 < 320_000) {
        return Err("Paced UART accepted an impossibly short MIDI frame".into());
    }
    let report = classify(
        &quiet,
        &paced,
        &instant,
        &bytes.iter().map(|v| v.1).collect::<Vec<_>>(),
        &control.iter().map(|v| v.1).collect::<Vec<_>>(),
    )?;
    for name in ["paced", "instant"] {
        let (_, trace) = read(&out.join(format!("{name}/keyboard.inputs")))?;
        let messages: Vec<_> = trace.chunks_exact(3).collect();
        if !messages.iter().any(|m| m[0].1 & 0xf0 == 0x90 && m[2].1 > 0)
            || !messages.iter().any(|m| m[0].1 & 0xf0 == 0x80)
            || !messages
                .iter()
                .any(|m| m[0].1 & 0xf0 == 0xb0 && m[1].1 == 7 && m[2].1 == 127)
        {
            return Err(format!("{name}: key/pot positive control failed").into());
        }
    }
    Ok(report)
}
fn main() -> Result<()> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() == 2 && args[0] == "grade" {
        let report = grade(Path::new(&args[1]))?;
        print!("{report}");
        return Ok(());
    }
    if args.len() != 6 || !(args[0] == "reproduce" || args[0] == "soak") {
        return Err("Usage: keyboard-seed-tests reproduce|soak VIRTUAL_DAISY_BINARY RENODE_RUNTIME FIRMWARE_ELF OUTPUT_DIRECTORY rev5|rev7; or grade OUTPUT_DIRECTORY".into());
    }
    let emulator = fs::canonicalize(&args[1])?;
    let runtime = fs::canonicalize(&args[2])?;
    let elf = fs::canonicalize(&args[3])?;
    let out = PathBuf::from(&args[4]);
    fs::create_dir_all(&out)?;
    let out = fs::canonicalize(out)?;
    if args[5] != "rev5" && args[5] != "rev7" {
        return Err("Select the WM8731 profile rev5 or PCM3060 profile rev7".into());
    }
    let summary = out.join("diagnosis.txt");
    let stimulus = out.join("all-notes-off.csv");
    if summary.exists()
        || stimulus.exists()
        || out.join("paced").exists()
        || out.join("instant").exists()
    {
        return Err("Use a fresh output directory; existing evidence is preserved".into());
    }
    let seconds = if args[0] == "soak" { 3661 } else { 1 };
    if seconds > 1 && env::var_os("VIRTUAL_DAISY_WALL_SECONDS").is_none() {
        return Err(
            "Set VIRTUAL_DAISY_WALL_SECONDS for the expensive continuous soak; see README".into(),
        );
    }
    fs::write(&stimulus, scenario(seconds))?;
    for (name, instant) in [("paced", false), ("instant", true)] {
        let mut command = Command::new(&emulator);
        command
            .arg("keyboard")
            .arg(&runtime)
            .arg(out.join(name))
            .arg("--elf")
            .arg(&elf)
            .arg("--board")
            .arg(&args[5])
            .arg(&stimulus);
        command.env(
            "VIRTUAL_DAISY_UART_INSTANT",
            if instant { "1" } else { "0" },
        );
        if !command.status()?.success() {
            return Err(
                format!("{name} firmware execution failed; no diagnosis was produced").into(),
            );
        }
    }
    let mut report = grade(&out)?;
    if seconds > 1 {
        for name in ["paced", "instant"] {
            let (before, _) = read(&out.join(format!("{name}/keyboard.before-late")))?;
            let (early, _) = read(&out.join(format!("{name}/keyboard.inputs")))?;
            if before.elapsed_ns < 3_600_000_000_000
                || before.scans.saturating_sub(early.scans) < 3_500_000
                || before.gaps != early.gaps
            {
                return Err(format!("{name}: one-hour liveness/timing gate failed").into());
            }
        }
        let (q, _) = read(&out.join("paced/keyboard.baseline"))?;
        let (p, b) = read(&out.join("paced/keyboard.late"))?;
        let (i, c) = read(&out.join("instant/keyboard.late"))?;
        report += &classify(
            &q,
            &p,
            &i,
            &b.iter().map(|v| v.1).collect::<Vec<_>>(),
            &c.iter().map(|v| v.1).collect::<Vec<_>>(),
        )?;
        report += "continuous_one_hour_soak=EXECUTED\n";
    } else {
        report += "continuous_one_hour_soak=NOT_RUN\n";
    }
    fs::write(&summary, &report)?;
    print!("{report}");
    Ok(())
}
