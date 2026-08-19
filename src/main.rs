#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(owos_prime::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::{BootInfo, entry_point};
use core::panic::PanicInfo;
use owos_prime::{
    memory,
    multi_tasking::{self, cooprative::Task, executor::Executor, premtive::thread, scheduler},
    println,
    tasks::keyboard,
};
use x86_64::VirtAddr;

entry_point!(kernel_main);

fn test_tread() {
    println!("test thread ran!");
}

fn test_tread_2() {
    println!("test thread 2 ran!");
    loop {}
}

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    println!("Hello World!");

    x86_64::instructions::interrupts::disable();
    owos_prime::init();
    x86_64::instructions::interrupts::enable();

    use owos_prime::allocator;
    use owos_prime::memory::BootFrameAllocator;

    let mut frame_allocator = unsafe { BootFrameAllocator::init(&boot_info.memory_map) };
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap allocation failed");

    {
        scheduler::SCHEDULER.lock().init();
    }

    #[cfg(test)]
    test_main();

    for i in 0..100 {
        multi_tasking::premtive::spawn(move || {
            loop {
                println!("thread {i} ran !")
            }
        });
    }

    let mut executor = Executor::default();
    executor.spawn(Task::new(keyboard::print_keypresses()));
    println!("Running execuor");
    executor.run();
}

/// This function is called on panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    owos_prime::halt_loop();
}

//this funciton is called on panics in tests
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    owos_prime::test_panic_handler(info)
}
