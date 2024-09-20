#![no_std]
#![no_main]
mod array_scaler;

mod dma_transfer;
mod gameboy;
mod pio_interface;
mod rp_hal;
mod scaler;
mod sdcard;
mod spi_device;
mod stream_display;
mod util;
mod clocks;

use alloc::boxed::Box;
use embedded_hal::digital::OutputPin;
extern crate alloc;
use alloc::vec::Vec;
use embedded_sdmmc::{SdCard, VolumeManager};
use gameboy::display::{GameVideoIter, GameboyLineBufferDisplay};
use gb_core::gameboy::GameBoy;
use hal::fugit::RateExtU32;
use ili9341::{DisplaySize, DisplaySize240x320};
// Ensure we halt the program on panic (if we don't mention this crate it won't
// be linked)
use panic_halt as _;

use rp235x_hal::{spi, Clock};
use rp_hal::hal::dma::DMAExt;
use rp_hal::hal::pio::PIOExt;
// Alias for our HAL crate
use rp_hal::hal;

// Some things we need
use embedded_alloc::Heap;
use util::DummyOutputPin;

/// Tell the Boot ROM about our application
#[link_section = ".start_block"]
#[used]
pub static IMAGE_DEF: hal::block::ImageDef = hal::block::ImageDef::secure_exe();

/// External high-speed crystal on the Raspberry Pi Pico 2 board is 12 MHz.
/// Adjust if your board has a different frequency
const XTAL_FREQ_HZ: u32 = 12_000_000u32;

#[global_allocator]
static ALLOCATOR: Heap = Heap::empty();

#[hal::entry]
fn main() -> ! {
    {
        use core::mem::MaybeUninit;
        const HEAP_SIZE: usize = 470_000;
        static mut HEAP: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe { ALLOCATOR.init(HEAP.as_ptr() as usize, HEAP_SIZE) }
    }

    
    // Grab our singleton objects
    let mut pac = hal::pac::Peripherals::take().unwrap();

    // Set up the watchdog driver - needed by the clock setup code
    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);

    pac.POWMAN
    .vreg()
    .write(|w| unsafe { w.vsel().bits(0b01111) }); //1.3v


    let clocks = clocks::configure_overclock(
        XTAL_FREQ_HZ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    ).unwrap();

    let mut timer = hal::Timer::new_timer0(pac.TIMER0, &mut pac.RESETS, &clocks);
    let sio = hal::Sio::new(pac.SIO);

    let pins = hal::gpio::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let mut led_pin = pins.gpio25.into_push_pull_output();

    let rs = pins.gpio28.into_push_pull_output();
    let rw = pins.gpio22.into_function::<hal::gpio::FunctionPio0>(); //AKA DC

    let _ = pins.gpio3.into_function::<hal::gpio::FunctionPio0>();
    let _ = pins.gpio4.into_function::<hal::gpio::FunctionPio0>();
    let _ = pins.gpio5.into_function::<hal::gpio::FunctionPio0>();
    let _ = pins.gpio6.into_function::<hal::gpio::FunctionPio0>();
    let _ = pins.gpio7.into_function::<hal::gpio::FunctionPio0>();
    let _ = pins.gpio8.into_function::<hal::gpio::FunctionPio0>();
    let _ = pins.gpio9.into_function::<hal::gpio::FunctionPio0>();
    let _ = pins.gpio10.into_function::<hal::gpio::FunctionPio0>();

    let (mut pio, sm0, _, _, _) = pac.PIO0.split(&mut pac.RESETS);

    ///////////////////////////////SD CARD
    let spi_sclk: hal::gpio::Pin<_, _, hal::gpio::PullDown> =
        pins.gpio18.into_function::<hal::gpio::FunctionSpi>();
    let spi_mosi: hal::gpio::Pin<_, _, hal::gpio::PullDown> =
        pins.gpio19.into_function::<hal::gpio::FunctionSpi>(); //tx
    let spi_cs = pins.gpio17.into_push_pull_output();
    let spi_miso: hal::gpio::Pin<_, _, hal::gpio::PullDown> =
        pins.gpio16.into_function::<hal::gpio::FunctionSpi>(); //rx

    // Create the SPI driver instance for the SPI0 device
    let spi = spi::Spi::<_, _, _, 8>::new(pac.SPI0, (spi_mosi, spi_miso, spi_sclk));
    let spi = spi.init(
        &mut pac.RESETS,
        clocks.peripheral_clock.freq(),
        400.kHz(),
        embedded_hal::spi::MODE_0,
    );
    let exclusive_spi = embedded_hal_bus::spi::ExclusiveDevice::new(spi, spi_cs, timer).unwrap();
    let sdcard = SdCard::new(exclusive_spi, timer);
    let mut volume_mgr = VolumeManager::new(sdcard, sdcard::DummyTimesource::default());

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

    let rom_file = root_dir
        .open_file_in_dir("sml.gb", embedded_sdmmc::Mode::ReadOnly)
        .unwrap();

    let roms = gameboy::rom::SdRomManager::new(rom_file);
    let gb_rom = gb_core::hardware::rom::Rom::from_bytes(roms);
    let cartridge = gb_rom.into_cartridge();

    ///////////////////////////////
    let spi_sclk: hal::gpio::Pin<_, _, hal::gpio::PullDown> =
        pins.gpio14.into_function::<hal::gpio::FunctionSpi>();
    let spi_mosi: hal::gpio::Pin<_, _, hal::gpio::PullDown> =
        pins.gpio15.into_function::<hal::gpio::FunctionSpi>(); //tx
    let mut spi_cs = pins.gpio13.into_push_pull_output();
    let spi_miso: hal::gpio::Pin<_, _, hal::gpio::PullDown> =
        pins.gpio12.into_function::<hal::gpio::FunctionSpi>(); //rx
    let spi_screen = spi::Spi::<_, _, _, 8>::new(pac.SPI1, (spi_mosi, spi_miso, spi_sclk));
    let screen_dc: hal::gpio::Pin<
        hal::gpio::bank0::Gpio11,
        hal::gpio::FunctionSio<hal::gpio::SioOutput>,
        hal::gpio::PullDown,
    > = pins.gpio11.into_push_pull_output();
    let spi_screen = spi_screen.init(
        &mut pac.RESETS,
        clocks.peripheral_clock.freq(),
        80.MHz(), // card initialization happens at low baud rate
        embedded_hal::spi::MODE_0,
    );
    spi_cs.set_low().unwrap();
    let exclusive_screen_spi = spi_device::ExclusiveDevice::new(spi_screen, spi_cs, timer).unwrap();
    let spi_display_interface =
        display_interface_spi::SPIInterface::new(exclusive_screen_spi, screen_dc);
    /////////////
    let interface =
        pio_interface::PioInterface::new(1, rs, &mut pio, sm0, rw.id().num, (3, 10), endianess);

    let mut display = ili9341::Ili9341::new_orig(
        spi_display_interface,
        DummyOutputPin,
        &mut timer,
        ili9341::Orientation::Landscape,
        ili9341::DisplaySize240x320,
    )
    .unwrap();

    let boot_rom = gb_core::hardware::boot_rom::Bootrom::new(Some(
        gb_core::hardware::boot_rom::BootromData::from_bytes(&*boot_rom_data),
    ));
    core::mem::drop(boot_rom_data);
    let screen = GameboyLineBufferDisplay::new();
    let mut gameboy = GameBoy::create(
        screen,
        cartridge,
        boot_rom,
        Box::new(gameboy::audio::NullAudioPlayer),
    );

    const SCREEN_WIDTH: usize =
        (<DisplaySize240x320 as DisplaySize>::WIDTH as f32 / 1.0f32) as usize;
    const SCREEN_HEIGHT: usize =
        (<DisplaySize240x320 as DisplaySize>::HEIGHT as f32 / 1.0f32) as usize;

    let spare: &'static mut [u8] =
        cortex_m::singleton!(: Vec<u8>  = alloc::vec![0; (SCREEN_WIDTH * SCREEN_HEIGHT) * 2 ])
            .unwrap()
            .as_mut_slice();

    let dm_spare: &'static mut [u8] =
        cortex_m::singleton!(: Vec<u8>  = alloc::vec![0; (SCREEN_WIDTH * SCREEN_HEIGHT) * 2 ])
            .unwrap()
            .as_mut_slice();

    let dma = pac.DMA.split(&mut pac.RESETS);

    let mut streamer = stream_display::Streamer::new(dma.ch0, dm_spare, spare);
    let scaler: scaler::ScreenScaler<144, 160, { SCREEN_WIDTH }, { SCREEN_HEIGHT }> =
        scaler::ScreenScaler::new();
    led_pin.set_high().unwrap();
    loop {
        display = display
            .async_transfer_mode(
                0,
                0,
                (SCREEN_HEIGHT - 1) as u16,
                (SCREEN_WIDTH - 1) as u16,
                |iface| {
                    let (mut sp, dc) = iface.release();
                    sp = sp.share_bus(|bus| {
                        streamer.stream::<_, _, _, _, 2>(
                            bus,
                            &mut scaler.scale_iterator(GameVideoIter::new(&mut gameboy)),
                            |d| d.to_be_bytes(),
                        )
                    });
                    display_interface_spi::SPIInterface::new(sp, dc)
                    // iface.transfer_16bit_mode(|sm| {
                    //     streamer.stream::<_, _, _, _, 1>(
                    //         sm,
                    //         &mut scaler.scale_iterator(GameVideoIter::new(&mut gameboy)),
                    //         |d| [d],
                    //     )
                    // })
                },
            )
            .unwrap();
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

#[inline(always)]
const fn endianess(be: bool, val: u16) -> u16 {
    if be {
        val.to_le()
    } else {
        val.to_be()
    }
}
