use libdaisy::hal::{i2c::I2c, stm32};
use ssd1306::{
    mode::BufferedGraphicsMode,
    prelude::{DisplaySize128x32, I2CInterface},
    Ssd1306,
};

pub type LcdDisplay = Ssd1306<
    I2CInterface<I2c<stm32::I2C1>>,
    DisplaySize128x32,
    BufferedGraphicsMode<DisplaySize128x32>,
>;

#[derive(Clone, Copy)]
pub enum LastEvent {
    Note { note: u8 },
    Cc { num: u8, value: u8 },
    Clear,
}

#[derive(Clone, Copy)]
pub struct DisplayState {
    pub last_event: Option<LastEvent>,
    pub melody_channel: u8,
    pub drum_channel: u8,
}

impl DisplayState {
    pub const fn new() -> Self {
        Self {
            last_event: None,
            melody_channel: 0,
            drum_channel: 9,
        }
    }
}
