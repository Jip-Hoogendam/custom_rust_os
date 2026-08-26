#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(cute_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader_api::{BootInfo, entry_point, info::Optional};
use core::{panic::PanicInfo, ptr::null_mut};
use cute_kernel::{
    acpi::{SDTables, load_sd_table},
    display::init_writer,
    memory::{self},
    multi_tasking::{self, cooprative::Task, executor::Executor, scheduler},
    println,
    tables::DiscriptorTables,
    tasks::keyboard,
};
use x86_64::{VirtAddr, instructions::interrupts::int3};

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
    if let Optional::Some(memory_offset) = boot_info.physical_memory_offset {
        println!("Virtual memory start {:#x}", memory_offset);
    }

    let phys_mem_offset = VirtAddr::new(
        boot_info
            .physical_memory_offset
            .into_option()
            .expect("No offset provided!"),
    );

    println!("Load SDT");
    let sd_tables = unsafe {
        load_sd_table(
            boot_info
                .rsdp_addr
                .into_option()
                .expect("Expected a rsdt pointer to be passed by bootloader"),
            phys_mem_offset,
        )
    }
    .unwrap();

    println!("Load MDAT");
    let mdat = if let Some(SDTables::MADT(mdat)) = sd_tables
        .tables_iter(phys_mem_offset.as_u64())
        .find(|x| matches!(x, SDTables::MADT(_)))
    {
        mdat
    } else {
        todo!("Load no multi prosessing or APICS");
    };

    let apic = unsafe { mdat.get_local_apic(phys_mem_offset.as_u64()) };

    x86_64::instructions::interrupts::disable();
    cute_kernel::init();
    x86_64::instructions::interrupts::enable();

    use cute_kernel::allocator;
    use cute_kernel::memory::BootFrameAllocator;

    let mut frame_allocator = unsafe { BootFrameAllocator::init(&boot_info.memory_regions) };
    println!("Init boot frame allocator");

    println!("get phys mem offset");
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    println!("memory mapper initlized");
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap allocation failed");
    println!("Heap created");

    {
        scheduler::SCHEDULER.lock().init();
    }

    println!("Scheduler initlized");

    #[cfg(test)]
    test_main();

    multi_tasking::premtive::spawn(move || {
        for c in 0..100 {
            println!("this is a test ! {}", c);
        }
    });

    println!("Creating Async executor");
    let mut executor = Executor::default();
    executor.spawn(Task::new(keyboard::print_keypresses()));
    println!("Launchinig into executor");
    executor.run();
}

/// This function is called on panic.
// #[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    cute_kernel::halt_loop();
}

//this funciton is called on panics in tests
// #[cfg(test)]
// #[panic_handler]
// fn panic(info: &PanicInfo) -> ! {
//     cute_kernel::test_panic_handler(info)
// }
