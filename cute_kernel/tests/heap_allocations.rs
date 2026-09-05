#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(cute_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader_api::config::Mapping;
use bootloader_api::{BootInfo, BootloaderConfig, entry_point};
use core::panic::PanicInfo;
use cute_kernel::interrupts::interupt_gaurd::{BlockInterupts, InteruptGaurd};
use cute_kernel::memory::addresses::PhysMemOffset;
use cute_kernel::{println, serial_println};

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

// add a `config` argument to the `entry_point` macro call
entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    use cute_kernel::allocator;
    use cute_kernel::memory::{self, BootFrameAllocator};

    InteruptGaurd::<BlockInterupts>::scope(|g| {
        cute_kernel::init(g);
    });

    let phys_mem_offset = PhysMemOffset::get(boot_info);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootFrameAllocator::init(&boot_info.memory_regions) };
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    test_main();
    loop {}
}

use alloc::boxed::Box;
use alloc::vec::{self, Vec};
use cute_kernel::allocator::HEAP_SIZE;

#[test_case]
fn simple_allocation() {
    let heap_value_1 = Box::new(41);
    let heap_value_2 = Box::new(13);
    assert_eq!(*heap_value_1, 41);
    assert_eq!(*heap_value_2, 13);
}

#[test_case]
fn large_vec() {
    let n = 1000;
    let mut vec = Vec::new();
    for i in 0..n {
        vec.push(i);
    }
    assert_eq!(vec.iter().sum::<u64>(), (n - 1) * n / 2);
}

#[test_case]
fn many_boxes() {
    let mut vec = Vec::new();
    for i in 0..100 {
        vec.push(i);
    }

    for i in 0..(HEAP_SIZE / 100) {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
    assert_eq!(vec.iter().sum::<u64>(), (100 - 1) * 100 / 2);
}
