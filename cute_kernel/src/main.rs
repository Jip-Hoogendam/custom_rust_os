#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(cute_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader_api::{BootInfo, entry_point};
use core::panic::PanicInfo;
use cute_kernel::{
    display::init_writer,
    memory::{self},
    multi_tasking::{cooprative::Task, executor::Executor, scheduler},
    println,
    tasks::keyboard,
};
use x86_64::VirtAddr;

use bootloader_api::config::{BootloaderConfig, Mapping};

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config.mappings.framebuffer = Mapping::Dynamic;
    config
};

// add a `config` argument to the `entry_point` macro call
entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    if let bootloader_api::info::Optional::Some(framebuffer) = &mut boot_info.framebuffer {
        init_writer(framebuffer);
    }

    println!("Hello World!");

    x86_64::instructions::interrupts::disable();
    cute_kernel::init();
    x86_64::instructions::interrupts::enable();

    use cute_kernel::allocator;
    use cute_kernel::memory::BootFrameAllocator;

    let mut frame_allocator = unsafe { BootFrameAllocator::init(&boot_info.memory_regions) };
    let phys_mem_offset = VirtAddr::new(
        boot_info
            .physical_memory_offset
            .into_option()
            .expect("No offset provided!"),
    );
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap allocation failed");

    {
        scheduler::SCHEDULER.lock().init();
    }

    #[cfg(test)]
    test_main();

    println!("{}", unsafe { *(0x000E0000 as *mut char) });

    // multi_tasking::premtive::spawn(move || {
    //     let ebda = 0x40E as *mut [char; 1024];
    //     for c in unsafe { *ebda } {
    //         println!("{}", c);
    //     }
    // });

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
    cute_kernel::halt_loop();
}

//this funciton is called on panics in tests
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    cute_kernel::test_panic_handler(info)
}
