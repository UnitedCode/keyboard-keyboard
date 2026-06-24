use crate::display::note_name;
use crate::settings::{Settings, SETTINGS_ITEMS, NUM_SETTINGS_ITEMS};
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
    primitives::{Line, PrimitiveStyle, Rectangle},
    text::{Alignment, Text},
};
use heapless::String;

const SPRITE_ATLAS: &[u8] = include_bytes!("../../../../images/Keyboard-Keyboard-Spritesheet.raw");

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
            write!(label, "{}", note).ok();
            write!(number, "{}{}", name, oct).ok();
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

    // Drums channel — top-right
    let mut drum_ch: String<4> = String::new();
    write!(drum_ch, "{}", state.drum_channel + 1).ok();
    Text::with_alignment(
        drum_ch.as_str(),
        center + Point::new(54, -9),
        MonoTextStyle::new(&FONT_6X9, off),
        Alignment::Center,
    )
    .draw(disp)
    .ok();

    disp.flush().ok();
}

/// Draws the settings screen: 3 items (prev / current / next) with name left,
/// value right. The current item has an underline under its value.
pub fn draw_settings(disp: &mut LcdDisplay, selected: usize, settings: &Settings) {
    disp.clear();

    let style = MonoTextStyle::new(&FONT_6X9, BinaryColor::On);
    let underline_style = PrimitiveStyle::with_stroke(BinaryColor::On, 1);

    // Baseline y-coordinates for the three rows (font is 9px tall).
    let y_rows: [i32; 3] = [9, 20, 30];

    let prev = (selected + NUM_SETTINGS_ITEMS - 1) % NUM_SETTINGS_ITEMS;
    let next = (selected + 1) % NUM_SETTINGS_ITEMS;
    let slots = [prev, selected, next];

    for (row, &item_idx) in slots.iter().enumerate() {
        let y = y_rows[row];
        let item = &SETTINGS_ITEMS[item_idx];
        let value = settings.get(item_idx);

        // Name: left-aligned
        Text::new(item.name, Point::new(0, y), style).draw(disp).ok();

        // Value: right-aligned
        let mut val_str: String<8> = String::new();
        write!(val_str, "{}", value).ok();
        Text::with_alignment(
            val_str.as_str(),
            Point::new(127, y),
            style,
            Alignment::Right,
        )
        .draw(disp)
        .ok();

        // Underline under the value of the current (middle) row
        if row == 1 {
            let val_px_width = val_str.len() as i32 * 6;
            let x0 = 127 - val_px_width + 1;
            let uy = y + 1;
            Line::new(Point::new(x0, uy), Point::new(127, uy))
                .into_styled(underline_style)
                .draw(disp)
                .ok();
        }
    }

    disp.flush().ok();
}
