//! STM32H750 USB2 adapter for alternate-setting endpoint reset.
use stm32h7xx_hal::stm32;

/// usb-device 0.3 does not reset bulk toggles on SET_INTERFACE. Reset only the
/// MIDI EP1 pair, keeping EP0/address intact. Called solely from the USB task,
/// between driver polls, with no DMA and no concurrent USB register access.
pub fn reset_midi_endpoints(input: u8, output: u8) -> bool {
    if input != 0x81 || output != 1 {
        return false;
    }
    // SAFETY: USB2 is exclusively owned by this task's UsbDevice. PAC access
    // here supplements its driver's missing alternate-setting operation.
    let (device, global) = unsafe {
        (
            &*stm32::OTG2_HS_DEVICE::ptr(),
            &*stm32::OTG2_HS_GLOBAL::ptr(),
        )
    };
    if device.diepctl1.read().epena().bit_is_set() {
        device
            .diepctl1
            .modify(|_, w| w.snak().set_bit().epdis().set_bit());
        let mut disabled = false;
        for _ in 0..10_000 {
            if device.diepctl1.read().epena().bit_is_clear() {
                disabled = true;
                break;
            }
        }
        if !disabled {
            return false;
        }
    }
    global
        .grstctl
        .write(|w| unsafe { w.txfnum().bits(1).txfflsh().set_bit() });
    let mut flushed = false;
    for _ in 0..10_000 {
        if global.grstctl.read().txfflsh().bit_is_clear() {
            flushed = true;
            break;
        }
    }
    if !flushed {
        return false;
    }
    device.diepctl1.modify(|_, w| w.sd0pid_sevnfrm().set_bit());
    device.doepctl1.modify(|_, w| w.sd0pid_sevnfrm().set_bit());
    true
}
