#![no_std]

mod message;
pub use message::Message;
mod outputs;
pub use outputs::{OutputRoute, Outputs};

use usb_device::{
    class_prelude::*,
    control::{Recipient, Request, RequestType},
};

/// USB MIDI 1.0 (alternate 0) and USB MIDI 2.0/UMP (alternate 1).
/// One bidirectional group; the performance source is the keyboard, not a DIN bridge.
pub struct UsbMidi<'a, B: UsbBus> {
    audio: InterfaceNumber,
    stream: InterfaceNumber,
    name: StringIndex,
    input: EndpointIn<'a, B>,
    output: EndpointOut<'a, B>,
    alternate: u8,
    instance_id: &'a str,
    protocol: u8,
    replies: heapless::Deque<[u32; 4], 32>,
    receive: [u8; 16],
    receive_len: usize,
    receive_target: usize,
    generation: u32,
    reset_endpoints: fn(u8, u8) -> bool,
}
impl<'a, B: UsbBus> UsbMidi<'a, B> {
    pub fn new(bus: &'a UsbBusAllocator<B>, instance_id: &'a str) -> Self {
        assert!(
            !instance_id.is_empty()
                && instance_id.len() <= 42
                && instance_id
                    .bytes()
                    .all(|b| (0x21..=0x7e).contains(&b) && b != b','),
            "UMP instance ID must be 1-42 printable non-comma ASCII bytes"
        );
        Self {
            audio: bus.interface(),
            stream: bus.interface(),
            name: bus.string(),
            input: bus.bulk(64),
            output: bus.bulk(64),
            alternate: 0,
            instance_id,
            protocol: 2,
            replies: heapless::Deque::new(),
            receive: [0; 16],
            receive_len: 0,
            receive_target: 0,
            generation: 0,
            reset_endpoints: |_, _| true,
        }
    }
    /// Supply the peripheral-specific SET_INTERFACE bulk endpoint reset.
    pub fn with_endpoint_reset(mut self, reset: fn(u8, u8) -> bool) -> Self {
        self.reset_endpoints = reset;
        self
    }
    pub fn mode_label(&self) -> &'static str {
        if self.alternate == 0 {
            "M1"
        } else if self.protocol == 2 {
            "M2"
        } else {
            "UMP1"
        }
    }
    pub fn generation(&self) -> u32 {
        self.generation
    }
    pub fn write_message(&mut self, message: Message) -> usb_device::Result<()> {
        if !self.replies.is_empty() {
            return Err(usb_device::UsbError::WouldBlock);
        }
        let mut packet = heapless::Vec::<u8, 64>::new();
        if self.alternate == 0 {
            for bytes in message.midi1().chunks(3) {
                packet.push(bytes[0] >> 4).ok();
                packet.extend_from_slice(bytes).ok();
                while !packet.len().is_multiple_of(4) {
                    packet.push(0).ok();
                }
            }
        } else if self.protocol == 1 {
            for bytes in message.midi1().chunks(3) {
                let word = 0x20000000
                    | ((bytes[0] as u32) << 16)
                    | ((bytes[1] as u32) << 8)
                    | bytes.get(2).copied().unwrap_or(0) as u32;
                packet.extend_from_slice(&word.to_le_bytes()).ok();
            }
        } else {
            for word in message.ump2() {
                packet.extend_from_slice(&word.to_le_bytes()).ok();
            }
        }
        self.input.write(&packet).map(|_| ())
    }
    fn configuration(&self) -> [u32; 4] {
        [0xf0060000 | ((self.protocol as u32) << 8), 0, 0, 0]
    }
    fn text_reply(&mut self, status: u32, block: Option<u8>, text: &str) {
        let capacity = if block.is_some() { 13 } else { 14 };
        let mut remaining = text.as_bytes();
        let mut first = true;
        loop {
            let len = remaining.len().min(capacity);
            let last = len == remaining.len();
            let format = match (first, last) {
                (true, true) => 0,
                (true, false) => 1,
                (false, true) => 3,
                (false, false) => 2,
            };
            let header = 0xf0000000 | (format << 26) | (status << 16);
            let mut bytes = [0; 16];
            bytes[..4].copy_from_slice(&header.to_be_bytes());
            let offset = if let Some(id) = block {
                bytes[2] = id;
                3
            } else {
                2
            };
            bytes[offset..offset + len].copy_from_slice(&remaining[..len]);
            let mut words = [0; 4];
            for (word, chunk) in words.iter_mut().zip(bytes.chunks_exact(4)) {
                *word = u32::from_be_bytes(chunk.try_into().unwrap());
            }
            self.replies.push_back(words).ok();
            if last {
                break;
            }
            remaining = &remaining[len..];
            first = false;
        }
    }
    fn process_ump(&mut self, words: [u32; 4]) {
        if words[0] >> 26 != 0x3c {
            return;
        }
        match (words[0] >> 16) & 1023 {
            0 => {
                let filter = words[1] as u8;
                if filter & 1 != 0 {
                    self.replies.push_back([0xf0010101, 0x81000300, 0, 0]).ok();
                }
                // 0x7D is the noncommercial/experimental manufacturer ID.
                if filter & 2 != 0 {
                    self.replies
                        .push_back([0xf0020000, 0x007d0000, 0x01000100, 0x00010000])
                        .ok();
                }
                if filter & 4 != 0 {
                    self.text_reply(3, None, "Keyboard Keyboard");
                }
                if filter & 8 != 0 {
                    self.text_reply(4, None, self.instance_id);
                }
                if filter & 16 != 0 {
                    self.replies.push_back(self.configuration()).ok();
                }
            }
            5 => {
                let protocol = (words[0] >> 8) as u8;
                if protocol == 1 || protocol == 2 {
                    self.protocol = protocol;
                    self.generation = self.generation.wrapping_add(1);
                }
                // No JR timestamp capability: always report both JR flags clear.
                self.replies.push_back(self.configuration()).ok();
            }
            0x10 => {
                let block = (words[0] >> 8) as u8;
                if block != 0 && block != 255 {
                    return;
                }
                let filter = words[0] as u8;
                // Active FB0, sender UI hint, bidirectional topology, group0,
                // one group, no MIDI-CI or SysEx8 capability advertised.
                if filter & 1 != 0 {
                    self.replies.push_back([0xf0118023, 0x00010000, 0, 0]).ok();
                }
                if filter & 2 != 0 {
                    self.text_reply(0x12, Some(0), "Keyboard");
                }
            }
            _ => {}
        }
    }
    fn receive_packet(&mut self, packet: &[u8]) {
        if self.alternate == 0 {
            return;
        }
        if !packet.len().is_multiple_of(4) {
            self.receive_len = 0;
            return;
        }
        for word_bytes in packet.chunks_exact(4) {
            if self.receive_len == 0 {
                let kind = word_bytes[3] >> 4;
                self.receive_target = match kind {
                    0..=2 | 6 | 7 => 4,
                    3 | 4 | 8..=10 => 8,
                    11 | 12 => 12,
                    _ => 16,
                };
            }
            self.receive[self.receive_len..self.receive_len + 4].copy_from_slice(word_bytes);
            self.receive_len += 4;
            if self.receive_len == self.receive_target {
                let mut words = [0; 4];
                for (dst, bytes) in words.iter_mut().zip(self.receive.chunks_exact(4)) {
                    *dst = u32::from_le_bytes(bytes.try_into().unwrap());
                }
                if self.receive_target == 16 {
                    self.process_ump(words);
                }
                self.receive_len = 0;
            }
        }
    }
    fn flush_replies(&mut self) {
        if let Some(words) = self.replies.front() {
            let mut bytes = [0; 16];
            for (chunk, word) in bytes.chunks_exact_mut(4).zip(words) {
                chunk.copy_from_slice(&word.to_le_bytes());
            }
            if self.input.write(&bytes).is_ok() {
                self.replies.pop_front();
            }
        }
    }
    fn blocks(&self) -> [u8; 18] {
        [
            5,
            0x26,
            1,
            18,
            0,
            13,
            0x26,
            2,
            1,
            0,
            0,
            1,
            self.name.into(),
            0x11,
            0,
            0,
            0,
            0,
        ]
    }
}
impl<B: UsbBus> UsbClass<B> for UsbMidi<'_, B> {
    fn get_configuration_descriptors(&self, w: &mut DescriptorWriter) -> usb_device::Result<()> {
        w.interface(self.audio, 1, 1, 0)?;
        w.write(0x24, &[1, 0, 1, 9, 0, 1, self.stream.into()])?;
        w.interface_alt(self.stream, 0, 1, 3, 0, Some(self.name))?;
        // MIDIStreaming header and four embedded/external jack descriptors.
        w.write(0x24, &[1, 0, 1, 65, 0])?;
        w.write(0x24, &[2, 1, 1, self.name.into()])?;
        w.write(0x24, &[2, 2, 2, 0])?;
        w.write(0x24, &[3, 1, 3, 1, 2, 1, self.name.into()])?;
        w.write(0x24, &[3, 2, 4, 1, 1, 1, 0])?;
        w.endpoint_ex(&self.output, |b| {
            b[..2].fill(0);
            Ok(2)
        })?;
        w.write(0x25, &[1, 1, 1])?;
        w.endpoint_ex(&self.input, |b| {
            b[..2].fill(0);
            Ok(2)
        })?;
        w.write(0x25, &[1, 1, 3])?;
        w.interface_alt(self.stream, 1, 1, 3, 0, Some(self.name))?;
        w.write(0x24, &[1, 0, 2, 7, 0])?;
        w.endpoint(&self.output)?;
        w.write(0x25, &[2, 1, 1])?;
        w.endpoint(&self.input)?;
        w.write(0x25, &[2, 1, 1])?;
        Ok(())
    }
    fn get_string(&self, index: StringIndex, _: LangID) -> Option<&str> {
        (index == self.name).then_some("Keyboard")
    }
    fn control_in(&mut self, transfer: ControlIn<B>) {
        let r = transfer.request();
        if r.request_type == RequestType::Standard
            && r.recipient == Recipient::Interface
            && r.request == Request::GET_DESCRIPTOR
            && r.index == u8::from(self.stream) as u16
            && r.value == 0x2601
        {
            transfer.accept_with(&self.blocks()).ok();
        }
    }
    fn get_alt_setting(&mut self, interface: InterfaceNumber) -> Option<u8> {
        if interface == self.stream {
            Some(self.alternate)
        } else if interface == self.audio {
            Some(0)
        } else {
            None
        }
    }
    fn set_alt_setting(&mut self, interface: InterfaceNumber, alternate: u8) -> bool {
        if interface == self.audio {
            return alternate == 0;
        }
        if interface != self.stream || alternate > 1 {
            return false;
        }
        // Drop old-format OUT data before resetting the pair for the new alt.
        let mut discard = [0; 64];
        self.output.read(&mut discard).ok();
        if !(self.reset_endpoints)(self.input.address().into(), self.output.address().into()) {
            return false;
        }
        self.alternate = alternate;
        self.protocol = 2;
        self.replies.clear();
        self.receive_len = 0;
        self.generation = self.generation.wrapping_add(1);
        true
    }
    fn poll(&mut self) {
        self.flush_replies();
    }
    fn endpoint_out(&mut self, address: EndpointAddress) {
        if address != self.output.address() || !self.replies.is_empty() {
            return;
        }
        let mut bytes = [0; 64];
        if let Ok(len) = self.output.read(&mut bytes) {
            self.receive_packet(&bytes[..len]);
        }
        self.flush_replies();
    }
    fn endpoint_in_complete(&mut self, address: EndpointAddress) {
        if address == self.input.address() {
            self.flush_replies();
        }
    }
    fn reset(&mut self) {
        self.alternate = 0;
        self.protocol = 2;
        self.replies.clear();
        self.receive_len = 0;
        self.generation = self.generation.wrapping_add(1);
    }
}
