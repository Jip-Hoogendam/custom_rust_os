use core::{
    arch::{asm, global_asm, naked_asm},
    hint::spin_loop,
    ptr::{addr_of, copy_nonoverlapping, read_volatile},
};

use alloc::vec;
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{
        FrameAllocator, Mapper, Page, PageTableFlags, PhysFrame, Size4KiB, mapper::MapToError,
    },
};

use crate::{
    DT,
    acpi::{
        apic::APIC,
        madt::{MADT, record::MADTRecord},
    },
    interrupts::interupt_gaurd::{self, BlockInterupts, InteruptGaurd},
    memory::get_l4_table_phys_ptr,
    println, serial_print,
    tables::{gdt::SystemSegmentDiscriptor, idt::InteruptDiscriptorTable},
    tools::cpu_id,
};

const PAE_MASK: u32 = 0b0110_0110_1000;
const PAGING_MASK: usize = 1 << 31;
const NOT_PAGING_MASK: usize = !PAGING_MASK;

// based off the intilization code from https://wiki.osdev.org/Symmetric_Multiprocessing#AP_Initialization_Code
// This code is fragil and contains hard addresses distances to locations. This is because it starts
// in real mode and so this code may have trouble linking to locations dynamically.
global_asm!(
    r#"
    .global ap_trampoline_start
    .global ap_trampoline_end

    .code16
ap_trampoline_start:
        cli
        cld
        ljmp 0x0,  0x8040 
        .align 16
_L8010_GDT_table:
        .long 0, 0
        .long 0x0000FFFF, 0x00CF9A00
        .long 0x0000FFFF, 0x008F9200
        .long 0x0000FFFF, 0x00AF9A00
        .long 0x0000FFFF, 0x00AF9200
_L8038_GDT_value:
        .word _L8038_GDT_value - _L8010_GDT_table - 1
        .long 0x8010
        .long 0, 0
        .align 64

_L8040:
        xor ax, ax
        mov ds, ax
        lgdt [0x8038]   // load basic gdt
        lidt [0x8010]   // Load null idt using null page of gdt kind of hacky
        mov eax, cr0
        or eax, 1       // enable protected mode
        mov cr0, eax
        ljmp 8, 0x80a0  //jump to protected mode

        .align 32
        .code32
_L80a0:

        mov ax, 16 // load intial ss and ds
        mov ds, ax
        mov ss, ax

        // enable PAE 
        mov eax, cr4
        or eax, {pae_mask} 
        mov cr4, eax

        // set cr3 addr
        mov eax, dword ptr [{cr3_copy}]
        mov cr3, eax

        //set EFER.LME
        mov ecx, 0xC0000080
        rdmsr
        or eax, 0b1001 << 8 
        wrmsr

        // enable paging
        mov ebx, 0x80010033
        mov cr0, ebx


        ljmp 24, 0x8100

        .code64
        .align 64
_L8100:
        mov rax, 32 // load long ds and ss
        mov ds, rax
        mov ss, rax
        mov es, rax
        mov fs, rax
        mov gs, rax

        mov eax, 1
        cpuid
        shr rbx, 24
        mov rdi, rbx

        mov rax, qword ptr [{stack_size}]
        add rdi, 1
        mul rdi
        mov rsp, qword ptr [{stack_top}]
        add rsp, rax

        mov ecx, 0xC0000080
        rdmsr


        mov rax, qword ptr [{ap_startup}]
        call rax

    noo: 
        nop
        jmp noo
ap_trampoline_end:
    ///{idt}
        "#,
    pae_mask = const(PAE_MASK),
    cr3_copy = const(PAGE_ADDRESS),
    stack_size = const(STACK_SIZE),
    stack_top = const(STACK_TOP),
    idt = const(IDT),
    ap_startup = const(AP_STARTUP),
);

static TEST_VALUE: u64 = 123;

/// # Panics
/// This function will panic if the GDT was not setup correctly
pub extern "C" fn ap_mode_startup() -> ! {
    {
        let gaurd = InteruptGaurd::<BlockInterupts>::block();
        let mut dt = DT.lock();
        let (cs, ds) = {
            let gdt = dt.get_mut_gdt();
            (
                gdt.find_code_segment(crate::PrivalageLevel::Ring0)
                    .expect("AP needs access to the CS"),
                gdt.find_data_segment(crate::PrivalageLevel::Ring0)
                    .expect("AP needs access to the DS"),
            )
        };

        // Task state segment
        let task_state_segment = SystemSegmentDiscriptor::new_tss(dt.get_mut_tss());
        let task_state_segment = dt
            .get_mut_gdt()
            .append_system_discriptor(&task_state_segment)
            .expect("GDT not large enough");

        dt.get_mut_idt().load(&gaurd);
        dt.get_mut_gdt().load();

        cs.set_as_cs_register();
        ds.set_as_ds_register();
        task_state_segment.set_as_tss_register();
    }

    println!("[CPU : {}] : UP", cpu_id());

    loop {
        spin_loop();
    }
}

unsafe extern "C" {
    static ap_trampoline_start: u8;
    static ap_trampoline_end: u8;
}

const TRAMPOLINE_ADDRESS: u32 = 0x8000;
const STACK_TOP: u64 = 0x8a10;
const STACK_SIZE: u64 = 0x8a20;
const AP_STARTUP: u64 = 0x8a30;
const PAGE_ADDRESS: u64 = 0x8a40;
const IDT: u64 = 0x8a50;

pub fn write_trampoline(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    // page lower end
    let page_range = {
        let trampoline_start = VirtAddr::new(TRAMPOLINE_ADDRESS as u64);
        let trampoline_end = trampoline_start + 0x0000_0FFF;
        let trampoline_start_page = Page::containing_address(trampoline_start);
        let trampoline_end_page = Page::containing_address(trampoline_end);
        Page::range_inclusive(trampoline_start_page, trampoline_end_page)
    };

    for page in page_range {
        let frame = PhysFrame::containing_address(PhysAddr::new(page.start_address().as_u64()));
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            mapper
                .map_to(page, frame, flags, frame_allocator)
                .unwrap()
                .flush();
        }
    }

    unsafe {
        copy_nonoverlapping(
            &raw const ap_trampoline_start,
            (TRAMPOLINE_ADDRESS) as *mut u8,
            (&raw const ap_trampoline_end).sub((&raw const ap_trampoline_start) as usize) as usize,
        );
    }
}

pub fn page_stack(
    start: usize,
    size: usize,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let stack_start = VirtAddr::new(start as u64);
        let stack_end = stack_start + size as u64 - 1u64;
        println!("End! {:#0x} ", stack_end.as_u64());
        let stack_start_page = Page::containing_address(stack_start);
        let stack_end_page = Page::containing_address(stack_end);
        Page::range_inclusive(stack_start_page, stack_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush();
        }
    }
    Ok(())
}

pub fn init_proscessors(
    mdat: &MADT,
    local_apic: &APIC,
    kernel_stack_bottom: u64,
    kernel_stack_size: u64,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    let idtr = {
        let mut dt = DT.lock();
        let idt = dt.get_mut_idt();
        idt.get_get_idtr()
    };
    //Write refrence data
    unsafe {
        core::ptr::write_volatile((STACK_SIZE) as *mut u64, kernel_stack_size);
        core::ptr::write_volatile((STACK_TOP) as *mut u64, kernel_stack_bottom);
        core::ptr::write_volatile(
            (AP_STARTUP) as *mut u64,
            ap_mode_startup as *const () as u64,
        );
        let cr3: u64;
        asm!("mov {cr3:r}, cr3", cr3 = out(reg) cr3);
        let pointer = cr3;
        core::ptr::write_volatile((PAGE_ADDRESS) as *mut u32, pointer as u32);
        core::ptr::write_volatile((IDT) as *mut u64, &raw const idtr as u64);
    }

    let local_apic_id = local_apic.getid();
    let mut other_apics = vec![];
    for record in mdat.records() {
        if let MADTRecord::LocalAPIC(apic) = record
            && apic.id != local_apic_id as u8
            && apic.can_be_used()
        {
            other_apics.push(apic);
            let offset = kernel_stack_size * apic.id as u64;
            let start = kernel_stack_bottom + offset;
            println!("start : {:#x}, size {:#x}", start, offset);

            page_stack(
                start as usize,
                kernel_stack_size as usize,
                mapper,
                frame_allocator,
            )
            .expect("Failed to allocate page for kernel");
        }
    }

    // Ipi code based off https://wiki.osdev.org/Symmetric_Multiprocessing#BSP_Initialization_Code
    for apic in other_apics {
        let target = u32::from(apic.id);
        local_apic.init_ipi(target);
        local_apic.await_delivery();
        local_apic.deassert(target);
        local_apic.await_delivery();

        // I dont have delays yet ...
        for _ in 0..100000 {}

        for _ in 0..2 {
            local_apic.clear_errors();
            local_apic.send_startup_ipi(target, TRAMPOLINE_ADDRESS);
            // I need to create a sleep function
            for _ in 0..10000 {}
            local_apic.await_delivery();
        }
    }
}
