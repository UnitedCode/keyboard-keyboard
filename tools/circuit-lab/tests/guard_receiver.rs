//! Stateful MIDI receiver checks with sustain and repeated NoteOn of the same pitch.
#[allow(dead_code)]
#[path = "../../../circuits/experiments/power-fault-guard/firmware/src/guard.rs"]
mod guard;
use std::collections::BTreeMap;
#[derive(Default)]
struct Receiver {
    down: BTreeMap<(u8, u8), u16>,
    sound: BTreeMap<(u8, u8), u16>,
    sustain: [bool; 16],
}
impl Receiver {
    fn message(&mut self, m: [u8; 3]) {
        let ch = m[0] & 15;
        let key = (ch, m[1]);
        match m[0] & 0xf0 {
            0x90 if m[2] > 0 => {
                *self.down.entry(key).or_default() += 1;
                *self.sound.entry(key).or_default() += 1;
            }
            0x80 | 0x90 => {
                let d = self.down.entry(key).or_default();
                *d = d.saturating_sub(1);
                if !self.sustain[ch as usize] {
                    let s = self.sound.entry(key).or_default();
                    *s = s.saturating_sub(1);
                }
            }
            0xb0 if m[1] == 64 => {
                self.sustain[ch as usize] = m[2] >= 64;
                if m[2] < 64 {
                    for (k, s) in &mut self.sound {
                        if k.0 == ch {
                            *s = *self.down.get(k).unwrap_or(&0);
                        }
                    }
                }
            }
            0xb0 if m[1] == 120 => {
                for (k, s) in &mut self.sound {
                    if k.0 == ch {
                        *s = 0;
                    }
                }
            }
            0xb0 if m[1] == 123 => {
                for (k, d) in &mut self.down {
                    if k.0 == ch {
                        *d = 0;
                    }
                }
                if !self.sustain[ch as usize] {
                    for (k, s) in &mut self.sound {
                        if k.0 == ch {
                            *s = 0;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    fn sounding(&self) -> u32 {
        self.sound.values().map(|n| *n as u32).sum()
    }
}
#[test]
fn all_notes_off_alone_is_a_failing_sustain_control() {
    let mut r = Receiver::default();
    r.message([0xb0, 64, 127]);
    r.message([0x90, 60, 100]);
    r.message([0xb0, 123, 0]);
    assert_eq!(r.sounding(), 1);
}
#[test]
fn actual_panic_releases_duplicate_notes_with_sustain_on_all_channels() {
    let mut r = Receiver::default();
    let mut notes = guard::Notes::default();
    for ch in 0..16 {
        r.message([0xb0 | ch, 64, 127]);
    }
    for n in 0..90u8 {
        let ch = n % 16;
        for _ in 0..2 {
            r.message([0x90 | ch, n, 100]);
            notes.set(ch, n, true);
        }
    }
    assert_eq!(r.sounding(), 180);
    let mut explicit = 0;
    notes.panic(|m| {
        if m[0] & 0xf0 == 0x80 {
            explicit += 1;
        }
        r.message(m);
    });
    assert_eq!(explicit, 90);
    assert_eq!(r.sounding(), 0);
    assert!(r.sustain.iter().all(|s| !*s));
}
