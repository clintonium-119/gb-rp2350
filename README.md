# Gameboy Emulator for the Pi Pico 2

This is a Gameboy emulator for the Pi Pico 2 completely written on Rust. The emulator is still a work in progress and optimizations are still being worked on but most games should be playable and accurate.

Supported Features:
* Save game support to SD Card.
* Loading games from SD Card or directly from flash.
* Running games bigger than the available memory of the Pi Pico.
* Sound support.
* Screen scaler, the image of the GB is scaled to different screens.
* PSRAM support for Pimoroni Pico Plus 2.
* Support for multiple displays from the mipidsi library (https://github.com/almindor/mipidsi/tree/master/mipidsi)

Pending Features:
* Game selection menu.
* Performance improvements.
* Gameboy color support.


# Hardware
## What you need
* (1x) [Raspberry Pi Pico](https://a.co/d/44gvGwD)
* (1x) [2.8inch ILI9341 240X320 LCD Display Module (or any `mipidsi` compatible display) ](https://a.co/d/auhlHku)
* (1x) [FAT 32 formatted Micro SD card + adapter](https://amzn.to/3ICKzcm) with roms you legally own. Roms must have the .gb extension and must be copied to the root folder.
* (1x) [MAX98357A amplifier](https://a.co/d/htfXmeY)
* (1x) [2W 8ohms speaker](https://a.co/d/fGtjUVC)
* (8x) [Micro Push Button Switch, Momentary Tactile Tact Touch, 6x6x6 mm, 4 pins](https://amzn.to/3dyXBsx)
* (1x) [Solderable Breadboard](https://amzn.to/3lwvfDi)
* [Dupont Wires Assorted Kit (Male to Female + Male to Male + Female to Female)](https://amzn.to/3HtbvdO)
* [Preformed Breadboard Jumper Wires](https://amzn.to/3rxwVjM)


# Pinout
* UP = GP21
* DOWN = GP19
* LEFT = GP20
* RIGHT = GP10
* BUTTON A = GP17
* BUTTON B = GP16
* SELECT = GP22
* START = GP26
* SD MISO = GP12
* SD CS = GP13
* SD CSK = GP14
* SD MOSI = GP15
* LCD CS = (GND)
* LCD CLK = GP4
* LCD SDI = GP5
* LCD RS = GP7
* LCD RST = GP8
* LCD LED = (3.3v)
* MAX98357A DIN = GP9
* MAX98357A BCLK = GP10
* MAX98357A LRC = GP11
* PSRAM_CS = 47 //This only required if you are using a Pico with PSRAM such as the Pimoroni Pico Plus 2 and enable PSRAM mode.

Notes: You can change the default mapping of the pins by setting `CUSTOM_PIN_MAP=my_custom_pin_map.env` setting.
Take a look at the `pin_mapping.env` file for a reference of all pin names.

# Installing the firmware
1. Install the latest stable version of Rust.
2. Then use `rustup` to grab the Rust Standard Library for the appropriate targets.
`rustup target add thumbv8m.main-none-eabihf`
3. Push and hold the BOOTSEL button on the Pico, then connect to your computer using a micro USB cable. Release BOOTSEL once the drive RPI-RP2 appears on your computer.
4. Set up the `.env` file to set the ROM and display configurations. Use `.env.example` as your reference.
5. Build the gameboy emulator using `cargo build --release`.
6. Drag and drop the UF2 file on to the RPI-RP2 drive. The Raspberry Pi Pico will reboot and will now run the emulator. The location of the UF2 should be `target/thumbv8m.main-none-eabihf/release/gb-rp2350`.

# Rom Loading Modes
The emulator supports 3 different ways to load roms:
* "SDCARD": Rom is loaded at runtime from the root of the sd card. In "SDCARD" mode the Rom may not fully fit on RAM, chunks of the ROM are cached and loaded as needed, you can control the size of this cache with by chaning "ROM_CACHE_SIZE", default = 10. SDCARD mode may have some stutter for roms that switch between banks too often.
* "FLASH": Rom is stored in the flash of the Pi Pico. The rom size is limited by the amount of flash available in the Pi Pico 2 (approx 3.5mb).
* "PSRAM": Load rom from SDCARD into PSRAM if it is available (Pimoroni Pico Plus 2).
 

 # Display drivers
 The emulator supports different displays thru the `mipidsi` library. To enable the settings for your display set the `DISPLAY_DRIVER` paramter from the environment variables files to point to the correct driver.
#### List of supported models
* GC9A01 = mipidsi::models::GC9A01
* ILI9341 = mipidsi::models::ILI9341Rgb565
* ILI9342C = mipidsi::models::ILI9342CRgb565
* ILI9486 = mipidsi::models::ILI9486Rgb565
* ST7735 = mipidsi::models::ST7735s
* ST7789 = mipidsi::models::ST7789
* ST7796 = mipidsi::models::ST7796


# Preparing the SD card
The SD card is used to store game roms and save game progress. For this project, you will need a FAT 32 formatted Micro SD card with roms you legally own. Roms must have the .gb extension.

* Insert your SD card in a Windows computer and format it as FAT 32
* Copy your .gb file to the SD card root folder (subfolders are not supported at this time)
* Insert the SD card into the SD card slot.

