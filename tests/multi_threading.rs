#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(owos_prime::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use alloc::vec;
use bootloader::{BootInfo, entry_point};
use core::panic::PanicInfo;
use owos_prime::multi_tasking::premtive::thread;
use owos_prime::{halt_loop, println};

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

fn fake_task() -> ! {
    halt_loop();
}

#[test_case]
fn create_many_tasks() {
    let mut alloc = vec![];
    for _i in 0..20 {
        alloc.push(thread(fake_task));
    }

    //we dont have a real wait yet :/
    for _i in 0..1000000 {}
}

