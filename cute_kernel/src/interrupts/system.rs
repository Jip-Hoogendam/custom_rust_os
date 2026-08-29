use super::InterruptStackFrame;
use crate::println;

/// The task switch function that gets run on a timer interupt.
#[unsafe(naked)]
pub extern "C" fn task_switch() {
    core::arch::naked_asm!(
        r"
        push rax
        push rcx
        push rdx
        push rsi
        push rdi
        push rbp
        push rbx
        push r8
        push r9
        push r10
        push r11
        push r12
        push r13
        push r14
        push r15

        mov rdi, rsp
        call schedule
        mov rsp, rax
   
        // call notfiy_end_of_timer_interrupt
        pop r15
        pop r14
        pop r13
        pop r12
        pop r11
        pop r10
        pop r9
        pop r8
        pop rbx
        pop rbp
        pop rdi
        pop rsi
        pop rdx
        pop rcx
        pop rax
        iretq
        "
    );
}

/// A breakpoint call for a program.
pub extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("EXCEPTION: BREAKPOINT\n{}", stack_frame);
}
