//! Experiment only. A power fault latches until MCU reset; never infer rest from a dead rail.
#[derive(Clone, Copy)]
pub struct PowerGuard {
    locked: bool,
    reported: bool,
}
impl PowerGuard {
    pub const fn new(boot_valid: bool) -> Self {
        Self {
            locked: !boot_valid,
            reported: false,
        }
    }
    pub fn observe(&mut self, valid: bool) -> bool {
        self.locked |= !valid;
        let first = self.locked && !self.reported;
        self.reported |= self.locked;
        first
    }
    pub fn locked(&self) -> bool {
        self.locked
    }
}

#[derive(Default)]
pub struct Notes {
    bits: [[u8; 16]; 16],
}
impl Notes {
    pub fn set(&mut self, ch: u8, note: u8, down: bool) {
        let b = &mut self.bits[(ch & 15) as usize][(note & 127) as usize / 8];
        let mask = 1 << (note & 7);
        if down {
            *b |= mask;
        } else {
            *b &= !mask;
        }
    }
    pub fn clear_channel(&mut self, ch: u8) {
        self.bits[(ch & 15) as usize] = [0; 16];
    }
    /// Explicit releases, then sustain off, All Sound Off, All Notes Off and neutral expression.
    /// The caller sends outside RTIC queue locks. No receiver cooperation/ack is assumed.
    pub fn panic(&mut self, mut send: impl FnMut([u8; 3])) {
        for ch in 0..16u8 {
            for note in 0..128u8 {
                if self.bits[ch as usize][note as usize / 8] & (1 << (note & 7)) != 0 {
                    send([0x80 | ch, note, 0]);
                }
            }
            self.clear_channel(ch);
        }
        for ch in 0..16u8 {
            for cc in [64, 120, 123, 1] {
                send([0xb0 | ch, cc, 0]);
            }
            send([0xe0 | ch, 0, 64]);
        }
    }
}
