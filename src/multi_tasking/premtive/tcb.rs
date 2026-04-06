use crate::{multi_tasking::premtive::stack::Stack, println};

#[repr(C)]
pub struct TCB {
    stack: Stack,
    pub rsp: *mut u8,
}

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
}

const KERNEL_CS: u64 = 0x08;
const RFLAGS_IF: u64 = 1 << 9;

impl TCB {
    pub fn init(stack: Stack, entry: fn() -> !) -> TCB {
        let sp = ((stack.end as usize) & !0xF) as *mut u8;

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

            (*init_state).rflags = RFLAGS_IF;
            (*init_state).cs = KERNEL_CS;
            (*init_state).rsp = stack.end as usize as u64;
            (*init_state).ss = 0x0;

            //entry point
            (*init_state).rip = entry as *const () as u64;

            println!(
                "pointer =  {:?}\n stack ptr = {:?}, head ={:?}, probe = {:#x}",
                (*init_state).rip as *const (),
                sp,
                init_state,
                (((init_state as *mut u64).add(2)) as *mut u8) as usize
            );
            TCB {
                stack,
                rsp: (init_state as *mut u8),
            }
        }
    }
}
