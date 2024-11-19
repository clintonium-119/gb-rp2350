#![no_std]
#![no_main]

mod clocks;

mod gameboy;
mod hardware;

mod rp_hal;
mod spi_device;
mod util;

use alloc::boxed::Box;
use alloc::collections::binary_heap::Iter;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use core::cell::{RefCell, UnsafeCell};
use core::convert::Infallible;
use core::sync::atomic::{compiler_fence, Ordering};
use embedded_graphics::mono_font::iso_8859_15::FONT_6X12;
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal_async::delay;
use embedded_sdmmc::sdcard::AcquireOpts;
use gb_core::hardware::boot_rom::Bootrom;
use gb_core::hardware::cartridge::Cartridge;
use hardware::flash::{FlashBlock, FLASH_SECTOR_SIZE};
use mipidsi::options::{Orientation, Rotation};
use panic_probe as _;
use util::DummyOutputPin;
extern crate alloc;

use embedded_sdmmc::{SdCard, VolumeManager};
use gameboy::display::GameboyLineBufferDisplay;
use gameboy::{GameEmulationHandler, InputButtonMapper};
use gb_core::gameboy::GameBoy;
use hal::fugit::RateExtU32;

use hardware::display::ScreenScaler;

use rp235x_hal::timer::TimerDevice;
use rp235x_hal::uart::{DataBits, StopBits, UartConfig};
use rp235x_hal::{spi, Clock};
use rp_hal::hal::dma::DMAExt;
use rp_hal::hal::pio::PIOExt;

// Alias for our HAL crate
use rp_hal::hal;
// Some things we need
use embedded_alloc::LlffHeap as Heap;

//Include selected display driver
include!(concat!(env!("OUT_DIR"), "/generated_display_driver.rs"));

/// Tell the Boot ROM about our application
#[link_section = ".start_block"]
#[used]
pub static IMAGE_DEF: hal::block::ImageDef = hal::block::ImageDef::secure_exe();

/// External high-speed crystal on the Raspberry Pi Pico 2 board is 12 MHz.
/// Adjust if your board has a different frequency
const XTAL_FREQ_HZ: u32 = 12_000_000u32;

#[global_allocator]
static ALLOCATOR: Heap = Heap::empty();

static SERIAL: static_cell::StaticCell<
    rp235x_hal::uart::UartPeripheral<
        rp235x_hal::uart::Enabled,
        rp235x_hal::pac::UART0,
        (
            rp235x_hal::gpio::Pin<
                rp235x_hal::gpio::bank0::Gpio0,
                rp235x_hal::gpio::FunctionUart,
                rp235x_hal::gpio::PullDown,
            >,
            rp235x_hal::gpio::Pin<
                rp235x_hal::gpio::bank0::Gpio1,
                rp235x_hal::gpio::FunctionUart,
                rp235x_hal::gpio::PullDown,
            >,
        ),
    >,
> = static_cell::StaticCell::new();

#[const_env::from_env]
const DISPLAY_WIDTH: u16 = 240;
#[const_env::from_env]
const DISPLAY_HEIGHT: u16 = 320;
#[const_env::from_env]
const GAMEBOY_RENDER_WIDTH: u16 = 240;
#[const_env::from_env]
const GAMEBOY_RENDER_HEIGHT: u16 = 320;

const ROM_FLASH_SIZE: usize = 1024 * 1024;

#[hal::entry]
fn main() -> ! {
    let mut pac = hal::pac::Peripherals::take().unwrap();

    // Grab our singleton objects
    let sio = hal::Sio::new(pac.SIO);
    let pins = hal::gpio::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // Set up the watchdog driver - needed by the clock setup code
    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);

    pac.POWMAN.vreg_ctrl().write(|w| unsafe {
        w.bits(0x5AFE_0000);
        w.unlock().set_bit();
        w.ht_th().bits(0b101);
        w.rst_n().set_bit();
        w
    });
    while pac.POWMAN.vreg().read().update_in_progress().bit_is_set() {
        rp235x_hal::arch::nop();
    }
    pac.POWMAN
        .vreg()
        .modify(|_, w| unsafe { w.bits(0x5AFE_0000).vsel().bits(0b01111) }); // 0b01111 = 1.30V

    while pac.POWMAN.vreg().read().update_in_progress().bit_is_set() {
        rp235x_hal::arch::nop();
    }

    let clocks = clocks::configure_overclock(
        XTAL_FREQ_HZ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let mut timer: rp235x_hal::Timer<rp235x_hal::timer::CopyableTimer0> =
        hal::Timer::new_timer0(pac.TIMER0, &mut pac.RESETS, &clocks);

    ///////////////////UART!
    let uart0_pins = (
        pin_select!(pins, env!("PIN_UART_RX")).into_function(),
        pin_select!(pins, env!("PIN_UART_TX")).into_function(),
    );

    let uart0 = hal::uart::UartPeripheral::new(pac.UART0, uart0_pins, &mut pac.RESETS)
        .enable(
            UartConfig::new(115200.Hz(), DataBits::Eight, None, StopBits::One),
            clocks.peripheral_clock.freq(),
        )
        .unwrap();
    defmt_serial::defmt_serial(SERIAL.init(uart0));

    defmt::info!("Console Start");

    {
        use core::mem::MaybeUninit;
        const HEAP_SIZE: usize = 300_000 + (0x4000 * 6);
        static mut HEAP: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe { ALLOCATOR.init(HEAP.as_ptr() as usize, HEAP_SIZE) }
    }

    let mut led_pin = pins.gpio25.into_push_pull_output();

    let (mut pio_0, sm0_0, sm0_1, _, _) = pac.PIO0.split(&mut pac.RESETS);

    let (mut pio_1, sm_1_0, _, _, _) = pac.PIO1.split(&mut pac.RESETS);
    let dma = pac.DMA.split(&mut pac.RESETS);

    ///////////////////////////////SD CARD
    let spi_sclk: hal::gpio::Pin<_, _, hal::gpio::PullDown> =
        pin_select!(pins, env!("PIN_SD_CARD_SCLK")).into_function::<hal::gpio::FunctionSpi>();

    let spi_mosi: hal::gpio::Pin<_, _, hal::gpio::PullDown> =
        pin_select!(pins, env!("PIN_SD_CARD_MOSI")).into_function::<hal::gpio::FunctionSpi>();
    let spi_cs = pin_select!(pins, env!("PIN_SD_CARD_CS")).into_push_pull_output();
    let spi_miso: hal::gpio::Pin<_, _, hal::gpio::PullDown> =
        pin_select!(pins, env!("PIN_SD_CARD_MISO")).into_function::<hal::gpio::FunctionSpi>();

    // Create the SPI driver instance for the SPI0 device
    let spi = spi::Spi::<_, _, _, 8>::new(pac.SPI1, (spi_mosi, spi_miso, spi_sclk));
    let spi = spi.init(
        &mut pac.RESETS,
        clocks.peripheral_clock.freq(),
        400.kHz(),
        embedded_hal::spi::MODE_0,
    );

    let exclusive_spi = embedded_hal_bus::spi::ExclusiveDevice::new_no_delay(spi, spi_cs).unwrap();
    let sdcard = SdCard::new_with_options(
        exclusive_spi,
        timer,
        AcquireOpts {
            acquire_retries: 100,
            use_crc: true,
        },
    );
    let mut volume_mgr: VolumeManager<_, _, 3, 2, 1> =
        VolumeManager::new_with_limits(sdcard, hardware::sdcard::DummyTimesource::default(), 5000);

    let mut volume0 = volume_mgr
        .open_volume(embedded_sdmmc::VolumeIdx(0))
        .unwrap();

    let rom_list = Rc::new(RefCell::new(alloc::vec::Vec::<String>::new()));
    let mut root_dir = volume0.open_root_dir().unwrap();
    root_dir
        .iterate_dir(|dir_entry| {
            let extension = String::from_utf8(dir_entry.name.extension().to_vec()).unwrap();
            if extension.eq("GB") {
                let rom_name =
                    String::from_utf8(dir_entry.name.base_name().to_vec()).unwrap() + ".GB";
                rom_list.borrow_mut().push(rom_name);
            }
        })
        .unwrap();
    let rom_list = rom_list.take();
    root_dir.close().unwrap();
    volume0.close().unwrap();
    let boot_rom = load_boot_rom(&mut volume_mgr);

    //////////////////////AUDIO SETUP

    let sample_rate: u32 = 16_000;
    let clock_divider: u32 = clocks.system_clock.freq().to_Hz() * 4 / sample_rate;

    let int_divider = (clock_divider >> 8) as u16;
    let frak_divider = (clock_divider & 0xFF) as u8;

    let i2s_din = pin_select!(pins, env!("PIN_I2S_DIN")).into_function::<hal::gpio::FunctionPio1>();
    let i2s_bclk =
        pin_select!(pins, env!("PIN_I2S_BCLK")).into_function::<hal::gpio::FunctionPio1>();
    let i2s_lrc = pin_select!(pins, env!("PIN_I2S_LRC")).into_function::<hal::gpio::FunctionPio1>();
    let audio_buffer: &'static mut [u16] =
        cortex_m::singleton!(: [u16; (2000 * 3) * 3]  = [0u16;  (2000 * 3) * 3 ])
            .unwrap()
            .as_mut_slice();
    let i2s_interface = hardware::sound::I2sPioInterface::new(
        sample_rate,
        dma.ch2,
        dma.ch3,
        (int_divider as u16, frak_divider as u8),
        &mut pio_1,
        sm_1_0,
        (i2s_bclk.id().num, i2s_lrc.id().num),
        i2s_din.id().num,
        audio_buffer,
    );

    let screen = GameboyLineBufferDisplay::new(timer);

    //SCREEN

    let display_buffer: &'static mut [u16] =
    cortex_m::singleton!(: [u16;(GAMEBOY_RENDER_WIDTH as usize) * 3]  = [0u16; (GAMEBOY_RENDER_WIDTH as usize ) * 3 ])
        .unwrap()
        .as_mut_slice();

    let screen_data_command_pin = pin_select!(pins, env!("PIN_SCREEN_DC")).into_push_pull_output();
    let display_reset = pin_select!(pins, env!("PIN_SCREEN_RESET")).into_push_pull_output();
    let spi_sclk: hal::gpio::Pin<_, _, hal::gpio::PullDown> =
        pin_select!(pins, 2).into_function::<hal::gpio::FunctionSpi>();
    let spi_mosi: hal::gpio::Pin<_, _, hal::gpio::PullDown> =
        pin_select!(pins, 3).into_function::<hal::gpio::FunctionSpi>();

    // let spi_bus = hal::spi::Spi::<_, _, _, 8>::new(pac.SPI0, (spi_mosi, spi_sclk));
    // let spi_bus = spi_bus.init(
    //     &mut pac.RESETS,
    //     clocks.peripheral_clock.freq(),
    //     40.MHz(),
    //     embedded_hal::spi::MODE_0,
    // );

    // let sk = spi_device::exclusive::ExclusiveDevice::new_no_delay(spi_bus, DummyOutputPin).unwrap();
    // let csj = display_interface_spi::SPIInterface::new(sk, screen_data_command_pin);

    // let mut display = mipidsi::Builder::new(DisplayDriver, csj)
    //     .reset_pin(display_reset)
    //     .display_size(DISPLAY_WIDTH as u16, DISPLAY_HEIGHT as u16)
    //     .orientation(Orientation {
    //         rotation: Rotation::Deg90,
    //         mirrored: true,
    //     })
    //     .init(&mut timer)
    //     .unwrap();

    let spi_sclk = spi_sclk.into_function::<hal::gpio::FunctionPio0>();
    let spi_mosi = spi_mosi.into_function::<hal::gpio::FunctionPio0>();

    let streamer = hardware::display::DmaStreamer::new(dma.ch0, dma.ch1, display_buffer);
    let display_interface = hardware::display::SpiPioDmaInterface::new(
        (2, 0),
        screen_data_command_pin,
        &mut pio_0,
        sm0_1,
        sm0_0,
        spi_sclk.id().num,
        spi_mosi.id().num,
        streamer,
        timer,
    );

    let mut display = mipidsi::Builder::new(DisplayDriver, display_interface)
        .reset_pin(display_reset)
        .display_size(DISPLAY_WIDTH as u16, DISPLAY_HEIGHT as u16)
        .orientation(Orientation {
            rotation: Rotation::Deg90,
            mirrored: true,
        })
        .init(&mut timer)
        .unwrap();

    ////////////////////// JOYPAD
    let mut b_button = pin_select!(pins, env!("PIN_B_BUTTON"))
        .into_pull_up_input()
        .into_dyn_pin();
    let mut a_button = pin_select!(pins, env!("PIN_A_BUTTON"))
        .into_pull_up_input()
        .into_dyn_pin();
    let mut right_button = pin_select!(pins, env!("PIN_RIGHT_BUTTON"))
        .into_pull_up_input()
        .into_dyn_pin();
    let mut down_button = pin_select!(pins, env!("PIN_DOWN_BUTTON"))
        .into_pull_up_input()
        .into_dyn_pin();
    let mut left_button = pin_select!(pins, env!("PIN_LEFT_BUTTON"))
        .into_pull_up_input()
        .into_dyn_pin();
    let mut up_button = pin_select!(pins, env!("PIN_UP_BUTTON"))
        .into_pull_up_input()
        .into_dyn_pin();
    let mut select_button = pin_select!(pins, env!("PIN_SELECT_BUTTON"))
        .into_pull_up_input()
        .into_dyn_pin();
    let mut start_button = pin_select!(pins, env!("PIN_START_BUTTON"))
        .into_pull_up_input()
        .into_dyn_pin();

    let selected_rom = select_rom(
        &mut display,
        rom_list.as_slice(),
        timer,
        &mut up_button,
        &mut down_button,
        &mut a_button,
    )
    .unwrap();
    let name = rom_list[selected_rom].clone();
    defmt::info!("Menu END: {}", defmt::Display2Format(&name));

    #[cfg(feature = "psram_rom")]
    let cartridge = {
        defmt::info!("Using PRSAM");

        let _ =
            pin_select!(pins, env!("PIN_PSRAM_CS")).into_function::<hal::gpio::FunctionXipCs1>();
        let psram_size = hardware::psram::psram_init(
            clocks.peripheral_clock.freq().to_Hz(),
            &pac.QMI,
            &pac.XIP_CTRL,
        );

        let psram = unsafe {
            const PSRAM_ADDRESS: usize = 0x11000000;
            let ptr = PSRAM_ADDRESS as *mut u8; // Using u8 for byte array
            let slice: &'static mut [u8] =
                alloc::slice::from_raw_parts_mut(ptr, psram_size as usize);
            slice
        };
        let cartridge = load_rom_to_psram(volume_mgr, timer, psram);
        cartridge
    };
    #[cfg(not(feature = "psram_rom"))]
    let cartridge = load_rom(volume_mgr, timer);

    let mut gameboy = GameBoy::create(screen, cartridge, boot_rom, Box::new(i2s_interface));
    let scaler: ScreenScaler<
        144,
        160,
        { GAMEBOY_RENDER_WIDTH as usize },
        { GAMEBOY_RENDER_HEIGHT as usize },
    > = ScreenScaler::new();

    let mut button_handler = InputButtonMapper::new(
        &mut a_button,
        &mut b_button,
        &mut start_button,
        &mut select_button,
        &mut up_button,
        &mut down_button,
        &mut left_button,
        &mut right_button,
    );
    led_pin.set_high().unwrap();

    // let things = display.release();
    // let spi_iterface = things.0;
    // let (spi, screen_data_command_pin) = spi_iterface.release();
    // let (bus, _sds) = spi.free();

    // let (df, (spi_sclk, spi_mosi)) = bus.free();

    // let spi_sclk = spi_sclk.into_function::<hal::gpio::FunctionPio0>();
    // let spi_mosi = spi_mosi.into_function::<hal::gpio::FunctionPio0>();

    // let streamer = hardware::display::DmaStreamer::new(dma.ch0, dma.ch1, display_buffer);
    // let display_interface = hardware::display::SpiPioDmaInterface::new(
    //     (2, 0),
    //     screen_data_command_pin,
    //     &mut pio_0,
    //     sm0_1,
    //     sm0_0,
    //     spi_sclk.id().num,
    //     spi_mosi.id().num,
    //     streamer,
    // );

    // let mut display = mipidsi::Builder::new(DisplayDriver, display_interface)
    //     .reset_pin(things.2.unwrap())
    //     .display_size(DISPLAY_WIDTH as u16, DISPLAY_HEIGHT as u16)
    //     .orientation(Orientation {
    //         rotation: Rotation::Deg90,
    //         mirrored: true,
    //     })
    //     .init(&mut timer)
    //     .unwrap();

    let mut loop_counter: usize = 0;

    const MIDDLE_HEIGHT: u16 = (DISPLAY_HEIGHT - GAMEBOY_RENDER_HEIGHT) / 2;
    const MIDDLE_WIDTH: u16 = (DISPLAY_WIDTH - GAMEBOY_RENDER_WIDTH) / 2;
    loop {
        defmt::info!("Free Mem: {}", ALLOCATOR.free());
        defmt::info!("Used Mem: {}", ALLOCATOR.used());

        let start_time = timer.get_counter();

        display
            .set_pixels(
                MIDDLE_HEIGHT,
                MIDDLE_WIDTH,
                (GAMEBOY_RENDER_HEIGHT - 1) as u16 + MIDDLE_HEIGHT,
                (GAMEBOY_RENDER_WIDTH - 1) as u16 + MIDDLE_WIDTH,
                scaler.scale_iterator(GameEmulationHandler::new(&mut gameboy, &mut button_handler)),
            )
            .unwrap();
        let end_time: hal::fugit::Instant<u64, 1, 1000000> = timer.get_counter();
        let diff: fugit::Duration<u64, 1, 1000000> = end_time - start_time;
        let milliseconds = diff.to_millis();
        defmt::info!(
            "Loop: {}, Time elapsed: {}:{}",
            loop_counter,
            milliseconds / 1000,
            milliseconds % 1000
        );
        loop_counter += 1;
    }
}

struct LimitedViewList<'a, T: Sized> {
    list: &'a [T],
    max: usize,
    current_cursor: usize,
}

impl<'a, T: Clone> LimitedViewList<'a, T> {
    pub fn new(list: &'a [T], max: usize) -> Self {
        Self {
            list,
            max: usize::min(max, list.len()),
            current_cursor: 0,
        }
    }
    pub fn next(&mut self) {
        // if (self.list.len() - self.max) > self.current_cursor {
        //     self.current_cursor += 1;
        // }
        if self.current_cursor < self.list.len() {
            self.current_cursor += 1;
        }
    }
    pub fn current_cursor(&self) -> usize {
        self.current_cursor
    }
    pub fn max(&self) -> usize {
        self.max
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn prev(&mut self) {
        if self.current_cursor != 0 {
            self.current_cursor -= 1;
        }
    }

    pub fn into_iter(&self) -> core::slice::Iter<'a, T> {
        defmt::info!(
            "Render list between: {} and {}",
            self.current_cursor,
            self.current_cursor + self.max
        );
        let iter = self.list[self.current_cursor..self.current_cursor + self.max].into_iter();

        iter
    }
}

// impl<'a, T> IntoIterator for LimitedViewList<'a, T> {
//     type IntoIter = &'a [T];

//     fn into_iter(self) -> Self::IntoIter {
//         todo!()
//     }
// }

// #[inline(never)]
// #[cold]

#[inline(always)]

pub fn select_rom<'a, D: DrawTarget<Color = Rgb565>, TD: TimerDevice>(
    display: &mut D,
    rom_list: &[String],
    mut _timer: crate::hal::Timer<TD>,
    up_button: &'a mut dyn InputPin<Error = Infallible>,
    down_button: &'a mut dyn InputPin<Error = Infallible>,
    select_button: &'a mut dyn InputPin<Error = Infallible>,
) -> Result<usize, D::Error> {
    let mut selected_rom = 0u8;
    let mut button_clicked = false;
    display.clear(Rgb565::CSS_GRAY)?;

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
    .draw(display)?;

    let list = ListDisplay::new(
        Point::new(0, 20),     // Starting position
        DISPLAY_HEIGHT as i32, // Width in pixels
        20,
        5,
    );
    let max_items_to_display = ((DISPLAY_WIDTH / (20 + 5)) as usize) - 1;
    let mut items = LimitedViewList::new(rom_list, max_items_to_display);
    list.draw(items.into_iter(), selected_rom, display)?;

    loop {
        if up_button.is_low().unwrap() && !button_clicked {
            if selected_rom != 0 {
                selected_rom = selected_rom - 1;
                defmt::info!("up_button Start redraw: {}", selected_rom);
                list.draw(items.into_iter(), selected_rom, display)?;
            } else {
                items.prev();
                list.draw(items.into_iter(), selected_rom, display)?;
            }
            button_clicked = true;
        }
        if down_button.is_low().unwrap() && !button_clicked {
            if selected_rom + 1 < items.max() as u8 {
                selected_rom = selected_rom + 1;
                defmt::info!("down_button Start redraw: {}", selected_rom);
                list.draw(items.into_iter(), selected_rom, display)?;
            } else if (items.len() - items.current_cursor()) > items.max() {
                items.next();
                list.draw(items.into_iter(), selected_rom, display)?;
            }
            button_clicked = true;
        }
        if select_button.is_low().unwrap() {
            return Ok(items.current_cursor + selected_rom as usize);
        }

        if down_button.is_high().unwrap() && up_button.is_high().unwrap() {
            button_clicked = false;
        }
    }
}

/// Program metadata for `picotool info`
#[link_section = ".bi_entries"]
#[used]
pub static PICOTOOL_ENTRIES: [hal::binary_info::EntryAddr; 5] = [
    hal::binary_info::rp_cargo_bin_name!(),
    hal::binary_info::rp_cargo_version!(),
    hal::binary_info::rp_program_description!(c"gb-rp2350"),
    hal::binary_info::rp_cargo_homepage_url!(),
    hal::binary_info::rp_program_build_attribute!(),
];

#[cfg(feature = "flash_rom")]
#[link_section = ".rodata"]
static FLASH_ROM_DATA: FlashBlock<ROM_FLASH_SIZE> = FlashBlock {
    data: UnsafeCell::new([0x55u8; ROM_FLASH_SIZE]),
};

#[inline(never)]
#[cfg(feature = "flash_rom")]
fn load_rom<
    'a,
    D: embedded_sdmmc::BlockDevice + 'a,
    T: embedded_sdmmc::TimeSource + 'a,
    DT: TimerDevice + 'a,
    const MAX_DIRS: usize,
    const MAX_FILES: usize,
    const MAX_VOLUMES: usize,
>(
    mut volume_manager: embedded_sdmmc::VolumeManager<D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>,
    timer: crate::hal::Timer<DT>,
) -> Box<dyn Cartridge + 'a> {
    let rom_name = env!("ROM_PATH");
    let mut volume = volume_manager
        .open_volume(embedded_sdmmc::VolumeIdx(0))
        .unwrap();
    let mut root_dir = volume.open_root_dir().unwrap();
    let mut rom_file = root_dir
        .open_file_in_dir(rom_name, embedded_sdmmc::Mode::ReadOnly)
        .unwrap();

    if ROM_FLASH_SIZE < rom_file.length() as usize {
        panic!(
            "Ram size not bigh enough for Rom of size: {}",
            rom_file.length()
        )
    }
    defmt::info!("Loading rom into flash");
    let mut offsets = rom_file.length() / FLASH_SECTOR_SIZE;
    if rom_file.length() % FLASH_SECTOR_SIZE != 0 {
        offsets += 1;
    }

    let mut buffer = [0u8; FLASH_SECTOR_SIZE as usize];
    for x in 0..offsets {
        defmt::info!("Loading rom into flash for offset: {}", x);
        rom_file.seek_from_start(x * FLASH_SECTOR_SIZE).unwrap();
        rom_file.read(&mut buffer).unwrap();
        let write_result = unsafe { FLASH_ROM_DATA.write_flash(x, &mut buffer) };
        defmt::info!(
            "Result from write into flash for offset: {}: {}",
            x,
            write_result
        );
    }

    rom_file.close().unwrap();
    root_dir.close().unwrap();
    volume.close().unwrap();
    defmt::info!("Loading complete");

    let rom_manager =
        gameboy::static_rom::StaticRomManager::new(FLASH_ROM_DATA.read(), volume_manager, timer);
    let gb_rom = gb_core::hardware::rom::Rom::from_bytes(rom_manager);
    gb_rom.into_cartridge()
}

#[cfg(feature = "ram_rom")]
#[inline(always)]
fn load_rom<
    'a,
    D: embedded_sdmmc::BlockDevice + 'a,
    T: embedded_sdmmc::TimeSource + 'a,
    DT: TimerDevice + 'a,
    const MAX_DIRS: usize,
    const MAX_FILES: usize,
    const MAX_VOLUMES: usize,
>(
    volume_manager: embedded_sdmmc::VolumeManager<D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>,
    timer: crate::hal::Timer<DT>,
) -> Box<dyn Cartridge + 'a> {
    defmt::info!("Loading from SDCARD");
    #[const_env::from_env]
    const ROM_CACHE_SIZE: usize = 10;
    let rom_manager: gameboy::rom::SdRomManager<
        D,
        T,
        DT,
        ROM_CACHE_SIZE,
        MAX_DIRS,
        MAX_FILES,
        MAX_VOLUMES,
    > = gameboy::rom::SdRomManager::new(env!("ROM_PATH"), volume_manager, timer);
    let gb_rom = gb_core::hardware::rom::Rom::from_bytes(rom_manager);
    gb_rom.into_cartridge()
}

#[inline(always)]
fn load_boot_rom<
    'a,
    D: embedded_sdmmc::BlockDevice,
    T: embedded_sdmmc::TimeSource,
    const MAX_DIRS: usize,
    const MAX_FILES: usize,
    const MAX_VOLUMES: usize,
>(
    volume_manager: &'a mut embedded_sdmmc::VolumeManager<D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>,
) -> Bootrom {
    use gb_core::hardware::boot_rom::BootromData;
    let mut volume0 = volume_manager
        .open_volume(embedded_sdmmc::VolumeIdx(0))
        .unwrap();
    let mut root_dir = volume0.open_root_dir().unwrap();

    if root_dir
        .find_directory_entry(env!("BOOT_ROM_PATH"))
        .is_err()
    {
        return Bootrom::new(None);
    }

    let dmg_boot_bin: &'static mut [u8] = cortex_m::singleton!(: [u8; 0x100]  = [0u8; 0x100 ])
        .unwrap()
        .as_mut_slice();
    let mut boot_rom_file = root_dir
        .open_file_in_dir(env!("BOOT_ROM_PATH"), embedded_sdmmc::Mode::ReadOnly)
        .unwrap();

    boot_rom_file.read(&mut *dmg_boot_bin).unwrap();
    Bootrom::new(Some(BootromData::from_bytes(dmg_boot_bin)))
}

#[inline(always)]
fn load_rom_to_psram<
    'a,
    D: embedded_sdmmc::BlockDevice + 'a,
    T: embedded_sdmmc::TimeSource + 'a,
    DT: TimerDevice + 'a,
    const MAX_DIRS: usize,
    const MAX_FILES: usize,
    const MAX_VOLUMES: usize,
>(
    mut volume_manager: embedded_sdmmc::VolumeManager<D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>,
    timer: crate::hal::Timer<DT>,
    ram: &'static mut [u8],
) -> Box<dyn Cartridge + 'a> {
    let rom_name = env!("ROM_PATH");
    let mut volume = volume_manager
        .open_volume(embedded_sdmmc::VolumeIdx(0))
        .unwrap();
    let mut root_dir = volume.open_root_dir().unwrap();
    let mut rom_file = root_dir
        .open_file_in_dir(rom_name, embedded_sdmmc::Mode::ReadOnly)
        .unwrap();

    if ram.len() < rom_file.length() as usize {
        panic!(
            "Ram size not bigh enough for Rom of size: {}",
            rom_file.length()
        )
    }
    defmt::info!("Loading rom into psram");
    rom_file.seek_from_start(0u32).unwrap();
    rom_file.read(ram).unwrap();
    rom_file.close().unwrap();
    root_dir.close().unwrap();
    volume.close().unwrap();
    defmt::info!("Loading complete");

    let rom_manager = gameboy::static_rom::StaticRomManager::new(ram, volume_manager, timer);
    let gb_rom = gb_core::hardware::rom::Rom::from_bytes(rom_manager);
    gb_rom.into_cartridge()
}

////////////////////

use embedded_graphics::{
    mono_font::{ascii::FONT_6X9, MonoTextStyleBuilder},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::{Baseline, Text},
};

struct ListDisplay {
    position: Point,
    item_height: i32,
    item_padding: i32,
    width: i32,
}

impl ListDisplay {
    pub fn new(position: Point, width: i32, item_height: i32, item_padding: i32) -> Self {
        ListDisplay {
            position,
            item_height: item_height,   // Height for each item
            item_padding: item_padding, // Padding between items
            width,
        }
    }

    pub fn draw<D>(
        &self,
        items: core::slice::Iter<'_, String>,
        selected: u8,
        display: &mut D,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        // Draw each item
        for (index, item) in items.enumerate() {
            let y_offset = index as i32 * (self.item_height + self.item_padding);
            let item_position = self.position + Point::new(0, y_offset);

            let rec_style = if index == selected as usize {
                let mut style = PrimitiveStyle::with_fill(Rgb565::BLACK);
                style.stroke_color = Some(Rgb565::WHITE);
                style.stroke_width = 1;
                style
            } else {
                let mut style = PrimitiveStyle::with_fill(Rgb565::WHITE);
                style.stroke_color = Some(Rgb565::WHITE);
                style.stroke_width = 1;
                style
            };

            let text_style = if index == selected as usize {
                MonoTextStyleBuilder::new()
                    .font(&FONT_6X9)
                    .text_color(Rgb565::WHITE)
                    .build()
            } else {
                MonoTextStyleBuilder::new()
                    .font(&FONT_6X9)
                    .text_color(Rgb565::BLACK)
                    .build()
            };

            Rectangle::new(
                item_position,
                Size::new(self.width as u32, self.item_height as u32),
            )
            .into_styled(rec_style)
            .draw(display)?;

            // Draw text
            Text::with_baseline(
                item,
                item_position + Point::new(5, self.item_height / 2),
                text_style,
                Baseline::Middle,
            )
            .draw(display)?;
        }

        Ok(())
    }
}
