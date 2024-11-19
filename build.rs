//! Set up linker scripts for the rp235x-hal examples

use std::env;
use std::fs::File;
use std::io::Write;
use std::path::{self, PathBuf};

#[dotenvy::load]
fn main() {
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

    let rom_location = std::env::var("ROM_LOCATION").expect("ROM_LOCATION needs to be set");
    match rom_location.as_str() {
        "RAM" => {
            println!("cargo:rustc-cfg=feature=\"ram_rom\"");
        }
        "FLASH" => {
            println!("cargo:rustc-cfg=feature=\"flash_rom\"");
        }
        "PSRAM" => {
            println!("cargo:rustc-cfg=feature=\"psram_rom\"");
        }
        _ => {
            panic!("Wrong value for ROM_LOCATION: {}", rom_location);
        }
    }

    let boot_rom_path = std::env::var("BOOT_ROM_PATH").unwrap_or("dmg_boot.bin".to_string());
    println!("cargo:rustc-env=BOOT_ROM_PATH={}", boot_rom_path);

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

    load_pin_mapping();
    load_display_driver();

    println!("cargo:rerun-if-changed=pin_mapping.env");
    println!("cargo:rerun-if-changed=.env");
}

fn load_pin_mapping() {
    let mut env_map = dotenvy::EnvLoader::with_path("pin_mapping.env")
        .load()
        .unwrap();

    let custom_map = option_env!("CUSTOM_PIN_MAP");
    if custom_map.is_some() {
        println!("cargo:rerun-if-changed={}", custom_map.unwrap());
        let custom_mapping_env = dotenvy::EnvLoader::with_path(custom_map.unwrap())
            .load()
            .unwrap();
        for (key, value) in custom_mapping_env {
            env_map.insert(key, value);
        }
    }

    for (key, value) in env_map {
        println!("cargo:rustc-env=PIN_{}={}", key, value);
    }
}

fn load_display_driver() {
    let display_driver = std::env::var("DISPLAY_DRIVER").expect("DISPLAY_DRIVER needs to be set");
    let code = format!(
        "
        use {display_driver} as DisplayDriver;
        ",
        display_driver = display_driver
    );

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = PathBuf::from(out_dir).join("generated_display_driver.rs");

    // Write the generated code to the file
    let mut f = File::create(&dest_path).unwrap();
    f.write_all(code.as_bytes()).unwrap();
}
