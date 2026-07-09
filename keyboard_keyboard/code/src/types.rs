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
    pub recalibrating: bool,
    pub current_voice: Option<u8>,
    pub volume_level: Option<u8>, // 0–10, mapped from CC7 (0–127)
    // (preset_mode, column) for the last preset button pressed — column is 0/1/2
    // for key A/B/C. Only drawn when preset_mode still matches the live setting.
    pub current_preset: Option<(u8, u8)>,
}

impl DisplayState {
    pub const fn new() -> Self {
        Self {
            last_event: None,
            melody_channel: 1,
            drum_channel: 9,
            recalibrating: false,
            current_voice: None,
            volume_level: None,
            current_preset: None,
        }
    }
}
