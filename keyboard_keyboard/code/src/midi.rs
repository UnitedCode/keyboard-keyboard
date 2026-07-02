use stm32h7xx_hal::{nb, prelude::*, serial::Tx, stm32};

pub struct MidiSender {
    tx: Tx<stm32::USART1>,
    channel: u8,
}

impl MidiSender {
    pub fn new(tx: Tx<stm32::USART1>, channel: u8) -> Self {
        Self {
            tx,
            channel: channel & 0x0F,
        }
    }

    pub fn note_on(&mut self, note: u8, velocity: u8) {
        let status = 0x90 | self.channel;
        self.send_byte(status);
        self.send_byte(note & 0x7F);
        self.send_byte(velocity & 0x7F);
    }

    pub fn note_off(&mut self, note: u8, velocity: u8) {
        let status = 0x80 | self.channel;
        self.send_byte(status);
        self.send_byte(note & 0x7F);
        self.send_byte(velocity & 0x7F);
    }

    pub fn control_change(&mut self, controller: u8, value: u8) {
        let status = 0xB0 | self.channel;
        self.send_byte(status);
        self.send_byte(controller & 0x7F);
        self.send_byte(value & 0x7F);
    }

    /// `value` is 14-bit: 0x2000 = center, 0x0000 = full down, 0x3FFF = full up.
    pub fn pitch_bend(&mut self, value: u16) {
        let status = 0xE0 | self.channel;
        let lsb = (value & 0x7F) as u8;
        let msb = ((value >> 7) & 0x7F) as u8;
        self.send_byte(status);
        self.send_byte(lsb);
        self.send_byte(msb);
    }

    pub fn all_notes_off(&mut self) {
        self.control_change(123, 0);
    }

    pub fn program_change(&mut self, program: u8) {
        let status = 0xC0 | self.channel;
        self.send_byte(status);
        self.send_byte(program & 0x7F);
    }

    /// Sets pitch bend range on the receiving synth via Registered Parameter Number 0.
    pub fn set_pitch_bend_range(&mut self, semitones: u8) {
        // Select RPN 0 (pitch bend range): MSB then LSB both = 0.
        self.control_change(101, 0);
        self.control_change(100, 0);
        // Write the value: CC 6 = semitones (MSB), CC 38 = 0 cents (LSB).
        self.control_change(6, semitones);
        self.control_change(38, 0);
        // Deselect — set RPN to null (127, 127).
        self.control_change(101, 127);
        self.control_change(100, 127);
    }

    pub fn set_channel(&mut self, channel: u8) {
        self.channel = channel & 0x0F;
    }

    fn send_byte(&mut self, byte: u8) {
        nb::block!(self.tx.write(byte)).ok();
    }
}
