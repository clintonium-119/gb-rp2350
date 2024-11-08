//! Set up linker scripts for the rp235x-hal examples

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    dotenv::dotenv().ok(); // Load .env file
                           // Put the linker script somewhere the linker can find it
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    println!("cargo:rustc-link-search={}", out.display());

    // The file `memory.x` is loaded by cortex-m-rt's `link.x` script, which
    // is what we specify in `.cargo/config.toml` for Arm builds
    let memory_x = include_bytes!("memory.x");
    let mut f = File::create(out.join("memory.x")).unwrap();
    f.write_all(memory_x).unwrap();
    println!("cargo:rerun-if-changed=memory.x");

    // The file `rp235x_riscv.x` is what we specify in `.cargo/config.toml` for
    // RISC-V builds
    let rp235x_riscv_x = include_bytes!("rp235x_riscv.x");
    let mut f = File::create(out.join("rp235x_riscv.x")).unwrap();
    f.write_all(rp235x_riscv_x).unwrap();
    println!("cargo:rerun-if-changed=rp235x_riscv.x");

    println!("cargo:rerun-if-changed=build.rs");

    let rom_path = std::env::var("ROM_PATH").expect("ROM_PATH needs to be set");
    println!("cargo:rustc-env=ROM_PATH={}", rom_path);
    let rom_location = std::env::var("ROM_LOCATION").expect("ROM_LOCATION needs to be set");
    match rom_location.as_str() {
        "SDCARD" => {
            println!("cargo:rustc-cfg=feature=\"sdcard_rom\"");
        }
        "FLASH" => {
            println!("cargo:rustc-cfg=feature=\"flash_rom\"");
        }
        _ => {
            panic!("Wrong value for ROM_LOCATION: {}", rom_location);
        }
    }

    let boot_rom_location =
        std::env::var("BOOT_ROM_LOCATION").expect("BOOT_ROM_LOCATION needs to be set");
    match boot_rom_location.as_str() {
        "SDCARD" => {
            let boot_rom_path =
                std::env::var("BOOT_ROM_PATH").expect("BOOT_ROM_PATH needs to be set");
            println!("cargo:rustc-env=BOOT_ROM_PATH={}", boot_rom_path);

            println!("cargo:rustc-cfg=feature=\"boot_sdcard_rom\"");
        }
        "FLASH" => {
            let boot_rom_path =
                std::env::var("BOOT_ROM_PATH").expect("BOOT_ROM_PATH needs to be set");
            println!("cargo:rustc-env=BOOT_ROM_PATH={}", boot_rom_path);

            println!("cargo:rustc-cfg=feature=\"boot_flash_rom\"");
        }
        "NONE" => {
            println!("cargo:rustc-cfg=feature=\"boot_none_rom\"");
        }
        _ => {
            panic!("Wrong value for ROM_LOCATION: {}", rom_location);
        }
    }

    println!(
        "cargo:rustc-env=DISPLAY_WIDTH={}",
        std::env::var("DISPLAY_WIDTH").expect("DISPLAY_WIDTH needs to be set")
    );
    println!(
        "cargo:rustc-env=DISPLAY_HEIGHT={}",
        std::env::var("DISPLAY_HEIGHT").expect("DISPLAY_HEIGHT needs to be set")
    );
    println!(
        "cargo:rustc-env=GAMEBOY_RENDER_WIDTH={}",
        std::env::var("GAMEBOY_RENDER_WIDTH").expect("GAMEBOY_RENDER_WIDTH needs to be set")
    );
    println!(
        "cargo:rustc-env=GAMEBOY_RENDER_HEIGHT={}",
        std::env::var("GAMEBOY_RENDER_HEIGHT").expect("GAMEBOY_RENDER_HEIGHT needs to be set")
    );
    println!("cargo:rerun-if-changed=.env");
}
