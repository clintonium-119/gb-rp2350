use alloc::{format, string::String};
use embedded_graphics::{
    mono_font::{ascii::FONT_8X13_BOLD, MonoTextStyleBuilder},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, RoundedRectangle},
    text::{Baseline, Text},
};

pub struct LoadingScreen {
    position: Point,
    size: Size,
    title: String,
}

impl LoadingScreen {
    pub fn new(position: Point, size: Size, title: String) -> Self {
        LoadingScreen {
            position,
            size,
            title,
        }
    }

    pub fn draw<D>(&self, display: &mut D, percentage: f32) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        // Colors
        let background_color = Rgb565::CSS_DARK_BLUE; // Dark blue
        let border_color = Rgb565::CSS_SKY_BLUE; // Slightly lighter blue
        let bar_color = Rgb565::GREEN; // Cyan
                                       //let bar_color = Rgb565::new(0, 31, 31); // Cyan
        let text_color = Rgb565::CSS_WHITE;

        // Draw background
        Rectangle::new(self.position, self.size)
            .into_styled(PrimitiveStyle::with_fill(background_color))
            .draw(display)?;

        // Text style
        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_8X13_BOLD)
            .text_color(text_color)
            .build();

        // Draw title
        Text::with_baseline(
            &self.title,
            self.position + Point::new(self.size.width as i32 / 2, 20),
            text_style,
            Baseline::Middle,
        )
        .draw(display)?;

        // Progress bar configuration
        let bar_width = (self.size.width as i32 * 4) / 5; // 80% of screen width
        let bar_height = 20;
        let bar_x = (self.size.width as i32 - bar_width) / 2;
        let bar_y = (self.size.height as i32 * 2) / 3; // Position at 2/3 down

        // // Draw progress bar background (border)
        // RoundedRectangle::new(
        //     Point::new(bar_x, bar_y),
        //     Size::new(bar_width as u32, bar_height as u32),
        //     Size::new(5, 5), // Corner radius
        // )
        // .into_styled(
        //     PrimitiveStyleBuilder::new()
        //         .stroke_color(border_color)
        //         .stroke_width(2)
        //         .fill_color(background_color)
        //         .build(),
        // )
        // .draw(display)?;

        // Draw progress bar background (border)
        RoundedRectangle::with_equal_corners(
            Rectangle::new(
                Point::new(bar_x, bar_y),
                Size::new(bar_width as u32, bar_height as u32),
            ),
            Size::new(5, 5), // Corner radius
        )
        .into_styled(
            PrimitiveStyleBuilder::new()
                .stroke_color(border_color)
                .stroke_width(2)
                .fill_color(background_color)
                .build(),
        )
        .draw(display)?;

        // Draw progress bar fill
        let fill_width = (bar_width as f32 * percentage.clamp(0.0, 100.0) / 100.0) as i32;
        if fill_width > 0 {
            // RoundedRectangle::new(
            //     Point::new(bar_x + 2, bar_y + 2),
            //     Size::new((fill_width - 4) as u32, (bar_height - 4) as u32),
            //     Size::new(4, 4),
            // )
            // .into_styled(PrimitiveStyle::with_fill(bar_color))
            // .draw(display)?;

            RoundedRectangle::with_equal_corners(
                Rectangle::new(
                    Point::new(bar_x + 2, bar_y + 2),
                    Size::new((fill_width - 4) as u32, (bar_height - 4) as u32),
                ),
                Size::new(4, 4),
            )
            .into_styled(PrimitiveStyle::with_fill(bar_color))
            .draw(display)?;
        }

        // Draw percentage text
        let percentage_text = format!("{}%", percentage as i32);
        Text::with_baseline(
            &percentage_text,
            self.position + Point::new(self.size.width as i32 / 2, bar_y + bar_height + 15),
            text_style,
            Baseline::Middle,
        )
        .draw(display)?;

        // Draw loading animation dots
        let dots = ".".repeat(((percentage as i32 / 20) % 4) as usize);
        let loading_text = format!("Loading{}", dots);
        Text::with_baseline(
            &loading_text,
            self.position + Point::new(self.size.width as i32 / 2, bar_y - 15),
            text_style,
            Baseline::Middle,
        )
        .draw(display)?;

        Ok(())
    }
}
