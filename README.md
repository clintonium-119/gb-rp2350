# Gameboy Emulator for the Pi Pico 2

This is a Gameboy emulator for the Pi Pico 2 completely written on Rust. The emulator is still a work in progress and optimizations are still being worked on but many games should be playable and accurate.

Supported Features:
* Save game support to SD Card.
* Loading games from SD Card or directly from flash.
* Running games bigger than the available memory of the Pi Pico.
* Sound support.
* Screen scaler, the image of the GB is scaled to different screens.

Pending Features:
* Game selection menu.
* Performance improvements.
* Gameboy color support.


# Hardware
## What you need
* (1x) [Raspberry Pi Pico](https://a.co/d/44gvGwD)
* (1x) [2.8inch ILI9341 240X320 LCD Display Module](https://a.co/d/auhlHku)
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

# Installing the firmware
1. Install the latest stable version of Rust.
2. Then use `rustup` to grab the Rust Standard Library for the appropriate targets.
`rustup target add riscv32imac-unknown-none-elf`
3. Push and hold the BOOTSEL button on the Pico, then connect to your computer using a micro USB cable. Release BOOTSEL once the drive RPI-RP2 appears on your computer.
4. Build the gameboy emulator using `cargo build --release`.
5. Set up the `.env` file to set the ROM configuration. Use `.env.example` as your reference.
5. Drag and drop the UF2 file on to the RPI-RP2 drive. The Raspberry Pi Pico will reboot and will now run the emulator. The location of the UF2 should be `target/thumbv8m.main-none-eabihf/release/gb-rp2350`.



# Preparing the SD card
The SD card is used to store game roms and save game progress. For this project, you will need a FAT 32 formatted Micro SD card with roms you legally own. Roms must have the .gb extension.

* Insert your SD card in a Windows computer and format it as FAT 32
* Copy your .gb file to the SD card root folder (subfolders are not supported at this time)
* Insert the SD card into the SD card slot.

