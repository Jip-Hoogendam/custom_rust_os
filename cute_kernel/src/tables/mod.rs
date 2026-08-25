use lazy_static::lazy_static;

use crate::{
    PrivalageLevel, println,
    tables::{
        gdt::{
            GlobalDescriptorTableBuilder, SegmentDiscriptor, SegmentSelector,
            SystemSegmentDiscriptor,
        },
        tss::{DOUBLE_FAULT_IST_INDEX, TaskStateSegment},
    },
};

/// A structure to save the Segment selectors and GDT in.
struct GDTSelectors {
    gdt: GlobalDescriptorTableBuilder<{ crate::variables::GDT_TABLE_SIZE }>,
    tss: TaskStateSegment,
    kernel_code_segment: SegmentSelector,
    kernel_data_segment: SegmentSelector,
    userspace_code_segment: SegmentSelector,
    userspace_data_segment: SegmentSelector,
    task_state_segment: SegmentSelector,
}

lazy_static! {
static ref GDT: GDTSelectors = {

    let mut tss = TaskStateSegment::new();
    tss.set_interupt_stack(DOUBLE_FAULT_IST_INDEX, {
        const STACK_SIZE: usize = 4096 * 5;
        static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
        unsafe { (&raw const STACK).add(STACK_SIZE) as u64 }
    });

    let mut gdt = GlobalDescriptorTableBuilder::new();
    // Kernal code segment
    let mut kernel_code_segment = SegmentDiscriptor::new_segment(0, usize::MAX);
    kernel_code_segment.set_executable(true);
    let kernel_code_segment = gdt.append_segment_discriptor(&kernel_code_segment).expect("GDT not large enough");
    // Kernal data segment
    let mut kernel_data_segment = SegmentDiscriptor::new_segment(0, usize::MAX);
    kernel_data_segment.set_executable(false);
    let kernel_data_segment = gdt.append_segment_discriptor(&kernel_data_segment).expect("GDT not large enough");

    // Userspace code segment
    let mut userspace_code_segment = SegmentDiscriptor::new_segment(0, usize::MAX);
    userspace_code_segment.set_executable(true).set_privalage_level(PrivalageLevel::Ring3);
    let userspace_code_segment = gdt.append_segment_discriptor(&userspace_code_segment).expect("GDT not large enough");
    // Userspace data segment
    let mut userspace_data_segment = SegmentDiscriptor::new_segment(0, usize::MAX);
    userspace_data_segment.set_executable(false).set_privalage_level(PrivalageLevel::Ring3);
    let userspace_data_segment =  gdt.append_segment_discriptor(&userspace_data_segment).expect("GDT not large enough");

    // Task state segment
    let task_state_segment = SystemSegmentDiscriptor::new_tss(&tss);
    let task_state_segment = gdt.append_system_discriptor(&task_state_segment).expect("GDT not large enough");


    GDTSelectors{
        gdt,
        tss,
        kernel_code_segment,
        kernel_data_segment,
        userspace_code_segment,
        userspace_data_segment,
        task_state_segment,
    }
};
}

pub fn init() {
    println!("Loading GDT!");
    GDT.gdt.load();
    GDT.kernel_code_segment.set_as_cs_register();
    GDT.kernel_data_segment.set_as_ds_register();
    GDT.task_state_segment.set_as_tss_register();
}

pub mod gdt;
/// A module to interact with the InteruptDiscriptorTable.
/// This only suppports 64 bit long mode interupt discriptors.
pub mod idt;
pub mod tss;
