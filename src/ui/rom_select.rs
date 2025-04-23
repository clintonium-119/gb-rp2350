use core::convert::Infallible;

use alloc::string::String;
use embedded_graphics::{
    mono_font::{ascii::FONT_6X12, MonoTextStyleBuilder},
    pixelcolor::Rgb565,
    prelude::*,
    prelude::{DrawTarget, Point, RgbColor, WebColors},
    text::{Baseline, Text},
};
use embedded_hal::digital::InputPin;
use rp235x_hal::timer::TimerDevice;

use crate::{util::LimitedViewList, RENDER_HEIGHT, RENDER_WIDTH, RENDER_LEFT_PADDING, RENDER_TOP_PADDING};

use super::ListDisplay;

#[inline(always)]
pub fn select_rom<D, B, S>(
    display: &mut D,
    rom_list: &[String],
    timer: crate::hal::Timer<S>,
    up_button: &mut B,
    down_button: &mut B,
    a_button: &mut B,
    start_button: &mut B,
) -> Option<usize>
where
    D: DrawTarget<Color = Rgb565>,
    B: InputPin<Error = Infallible>,
    S: TimerDevice,
{
    let mut selected_rom = 0u8;
    let mut button_clicked = false;

    display.clear(Rgb565::CSS_GRAY).ok()?;

    let title_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X12)
        .text_color(Rgb565::WHITE)
        .build();

    Text::with_baseline(
        "Select Rom:",
        Point::new(0, 7),
        title_style,
        Baseline::Middle,
    )
    .draw(display)
    .ok()?;

    let list = ListDisplay::new(
        Point::new(RENDER_LEFT_PADDING as i32, RENDER_TOP_PADDING as i32 + 20),   // Starting position
        RENDER_WIDTH as i32, // Width in pixels
        20,
        5,
    );
    let max_items_to_display = ((RENDER_HEIGHT / (20 + 5)) as usize) - 1;
    let mut items = LimitedViewList::new(rom_list, max_items_to_display);
    list.draw(items.iter(), 0, display).ok()?;
    loop {
        if up_button.is_low().unwrap() && !button_clicked {
            if selected_rom != 0 {
                selected_rom = selected_rom - 1;
                defmt::info!("up_button Start redraw: {}", selected_rom);
                list.draw(items.iter(), selected_rom, display).ok()?;
            } else {
                items.prev();
                list.draw(items.iter(), selected_rom, display).ok()?;
            }
            button_clicked = true;
        }
        if down_button.is_low().unwrap() && !button_clicked {
            if selected_rom + 1 < items.max() as u8 {
                selected_rom = selected_rom + 1;
                defmt::info!("down_button Start redraw: {}", selected_rom);
                list.draw(items.iter(), selected_rom, display).ok()?;
            } else if (items.len() - items.current_cursor()) > items.max() {
                items.next();
                list.draw(items.iter(), selected_rom, display).ok()?;
            }
            button_clicked = true;
        }
        if a_button.is_low().unwrap() {
            return Some(items.current_cursor() + selected_rom as usize);
        }
        if start_button.is_low().unwrap() {
            return None;
        }

        if down_button.is_high().unwrap() && up_button.is_high().unwrap() {
            button_clicked = false;
        }
    }
}
