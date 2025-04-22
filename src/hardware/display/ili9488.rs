use embedded_graphics::pixelcolor::Rgb565;
use embedded_hal::delay::DelayNs;
use mipidsi::models::Model;
use mipidsi::{
    dcs::{BitsPerPixel, PixelFormat, SetAddressMode},
    options::ModelOptions,
};
use core::convert::TryInto;

pub struct ILI9488Rgb565;

impl Model for ILI9488Rgb565 {
    type ColorFormat = Rgb565;
    const FRAMEBUFFER_SIZE: (u16, u16) = (240, 320);

    fn init<DELAY, DI>(
        &mut self,
        di: &mut DI,
        delay: &mut DELAY,
        options: &ModelOptions,
    ) -> Result<SetAddressMode, <DI as mipidsi::interface::Interface>::Error>
    where
        DELAY: DelayNs,
        DI: mipidsi::interface::Interface,
    {
        di.send_command(0x01, &[])?;
        delay.delay_us(5_000);
        let pf = PixelFormat::with_all(BitsPerPixel::from_rgb_color::<Self::ColorFormat>());
        init_common(di, delay, options, pf)
    }
}

/// Common init for all ILI934x controllers and color formats.
pub fn init_common<DELAY, DI>(
    di: &mut DI,
    delay: &mut DELAY,
    options: &ModelOptions,
    pixel_format: PixelFormat,
) -> Result<SetAddressMode, <DI as mipidsi::interface::Interface>::Error>
where
    DELAY: DelayNs,
    DI: mipidsi::interface::Interface,
{
    let madctl = SetAddressMode::from(options);
    di.send_command(0x36, &[madctl.bits()])?;
    di.send_command(0xB4, &[0x0])?;
    let inv_cmd = match options.invert_colors {
        mipidsi::options::ColorInversion::Inverted => 0x21,
        mipidsi::options::ColorInversion::Normal => 0x20,
    };
    di.send_command(inv_cmd, &[])?;
    di.send_command(0x3A, &[*pixel_format])?;
    di.send_command(0x13, &[])?;
    delay.delay_us(120_000);
    di.send_command(0x11, &[])?;
    delay.delay_us(140_000);
    di.send_command(0x29, &[])?;
    Ok(madctl)
}
