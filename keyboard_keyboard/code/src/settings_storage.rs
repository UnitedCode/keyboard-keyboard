//! Persistent settings stored in the final two sectors of the Daisy Seed QSPI flash.
//!
//! Two alternating records keep the previous settings intact if power is lost while
//! erasing or programming the next record.

use crate::settings::Settings;
use libdaisy::flash::{Flash, FlashErase};

const SLOT_ADDRESSES: [u32; 2] = [0x7F_E000, 0x7F_F000];
const RECORD_SIZE: usize = 32;
const MAGIC: [u8; 4] = *b"KKEY";
const VERSION: u8 = 1;

#[derive(Clone, Copy)]
struct Record {
    generation: u32,
    settings: Settings,
}

/// Load the newest valid settings record, or return `None` for a fresh/corrupt flash.
pub fn load(flash: &mut Flash) -> Option<Settings> {
    newest_record(flash).map(|(_, record)| record.settings)
}

/// Atomically save settings to the slot older than the current valid record.
pub fn save(flash: &mut Flash, settings: Settings) -> bool {
    let (slot, generation) = match newest_record(flash) {
        Some((current_slot, record)) => (1 - current_slot, record.generation.wrapping_add(1)),
        None => (0, 0),
    };
    let bytes = encode(Record {
        generation,
        settings,
    });

    if stm32h7xx_hal::nb::block!(flash.erase(FlashErase::Sector4K(SLOT_ADDRESSES[slot]))).is_err() {
        return false;
    }
    stm32h7xx_hal::nb::block!(flash.program(SLOT_ADDRESSES[slot], &bytes)).is_ok()
}

fn newest_record(flash: &mut Flash) -> Option<(usize, Record)> {
    let a = read_slot(flash, 0);
    let b = read_slot(flash, 1);
    match (a, b) {
        (Some(a), Some(b)) => {
            // Wrapping comparison is safe because the two generations can differ by only one.
            if b.generation.wrapping_sub(a.generation) < 0x8000_0000 {
                Some((1, b))
            } else {
                Some((0, a))
            }
        }
        (Some(a), None) => Some((0, a)),
        (None, Some(b)) => Some((1, b)),
        (None, None) => None,
    }
}

fn read_slot(flash: &mut Flash, slot: usize) -> Option<Record> {
    let mut bytes = [0u8; RECORD_SIZE];
    flash.read(SLOT_ADDRESSES[slot], &mut bytes).ok()?;
    decode(&bytes)
}

fn encode(record: Record) -> [u8; RECORD_SIZE] {
    let mut bytes = [0xFFu8; RECORD_SIZE];
    bytes[0..4].copy_from_slice(&MAGIC);
    bytes[4] = VERSION;
    bytes[5] = 7;
    bytes[8..12].copy_from_slice(&record.generation.to_le_bytes());
    bytes[12] = record.settings.melody_channel;
    bytes[13] = record.settings.drum_channel;
    bytes[14] = record.settings.octave as u8;
    bytes[15] = record.settings.pitch_bend_range;
    bytes[16] = record.settings.melody_program;
    bytes[17] = record.settings.drum_program;
    bytes[18] = record.settings.vibrato_enabled as u8;
    let checksum = crc32(&bytes[..28]);
    bytes[28..32].copy_from_slice(&checksum.to_le_bytes());
    bytes
}

fn decode(bytes: &[u8; RECORD_SIZE]) -> Option<Record> {
    if bytes[0..4] != MAGIC || bytes[4] != VERSION || bytes[5] != 7 {
        return None;
    }
    let expected = u32::from_le_bytes(bytes[28..32].try_into().ok()?);
    if crc32(&bytes[..28]) != expected {
        return None;
    }

    let settings = Settings {
        melody_channel: bytes[12],
        drum_channel: bytes[13],
        octave: bytes[14] as i8,
        pitch_bend_range: bytes[15],
        melody_program: bytes[16],
        drum_program: bytes[17],
        vibrato_enabled: bytes[18] != 0,
    };
    if settings.melody_channel > 15
        || settings.drum_channel > 15
        || !(2..=5).contains(&settings.octave)
        || !(1..=12).contains(&settings.pitch_bend_range)
        || bytes[18] > 1
    {
        return None;
    }

    Some(Record {
        generation: u32::from_le_bytes(bytes[8..12].try_into().ok()?),
        settings,
    })
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320u32 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}
