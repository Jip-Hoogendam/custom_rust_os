#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(owos_prime::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::{BootInfo, entry_point};
use core::{arch::asm, panic::PanicInfo};
use owos_prime::{
    halt_loop, memory,
    multi_tasking::{cooprative::Task, executor::Executor, premtive::thread},
    println,
    tasks::keyboard,
};
use x86_64::VirtAddr;

entry_point!(kernel_main);

fn test_tread() -> ! {
    println!("test :/");
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    println!("Hello World{}", "!");

    owos_prime::init();

    use owos_prime::allocator;
    use owos_prime::memory::BootFrameAllocator;

    let mut frame_allocator = unsafe { BootFrameAllocator::init(&boot_info.memory_map) };
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap allocation failed");

    #[cfg(test)]
    test_main();

    let five = thread(test_tread);

    let mut executor = Executor::default();
    executor.spawn(Task::new(keyboard::print_keypresses()));
    executor.run();
}

/// This function is called on panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    halt_loop();
}

//this funciton is called on panics in tests
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    owos_prime::test_panic_handler(info)
}
