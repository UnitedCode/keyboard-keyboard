use crate::{Message, UsbMidi};
use usb_device::bus::UsbBus;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputRoute {
    Din,
    Usb,
    Both,
}
impl OutputRoute {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Din => "DIN",
            Self::Usb => "USB",
            Self::Both => "BOTH",
        }
    }
    pub const fn din(self) -> bool {
        matches!(self, Self::Din | Self::Both)
    }
    pub const fn usb(self) -> bool {
        matches!(self, Self::Usb | Self::Both)
    }
}

/// Independent, bounded transport queues. Neither destination waits for the other.
/// An overloaded destination discards its backlog and schedules All Notes Off.
pub struct Outputs {
    route: OutputRoute,
    din: heapless::Deque<Message, 256>,
    usb: heapless::Deque<Message, 256>,
    din_packet: heapless::Vec<u8, 18>,
    din_offset: usize,
    usb_ready: bool,
    usb_generation: u32,
    pub overflows: u32,
}
impl Default for Outputs {
    fn default() -> Self {
        Self::new()
    }
}
impl Outputs {
    pub const fn new() -> Self {
        Self {
            route: OutputRoute::Both,
            din: heapless::Deque::new(),
            usb: heapless::Deque::new(),
            din_packet: heapless::Vec::new(),
            din_offset: 0,
            usb_ready: false,
            usb_generation: 0,
            overflows: 0,
        }
    }
    pub fn route(&self) -> OutputRoute {
        self.route
    }
    pub fn set_route(&mut self, route: OutputRoute) {
        if self.route == route {
            return;
        }
        // Finish any partial DIN message, then silence old and new destinations.
        if self.route.din() || route.din() {
            Self::panic_queue(&mut self.din);
        }
        if self.usb_ready && (self.route.usb() || route.usb()) {
            Self::panic_queue(&mut self.usb);
        }
        self.route = route;
    }
    pub fn send(&mut self, message: Message) {
        if self.route.din() {
            Self::enqueue(&mut self.din, message, &mut self.overflows);
        }
        if self.route.usb() && self.usb_ready {
            Self::enqueue(&mut self.usb, message, &mut self.overflows);
        }
    }
    fn enqueue(queue: &mut heapless::Deque<Message, 256>, message: Message, overflows: &mut u32) {
        if queue.push_back(message).is_err() {
            *overflows = overflows.saturating_add(1);
            Self::panic_queue(queue);
            queue.push_back(message).ok();
        }
    }
    fn panic_queue(queue: &mut heapless::Deque<Message, 256>) {
        queue.clear();
        for channel in 0..16 {
            queue
                .push_back(Message::ControlChange {
                    channel,
                    controller: 123,
                    value: 0,
                })
                .ok();
        }
    }
    pub fn peek_din_byte(&mut self) -> Option<u8> {
        if self.din_offset == self.din_packet.len() {
            self.din_packet = self.din.pop_front()?.midi1();
            self.din_offset = 0;
        }
        self.din_packet.get(self.din_offset).copied()
    }
    pub fn advance_din(&mut self) {
        self.din_offset += 1;
    }
    pub fn service_usb<B: UsbBus>(&mut self, midi: &mut UsbMidi<B>, configured: bool) {
        let generation = midi.generation();
        if self.usb_ready != configured || generation != self.usb_generation {
            self.usb.clear();
            if configured {
                Self::panic_queue(&mut self.usb);
            }
            self.usb_ready = configured;
            self.usb_generation = generation;
        }
        if !configured {
            return;
        }
        if let Some(&message) = self.usb.front() {
            if midi.write_message(message).is_ok() {
                self.usb.pop_front();
            }
        }
    }
}
