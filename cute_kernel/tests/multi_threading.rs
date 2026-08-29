#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(cute_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use alloc::vec;
use bootloader_api::config::Mapping;
use bootloader_api::{BootInfo, BootloaderConfig, entry_point};
use core::panic::PanicInfo;
use cute_kernel::multi_tasking::premtive::thread;
use cute_kernel::{halt_loop, println};

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

entry_point!(main, config = &BOOTLOADER_CONFIG);

fn main(boot_info: &'static mut BootInfo) -> ! {
    use cute_kernel::allocator;
    use cute_kernel::memory::{self, BootFrameAllocator};
    use x86_64::VirtAddr;

    cute_kernel::init();
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset.into_option().unwrap());
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootFrameAllocator::init(&boot_info.memory_regions) };
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    test_main();
    loop {}
}

fn fake_task() {
    halt_loop();
}

#[test_case]
fn create_many_tasks() {
    let mut alloc = vec![];
    for _i in 0..20 {
        alloc.push(thread(fake_task));
    }
}
