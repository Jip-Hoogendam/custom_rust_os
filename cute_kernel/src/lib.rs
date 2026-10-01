#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(abi_x86_interrupt)]
#![feature(ptr_metadata)]
#![feature(must_not_suspend)]
#![feature(portable_simd)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]

extern crate alloc;

pub mod acpi;
pub mod allocator;
pub mod arch;
pub mod collections;
pub mod display;
pub mod files;
pub mod interrupts;
pub mod memory;
pub mod msr;
pub mod multi_processors;
pub mod multi_tasking;
pub mod serial;
pub mod sync;
pub mod syscall;
pub mod tables;
pub mod tasks;
pub mod time;
pub mod tools;
pub mod variables;

use core::{fmt::Display, panic::PanicInfo};

/// This enum is used to denote diffrent operating systems privalage levels.
/// Diffrent rings have diffrent acess to systems.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum PrivalageLevel {
    /// The kernel lives in this level
    Ring0 = 0,
    /// To be used
    Ring1 = 1,
    /// To be used
    Ring2 = 2,
    /// Userland to be used
    Ring3 = 3,
}

impl Display for PrivalageLevel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Ring0 => "Ring-0",
                Self::Ring1 => "Ring-1",
                Self::Ring2 => "Ring-2",
                Self::Ring3 => "Ring-3",
            }
        )
    }
}

pub static DT: spin::Mutex<DiscriptorTables> = spin::Mutex::new(DiscriptorTables::init());
/// Iniltilizies the kernel or application prosessor. Loading the `DiscriptorTables` and ACIP
/// tables.
///
/// # Panics
/// panics if any `DiscriptorTables` is full.
pub fn init<T>(interupt_gaurd: &InteruptGaurd<T>)
where
    T: InteruptController,
{
    #[cfg(feature = "qemu-test")]
    {
        serial_println!("CuteKernelInit");
    }
    //---- setup gdt
    let mut dt = DT.lock();
    // Kernal code segment
    let kernel_code_segment = dt
        .get_mut_gdt()
        .append_segment_discriptor(
            *SegmentDiscriptor::new_segment(0, usize::MAX).set_executable(true),
        )
        .expect("GDT not large enough");
    // Kernal data segment
    let kernel_data_segment = dt
        .get_mut_gdt()
        .append_segment_discriptor(
            *SegmentDiscriptor::new_segment(0, usize::MAX).set_executable(false),
        )
        .expect("GDT not large enough");
    // Userspace code segment
    let _userspace_code_segment = dt
        .get_mut_gdt()
        .append_segment_discriptor(
            *SegmentDiscriptor::new_segment(0, usize::MAX)
                .set_executable(true)
                .set_privalage_level(PrivalageLevel::Ring3),
        )
        .expect("GDT not large enough");
    // Userspace data segment
    let _userspace_data_segment = dt
        .get_mut_gdt()
        .append_segment_discriptor(
            *SegmentDiscriptor::new_segment(0, usize::MAX)
                .set_executable(false)
                .set_privalage_level(PrivalageLevel::Ring3),
        )
        .expect("GDT not large enough");

    // Task state segment
    let task_state_segment = SystemSegmentDiscriptor::new_tss(dt.get_mut_tss());
    let task_state_segment = dt
        .get_mut_gdt()
        .append_system_discriptor(&task_state_segment)
        .expect("GDT not large enough");

    interrupts::load_interupts(&mut dt, kernel_code_segment, interupt_gaurd);
    dt.get_mut_idt().load(interupt_gaurd);

    dt.get_mut_gdt().load();
    // Load registers
    kernel_code_segment.set_as_cs_register();
    kernel_data_segment.set_as_ds_register();
    task_state_segment.set_as_tss_register();
}

pub fn halt_loop() -> ! {
    loop {
        arch::instructions::hlt();
    }
}

//[!TESTING SETUP]
pub trait Testable {
    fn run(&self);
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    halt_loop();
}

#[cfg(test)]
#[cfg(test)]
use crate::{
    acpi::{
        SDTables,
        apic::{APICClockDivider, LOCAL_APIC},
        ioapic::IOAPIC,
        load_sd_table,
    },
    display::init_writer,
    interrupts::interupt_gaurd::BlockInterupts,
    memory::addresses::PhysMemOffset,
    multi_tasking::scheduler,
    tables::idt::InteruptDiscriptorIndex,
};
use crate::{
    interrupts::interupt_gaurd::{InteruptController, InteruptGaurd},
    tables::{
        DiscriptorTables,
        gdt::{SegmentDiscriptor, SystemSegmentDiscriptor},
    },
};
#[cfg(test)]
use crate::{
    memory::pageing::MemoryMapper,
    multi_processors::{init_proscessors, write_trampoline},
    time::set_system_clock_source_8253pit,
};
#[cfg(test)]
use bootloader_api::{BootInfo, BootloaderConfig, config::Mapping, entry_point, info::Optional};

/// This function is called on panic.
#[cfg(not(feature = "qemu-test"))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    halt_loop();
}

//this funciton is called on panics in tests
#[cfg(feature = "qemu-test")]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}

#[cfg(test)]
pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

#[cfg(test)]
entry_point!(test_kernel_main, config = &BOOTLOADER_CONFIG);

/// Entry point for `cargo test`
#[cfg(test)]
fn test_kernel_main(boot_info: &'static mut BootInfo) -> ! {
    let phys_mem_offset = PhysMemOffset::get(boot_info);
    let mut sd_tables = unsafe { load_sd_table(boot_info, phys_mem_offset) }.unwrap();

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
        init(g);
    });

    println!("Init boot frame allocator");

    println!("Init heap");
    allocator::init_heap(&mut memory_mapper).expect("heap allocation failed");

    InteruptGaurd::<BlockInterupts>::scope(|g| {
        println!("Setting apic timer");
        mdat.init_apic(phys_mem_offset, &LOCAL_APIC, g);
        LOCAL_APIC.set_timer_div(APICClockDivider::Div128, g);
        LOCAL_APIC.set_timer_interupt_vector(
            InteruptDiscriptorIndex::TaskSwitch,
            acpi::apic::APICTimerMode::OneShot,
            g,
        );

        LOCAL_APIC.set_timer_intial_count(10000, g);

        println!("Initlilizing Scheduler");
        scheduler::SCHEDULER.init(4, g);
    });

    println!("Setting up io apic");
    mdat.init_io_apic(phys_mem_offset, IOAPIC.lock());
    let keyboard_opt = mdat.get_irq_options(acpi::ioapic::IRQ::Keyboard);
    IOAPIC.lock().set_irq_physical(
        keyboard_opt,
        InteruptDiscriptorIndex::Keyboard,
        LOCAL_APIC.getid(),
    );

    println!("Setting up system clock");
    set_system_clock_source_8253pit();
    let pit_opt = mdat.get_irq_options(acpi::ioapic::IRQ::PIT);
    IOAPIC.lock().set_irq_physical(
        pit_opt,
        InteruptDiscriptorIndex::ClockTick,
        LOCAL_APIC.getid(),
    );

    write_trampoline(&mut memory_mapper);

    init_proscessors(
        mdat,
        &LOCAL_APIC,
        boot_info.kernel_stack_bottom,
        boot_info.kernel_stack_len,
        &mut memory_mapper,
    )
    .expect("Multiple prossessors failed to start");

    test_main();
    halt_loop();
}

//QEMU config

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use arch::port::Port;

    let mut port = Port::new(0xf4);
    port.write(exit_code as u32);
}
