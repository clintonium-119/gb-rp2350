#![no_std]
#![no_main]

mod clocks;

mod gameboy;
mod hardware;

mod rp_hal;

mod spi_device;

mod util;

use alloc::boxed::Box;
use cortex_m::asm;
use embedded_hal::digital::OutputPin;
extern crate alloc;

use embedded_sdmmc::{SdCard, VolumeManager};
use gameboy::display::GameboyLineBufferDisplay;
use gameboy::{GameEmulationHandler, InputButtonMapper};
use gb_core::gameboy::GameBoy;
use hal::fugit::RateExtU32;

use hardware::display::ScreenScaler;

use ili9341::{DisplaySize, DisplaySize240x320};
// Ensure we halt the program on panic (if we don't mention this crate it won't
// be linked)
use panic_halt as _;

use rp235x_hal::uart::{DataBits, StopBits, UartConfig};
use rp235x_hal::{spi, Clock};
use rp_hal::hal::dma::DMAExt;
use rp_hal::hal::pio::PIOExt;

// Alias for our HAL crate

use rp_hal::hal;
// Some things we need
use embedded_alloc::Heap;

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

#[hal::entry]
fn main() -> ! {
    {
        use core::mem::MaybeUninit;
        const HEAP_SIZE: usize = 300_000 + (0x4000 * 6);
        static mut HEAP: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe { ALLOCATOR.init(HEAP.as_ptr() as usize, HEAP_SIZE) }
    }

    // Grab our singleton objects
    let mut pac = hal::pac::Peripherals::take().unwrap();
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
        asm::nop();
    }
    pac.POWMAN
        .vreg()
        .modify(|_, w| unsafe { w.bits(0x5AFE_0000).vsel().bits(0b01111) }); // 0b01111 = 1.30V

    while pac.POWMAN.vreg().read().update_in_progress().bit_is_set() {
        asm::nop();
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
        // UART TX (characters sent from rp235x) on pin 4 (GPIO2) in Aux mode
        pins.gpio0.into_function(),
        // UART RX (characters received by rp235x) on pin 5 (GPIO3) in Aux mode
        pins.gpio1.into_function(),
    );
    let uart0 = hal::uart::UartPeripheral::new(pac.UART0, uart0_pins, &mut pac.RESETS)
        .enable(
            UartConfig::new(115200.Hz(), DataBits::Eight, None, StopBits::One),
            clocks.peripheral_clock.freq(),
        )
        .unwrap();
    defmt_serial::defmt_serial(SERIAL.init(uart0));

    defmt::info!("START2");

    let mut led_pin = pins.gpio25.into_push_pull_output();

    let (mut pio_0, sm0_0, sm0_1, _, _) = pac.PIO0.split(&mut pac.RESETS);

    let (mut pio_1, sm_1_0, _, _, _) = pac.PIO1.split(&mut pac.RESETS);
    let dma = pac.DMA.split(&mut pac.RESETS);

    const SCREEN_WIDTH: usize =
        (<DisplaySize240x320 as DisplaySize>::WIDTH as f32 / 1.0f32) as usize;
    const SCREEN_HEIGHT: usize =
        (<DisplaySize240x320 as DisplaySize>::HEIGHT as f32 / 1.0f32) as usize;
    ///////////////////////////////SD CARD
    let spi_sclk: hal::gpio::Pin<_, _, hal::gpio::PullDown> =
        pins.gpio14.into_function::<hal::gpio::FunctionSpi>();
    let spi_mosi: hal::gpio::Pin<_, _, hal::gpio::PullDown> =
        pins.gpio15.into_function::<hal::gpio::FunctionSpi>();
    let spi_cs = pins.gpio13.into_push_pull_output();
    let spi_miso: hal::gpio::Pin<_, _, hal::gpio::PullDown> =
        pins.gpio12.into_function::<hal::gpio::FunctionSpi>();

    // Create the SPI driver instance for the SPI0 device
    let spi = spi::Spi::<_, _, _, 8>::new(pac.SPI1, (spi_mosi, spi_miso, spi_sclk));
    let spi = spi.init(
        &mut pac.RESETS,
        clocks.peripheral_clock.freq(),
        400.kHz(), // card initialization happens at low baud rate
        embedded_hal::spi::MODE_0,
    );

    let exclusive_spi = embedded_hal_bus::spi::ExclusiveDevice::new(spi, spi_cs, timer).unwrap();
    let sdcard = SdCard::new(exclusive_spi, timer);
    let mut volume_mgr = VolumeManager::new(sdcard, hardware::sdcard::DummyTimesource::default());

    let mut volume0 = volume_mgr
        .open_volume(embedded_sdmmc::VolumeIdx(0))
        .unwrap();

    let mut root_dir = volume0.open_root_dir().unwrap();

    //Read boot rom
    let mut boot_rom_file = root_dir
        .open_file_in_dir("dmg_boot.bin", embedded_sdmmc::Mode::ReadOnly)
        .unwrap();
    let mut boot_rom_data = Box::new([0u8; 0x100]);
    boot_rom_file.read(&mut *boot_rom_data).unwrap();
    boot_rom_file.close().unwrap();
    root_dir.close().unwrap();
    volume0.close().unwrap();

    let roms = gameboy::rom::SdRomManager::new("pkred.gb", volume_mgr, timer);
    let gb_rom = gb_core::hardware::rom::Rom::from_bytes(roms);

    //writeln!(uart0, "Loading game: {}", &gb_rom.title).unwrap();
    let cartridge = gb_rom.into_cartridge();
    let boot_rom = gb_core::hardware::boot_rom::Bootrom::new(Some(
        gb_core::hardware::boot_rom::BootromData::from_bytes(&*boot_rom_data),
    ));
    core::mem::drop(boot_rom_data);

    //////////////////////AUDIO SETUP

    let sample_rate: u32 = 16_000;
    let clock_divider: u32 = clocks.system_clock.freq().to_Hz() * 4 / sample_rate;

    let int_divider = (clock_divider >> 8) as u16;
    let frak_divider = (clock_divider & 0xFF) as u8;

    let _i2s_din = pins.gpio9.into_function::<hal::gpio::FunctionPio1>();
    let _i2s_bclk = pins.gpio10.into_function::<hal::gpio::FunctionPio1>();
    let _i2s_lrc = pins.gpio11.into_function::<hal::gpio::FunctionPio1>();
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
        (10, 11),
        9,
        audio_buffer,
    );
    // writeln!(uart0, "Check 1").unwrap();
    let screen = GameboyLineBufferDisplay::new(timer);
    let mut gameboy = GameBoy::create(screen, cartridge, boot_rom, Box::new(i2s_interface));

    //SCREEN
    let screen_data_command_pin = pins.gpio3.into_push_pull_output();

    let spi_sclk: hal::gpio::Pin<_, _, hal::gpio::PullDown> =
        pins.gpio5.into_function::<hal::gpio::FunctionPio0>();
    let spi_mosi: hal::gpio::Pin<_, _, hal::gpio::PullDown> =
        pins.gpio4.into_function::<hal::gpio::FunctionPio0>();

    let display_buffer: &'static mut [u16] =
        cortex_m::singleton!(: [u16;(SCREEN_WIDTH) * 3]  = [0u16; (SCREEN_WIDTH ) * 3 ])
            .unwrap()
            .as_mut_slice();
    // writeln!(uart0, "Check 2").unwrap();
    let streamer = hardware::display::DmaStreamer::new(dma.ch0, dma.ch1, display_buffer);

    let display_interface = hardware::display::SpiPioDmaInterface::new(
        (3, 0),
        screen_data_command_pin,
        &mut pio_0,
        sm0_1,
        sm0_0,
        spi_sclk.id().num,
        spi_mosi.id().num,
        streamer,
    );
    // writeln!(uart0, "Check 4").unwrap();
    let display_reset = pins.gpio2.into_push_pull_output();
    let mut display = ili9341::Ili9341::new(
        display_interface,
        display_reset,
        &mut timer,
        ili9341::Orientation::LandscapeFlipped,
        ili9341::DisplaySize240x320,
    )
    .unwrap();

    let scaler: ScreenScaler<144, 160, { SCREEN_WIDTH }, { SCREEN_HEIGHT }> = ScreenScaler::new();

    ////////////////////// JOYPAD
    let mut b_button = pins.gpio16.into_pull_up_input().into_dyn_pin();
    let mut a_button = pins.gpio17.into_pull_up_input().into_dyn_pin();
    let mut right_button = pins.gpio18.into_pull_up_input().into_dyn_pin();
    let mut down_button = pins.gpio19.into_pull_up_input().into_dyn_pin();
    let mut left_button = pins.gpio20.into_pull_up_input().into_dyn_pin();
    let mut up_button = pins.gpio21.into_pull_up_input().into_dyn_pin();
    let mut select_button = pins.gpio22.into_pull_up_input().into_dyn_pin();
    let mut start_button = pins.gpio26.into_pull_up_input().into_dyn_pin();
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

    let mut loop_counter: usize = 0;
    loop {
        // writeln!(uart0, "Free Mem: {}", ALLOCATOR.free()).unwrap();
        // writeln!(uart0, "Used Mem: {}", ALLOCATOR.used()).unwrap();
        defmt::info!("Free Mem: {}", ALLOCATOR.free());
        defmt::info!("Used Mem: {}", ALLOCATOR.used());
        //   defmt::info!("START1 DFMT");
        let start_time = timer.get_counter();

        display
            .draw_raw_iter(
                0,
                0,
                // (160 - 1) as u16,
                // (144 - 1) as u16,
                (SCREEN_HEIGHT - 1) as u16,
                (SCREEN_WIDTH - 1) as u16,
                scaler.scale_iterator(GameEmulationHandler::new(&mut gameboy, &mut button_handler)),
            )
            .unwrap();
        let end_time: hal::fugit::Instant<u64, 1, 1000000> = timer.get_counter();
        let diff = end_time - start_time;
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

/// Program metadata for `picotool info`
#[link_section = ".bi_entries"]
#[used]
pub static PICOTOOL_ENTRIES: [hal::binary_info::EntryAddr; 5] = [
    hal::binary_info::rp_cargo_bin_name!(),
    hal::binary_info::rp_cargo_version!(),
    hal::binary_info::rp_program_description!(c"SPI Example"),
    hal::binary_info::rp_cargo_homepage_url!(),
    hal::binary_info::rp_program_build_attribute!(),
];
