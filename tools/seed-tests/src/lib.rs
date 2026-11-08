//! Grade externally observed firmware scan timing and MIDI bytes.
use std::collections::BTreeMap;
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug)]
pub struct Evidence {
    pub elapsed_ns: u64,
    pub scans: u64,
    pub max_gap_ns: u64,
    pub gaps: u64,
    pub midi_bytes: u64,
    pub instant: bool,
}
impl Evidence {
    pub fn parse(text: &str) -> Result<Self> {
        let mut fields = BTreeMap::new();
        for line in text.lines() {
            if let Some((key, value)) = line.split_once('=') {
                if fields.insert(key, value).is_some() {
                    return Err(format!("Duplicate {key}").into());
                }
            }
        }
        let field = |key| {
            fields
                .get(key)
                .copied()
                .ok_or_else(|| format!("Missing {key}"))
        };
        Ok(Self {
            elapsed_ns: field("elapsed_ns")?.parse()?,
            scans: field("scans")?.parse()?,
            max_gap_ns: field("max_scan_gap_ns")?.parse()?,
            gaps: field("scan_gaps_over_2ms")?.parse()?,
            midi_bytes: field("midi_bytes")?.parse()?,
            instant: field("instant_uart")?.parse()?,
        })
    }
}
pub fn classify(
    quiet: &Evidence,
    paced: &Evidence,
    instant: &Evidence,
    paced_bytes: &[u8],
    instant_bytes: &[u8],
) -> Result<String> {
    if quiet.scans < 10
        || quiet.max_gap_ns > 2_000_000
        || quiet.instant
        || paced.instant
        || !instant.instant
    {
        return Err("Missing healthy quiet baseline or correctly labelled UART controls".into());
    }
    let expected: Vec<u8> = (0..16).flat_map(|ch| [0xb0 | ch, 123, 0]).collect();
    // Startup CC messages may precede the exact sixteen-channel all-notes-off
    // sequence. Require the complete sequence, not merely a byte count.
    for (evidence, bytes) in [(paced, paced_bytes), (instant, instant_bytes)] {
        if evidence.midi_bytes != bytes.len() as u64
            || !bytes
                .windows(expected.len())
                .any(|window| window == expected)
        {
            return Err("All-notes-off stimulus did not produce its specified MIDI output".into());
        }
        if evidence.elapsed_ns < 200_000_000 || evidence.scans < 50 {
            return Err("Firmware run was too short or stalled at startup".into());
        }
    }
    if paced.elapsed_ns != instant.elapsed_ns {
        return Err("Control duration differs".into());
    }
    if paced.max_gap_ns > 5_000_000
        && paced.gaps > 0
        && instant.max_gap_ns <= 2_000_000
        && instant.gaps == 0
    {
        Ok(format!("SOFTWARE_STALL_REPRODUCED\nquiet_max_gap_ns={}\npaced_max_gap_ns={}\ninstant_control_max_gap_ns={}\nphysical_hardware_cause=UNDETERMINED\n",quiet.max_gap_ns,paced.max_gap_ns,instant.max_gap_ns))
    } else {
        Err(format!("No isolated UART-dependent stall: paced={} ns, control={} ns. This does not prove a hardware cause.",paced.max_gap_ns,instant.max_gap_ns).into())
    }
}

pub fn midi_csv(text: &str) -> Result<Vec<(u64, u8)>> {
    let mut lines = text.lines();
    if lines.next() != Some("completed_ns,byte") {
        return Err("Invalid MIDI trace header".into());
    }
    let mut bytes = Vec::new();
    for line in lines {
        let (t, b) = line.split_once(',').ok_or("Invalid MIDI trace row")?;
        let row = (t.parse()?, b.parse()?);
        if bytes.last().is_some_and(|last: &(u64, u8)| last.0 > row.0) {
            return Err("MIDI trace time moved backwards".into());
        }
        bytes.push(row);
    }
    Ok(bytes)
}
