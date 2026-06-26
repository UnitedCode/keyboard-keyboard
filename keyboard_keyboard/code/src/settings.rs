pub const NUM_SETTINGS_ITEMS: usize = 6;

pub struct SettingsItem {
    pub name: &'static str,
    pub min: i16,
    pub max: i16,
}

pub const SETTINGS_ITEMS: [SettingsItem; NUM_SETTINGS_ITEMS] = [
    SettingsItem {
        name: "MELODY CHANNEL",
        min: 1,
        max: 16,
    },
    SettingsItem {
        name: "DRUM CHANNEL",
        min: 1,
        max: 16,
    },
    SettingsItem {
        name: "OCTAVE",
        min: 2,
        max: 5,
    },
    SettingsItem {
        name: "BEND RANGE",
        min: 1,
        max: 12,
    },
    SettingsItem {
        name: "MELODY PROGRAM",
        min: 0,
        max: 127,
    },
    SettingsItem {
        name: "DRUM PROGRAM",
        min: 0,
        max: 127,
    },
];

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub melody_channel: u8,   // 0-indexed (0–15), displayed as 1–16
    pub drum_channel: u8,     // 0-indexed (0–15), displayed as 1–16
    pub octave: i8,           // 0–8; offset = (octave - 4) * 12
    pub pitch_bend_range: u8, // semitones 1–12
    pub melody_program: u8,   // 0–127, sent as PC on melody channel when settings closes
    pub drum_program: u8,     // 0–127, sent as PC on drum channel when settings closes
}

impl Settings {
    pub const fn default() -> Self {
        Self {
            melody_channel: 0,
            drum_channel: 9,
            octave: 2,
            pitch_bend_range: 2,
            melody_program: 0,
            drum_program: 0,
        }
    }

    /// Returns the display value for menu item `idx` (channels are 1-indexed).
    pub fn get(&self, idx: usize) -> i16 {
        match idx {
            0 => self.melody_channel as i16 + 1,
            1 => self.drum_channel as i16 + 1,
            2 => self.octave as i16,
            3 => self.pitch_bend_range as i16,
            4 => self.melody_program as i16,
            5 => self.drum_program as i16,
            _ => 0,
        }
    }

    /// Sets menu item `idx` from a display value, clamping to the item's range.
    pub fn set(&mut self, idx: usize, value: i16) {
        let item = &SETTINGS_ITEMS[idx];
        let v = value.clamp(item.min, item.max);
        match idx {
            0 => self.melody_channel = (v - 1) as u8,
            1 => self.drum_channel = (v - 1) as u8,
            2 => self.octave = v as i8,
            3 => self.pitch_bend_range = v as u8,
            4 => self.melody_program = v as u8,
            5 => self.drum_program = v as u8,
            _ => {}
        }
    }

    pub fn adjust(&mut self, idx: usize, delta: i16) {
        let current = self.get(idx);
        self.set(idx, current + delta);
    }
}
