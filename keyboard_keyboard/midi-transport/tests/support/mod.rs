use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use usb_device::{
    bus::{PollResult, UsbBus},
    class::UsbClass,
    device::UsbDevice,
    endpoint::{EndpointAddress, EndpointType},
    Result, UsbDirection, UsbError,
};

#[derive(Default)]
struct State {
    out: VecDeque<(u8, Vec<u8>, bool)>,
    incoming: VecDeque<(u8, Vec<u8>)>,
    complete: u16,
    reset: bool,
    stalled: u32,
    allocated: u32,
    blocked: bool,
}
#[derive(Clone, Default)]
pub struct Host(Arc<Mutex<State>>);
pub struct Bus(Host);
impl Host {
    pub fn bus(&self) -> Bus {
        Bus(self.clone())
    }
    pub fn reset<C: UsbClass<Bus>>(&self, dev: &mut UsbDevice<Bus>, class: &mut C) {
        self.0.lock().unwrap().reset = true;
        dev.poll(&mut [class]);
    }
    pub fn setup<C: UsbClass<Bus>>(
        &self,
        dev: &mut UsbDevice<Bus>,
        class: &mut C,
        request: [u8; 8],
    ) {
        {
            let mut s = self.0.lock().unwrap();
            s.stalled = 0;
            s.out.push_back((0, request.to_vec(), true));
        }
        dev.poll(&mut [class]);
    }
    pub fn control_in<C: UsbClass<Bus>>(
        &self,
        dev: &mut UsbDevice<Bus>,
        class: &mut C,
        request: [u8; 8],
    ) -> Vec<u8> {
        self.setup(dev, class, request);
        let mut result = Vec::new();
        for _ in 0..32 {
            if let Some(data) = self.take(0x80) {
                let n = data.len();
                result.extend(data);
                dev.poll(&mut [class]);
                if n < 64 || result.len() >= u16::from_le_bytes([request[6], request[7]]) as usize {
                    break;
                }
            } else {
                break;
            }
        }
        assert!(!self.stalled(0x80), "control request stalled");
        self.send(0, &[]);
        dev.poll(&mut [class]);
        result
    }
    pub fn control_out<C: UsbClass<Bus>>(
        &self,
        dev: &mut UsbDevice<Bus>,
        class: &mut C,
        request: [u8; 8],
    ) {
        self.setup(dev, class, request);
        assert_eq!(self.take(0x80), Some(vec![]), "status stage");
        dev.poll(&mut [class]);
    }
    pub fn send(&self, ep: u8, data: &[u8]) {
        self.0
            .lock()
            .unwrap()
            .out
            .push_back((ep, data.to_vec(), false));
    }
    pub fn take(&self, ep: u8) -> Option<Vec<u8>> {
        let mut s = self.0.lock().unwrap();
        let i = s.incoming.iter().position(|p| p.0 == ep)?;
        let data = s.incoming.remove(i).unwrap().1;
        s.complete |= 1 << (ep & 15);
        Some(data)
    }
    pub fn blocked(&self, blocked: bool) {
        self.0.lock().unwrap().blocked = blocked;
    }
    pub fn stalled(&self, ep: u8) -> bool {
        self.0.lock().unwrap().stalled & (1 << slot(ep)) != 0
    }
}
fn slot(ep: u8) -> u8 {
    (ep & 15) + if ep & 128 != 0 { 16 } else { 0 }
}
impl UsbBus for Bus {
    fn alloc_ep(
        &mut self,
        dir: UsbDirection,
        address: Option<EndpointAddress>,
        _: EndpointType,
        _: u16,
        _: u8,
    ) -> Result<EndpointAddress> {
        let mut s = self.0 .0.lock().unwrap();
        let bit = if dir == UsbDirection::In { 128 } else { 0 };
        let ep = address
            .map(u8::from)
            .or_else(|| {
                (1..16)
                    .map(|i| i | bit)
                    .find(|&ep| s.allocated & (1 << slot(ep)) == 0)
            })
            .ok_or(UsbError::EndpointOverflow)?;
        s.allocated |= 1 << slot(ep);
        Ok(EndpointAddress::from(ep))
    }
    fn enable(&mut self) {}
    fn reset(&self) {
        let mut s = self.0 .0.lock().unwrap();
        s.out.clear();
        s.incoming.clear();
        s.complete = 0;
        s.stalled = 0;
    }
    fn set_device_address(&self, _: u8) {}
    fn write(&self, ep: EndpointAddress, bytes: &[u8]) -> Result<usize> {
        let mut s = self.0 .0.lock().unwrap();
        let ep = u8::from(ep);
        if (s.blocked && ep != 0x80) || s.incoming.iter().any(|p| p.0 == ep) {
            return Err(UsbError::WouldBlock);
        }
        s.incoming.push_back((ep, bytes.to_vec()));
        Ok(bytes.len())
    }
    fn read(&self, ep: EndpointAddress, bytes: &mut [u8]) -> Result<usize> {
        let mut s = self.0 .0.lock().unwrap();
        let ep = u8::from(ep);
        let i = s
            .out
            .iter()
            .position(|p| p.0 == ep)
            .ok_or(UsbError::WouldBlock)?;
        let (_, data, _) = s.out.remove(i).unwrap();
        if data.len() > bytes.len() {
            return Err(UsbError::BufferOverflow);
        }
        bytes[..data.len()].copy_from_slice(&data);
        Ok(data.len())
    }
    fn set_stalled(&self, ep: EndpointAddress, stalled: bool) {
        let mut s = self.0 .0.lock().unwrap();
        let bit = 1 << slot(ep.into());
        if stalled {
            s.stalled |= bit;
        } else {
            s.stalled &= !bit;
        }
    }
    fn is_stalled(&self, ep: EndpointAddress) -> bool {
        self.0.stalled(ep.into())
    }
    fn suspend(&self) {}
    fn resume(&self) {}
    fn poll(&self) -> PollResult {
        let mut s = self.0 .0.lock().unwrap();
        if core::mem::take(&mut s.reset) {
            return PollResult::Reset;
        }
        let mut out = 0;
        let mut setup = 0;
        for &(ep, _, is_setup) in &s.out {
            if is_setup {
                setup |= 1 << ep;
            } else {
                out |= 1 << ep;
            }
        }
        let complete = core::mem::take(&mut s.complete);
        if out | setup | complete == 0 {
            PollResult::None
        } else {
            PollResult::Data {
                ep_out: out,
                ep_setup: setup,
                ep_in_complete: complete,
            }
        }
    }
}
pub fn descriptors(mut data: &[u8]) -> impl Iterator<Item = &[u8]> {
    std::iter::from_fn(move || {
        if data.is_empty() {
            return None;
        }
        let len = data[0] as usize;
        assert!(len >= 2 && len <= data.len());
        let (d, rest) = data.split_at(len);
        data = rest;
        Some(d)
    })
}
