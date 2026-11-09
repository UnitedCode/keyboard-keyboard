//! Render the actual firmware drawing routines without a physical OLED.
#![allow(dead_code)]
use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};
use std::{convert::Infallible, fs, path::Path};
#[path = "../../code/src/types.rs"]
mod firmware_types;
mod types {
    pub use super::firmware_types::*;
    pub type LcdDisplay = super::Frame;
}
#[allow(unused_imports)]
#[path = "../../code/src/display/mod.rs"]
mod display;
#[path = "../../code/src/settings.rs"]
mod settings;

struct Frame([bool; 128 * 32]);
impl Frame {
    fn clear(&mut self) {
        self.0.fill(false);
    }
    fn flush(&mut self) -> Result<(), Infallible> {
        Ok(())
    }
    fn save(&self, path: &Path) -> std::io::Result<()> {
        const SCALE: usize = 4;
        const WIDTH: usize = 128 * SCALE;
        const HEIGHT: usize = 32 * SCALE;
        let size = 54 + WIDTH * HEIGHT * 3;
        let mut bytes = vec![0u8; size];
        bytes[..2].copy_from_slice(b"BM");
        bytes[2..6].copy_from_slice(&(size as u32).to_le_bytes());
        bytes[10..14].copy_from_slice(&54u32.to_le_bytes());
        bytes[14..18].copy_from_slice(&40u32.to_le_bytes());
        bytes[18..22].copy_from_slice(&(WIDTH as i32).to_le_bytes());
        bytes[22..26].copy_from_slice(&(HEIGHT as i32).to_le_bytes());
        bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
        bytes[28..30].copy_from_slice(&24u16.to_le_bytes());
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let pixel = if self.0[y / SCALE * 128 + x / SCALE] {
                    [210, 255, 235]
                } else {
                    [15, 15, 10]
                };
                let offset = 54 + ((HEIGHT - 1 - y) * WIDTH + x) * 3;
                bytes[offset..offset + 3].copy_from_slice(&pixel);
            }
        }
        fs::write(path, bytes)
    }
}
impl OriginDimensions for Frame {
    fn size(&self) -> Size {
        Size::new(128, 32)
    }
}
impl DrawTarget for Frame {
    type Color = BinaryColor;
    type Error = Infallible;
    fn draw_iter<I: IntoIterator<Item = Pixel<BinaryColor>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Infallible> {
        for Pixel(Point { x, y }, color) in pixels {
            if (0..128).contains(&x) && (0..32).contains(&y) {
                self.0[y as usize * 128 + x as usize] = color == BinaryColor::On;
            }
        }
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .ok_or("Usage: screen_preview OUTPUT_DIRECTORY")?;
    let output = Path::new(&output);
    fs::create_dir_all(output)?;
    let mut frame = Frame([false; 128 * 32]);
    let mut state = types::DisplayState::new();
    state.usb_status = "M2";
    state.last_event = Some(types::LastEvent::Note { note: 60 });
    display::draw_main(&mut frame, &state);
    frame.save(&output.join("both-midi2.bmp"))?;
    let mut menu = settings::Settings::default();
    display::draw_settings(&mut frame, settings::OUTPUT_SETTING, &menu);
    frame.save(&output.join("output-both.bmp"))?;
    menu.adjust(settings::OUTPUT_SETTING, -1);
    display::draw_settings(&mut frame, settings::OUTPUT_SETTING, &menu);
    frame.save(&output.join("output-usb.bmp"))?;
    state.output_route = keyboard_midi_transport::OutputRoute::Din;
    state.usb_status = "WAIT";
    display::draw_main(&mut frame, &state);
    frame.save(&output.join("din-waiting.bmp"))?;
    Ok(())
}
