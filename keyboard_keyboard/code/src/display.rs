use crate::types::LcdDisplay;
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};

/// Draw the boot splash screen ("key*key" centred on 128×32).
pub fn draw_startup(disp: &mut LcdDisplay) {
    disp.clear();
    let style = MonoTextStyleBuilder::new()
        .font(&FONT_10X20)
        .text_color(BinaryColor::On)
        .build();
    Text::with_baseline("key*key", Point::new(29, 6), style, Baseline::Top)
        .draw(disp)
        .ok();
    disp.flush().ok();
}
