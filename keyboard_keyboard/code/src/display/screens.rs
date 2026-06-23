use crate::display::note_name;
use crate::types::{DisplayState, LastEvent, LcdDisplay};
use core::fmt::Write;
use embedded_graphics::{
    image::{Image, ImageRawBE},
    mono_font::{
        ascii::{FONT_10X20, FONT_6X9},
        MonoTextStyle,
    },
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::Rectangle,
    text::{Alignment, Text},
};
use heapless::String;

const SPRITE_ATLAS: &[u8] =
    include_bytes!("../../../../images/Keyboard-Keyboard-Spritesheet.raw");

pub fn draw_splash(disp: &mut LcdDisplay) {
    disp.clear();
    let atlas = ImageRawBE::<BinaryColor>::new(SPRITE_ATLAS, 128);
    let splash = atlas.sub_image(&Rectangle::new(Point::new(0, 0), Size::new(128, 32)));
    Image::new(&splash, Point::new(0, 0)).draw(disp).ok();
    disp.flush().ok();
}

pub fn draw_main(disp: &mut LcdDisplay, state: &DisplayState) {
    disp.clear();

    let atlas = ImageRawBE::<BinaryColor>::new(SPRITE_ATLAS, 128);

    // Background from atlas rows 32–63
    let bg = atlas.sub_image(&Rectangle::new(Point::new(0, 32), Size::new(128, 32)));
    Image::new(&bg, Point::new(0, 0)).draw(disp).ok();

    // All text is Off (dark on the light background)
    let center = Point::new(64, 16);
    let off = BinaryColor::Off;

    // Small label above center: note name+octave ("C4") or CC number ("CC 12")
    let mut label: String<16> = String::new();
    // Large value in center: MIDI note number ("60") or CC value ("127")
    let mut number: String<8> = String::new();

    match state.last_event {
        Some(LastEvent::Note { note }) => {
            let (name, oct) = note_name(note);
            write!(label, "{}{}", name, oct).ok();
            write!(number, "{}", note).ok();
        }
        Some(LastEvent::Cc { num, value }) => {
            write!(label, "CC {}", num).ok();
            write!(number, "{}", value).ok();
        }
        Some(LastEvent::Clear) | None => {}
    }

    if !label.is_empty() {
        Text::with_alignment(
            label.as_str(),
            center + Point::new(0, -8),
            MonoTextStyle::new(&FONT_6X9, off),
            Alignment::Center,
        )
        .draw(disp)
        .ok();
    }

    if !number.is_empty() {
        Text::with_alignment(
            number.as_str(),
            center + Point::new(0, 12),
            MonoTextStyle::new(&FONT_10X20, off),
            Alignment::Center,
        )
        .draw(disp)
        .ok();
    }

    // Keys channel — top-left
    let mut keys_ch: String<4> = String::new();
    write!(keys_ch, "{}", state.melody_channel + 1).ok();
    Text::with_alignment(
        keys_ch.as_str(),
        center + Point::new(-42, -9),
        MonoTextStyle::new(&FONT_6X9, off),
        Alignment::Center,
    )
    .draw(disp)
    .ok();

    // Drums channel — top-right, always 10
    Text::with_alignment(
        "10",
        center + Point::new(54, -9),
        MonoTextStyle::new(&FONT_6X9, off),
        Alignment::Center,
    )
    .draw(disp)
    .ok();

    disp.flush().ok();
}
