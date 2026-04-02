#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(owos_prime::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::{BootInfo, entry_point};
use core::panic::PanicInfo;
use owos_prime::println;

entry_point!(main);

fn main(boot_info: &'static BootInfo) -> ! {
    use owos_prime::allocator;
    use owos_prime::memory::{self, BootFrameAllocator};
    use x86_64::VirtAddr;

    owos_prime::init();
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootFrameAllocator::init(&boot_info.memory_map) };
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    test_main();
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    owos_prime::test_panic_handler(info)
}

use alloc::boxed::Box;
use alloc::vec::{self, Vec};
use owos_prime::allocator::HEAP_SIZE;

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

    for i in 0..HEAP_SIZE {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
    assert_eq!(vec.iter().sum::<u64>(), (100 - 1) * 100 / 2);
}
