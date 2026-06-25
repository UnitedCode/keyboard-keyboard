pub mod screens;
pub use screens::{draw_main, draw_settings, draw_splash};

const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

pub fn note_name(midi: u8) -> (&'static str, i8) {
    (NOTE_NAMES[(midi % 12) as usize], (midi / 12) as i8 - 1)
}
