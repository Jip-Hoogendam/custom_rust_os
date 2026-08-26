#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(abi_x86_interrupt)]
#![warn(clippy::all)]
// enable these for fun !
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]

extern crate alloc;

pub mod acpi;
pub mod allocator;
pub mod display;
pub mod interrupts;
pub mod memory;
pub mod msr;
pub mod multi_tasking;
pub mod serial;
pub mod tables;
pub mod tasks;
pub mod variables;

use core::{fmt::Display, panic::PanicInfo};

/// This enum is used to denote diffrent operating systems privalage levels.
/// Diffrent rings have diffrent acess to systems.
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
                PrivalageLevel::Ring0 => "Ring-0",
                PrivalageLevel::Ring1 => "Ring-1",
                PrivalageLevel::Ring2 => "Ring-2",
                PrivalageLevel::Ring3 => "Ring-3",
            }
        )
    }
}

pub static DT: spin::Mutex<DiscriptorTables> = spin::Mutex::new(DiscriptorTables::init());
pub fn init() {
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
    let userspace_code_segment = dt
        .get_mut_gdt()
        .append_segment_discriptor(
            *SegmentDiscriptor::new_segment(0, usize::MAX)
                .set_executable(true)
                .set_privalage_level(PrivalageLevel::Ring3),
        )
        .expect("GDT not large enough");
    // Userspace data segment
    let userspace_data_segment = dt
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

    interrupts::load_interupts(&mut dt, kernel_code_segment);
    dt.get_mut_idt().load();

    dt.get_mut_gdt().load();
    // Load registers
    kernel_code_segment.set_as_cs_register();
    kernel_data_segment.set_as_ds_register();
    task_state_segment.set_as_tss_register();
}

pub fn halt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
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
use bootloader_api::{BootInfo, entry_point};

use crate::tables::{
    DiscriptorTables,
    gdt::{SegmentDiscriptor, SystemSegmentDiscriptor},
};

#[cfg(test)]
entry_point!(test_kernel_main);

/// Entry point for `cargo test`
#[cfg(test)]
#[unsafe(no_mangle)]
fn test_kernel_main(_boot_info: &'static mut BootInfo) -> ! {
    init();
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
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}
