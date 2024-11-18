use core::{
    cell::UnsafeCell,
    marker::PhantomData,
    sync::atomic::{compiler_fence, Ordering},
};
use rp235x_hal::rom_data;

#[repr(C)]
struct FlashFunctionPointers<'a> {
    connect_internal_flash: unsafe extern "C" fn() -> (),
    flash_exit_xip: unsafe extern "C" fn() -> (),
    flash_range_erase:
        unsafe extern "C" fn(addr: u32, count: usize, block_size: u32, block_cmd: u8) -> (),
    flash_range_program: unsafe extern "C" fn(addr: u32, data: *const u8, count: usize) -> (),
    flash_flush_cache: unsafe extern "C" fn() -> (),
    flash_enter_cmd_xip: unsafe extern "C" fn() -> (),
    flash_op: unsafe extern "C" fn(flags: u32, addr: u32, block_size: u32, data: *mut u8) -> i32,
    phantom: PhantomData<&'a ()>,
}

unsafe fn flash_function_pointers_with_boot2(boot2: &[u32; 64]) -> FlashFunctionPointers {
    let boot2_fn_ptr = (boot2 as *const u32 as *const u8).offset(1);
    let boot2_fn: unsafe extern "C" fn() -> () = core::mem::transmute(boot2_fn_ptr);

    FlashFunctionPointers {
        connect_internal_flash: rom_data::connect_internal_flash::ptr(),
        flash_exit_xip: rom_data::flash_exit_xip::ptr(),
        flash_range_erase: rom_data::flash_range_erase::ptr(),
        flash_range_program: rom_data::flash_range_program::ptr(),
        flash_flush_cache: rom_data::flash_flush_cache::ptr(),
        flash_op: rom_data::flash_op::ptr(),
        flash_enter_cmd_xip: boot2_fn,
        phantom: PhantomData,
    }
}

unsafe fn get_boot2_copy() -> [u32; BOOT2_SIZE_WORDS] {
    let mut boot2 = [0u32; BOOT2_SIZE_WORDS];
    let boot_rom: &[u32] = unsafe {
        alloc::slice::from_raw_parts(BOOT_ROM_ADDRESS as *const u32, BOOT2_SIZE_WORDS as usize)
    };
    boot2.copy_from_slice(boot_rom);
    boot2
}

#[inline(never)]
#[link_section = ".data.ram_func"]
pub unsafe fn flash_range_erase_and_program(addr: u32, data: &mut [u8]) -> i32 {
    let boot2 = get_boot2_copy();
    let boot_rom_pointers = flash_function_pointers_with_boot2(&boot2);

    compiler_fence(Ordering::SeqCst);

    (boot_rom_pointers.connect_internal_flash)();
    (boot_rom_pointers.flash_exit_xip)();

    const DEFAULT_FLASH_OP_FLAGS: u32 = (CFLASH_SECLEVEL_VALUE_SECURE << CFLASH_SECLEVEL_LSB)
        | (CFLASH_ASPACE_VALUE_RUNTIME << CFLASH_ASPACE_LSB);

    let erase_result = (boot_rom_pointers.flash_op)(
        (CFLASH_OP_VALUE_ERASE << CFLASH_OP_LSB) | DEFAULT_FLASH_OP_FLAGS,
        addr,
        data.len() as u32,
        core::ptr::null_mut(),
    );
    if erase_result != 0 {
        (boot_rom_pointers.flash_flush_cache)();
        (boot_rom_pointers.flash_enter_cmd_xip)();
        return erase_result;
    }

    let program_result = (boot_rom_pointers.flash_op)(
        (CFLASH_OP_VALUE_PROGRAM << CFLASH_OP_LSB) | DEFAULT_FLASH_OP_FLAGS,
        addr,
        data.len() as u32,
        data.as_mut_ptr(),
    );
    (boot_rom_pointers.flash_flush_cache)();
    (boot_rom_pointers.flash_enter_cmd_xip)();
    if program_result != 0 {
        return erase_result;
    }
    erase_result
}

//Boot rom constants
const BOOT_ROM_ADDRESS: usize = 0x400e0000;
const BOOT2_SIZE_WORDS: usize = 64;

// Address space related constants
pub const CFLASH_ASPACE_LSB: u32 = 0;
pub const CFLASH_ASPACE_VALUE_RUNTIME: u32 = 1;

// Security level related constants
pub const CFLASH_SECLEVEL_LSB: u32 = 8;
// Zero is not a valid security level
pub const CFLASH_SECLEVEL_VALUE_SECURE: u32 = 1;

// Operation related constants
pub const CFLASH_OP_LSB: u32 = 16;

// Erase size_bytes bytes of flash, starting at address addr.
// Both addr and size_bytes must be a multiple of 4096 bytes (one flash sector).
pub const CFLASH_OP_VALUE_ERASE: u32 = 0;

// Program size_bytes bytes of flash, starting at address addr.
// Both addr and size_bytes must be a multiple of 256 bytes (one flash page).
pub const CFLASH_OP_VALUE_PROGRAM: u32 = 1;

pub const FLASH_SECTOR_SIZE: u32 = 1 << 12;

#[repr(C, align(4096))]
pub struct FlashBlock<const SIZE: usize> {
    pub data: UnsafeCell<[u8; SIZE]>,
}

impl<const SIZE: usize> FlashBlock<SIZE> {
    #[inline(never)]
    pub fn addr(&self) -> u32 {
        &self.data as *const _ as u32
    }

    #[inline(never)]
    pub fn read(&self) -> &'static [u8; SIZE] {
        let addr = self.addr();
        unsafe { &*(*(addr as *const Self)).data.get() }
    }

    pub unsafe fn write_flash(
        &self,
        offset: u32,
        data: &mut [u8; FLASH_SECTOR_SIZE as usize],
    ) -> (u32, i32) {
        let addr = self.addr() + (FLASH_SECTOR_SIZE * offset);
        let result = cortex_m::interrupt::free(|_cs| flash_range_erase_and_program(addr, data));
        (addr, result)
    }
}

unsafe impl<const SIZE: usize> Sync for FlashBlock<SIZE> {}
