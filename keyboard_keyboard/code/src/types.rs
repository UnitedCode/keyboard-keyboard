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
