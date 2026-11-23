//! Complete ARM execution with a declared supervisor and isolation boundary.
use crate::power;
use circuit_workbench::{
    carrier::shared, electrical::KeyboardCircuit, fixture::Fixture, netlist::Graph, oled::Oled,
    sha256, Result,
};
use std::{fs, path::Path, time::Instant};
use virtual_daisy::{
    board::Board,
    machine::{Carrier, Error, Machine},
};
const MS: u64 = 1_000_000;
struct Device {
    scenario: String,
    circuit: KeyboardCircuit,
    oled: Oled,
    edges: Vec<(u64, bool)>,
    next: usize,
    valid: bool,
    source_events: Vec<(u64, String, u32)>,
    event: usize,
    error: Option<String>,
    adc: Vec<(u64, u8, u32)>,
    gpio: Vec<(u64, [u16; 11])>,
    outputs: [u16; 11],
    scans: u64,
    last_ch: u16,
    last_scan: u64,
    max_gap: u64,
}
impl Device {
    fn advance(&mut self, outputs: &[u16; 11], now: u64) -> Result<()> {
        while self.next < self.edges.len() && self.edges[self.next].0 <= now {
            self.valid = self.edges[self.next].1;
            self.next += 1;
        }
        while self.event < self.source_events.len() && self.source_events[self.event].0 <= now {
            let (t, r, v) = &self.source_events[self.event];
            self.circuit.observe(*t, outputs)?;
            self.circuit.set_microvolts(*t, r, *v)?;
            self.event += 1;
        }
        self.circuit.observe(now, outputs)?;
        if outputs != &self.outputs {
            self.gpio.push((now, *outputs));
            self.outputs = *outputs;
        }
        Ok(())
    }
}
impl Carrier for Device {
    fn pins(&mut self, outputs: &[u16; 11], now: u64) -> [(u16, u16); 11] {
        if outputs != &self.outputs
            || self.edges.get(self.next).is_some_and(|e| e.0 <= now)
            || self
                .source_events
                .get(self.event)
                .is_some_and(|e| e.0 <= now)
        {
            if let Err(e) = self.advance(outputs, now) {
                self.error = Some(e.to_string());
            }
        }
        let mut p = [(0, 0); 11];
        p[1] = (1 << 5, if self.valid { 1 << 5 } else { 0 });
        p
    }
    fn adc_input(
        &mut self,
        adc: u8,
        ch: u8,
        outputs: &[u16; 11],
        now: u64,
    ) -> std::result::Result<u32, Error> {
        if adc != 1 {
            return Err("only ADC1 connected".into());
        }
        self.advance(outputs, now)
            .map_err(|_| Error::from("fault circuit advance"))?;
        // Fault isolation is an explicit ideal boundary; detailed new switch loading
        // and bias settling remain outside this full-ARM regression.
        let v = if self.valid && power::rail(&self.scenario, now) >= 3.0 {
            self.circuit
                .sample(now, ch, outputs)
                .map_err(|_| Error::from("fault ADC"))?
        } else {
            1_650_000
        };
        self.adc.push((now, ch, v));
        if ch == 10 {
            let channel = ((outputs[6] >> 10) & 1)
                | (((outputs[6] >> 11) & 1) << 1)
                | (((outputs[1] >> 4) & 1) << 2);
            if channel == 0 && self.last_ch == 7 {
                self.scans += 1;
                if self.last_scan != 0 {
                    self.max_gap = self.max_gap.max(now - self.last_scan);
                }
                self.last_scan = now;
            }
            self.last_ch = channel;
        }
        Ok(v)
    }
    fn i2c_write(&mut self, bus: u8, address: u8, bytes: &[u8]) -> std::result::Result<(), Error> {
        if bus != 1 || address != 0x3c {
            return Err("unconnected I2C".into());
        }
        self.oled
            .write(bytes)
            .map_err(|_| Error::from("OLED protocol"))
    }
    fn next_event(&self) -> Option<u64> {
        self.edges
            .get(self.next)
            .map(|e| e.0)
            .into_iter()
            .chain(self.source_events.get(self.event).map(|e| e.0))
            .min()
    }
}
fn u32at(b: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        b.get(at..at + 4).ok_or("truncated ELF")?.try_into()?,
    ))
}
fn rtt(b: &[u8]) -> Result<u32> {
    let table = u32at(b, 32)? as usize;
    let size = (u32at(b, 44)? >> 16) as usize;
    let count = (u32at(b, 48)? & 65535) as usize;
    if size < 40 {
        return Err("missing ELF sections".into());
    }
    for i in 0..count {
        let h = table + i * size;
        if u32at(b, h + 4)? != 2 {
            continue;
        }
        let strings = table + u32at(b, h + 24)? as usize * size;
        let str_at = u32at(b, strings + 16)? as usize;
        let offset = u32at(b, h + 16)? as usize;
        let len = u32at(b, h + 20)? as usize;
        for s in (offset..offset + len).step_by(16) {
            let at = str_at + u32at(b, s)? as usize;
            if b.get(at..at + 12) == Some(b"_SEGGER_RTT\0") {
                return u32at(b, s + 4);
            }
        }
    }
    Err("missing RTT symbol".into())
}
fn drain(m: &mut Machine, at: u32, log: &mut Vec<u8>) -> Result<()> {
    if m.read32(at)? != u32::from_le_bytes(*b"SEGG") {
        return Ok(());
    }
    let d = at + 24;
    let buffer = m.read32(d + 4)?;
    let size = m.read32(d + 8)?;
    let write = m.read32(d + 12)?;
    let mut read = m.read32(d + 16)?;
    if size == 0 || size > 65536 || write >= size || read >= size {
        return Err("invalid RTT".into());
    }
    while read != write {
        let a = buffer + read;
        log.push((m.read32(a & !3)? >> ((a & 3) * 8)) as u8);
        read = (read + 1) % size;
    }
    m.write32(d + 16, read)?;
    Ok(()) // standard RTT consumer pointer only
}
pub fn cli(a: &[String]) -> Result<()> {
    if a.len() != 7 {
        return Err(
            "guard-device PARENT CANDIDATE FIXTURE ELF SCENARIO FRESH_DIR positive|negative".into(),
        );
    }
    let name = &a[4];
    let e = power::edges(name)?;
    let expected = match a[6].as_str() {
        "positive" => true,
        "negative" => false,
        _ => return Err("choose positive or negative".into()),
    };
    let out = Path::new(&a[5]);
    fs::create_dir(out)?;
    let b = Graph::parse(&fs::read_to_string(&a[0])?)?;
    let g = Graph::parse(&fs::read_to_string(&a[1])?)?;
    power::validate(&b, &g)?;
    let f = Fixture::parse(&fs::read_to_string(&a[2])?)?;
    let elf = fs::read(&a[3])?;
    for (p, n) in [
        (&a[0], "parent.net"),
        (&a[1], "candidate.net"),
        (&a[2], "assumptions.fixture"),
        (&a[3], "firmware.elf"),
    ] {
        fs::copy(p, out.join(n))?;
    }
    fs::write(
        out.join("sources.sha256"),
        env!("KEYBOARD_NOISE_SOURCES").replace(';', "\n"),
    )?;
    fs::write(
        out.join("shared-runner-sources.sha256"),
        circuit_workbench::RUNNER_SOURCES.replace(';', "\n"),
    )?;
    let mut circuit = KeyboardCircuit::new(&b, &f)?;
    for i in 1..=100 {
        circuit.set_microvolts(0, &format!("HE{i}"), 1_300_000)?;
    }
    let mut source_events = Vec::new();
    // 14 simultaneous keys exercise queued and in-flight MIDI during brownout.
    for (t, v) in [
        (270 * MS, 1_850_000),
        (340 * MS, 1_300_000),
        (420 * MS, 1_850_000),
        (480 * MS, 1_300_000),
    ] {
        for i in 1..=14 {
            source_events.push((t, format!("HE{i}"), v));
        }
    }
    if name == "reset-after-loss" {
        for (t, v) in [(700 * MS, 1_850_000), (750 * MS, 1_300_000)] {
            for i in 1..=14 {
                source_events.push((t, format!("HE{i}"), v));
            }
        }
    }
    let d = Device {
        scenario: name.clone(),
        circuit,
        oled: Oled::default(),
        edges: e.clone(),
        next: 0,
        valid: false,
        source_events,
        event: 0,
        error: None,
        adc: Vec::new(),
        gpio: Vec::new(),
        outputs: [0; 11],
        scans: 0,
        last_ch: 0,
        last_scan: 0,
        max_gap: 0,
    };
    let (handle, attach) = shared(d);
    let mut m = Machine::from_elf(Board::Rev4, &elf)?;
    m.attach(attach);
    let symbol = rtt(&elf)?;
    let start = Instant::now();
    let mut serial = Vec::new();
    let mut log = Vec::new();
    let mut errors = Vec::new();
    let mut audio_frames = 0;
    let end = if name == "reset-after-loss" { 850 } else { 600 };
    for ms in 1..=end {
        if name == "reset-after-loss" && ms == 500 {
            m.write32(0xe000ed0c, 0x05fa0004)?; // architectural reset, never private application state
        }
        if let Err(e) = m.run_until(ms * MS, 3_000_000) {
            errors.push(e.to_string());
            break;
        }
        audio_frames += m.take_audio().len();
        serial.extend(m.take_serial());
        if ms % 5 == 0 {
            drain(&mut m, symbol, &mut log)?;
        }
        if m.read32(0xe000ed28)? != 0 || m.read32(0xe000ed2c)? != 0 {
            errors.push("Cortex-M fault".into());
            break;
        }
        if ms % 100 == 0 {
            eprintln!(
                "guard/{name}: {ms} virtual ms, {} wall seconds",
                start.elapsed().as_secs()
            );
        }
    }
    drain(&mut m, symbol, &mut log)?;
    let input_mode = (m.read32(0x58020400)? >> 10) & 3;
    let input_pull = (m.read32(0x5802040c)? >> 10) & 3;
    if expected && (input_mode != 0 || input_pull != 0) {
        errors.push("PB5 input mode/pull mismatch".into());
    }
    let d = handle.borrow();
    if let Some(e) = &d.error {
        errors.push(e.clone());
    }
    let rtt = String::from_utf8_lossy(&log);
    if rtt.contains("panicked") || !rtt.contains("keyboard_keyboard ready:") {
        errors.push("startup/panic RTT".into());
    }
    let mut text = String::from("time_ns,byte\n");
    for (t, b) in &serial {
        text.push_str(&format!("{t},{b}\n"));
    }
    fs::write(out.join("midi.csv"), text)?;
    let mut text = String::from("time_ns,channel,microvolts\n");
    for (t, c, v) in &d.adc {
        text.push_str(&format!("{t},{c},{v}\n"));
    }
    fs::write(out.join("adc.csv"), text)?;
    let mut text = String::from("time_ns,power_valid\n");
    for (t, v) in &e {
        text.push_str(&format!("{t},{v}\n"));
    }
    fs::write(out.join("power-valid.csv"), text)?;
    fs::write(out.join("firmware.rtt"), &log)?;
    fs::write(out.join("oled-final.svg"), d.oled.svg())?;
    let mut gpio = String::from("time_ns,pb,pc,pg\n");
    for (t, p) in &d.gpio {
        gpio.push_str(&format!("{t},{},{},{}\n", p[1], p[2], p[6]));
    }
    fs::write(out.join("gpio.csv"), gpio)?;
    let mut status = 0;
    let mut pending = Vec::new();
    let mut first = 0;
    let mut held = std::collections::BTreeSet::new();
    let mut ons = 0;
    let mut late = 0;
    let mut explicit_offs = 0;
    let mut pot_cc = std::collections::BTreeSet::new();
    let mut post_assert_ons = 0;
    let mut panic_complete = None;
    let fault = e
        .iter()
        .find(|(t, v)| *t > 100 * MS && !*v)
        .map(|(t, _)| *t);
    let blocked_start = if name.starts_with("startup-") {
        Some(0)
    } else {
        fault.map(|t| t + 3 * MS)
    };
    for (t, b) in &serial {
        if *b >= 0xf8 {
            continue;
        }
        if b & 0x80 != 0 {
            status = *b;
            pending.clear();
            first = *t;
            continue;
        }
        pending.push(*b);
        let len = if status & 0xe0 == 0xc0 { 1 } else { 2 };
        if pending.len() != len {
            continue;
        }
        let ch = status & 15;
        match status & 0xf0 {
            0x90 if pending[1] > 0 => {
                held.insert((ch, pending[0]));
                ons += 1;
                if fault.is_some_and(|at| first >= at)
                    && !(name == "reset-after-loss" && first >= 650 * MS)
                {
                    post_assert_ons += 1;
                }
                if blocked_start.is_some_and(|at| first >= at)
                    && !(name == "reset-after-loss" && first >= 650 * MS)
                {
                    late += 1;
                }
            }
            0x80 | 0x90 => {
                held.remove(&(ch, pending[0]));
                explicit_offs += 1;
            }
            0xb0 if [120, 123].contains(&pending[0]) => held.retain(|(c, _)| *c != ch),
            0xb0 if ch == 0 && pending[1] == 127 => {
                pot_cc.insert(pending[0]);
            }
            0xe0 if ch == 15 && pending == [0, 64] && fault.is_some_and(|at| first >= at) => {
                panic_complete.get_or_insert(*t);
            }
            _ => {}
        }
        pending.clear();
    }
    let pace = serial.windows(2).all(|w| w[1].0 - w[0].0 >= 320_000);
    let expected_notes = if name == "healthy" || name == "reset-after-loss" {
        ons == 28 && explicit_offs == 28
    } else if name.starts_with("startup-") {
        ons == 0
    } else {
        ons > 0 && explicit_offs > 0
    };
    let all_pots = [7, 10, 11, 74, 36, 37, 38, 39, 40, 41, 42, 43]
        .iter()
        .all(|cc| pot_cc.contains(cc));
    if expected && !name.starts_with("startup-") && !all_pots {
        errors.push("initial pot controllers missing".into());
    }
    if expected
        && !name.starts_with("startup-")
        && name != "healthy"
        && !fault
            .zip(panic_complete)
            .is_some_and(|(at, end)| end >= at && end - at < 170 * MS)
    {
        errors.push("panic did not finish within declared 170ms wire budget".into());
    }
    let functional = late == 0 && held.is_empty() && expected_notes;
    let gap_limit = if name == "reset-after-loss" {
        150 * MS
    } else {
        2 * MS
    };
    let live = (d.scans > 20 && d.max_gap < gap_limit) || name.starts_with("startup-");
    if !pace || !live || audio_frames < 20000 || d.oled.data_bytes < 512 {
        errors.push("serial/scan/audio/OLED liveness".into());
    }
    let pass = errors.is_empty() && functional;
    let report=format!("ARM_POWER_DEVICE={}\nscenario={name}\nexpected_positive={expected}\nfirmware_sha256={}\ncandidate_sha256={}\nvirtual_time_ns={}\nwall_seconds={}\nscheduler_steps={}\nnotes_on={ons}\nexplicit_note_off={explicit_offs}\nlate_note_on={late}\nheld_notes={}\npaced_31250_baud={pace}\nscans={}\nmax_scan_gap_ns={}\naudio_frames={audio_frames}\nerrors={errors:?}\nPOWER_MODEL=TPS3808 behavioral20us/28ms; physical rail scenario to GPIO PB5 and ideal ADC mid-bias isolation\nANALOG_SCOPE=original RC kernel as signal projection; added TMUX/bias loading and vendor transistor dynamics not integrated\nSENSOR_FIXTURE=1.3V rest,+0.55V stroke on14keys; not measured magnets\nPHYSICAL_MEASURED=NOT_RUN\n",if pass{"PASS"}else{"FAIL"},sha256(&out.join("firmware.elf"))?,sha256(&out.join("candidate.net"))?,m.time_ns(),start.elapsed().as_secs_f64(),m.steps(),held.len(),d.scans,d.max_gap);
    let report=format!("{report}note_on_after_assertion_including_detection_window={post_assert_ons}\npanic_complete_ns={panic_complete:?}\ninitial_pot_controllers={all_pots}\nPB5_input_mode={input_mode}\nPB5_pull={input_pull}\nRESET_GAP_NOTE=reset-after-loss allows150ms observer scan gap for architectural reboot; ordinary scan windows require2ms\nWIRE_NOTE=up to3ms detection/in-flight window is observed separately from forbidden later NoteOn; not instantaneous silence\n");
    fs::write(out.join("report.txt"), &report)?;
    print!("{report}");
    // A negative control must be a functional failure, never a crashed emulator.
    if pass != expected || (!expected && (!errors.is_empty() || functional)) {
        return Err("unexpected device acceptance/control".into());
    }
    Ok(())
}
