use core::{arch::asm, mem::transmute, time::Duration};

use crate::{
    acpi::apic::LOCAL_APIC,
    arch::instructions::int3,
    interrupts::interupt_gaurd::{InteruptAlreadyBlocked, InteruptGaurd},
    multi_tasking::{
        ScheduleStackCall,
        scheduler::{SCHEDULER, SchedulerResult},
    },
    serial_println,
    sync::mutex::SysMutex,
    tables::idt::InteruptDiscriptorIndex,
    tools::{cpu_id, sleep},
};

#[unsafe(no_mangle)]
pub extern "C" fn syscall_handeler(stack: *mut ScheduleStackCall) -> *mut ScheduleStackCall {
    let interupt_gaurd = InteruptGaurd::<InteruptAlreadyBlocked>::block();
    LOCAL_APIC.set_timer_intial_count(0, &interupt_gaurd);
    let call_num = unsafe { transmute::<u64, SysCalls>((&*stack).rdi) };

    let resolved = false;

    match call_num {
        SysCalls::Read => todo!("Impl read"),
        SysCalls::Open => todo!("create open file"),
        SysCalls::Write => todo!("WriteFile"),
        SysCalls::Close => todo!("Close file"),
        SysCalls::MtxLock => SCHEDULER.await_resource(
            cpu_id(),
            unsafe {
                ((&*stack).rsi as *const u8 as *mut SysMutex)
                    .as_mut()
                    .unwrap()
            },
            &interupt_gaurd,
        ),
    }

    if resolved {
        stack
    } else {
        SCHEDULER.next(stack, &interupt_gaurd)
    }
}

#[derive(Debug)]
#[repr(usize)]
pub enum SysCalls {
    ///These syscalls are based off linux
    Read = 0,
    Write = 1,
    Open = 2,
    Close = 3,

    ///Beyond linux syscalls for inter kernal use only
    MtxLock = 500,
}

#[inline]
#[must_use]
pub fn syscall(
    syscall: SysCalls,
    mut var_1: usize,
    mut var_2: usize,
    mut var_3: usize,
) -> (usize, usize, usize, usize) {
    let mut syscall = syscall as usize;
    unsafe {
        asm! {"int {x}", x = const(InteruptDiscriptorIndex::SysCall as u8), inout("rdi") syscall, inout("rsi") var_1, inout("rdx") var_2, inout("rcx") var_3}
    }
    (syscall, var_1, var_2, var_3)
}
