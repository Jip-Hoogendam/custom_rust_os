use core::arch::asm;

use crate::{
    interrupts::interupt_gaurd::{InteruptAlreadyBlocked, InteruptGaurd},
    multi_tasking::scheduler::SCHEDULER,
    tools::cpu_id,
};

use super::ExceptionStackFrame;

/// Page faults get called when refrenceing a page that is not allowed to be acessed at this level.
pub extern "x86-interrupt" fn page_fault_handeler(
    stack_frame: ExceptionStackFrame,
    error_code: u64,
) -> ! {
    let cr2: usize;
    unsafe {
        asm!("mov {}, cr2", out(reg) cr2, options(nomem, nostack, preserves_flags));
    }

    panic!(
        r"EXCEPTION: PAGE FAULT \n Acessed Address: {cr2:#x} \n Error Code: {error_code:?} \n {stack_frame} \n Task : \n{:?}",
        SCHEDULER.get_current_task_id(cpu_id(), &InteruptGaurd::<InteruptAlreadyBlocked>::block())
    );
}

/// Double faults get called when a previus interupt can not be handeld.
pub extern "x86-interrupt" fn double_fault_handler(
    stack_frame: ExceptionStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{stack_frame}");
}
