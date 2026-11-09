use keyboard_midi_transport::{Message, OutputRoute, Outputs, UsbMidi};
use stm32h7xx_hal::{prelude::*, serial::Tx, stm32};
use usb_device::bus::UsbBus;

pub struct MidiSender {
    tx: Tx<stm32::USART1>,
    channel: u8,
    outputs: Outputs,
}
impl MidiSender {
    pub fn new(tx: Tx<stm32::USART1>, channel: u8) -> Self {
        Self {
            tx,
            channel: channel & 15,
            outputs: Outputs::new(),
        }
    }
    fn send(&mut self, message: Message) {
        self.outputs.send(message);
        self.service_din();
    }
    /// Called by TXE IRQ: never waits for the 31.25 kbaud wire.
    pub fn service_din(&mut self) {
        for _ in 0..16 {
            let Some(byte) = self.outputs.peek_din_byte() else {
                self.tx.unlisten();
                return;
            };
            if self.tx.write(byte).is_err() {
                self.tx.listen();
                return;
            }
            self.outputs.advance_din();
        }
        self.tx.listen();
    }
    pub fn service_usb<B: UsbBus>(&mut self, midi: &mut UsbMidi<B>, configured: bool) {
        self.outputs.service_usb(midi, configured);
    }
    pub fn set_route(&mut self, route: OutputRoute) {
        self.outputs.set_route(route);
        self.service_din();
    }
    pub fn overflows(&self) -> u32 {
        self.outputs.overflows
    }
    pub fn note_on(&mut self, note: u8, velocity: u8) {
        self.send(Message::NoteOn {
            channel: self.channel,
            note,
            velocity,
        });
    }
    pub fn note_off(&mut self, note: u8, velocity: u8) {
        self.send(Message::NoteOff {
            channel: self.channel,
            note,
            velocity,
        });
    }
    pub fn control_change(&mut self, controller: u8, value: u8) {
        self.send(Message::ControlChange {
            channel: self.channel,
            controller,
            value,
        });
    }
    pub fn pitch_bend(&mut self, value: u16) {
        self.send(Message::PitchBend {
            channel: self.channel,
            value,
        });
    }
    pub fn all_notes_off(&mut self) {
        self.control_change(123, 0);
    }
    pub fn program_change(&mut self, program: u8) {
        self.send(Message::ProgramChange {
            channel: self.channel,
            program,
        });
    }
    pub fn set_pitch_bend_range(&mut self, semitones: u8) {
        self.send(Message::PitchBendRange {
            channel: self.channel,
            semitones,
        });
    }
    pub fn set_channel(&mut self, channel: u8) {
        self.channel = channel & 15;
    }
}
