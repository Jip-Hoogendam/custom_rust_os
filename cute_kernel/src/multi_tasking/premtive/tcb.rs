use core::{mem::MaybeUninit, ptr};

use crate::{
    interrupts::InterruptStackFrame,
    multi_tasking::premtive::{stack::Stack, thread_exit},
    tables::gdt::SegmentSelector,
};

//the context saved in a interupt so whe can just "subscribe" the thread to the
//scheduler. This means that in a context switch one time it will get picked up and get called!
#[repr(C, packed)]
struct RingZeroInteruptContext {
    r15: usize,
    r14: usize,
    r13: usize,
    r12: usize,
    r11: usize,
    r10: usize,
    r9: usize,
    r8: usize,
    rbx: usize,
    rbp: usize,
    rdi: usize,
    rsi: usize,
    rdx: usize,
    rcx: usize,
    rax: usize,

    interupt_stack: InterruptStackFrame,

    unwind: usize, //unwind function
}

const RFLAGS_IF: usize = 1 << 9;

/// Allows 6 options to be passed to the function that runs the thread according to the x86 calling
/// convention.
#[derive(Default)]
pub struct ThreadOptions {
    /// These are not in array to make creation and calling easyer
    pub argument_0: usize,
    pub argument_1: usize,
    pub argument_2: usize,
    pub argument_3: usize,
    pub argument_4: usize,
    pub argument_5: usize,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct Thread {
    stack: Stack,
    pub rsp: u64,
}

impl Thread {
    #[must_use]
    pub const fn init_root() -> Self {
        Self {
            stack: Stack::null_stack(),
            rsp: 0,
        }
    }

    #[must_use]
    pub fn spawn(mut stack: Stack, function: *const (), options: &ThreadOptions) -> Self {
        let mut stack_default: MaybeUninit<usize> = MaybeUninit::new(0);
        let sp = ptr::from_mut(stack.get_base().unwrap_or(&mut stack_default)).cast::<u8>();

        //setup the stack of the thread
        let init_state = unsafe {
            sp.byte_sub(core::mem::size_of::<RingZeroInteruptContext>())
                .cast::<RingZeroInteruptContext>()
        };

        unsafe {
            (*init_state).rax = 0;
            (*init_state).rbx = 0;
            (*init_state).rcx = options.argument_3;
            (*init_state).rdx = options.argument_2;
            (*init_state).rsi = options.argument_1;
            (*init_state).rdi = options.argument_0;
            (*init_state).rbp = 0;
            (*init_state).r8 = options.argument_4;
            (*init_state).r9 = options.argument_5;
            (*init_state).r10 = 0;
            (*init_state).r11 = 0;
            (*init_state).r12 = 0;
            (*init_state).r13 = 0;
            (*init_state).r14 = 0;
            (*init_state).r15 = 0;

            //Some special flags for the interupt
            (*init_state).interupt_stack.flags = RFLAGS_IF;
            (*init_state).interupt_stack.cs = SegmentSelector::from_cs().into();
            (*init_state).interupt_stack.sp = sp.byte_sub(8) as usize;
            (*init_state).interupt_stack.ss = SegmentSelector::from_ss().into();

            //unwind function
            (*init_state).unwind = thread_exit as *const () as usize;

            //entry point
            (*init_state).interupt_stack.ip = function as usize;
        }

        Self {
            stack,
            rsp: (init_state as u64),
        }
    }
}
