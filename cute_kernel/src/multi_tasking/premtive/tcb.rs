use core::ptr::null_mut;

use alloc::boxed::Box;

use crate::multi_tasking::premtive::{stack::Stack, thread_exit};

#[derive(Debug)]
pub struct Thread {
    stack: Stack,
    pub rsp: u64,
}

//the context saved in a interupt so whe can just "subscribe" the thread to the
//scheduler. This means that in a context switch one time it will get picked up and get called!
#[repr(C)]
struct RingZeroInteruptContext {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rbx: u64,
    rbp: u64,
    rdi: u64,
    rsi: u64,
    rdx: u64,
    rcx: u64,
    rax: u64,

    //pushed by cpu
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,

    unwind: u64, //unwind function
}

const KERNEL_CS: u64 = 0x08;
const RFLAGS_IF: u64 = 1 << 9;

extern "C" fn trampoline(arg: *mut u8) {
    unsafe {
        Box::from_raw(arg as *mut Box<dyn FnOnce() + Send>)();
    }
}

impl Thread {
    #[inline]
    pub fn init_root() -> Self {
        Self {
            stack: Stack {
                begin: null_mut(),
                end: null_mut(),
            },
            rsp: 0,
        }
    }

    pub fn spawn(stack: Stack, capture: *mut Box<dyn FnOnce() + Send>) -> Thread {
        let sp = ((stack.end as usize) & !0xF) as *mut u8;

        //setup the stack of the thread
        let init_state = unsafe {
            sp.byte_sub(core::mem::size_of::<RingZeroInteruptContext>())
                as *mut RingZeroInteruptContext
        };

        unsafe {
            (*init_state).rax = 0;
            (*init_state).rbx = 0;
            (*init_state).rcx = 0;
            (*init_state).rdx = 0;
            (*init_state).rsi = 0;
            (*init_state).rdi = capture as *mut u8 as u64;
            (*init_state).rbp = 0;
            (*init_state).r8 = 0;
            (*init_state).r9 = 0;
            (*init_state).r10 = 0;
            (*init_state).r11 = 0;
            (*init_state).r12 = 0;
            (*init_state).r13 = 0;
            (*init_state).r14 = 0;
            (*init_state).r15 = 0;

            //Some special flags for the interupt
            (*init_state).rflags = RFLAGS_IF;
            (*init_state).cs = KERNEL_CS;
            (*init_state).rsp = sp.byte_sub(8) as usize as u64;
            (*init_state).ss = 0x0;

            //unwind function
            (*init_state).unwind = thread_exit as *const () as u64;

            //entry point
            (*init_state).rip = trampoline as *const () as u64;
        }

        // println!(
        //     "pointer =  {:?}\nstack ptr = {:?}, \nhead ={:?},\nprobe = {:#x}\nunwind = {:#x}",
        //     (*init_state).rip as *const (),
        //     sp,
        //     init_state,
        //     (((init_state as *mut u64).add(2)) as *mut u8) as usize,
        //     (*init_state).unwind
        // );
        Thread {
            stack,
            rsp: (init_state as u64),
        }
    }

    pub fn init_fn(stack: Stack, entry: fn()) -> Thread {
        let sp = ((stack.end as usize) & !0xF) as *mut u8;

        //setup the stack of the thread
        let init_state = unsafe {
            sp.byte_sub(core::mem::size_of::<RingZeroInteruptContext>())
                as *mut RingZeroInteruptContext
        };

        unsafe {
            (*init_state).rax = 0;
            (*init_state).rbx = 0;
            (*init_state).rcx = 0;
            (*init_state).rdx = 0;
            (*init_state).rsi = 0;
            (*init_state).rdi = 0;
            (*init_state).rbp = 0;
            (*init_state).r8 = 0;
            (*init_state).r9 = 0;
            (*init_state).r10 = 0;
            (*init_state).r11 = 0;
            (*init_state).r12 = 0;
            (*init_state).r13 = 0;
            (*init_state).r14 = 0;
            (*init_state).r15 = 0;

            //Some special flags for the interupt
            (*init_state).rflags = RFLAGS_IF;
            (*init_state).cs = KERNEL_CS;
            (*init_state).rsp = sp.byte_sub(8) as usize as u64;
            (*init_state).ss = 0x0;

            //unwind function
            (*init_state).unwind = thread_exit as *const () as u64;

            //entry point
            (*init_state).rip = entry as *const () as u64;
        }

        // println!(
        //     "pointer =  {:?}\nstack ptr = {:?}, \nhead ={:?},\nprobe = {:#x}\nunwind = {:#x}",
        //     (*init_state).rip as *const (),
        //     sp,
        //     init_state,
        //     (((init_state as *mut u64).add(2)) as *mut u8) as usize,
        //     (*init_state).unwind
        // );
        Thread {
            stack,
            rsp: (init_state as u64),
        }
    }
}
