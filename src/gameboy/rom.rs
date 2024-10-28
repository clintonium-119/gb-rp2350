use crate::hal::timer::Instant;
use core::cell::RefCell;

use crate::hal::timer::TimerDevice;
use alloc::{
    boxed::Box,
    string::{String, ToString},
};
use const_lru::ConstLru;
use defmt::info;
pub struct SdRomManager<
    'a,
    D: embedded_sdmmc::BlockDevice,
    T: embedded_sdmmc::TimeSource,
    DT: TimerDevice,
    const MAX_DIRS: usize,
    const MAX_FILES: usize,
    const MAX_VOLUMES: usize,
> {
    rom_name: String,
    root_dir: RefCell<embedded_sdmmc::Directory<'a, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>>,
    bank_0: Box<[u8; 0x4000]>,
    bank_lru: RefCell<ConstLru<usize, Box<[u8; 0x4000]>, 9, u8>>,
    start_time: Instant,
    timer: crate::hal::Timer<DT>,
}
impl<
        'a,
        D: embedded_sdmmc::BlockDevice,
        T: embedded_sdmmc::TimeSource,
        DT: TimerDevice,
        const MAX_DIRS: usize,
        const MAX_FILES: usize,
        const MAX_VOLUMES: usize,
    > SdRomManager<'a, D, T, DT, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
{
    pub fn new(
        rom_name: &str,
        mut root_dir: embedded_sdmmc::Directory<'a, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>,
        timer: crate::hal::Timer<DT>,
    ) -> Self {
        let mut rom_file = root_dir
            .open_file_in_dir(rom_name, embedded_sdmmc::Mode::ReadOnly)
            .unwrap();
        let mut bank_0 = Box::new([0u8; 0x4000]);
        rom_file.seek_from_start(0u32).unwrap();
        rom_file.read(&mut *bank_0).unwrap();
        rom_file.close().unwrap();
        let lru = ConstLru::new();
        let result: SdRomManager<'a, D, T, DT, MAX_DIRS, MAX_FILES, MAX_VOLUMES> = Self {
            rom_name: rom_name.to_string(),
            bank_0: bank_0,
            root_dir: RefCell::new(root_dir),
            bank_lru: RefCell::new(lru),
            start_time: timer.get_counter(),
            timer,
        };

        result
    }
    fn read_bank(&self, bank_offset: usize) -> Box<[u8; 0x4000]> {
        let mut binding = self.root_dir.borrow_mut();
        let mut file = binding
            .open_file_in_dir(self.rom_name.as_str(), embedded_sdmmc::Mode::ReadOnly)
            .unwrap();

        let mut buffer: Box<[u8; 0x4000]> = Box::new([0u8; 0x4000]);

        file.seek_from_start(bank_offset as u32).unwrap();
        file.read(&mut *buffer).unwrap();

        file.close().unwrap();
        buffer
    }
}
impl<
        'a,
        D: embedded_sdmmc::BlockDevice,
        T: embedded_sdmmc::TimeSource,
        DT: TimerDevice,
        const MAX_DIRS: usize,
        const MAX_FILES: usize,
        const MAX_VOLUMES: usize,
    > gb_core::hardware::rom::RomManager
    for SdRomManager<'a, D, T, DT, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
{
    fn read_from_offset(&self, seek_offset: usize, index: usize) -> u8 {
        if seek_offset == 0x0000 {
            return self.bank_0[index as usize];
        }
        let mut bank_lru = self.bank_lru.borrow_mut();
        let bank = bank_lru.get(&seek_offset);
        let value = match bank {
            Some(buffer) => buffer[index],
            None => {
                info!("LOADING BANK: {}", index);
                let buffer: Box<[u8; 0x4000]> = self.read_bank(seek_offset);
                let result = buffer[index];
                bank_lru.insert(seek_offset, buffer);
                result
            }
        };
        value
    }

    fn clock(&self) -> u64 {
        let current_time = self.timer.get_counter();
        let diff = current_time - self.start_time;
        diff.to_micros()
    }

    fn save(&mut self, game_title: &str, bank_index: u8, bank: &[u8]) {
        let mut root_directory = self.root_dir.borrow_mut();

        if root_directory.find_directory_entry("saves").is_err() {
            root_directory.make_dir_in_dir("saves").unwrap();
        }
        let mut game_dir_name = game_title.replace(" ", "").to_lowercase();
        game_dir_name.truncate(game_dir_name.len().min(8));

        let mut save_directory = root_directory.open_dir("saves").unwrap();
        if save_directory
            .find_directory_entry(game_dir_name.as_str())
            .is_err()
        {
            save_directory
                .make_dir_in_dir(game_dir_name.as_str())
                .unwrap();
        }
        let mut game_directory = save_directory.open_dir(game_dir_name.as_str()).unwrap();

        let mut bank_file = game_directory
            .open_file_in_dir(
                alloc::format!("{}", bank_index).as_str(),
                embedded_sdmmc::Mode::ReadWriteCreateOrTruncate,
            )
            .unwrap();

        bank_file.write(bank).unwrap();
    }

    fn load_to_bank(&mut self, game_title: &str, bank_index: u8, bank: &mut [u8]) {
        let mut root_directory = self.root_dir.borrow_mut();

        if root_directory.find_directory_entry("saves").is_err() {
            root_directory.make_dir_in_dir("saves").unwrap();
        }
        let mut game_dir_name = game_title.replace(" ", "").to_lowercase();
        game_dir_name.truncate(game_dir_name.len().min(8));

        let mut save_directory = root_directory.open_dir("saves").unwrap();
        if save_directory
            .find_directory_entry(game_dir_name.as_str())
            .is_err()
        {
            save_directory
                .make_dir_in_dir(game_dir_name.as_str())
                .unwrap();
        }
        let mut game_directory = save_directory.open_dir(game_dir_name.as_str()).unwrap();

        let bank_name = alloc::format!("{}", bank_index);
        if game_directory
            .find_directory_entry(bank_name.as_str())
            .is_ok()
        {
            let mut bank_file = game_directory
                .open_file_in_dir(bank_name.as_str(), embedded_sdmmc::Mode::ReadOnly)
                .unwrap();

            bank_file.read(bank).unwrap();
        }
    }
}
impl<
        'a,
        D: embedded_sdmmc::BlockDevice,
        T: embedded_sdmmc::TimeSource,
        DT: TimerDevice,
        const MAX_DIRS: usize,
        const MAX_FILES: usize,
        const MAX_VOLUMES: usize,
    > core::ops::Index<usize> for SdRomManager<'a, D, T, DT, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
{
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        &self.bank_0[index as usize]
    }
}
impl<
        'a,
        D: embedded_sdmmc::BlockDevice,
        T: embedded_sdmmc::TimeSource,
        DT: TimerDevice,
        const MAX_DIRS: usize,
        const MAX_FILES: usize,
        const MAX_VOLUMES: usize,
    > core::ops::Index<core::ops::Range<usize>>
    for SdRomManager<'a, D, T, DT, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
{
    type Output = [u8];

    fn index(&self, index: core::ops::Range<usize>) -> &Self::Output {
        return &self.bank_0[index];
    }
}
