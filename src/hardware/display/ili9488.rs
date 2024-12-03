use display_interface::{DataFormat, WriteOnlyDataCommand};
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::IntoStorage;
use embedded_hal::delay::DelayNs;
use mipidsi::dcs::{
    EnterNormalMode, ExitSleepMode, SetDisplayOn, SetInvertMode, SetPixelFormat, WriteMemoryStart,
};
use mipidsi::models::Model;
use mipidsi::{
    dcs::{BitsPerPixel, Dcs, PixelFormat, SetAddressMode, SoftReset},
    error::Error,
    options::ModelOptions,
};
pub struct ILI9488Rgb565;

impl Model for ILI9488Rgb565 {
    type ColorFormat = Rgb565;

    const FRAMEBUFFER_SIZE: (u16, u16) = (240, 320);

    fn init<RST, DELAY, DI>(
        &mut self,
        dcs: &mut mipidsi::dcs::Dcs<DI>,
        delay: &mut DELAY,
        options: &mipidsi::options::ModelOptions,
        rst: &mut Option<RST>,
    ) -> Result<mipidsi::dcs::SetAddressMode, mipidsi::error::InitError<RST::Error>>
    where
        RST: embedded_hal::digital::OutputPin,
        DELAY: embedded_hal::delay::DelayNs,
        DI: display_interface::WriteOnlyDataCommand,
    {
        match rst {
            Some(ref mut rst) => self.hard_reset(rst, delay)?,
            None => dcs.write_command(SoftReset)?,
        }
        let pf = PixelFormat::with_all(BitsPerPixel::from_rgb_color::<Self::ColorFormat>());
        init_common(dcs, delay, options, pf).map_err(Into::into)
    }

    fn write_pixels<DI, I>(
        &mut self,
        dcs: &mut mipidsi::dcs::Dcs<DI>,
        colors: I,
    ) -> Result<(), mipidsi::error::Error>
    where
        DI: display_interface::WriteOnlyDataCommand,
        I: IntoIterator<Item = Self::ColorFormat>,
    {
        dcs.write_command(WriteMemoryStart)?;
        let mut iter = colors.into_iter().map(|c| c.into_storage());

        let buf = DataFormat::U16BEIter(&mut iter);
        dcs.di.send_data(buf)
    }
}

/// Common init for all ILI934x controllers and color formats.
pub fn init_common<DELAY, DI>(
    dcs: &mut Dcs<DI>,
    delay: &mut DELAY,
    options: &ModelOptions,
    pixel_format: PixelFormat,
) -> Result<SetAddressMode, Error>
where
    DELAY: DelayNs,
    DI: WriteOnlyDataCommand,
{
    let madctl = SetAddressMode::from(options);

    // 15.4:  It is necessary to wait 5msec after releasing RESX before sending commands.
    // 8.2.2: It will be necessary to wait 5msec before sending new command following software reset.
    delay.delay_us(5_000);

    dcs.write_command(madctl)?;
    dcs.write_raw(0xB4, &[0x0])?;
    dcs.write_command(SetInvertMode::new(options.invert_colors))?;
    dcs.write_command(SetPixelFormat::new(pixel_format))?;

    dcs.write_command(EnterNormalMode)?;

    // 8.2.12: It will be necessary to wait 120msec after sending Sleep In command (when in Sleep Out mode)
    //          before Sleep Out command can be sent.
    // The reset might have implicitly called the Sleep In command if the controller is reinitialized.
    delay.delay_us(120_000);

    dcs.write_command(ExitSleepMode)?;

    // 8.2.12: It takes 120msec to become Sleep Out mode after SLPOUT command issued.
    // 13.2 Power ON Sequence: Delay should be 60ms + 80ms
    delay.delay_us(140_000);

    dcs.write_command(SetDisplayOn)?;

    Ok(madctl)
}
