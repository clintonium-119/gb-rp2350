use alloc::{format, string::String};
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle},
    text::Text,
};

use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{Point, Size},
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    Drawable,
};

pub struct LoadingScreen {
    position: Point,
    size: Size,
    current_progress: u8,
    rom_name: String,
}

impl LoadingScreen {
    pub fn new(position: Point, size: Size, rom_name: String) -> Self {
        Self {
            position,
            size,
            current_progress: 0,
            rom_name,
        }
    }

    /// Draws the complete loading screen including background, border and progress bar
    pub fn draw<D>(&mut self, display: &mut D, progress: u8) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        self.current_progress = progress.min(100);

        // Background
        Rectangle::new(self.position, self.size)
            .into_styled(
                PrimitiveStyleBuilder::new()
                    .fill_color(Rgb565::new(2, 10, 25)) // Dark blue background
                    .build(),
            )
            .draw(display)?;

        // Border
        Rectangle::new(self.position, self.size)
            .into_styled(
                PrimitiveStyleBuilder::new()
                    .stroke_color(Rgb565::new(10, 31, 31)) // Lighter blue border
                    .stroke_width(2)
                    .build(),
            )
            .draw(display)?;

        // "Loading..." text
        let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::new(31, 31, 31)); // White text
        Text::new(
            "Loading...",
            Point::new(self.position.x + 10, self.position.y + 20),
            text_style,
        )
        .draw(display)?;

        self.draw_progress_bar(display)?;

        Text::new(
            &self.rom_name,
            Point::new(
                self.position.x + (self.size.width as i32 / 2) - 15, // Centered horizontally
                self.position.y + (self.size.height as i32 / 2) + 25, // Below progress bar
            ),
            text_style,
        )
        .draw(display)?;

        Ok(())
    }

    /// Only updates the progress bar portion of the loading screen
    pub fn update_progress<D>(&mut self, display: &mut D, progress: u8) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        self.current_progress = progress.min(100);
        self.draw_progress_bar(display)
    }

    /// Helper function to draw the progress bar
    fn draw_progress_bar<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        // Progress bar background
        let bar_height: u32 = 20;
        //let bar_y = self.position.y + self.size.height as i32 - bar_height as i32 - 10;
        let bar_y = self.position.y + (self.size.height as i32 / 2) - (bar_height as i32 / 2);

        Rectangle::new(
            Point::new(self.position.x + 10, bar_y),
            Size::new(self.size.width - 20, bar_height),
        )
        .into_styled(
            PrimitiveStyleBuilder::new()
                .fill_color(Rgb565::new(5, 15, 15)) // Darker progress bar background
                .build(),
        )
        .draw(display)?;

        // Active progress bar
        let progress_width = ((self.size.width - 20) as u32 * self.current_progress as u32) / 100;
        if progress_width > 0 {
            Rectangle::new(
                Point::new(self.position.x + 10, bar_y),
                Size::new(progress_width, bar_height),
            )
            .into_styled(
                PrimitiveStyleBuilder::new()
                    .fill_color(Rgb565::new(0, 31, 31)) // Bright blue progress
                    .build(),
            )
            .draw(display)?;
        }
        // Progress percentage text
        // "Loading..." text
        let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::new(31, 31, 31)); // White text
        let progress_text = format!("{}%", self.current_progress);
        Text::new(
            &progress_text,
            Point::new(
                self.position.x + self.size.width as i32 - 35,
                self.position.y + 20,
            ),
            text_style,
        )
        .draw(display)?;
        Ok(())
    }
}
