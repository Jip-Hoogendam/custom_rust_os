use core::{
    arch::{asm, global_asm},
    ptr::copy_nonoverlapping,
    time::Duration,
};

use alloc::vec;

use crate::{
    DT,
    acpi::{
        apic::{APIC, APICClockDivider, LOCAL_APIC},
        madt::{MADT, record::MADTRecord},
    },
    interrupts::interupt_gaurd::{BlockInterupts, InteruptGaurd},
    memory::{
        addresses::{Address, PhysAddr, VirtAddr},
        frames::PhysFrame,
        pageing::{MemoryMapper, MemoryMapperError, page_table::PageEntryOption},
    },
    multi_tasking::executor::Executor,
    println,
    tables::{gdt::SystemSegmentDiscriptor, idt::InteruptDiscriptorIndex},
    tools::{cpu_id, sleep},
};

const PAE_MASK: u32 = 0b0110_0110_1000;

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

    InteruptGaurd::<BlockInterupts>::scope(|g| {
        LOCAL_APIC.setup_local(g);

        LOCAL_APIC.set_timer_div(APICClockDivider::Div128, g);
        LOCAL_APIC.set_timer_interupt_vector(
            InteruptDiscriptorIndex::TaskSwitch,
            crate::acpi::apic::APICTimerMode::OneShot,
            g,
        );

        LOCAL_APIC.set_timer_intial_count(10000, g);
    });

    println!("[CPU : {}] : UP", cpu_id());
    let mut executor = Executor::default();
    executor.run();
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

/// # Panics
/// This function can panic if the page allocator fails to allocate certain frames
pub fn write_trampoline(mapper: &mut MemoryMapper) {
    mapper
        .allocate_range(
            VirtAddr::new(TRAMPOLINE_ADDRESS as usize),
            PageEntryOption::ReadWrite | PageEntryOption::Present,
            PhysFrame::new(PhysAddr::new(TRAMPOLINE_ADDRESS as usize)),
            PhysFrame::new(PhysAddr::new(TRAMPOLINE_ADDRESS as usize + 0x1FFF)),
        )
        .expect("Expect 0x8000 to be writable");

    unsafe {
        copy_nonoverlapping(
            &raw const ap_trampoline_start,
            (TRAMPOLINE_ADDRESS) as *mut u8,
            (&raw const ap_trampoline_end).sub((&raw const ap_trampoline_start) as usize) as usize,
        );
    }
}

/// # Errors
/// Will retrun a `FrameAllocationFailed` if the allocator fails to allocate the frames
pub fn page_stack(
    start: VirtAddr,
    size: usize,
    memory_mapper: &mut MemoryMapper,
) -> Result<(), MemoryMapperError> {
    memory_mapper.allocate_bytes(
        start,
        PageEntryOption::Present | PageEntryOption::ReadWrite,
        size,
    )?;
    Ok(())
}

/// # Panics
/// This function may panic if the frame allocation fails
///
/// # Errors
/// This function returns errors if the memory mapper fails to allocate the stacks for the ap's
pub fn init_proscessors(
    mdat: &MADT,
    local_apic: &APIC,
    kernel_stack_bottom: u64,
    kernel_stack_size: u64,
    memory_mapper: &mut MemoryMapper,
) -> Result<(), MemoryMapperError> {
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
        #[allow(clippy::cast_possible_truncation)]
        core::ptr::write_volatile((PAGE_ADDRESS) as *mut u32, cr3 as u32);
        core::ptr::write_volatile((IDT) as *mut u64, &raw const idtr as u64);
    }

    let local_apic_id = local_apic.getid();
    let mut other_apics = vec![];
    for record in mdat.records() {
        #[allow(clippy::cast_possible_truncation)]
        if let MADTRecord::LocalAPIC(apic) = record
            && apic.id != local_apic_id as u8
            && apic.can_be_used()
        {
            other_apics.push(apic);
            let offset = kernel_stack_size * u64::from(apic.id);
            let start = kernel_stack_bottom + offset;
            println!("start : {:#x}, size {:#x}", start, offset);

            page_stack(
                VirtAddr::new(start as usize),
                kernel_stack_size as usize,
                memory_mapper,
            )?;
        }
    }

    // Ipi code based off https://wiki.osdev.org/Symmetric_Multiprocessing#BSP_Initialization_Code
    for apic in other_apics {
        let target = u32::from(apic.id);
        local_apic.init_ipi(target);
        local_apic.await_delivery();
        local_apic.deassert(target);
        local_apic.await_delivery();

        sleep(Duration::from_millis(100));

        for _ in 0..2 {
            local_apic.clear_errors();
            local_apic.send_startup_ipi(target, TRAMPOLINE_ADDRESS);
            // I need to create a sleep function
            sleep(Duration::from_millis(10));

            local_apic.await_delivery();
        }
    }
    Ok(())
}
