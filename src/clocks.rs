use crate::rp_hal::hal::{self as hal};


use hal::clocks::ClockSource;
use hal::Clock;
use hal::{clocks::ClocksManager, pac, Watchdog};
use fugit::{HertzU32, RateExtU32};

pub const XOSC_CRYSTAL_FREQ: u32 = 12_000_000;

pub fn configure_normal(
    xosc_dev: pac::XOSC,
    clocks_dev: pac::CLOCKS,
    pll_sys_dev: pac::PLL_SYS,
    pll_usb_dev: pac::PLL_USB,
    resets: &mut pac::RESETS,
    watchdog: &mut Watchdog,
) {
    hal::clocks::init_clocks_and_plls(
        XOSC_CRYSTAL_FREQ,
        xosc_dev,
        clocks_dev,
        pll_sys_dev,
        pll_usb_dev,
        resets,
        watchdog,
    )
    .ok()
    .unwrap();
}

pub const PLL_SYS_288MHZ: hal::pll::PLLConfig = hal::pll::PLLConfig {
    vco_freq: HertzU32::Hz(1440000000),
    refdiv: 1,
    post_div1: 5,
    post_div2: 1,
};

pub const PLL_SYS_296MHZ: hal::pll::PLLConfig = hal::pll::PLLConfig {
    vco_freq: HertzU32::Hz(888000000),
    refdiv: 1,
    post_div1: 3,
    post_div2: 1,
};
pub const PLL_SYS_348MHZ: hal::pll::PLLConfig = hal::pll::PLLConfig {
    vco_freq: HertzU32::Hz(1392000000),
    refdiv: 1,
    post_div1: 4,
    post_div2: 1,
};
pub fn configure_overclock(
    xosc_crystal_freq: u32,
    xosc_dev: pac::XOSC,
    clocks_dev: pac::CLOCKS,
    pll_sys_dev: pac::PLL_SYS,
    pll_usb_dev: pac::PLL_USB,
    resets: &mut pac::RESETS,
    watchdog: &mut Watchdog,
) -> Result<ClocksManager, ()> {
    let xosc = hal::xosc::setup_xosc_blocking(xosc_dev, xosc_crystal_freq.Hz()).unwrap();

    watchdog.enable_tick_generation((xosc_crystal_freq / 1_000_000) as u16);

    let mut clocks = ClocksManager::new(clocks_dev);

    let pll_sys = hal::pll::setup_pll_blocking(
        pll_sys_dev,
        xosc.operating_frequency(),
        PLL_SYS_296MHZ,
        &mut clocks,
        resets,
    )
    .unwrap();

    let pll_usb = hal::pll::setup_pll_blocking(
        pll_usb_dev,
        xosc.operating_frequency(),
        hal::pll::common_configs::PLL_USB_48MHZ,
        &mut clocks,
        resets,
    )
    .unwrap();

    clocks
        .system_clock
        .configure_clock(&pll_sys, pll_sys.get_freq())
        .unwrap();

    clocks.init_default(&xosc, &pll_sys, &pll_usb).unwrap();

    Ok(clocks)
}