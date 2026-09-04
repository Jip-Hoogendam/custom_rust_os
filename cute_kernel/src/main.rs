#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(cute_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use core::ops::Add;

use bootloader_api::{BootInfo, entry_point, info::Optional};
use cute_kernel::{
    acpi::{
        SDTables,
        apic::{APICClockDivider, LOCAL_APIC},
        ioapic::IOAPIC,
        load_sd_table,
        madt::record::LocalAPIC,
    },
    display::init_writer,
    interrupts::interupt_gaurd::{BlockInterupts, InteruptGaurd},
    memory::{self},
    multi_processors::{ap_mode_startup, init_proscessors, write_trampoline},
    multi_tasking::{self, cooprative::Task, executor::Executor, scheduler},
    println,
    sync::Mutex,
    tables::idt::InteruptDiscriptorIndex,
    tasks::keyboard,
};
use x86_64::VirtAddr;

use bootloader_api::config::{BootloaderConfig, Mapping};

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
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
        println!("Virtual memory start {:#x}", boot_info.kernel_image_offset);
        println!("Virtual memory start {:#x}", boot_info.kernel_stack_len);
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

    InteruptGaurd::<BlockInterupts>::scope(|g| {
        cute_kernel::init(g);
    });

    use cute_kernel::allocator;
    use cute_kernel::memory::BootFrameAllocator;

    let mut frame_allocator = unsafe { BootFrameAllocator::init(&boot_info.memory_regions) };
    println!("Init boot frame allocator");

    println!("get phys mem offset");
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    println!("memory mapper initlized");
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap allocation failed");
    println!("Heap created");

    InteruptGaurd::<BlockInterupts>::scope(|g| {
        scheduler::SCHEDULER.init(4, g);
        println!("Scheduler initlized");
        println!("Setting apic timer");
        mdat.init_apic(phys_mem_offset.as_u64(), &LOCAL_APIC, g);
        println!("Setting up io apic");
        mdat.init_io_apic(phys_mem_offset.as_u64(), IOAPIC.lock());
        IOAPIC.lock().set_irq_physical(
            cute_kernel::acpi::ioapic::IRQ::Keyboard,
            InteruptDiscriptorIndex::Keyboard,
            LOCAL_APIC.getid(),
        );
        println!("APIC init!");
        LOCAL_APIC.set_timer_div(APICClockDivider::Div128, g);
        LOCAL_APIC.set_timer_interupt_vector(
            InteruptDiscriptorIndex::TaskSwitch,
            cute_kernel::acpi::apic::APICTimerMode::OneShot,
            g,
        );

        write_trampoline(&mut mapper, &mut frame_allocator);
        init_proscessors(
            mdat,
            &LOCAL_APIC,
            boot_info.kernel_stack_bottom,
            boot_info.kernel_stack_len,
            &mut mapper,
            &mut frame_allocator,
        );
    });

    #[cfg(test)]
    test_main();

    println!("Creating Async executor");
    let mut executor = Executor::default();
    executor
        .spawn(Task::new(keyboard::print_keypresses()))
        .expect("Could not create keyboard runner");
    println!("Launchinig into executor");
    executor.run();
}
