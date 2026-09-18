#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(cute_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use core::time::Duration;

use bootloader_api::{BootInfo, entry_point, info::Optional};
use cute_kernel::{
    acpi::{
        SDTables,
        apic::{APICClockDivider, LOCAL_APIC},
        ioapic::IOAPIC,
        load_sd_table,
    },
    display::init_writer,
    interrupts::interupt_gaurd::{BlockInterupts, InteruptGaurd},
    memory::{addresses::PhysMemOffset, pageing::MemoryMapper},
    multi_processors::{init_proscessors, write_trampoline},
    multi_tasking::{cooprative::Task, executor::Executor, premtive::spawn, scheduler},
    println,
    tables::idt::InteruptDiscriptorIndex,
    tasks::keyboard,
    time::{Instant, set_system_clock_source_8253pit},
    tools::{cpu_id, sleep},
};

use bootloader_api::config::{BootloaderConfig, Mapping};

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

// add a `config` argument to the `entry_point` macro call
entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    let phys_mem_offset = PhysMemOffset::get(boot_info);
    let sd_tables = unsafe { load_sd_table(boot_info, phys_mem_offset) }.unwrap();

    if let bootloader_api::info::Optional::Some(framebuffer) = &mut boot_info.framebuffer {
        init_writer(framebuffer);
    }

    println!("Hello World!");
    if let Optional::Some(memory_offset) = boot_info.physical_memory_offset {
        println!("Virtual memory start {:#x}", memory_offset);
    }

    let mut memory_mapper = MemoryMapper::init(&boot_info.memory_regions, phys_mem_offset)
        .expect("Expect memory mapper to be made");

    println!("Load MDAT");
    let mdat = if let Some(SDTables::MADT(mdat)) = sd_tables
        .tables_iter(phys_mem_offset)
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

    println!("Init boot frame allocator");

    println!("Init heap");
    allocator::init_heap(&mut memory_mapper).expect("heap allocation failed");

    InteruptGaurd::<BlockInterupts>::scope(|g| {
        println!("Setting apic timer");
        mdat.init_apic(phys_mem_offset, &LOCAL_APIC, g);
        LOCAL_APIC.set_timer_div(APICClockDivider::Div128, g);
        LOCAL_APIC.set_timer_interupt_vector(
            InteruptDiscriptorIndex::TaskSwitch,
            cute_kernel::acpi::apic::APICTimerMode::OneShot,
            g,
        );

        println!("Setting up io apic");
        mdat.init_io_apic(phys_mem_offset, IOAPIC.lock());
        let keyboard_opt = mdat.get_irq_options(cute_kernel::acpi::ioapic::IRQ::Keyboard);
        IOAPIC.lock().set_irq_physical(
            keyboard_opt,
            InteruptDiscriptorIndex::Keyboard,
            LOCAL_APIC.getid(),
        );

        println!("Setting up system clock");
        set_system_clock_source_8253pit();
        let pit_opt = mdat.get_irq_options(cute_kernel::acpi::ioapic::IRQ::PIT);
        IOAPIC.lock().set_irq_physical(
            pit_opt,
            InteruptDiscriptorIndex::ClockTick,
            LOCAL_APIC.getid(),
        );

        println!("Initlilizing Scheduler");
        scheduler::SCHEDULER.init(4, g);

        LOCAL_APIC.set_timer_intial_count(10000, g);
    });

    write_trampoline(&mut memory_mapper);

    init_proscessors(
        mdat,
        &LOCAL_APIC,
        boot_info.kernel_stack_bottom,
        boot_info.kernel_stack_len,
        &mut memory_mapper,
    )
    .expect("Multiple prossessors failed to start");

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
