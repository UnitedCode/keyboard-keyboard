use keyboard_seed_tests::{classify, midi_csv, verify_fixed, Evidence, Result};
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
fn chord_scenario() -> String {
    let mut s = String::from("100000000,reset\n125000000,save,baseline\n125000000,reset\n");
    // Forty populated sensor inputs, independent of firmware event buffers.
    for (time, uv) in [(130_000_000, 2_200_000), (240_000_000, 1_650_000)] {
        for mux in 0..5 {
            for ch in 0..8 {
                s += &format!("{time},input,{mux},{ch},{uv}\n");
            }
        }
    }
    s += "400000000,save,early\n";
    s
}
fn overload_scenario() -> String {
    let mut s = String::from("100000000,reset\n");
    // More events than a 31,250-baud link can carry: recovery must be explicit.
    for step in 0..30 {
        let time = 130_000_000 + step * 10_000_000;
        let uv = if step % 2 == 0 { 2_200_000 } else { 1_650_000 };
        for mux in 0..5 {
            for ch in 0..8 {
                s += &format!("{time},input,{mux},{ch},{uv}\n");
            }
        }
    }
    s += "800000000,save,early\n";
    s
}
fn grade_overload(out: &Path) -> Result<String> {
    let (e, bytes) = read(&out.join("paced/keyboard.early"))?;
    let rtt = fs::read_to_string(out.join("paced/keyboard.rtt"))?;
    if e.scans < 650
        || e.gaps != 0
        || e.max_gap_ns > 2_000_000
        || e.instant
        || e.midi_bytes != bytes.len() as u64
        || !bytes.len().is_multiple_of(3)
        || !rtt.contains("MIDI event queue overflow:")
    {
        return Err("Missing bounded scan timing or explicit queue-overload recovery".into());
    }
    let expected: Vec<_> = (0..16).flat_map(|ch| [0xb0 | ch, 123, 0]).collect();
    let values: Vec<_> = bytes.iter().map(|(_, b)| *b).collect();
    if !values.windows(48).any(|w| w == expected) {
        return Err("Missing complete all-channel panic sequence".into());
    }
    let mut held = std::collections::BTreeMap::new();
    for msg in values.chunks_exact(3) {
        let ch = msg[0] & 15;
        match msg[0] & 0xf0 {
            0x90 if msg[2] > 0 => {
                held.insert((ch, msg[1]), true);
            }
            0x80 => {
                held.insert((ch, msg[1]), false);
            }
            0xb0 if msg[1] == 123 && msg[2] == 0 => {
                held.retain(|(channel, _), _| *channel != ch);
            }
            _ => return Err("Unexpected MIDI message during overload".into()),
        }
    }
    if held.values().any(|v| *v) {
        return Err("Overload recovery left a sounding note".into());
    }
    Ok(format!(
        "QUEUE_OVERLOAD_RECOVERY_VERIFIED\nmax_scan_gap_ns={}\nsounding_notes_at_end=0\n",
        e.max_gap_ns
    ))
}
fn grade_chord(out: &Path) -> Result<String> {
    let mut report = String::from("CHORD_DELIVERY_VERIFIED\n");
    for name in ["paced", "instant"] {
        let (e, bytes) = read(&out.join(format!("{name}/keyboard.early")))?;
        if e.scans < 250
            || e.max_gap_ns > 2_000_000
            || e.gaps != 0
            || e.instant != (name == "instant")
            || e.midi_bytes != 240
            || bytes.len() != 240
        {
            return Err(
                format!("{name}: 40-key chord lost events or scan deadlines: {e:?}").into(),
            );
        }
        let mut held = std::collections::BTreeMap::new();
        let mut note_ons = 0;
        for msg in bytes.chunks_exact(3) {
            let count = held.entry((msg[0].1 & 15, msg[1].1)).or_insert(0i32);
            match msg[0].1 & 0xf0 {
                0x90 if msg[2].1 > 0 => {
                    *count += 1;
                    note_ons += 1;
                }
                0x80 => {
                    *count -= 1;
                    if *count < 0 {
                        return Err("Note-off preceded its note-on".into());
                    }
                }
                _ => return Err("Unexpected MIDI message in chord".into()),
            }
        }
        if note_ons != 40 || held.values().any(|v| *v != 0) {
            return Err("Chord left missing or stuck notes".into());
        }
        report += &format!(
            "{name}_note_ons=40\n{name}_note_offs=40\n{name}_max_gap_ns={}\n",
            e.max_gap_ns
        );
    }
    Ok(report)
}
fn grade(out: &Path, fixed: bool) -> Result<String> {
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
    let check = if fixed { verify_fixed } else { classify };
    let report = check(
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
    if args.len() == 2
        && (args[0] == "grade"
            || args[0] == "grade-fixed"
            || args[0] == "grade-chord"
            || args[0] == "grade-overload")
    {
        let report = if args[0] == "grade-overload" {
            grade_overload(Path::new(&args[1]))?
        } else if args[0] == "grade-chord" {
            grade_chord(Path::new(&args[1]))?
        } else {
            grade(Path::new(&args[1]), args[0] == "grade-fixed")?
        };
        print!("{report}");
        return Ok(());
    }
    if args.len() != 6
        || !(args[0] == "reproduce"
            || args[0] == "verify"
            || args[0] == "verify-chord"
            || args[0] == "verify-overload"
            || args[0] == "verify-soak"
            || args[0] == "soak")
    {
        return Err("Usage: keyboard-seed-tests reproduce|verify|verify-chord|verify-overload|soak|verify-soak VIRTUAL_DAISY_BINARY RENODE_RUNTIME FIRMWARE_ELF OUTPUT_DIRECTORY rev5|rev7; or grade|grade-fixed|grade-chord|grade-overload OUTPUT_DIRECTORY".into());
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
    let fixed = args[0] == "verify" || args[0] == "verify-soak";
    let seconds = if args[0] == "soak" || args[0] == "verify-soak" {
        3661
    } else {
        1
    };
    if seconds > 1 && env::var_os("VIRTUAL_DAISY_WALL_SECONDS").is_none() {
        return Err(
            "Set VIRTUAL_DAISY_WALL_SECONDS for the expensive continuous soak; see README".into(),
        );
    }
    fs::write(
        &stimulus,
        if args[0] == "verify-overload" {
            overload_scenario()
        } else if args[0] == "verify-chord" {
            chord_scenario()
        } else {
            scenario(seconds)
        },
    )?;
    for (name, instant) in [("paced", false), ("instant", true)] {
        if args[0] == "verify-overload" && instant {
            continue;
        }
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
    let mut report = if args[0] == "verify-overload" {
        grade_overload(&out)?
    } else if args[0] == "verify-chord" {
        grade_chord(&out)?
    } else {
        grade(&out, fixed)?
    };
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
        let check = if fixed { verify_fixed } else { classify };
        report += &check(
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
