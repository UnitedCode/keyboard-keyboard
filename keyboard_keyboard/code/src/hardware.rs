use crate::constants::{MUX_CHANNELS, NUM_ADC_PINS, NUM_MUXES};

use libdaisy::gpio::*;
use libdaisy::hal::{
    adc::{self, Adc},
    gpio::{Analog, Output, PushPull},
    prelude::*,
    stm32,
};

pub type AdcPins = (
    Daisy15<Analog>,
    Daisy16<Analog>,
    Daisy17<Analog>,
    Daisy18<Analog>,
    Daisy19<Analog>,
    Daisy20<Analog>,
    Daisy21<Analog>,
    Daisy22<Analog>,
    Daisy23<Analog>,
);

pub type MuxRaw = [[u16; MUX_CHANNELS]; NUM_MUXES];

/// Drive the three mux select lines to address channel `ch` (0–7).
#[inline(always)]
pub fn set_mux_channel(
    ch: usize,
    s0: &mut Daisy7<Output<PushPull>>,
    s1: &mut Daisy8<Output<PushPull>>,
    s2: &mut Daisy9<Output<PushPull>>,
) {
    if ch & 0b001 != 0 {
        s0.set_high()
    } else {
        s0.set_low()
    }
    if ch & 0b010 != 0 {
        s1.set_high()
    } else {
        s1.set_low()
    }
    if ch & 0b100 != 0 {
        s2.set_high()
    } else {
        s2.set_low()
    }
}

/// Select one of AM10–AM15 via the 74HC138 decoder.
/// idx: 0=AM10, 1=AM11, 2=AM12, 3=AM13, 4=AM14 (pots), 5=AM15 (pots).
#[inline(always)]
pub fn set_decoder(
    idx: u8,
    enb_a: &mut Daisy4<Output<PushPull>>,
    enb_b: &mut Daisy3<Output<PushPull>>,
    enb_c: &mut Daisy2<Output<PushPull>>,
) {
    if idx & 0b001 != 0 {
        enb_a.set_high()
    } else {
        enb_a.set_low()
    }
    if idx & 0b010 != 0 {
        enb_b.set_high()
    } else {
        enb_b.set_low()
    }
    // A2 bit: 0 = AM10–13 (switches), 1 = AM14–15 (pots)
    if idx & 0b100 != 0 {
        enb_c.set_high()
    } else {
        enb_c.set_low()
    }
}

/// Read all 9 ADC mux inputs in one shot.
#[inline(always)]
pub fn read_all_adcs(
    adc: &mut Adc<stm32::ADC1, adc::Enabled>,
    pins: &mut AdcPins,
) -> [u16; NUM_ADC_PINS] {
    let r = |res: Result<u32, _>| res.unwrap_or(0) as u16;
    [
        r(adc.read(&mut pins.0)),
        r(adc.read(&mut pins.1)),
        r(adc.read(&mut pins.2)),
        r(adc.read(&mut pins.3)),
        r(adc.read(&mut pins.4)),
        r(adc.read(&mut pins.5)),
        r(adc.read(&mut pins.6)),
        r(adc.read(&mut pins.7)),
        r(adc.read(&mut pins.8)),
    ]
}

/// Pulse SCL 9× as GPIO then send a STOP condition to release any I2C device
/// holding SDA low after a reset. Call this before handing the pins to the I2C
/// peripheral.
pub fn i2c_bus_recovery(scl: &mut Daisy11<Output<PushPull>>, sda: &mut Daisy12<Output<PushPull>>) {
    scl.set_high();
    sda.set_high();
    cortex_m::asm::delay(480 * 10);
    for _ in 0..9 {
        scl.set_low();
        cortex_m::asm::delay(480 * 5);
        scl.set_high();
        cortex_m::asm::delay(480 * 5);
    }
    // STOP condition: SDA rises while SCL is high
    scl.set_low();
    cortex_m::asm::delay(480 * 5);
    sda.set_low();
    cortex_m::asm::delay(480 * 5);
    scl.set_high();
    cortex_m::asm::delay(480 * 5);
    sda.set_high();
    cortex_m::asm::delay(480 * 10);
}
