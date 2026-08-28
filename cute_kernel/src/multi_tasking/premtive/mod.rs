use crate::{
    multi_tasking::{TaskId, premtive::tcb::ThreadOptions, scheduler::SCHEDULER},
    println,
    variables::KERNEL_THREAD_STACK_SIZE,
};

pub mod stack;
pub mod tcb;
use alloc::boxed::Box;
use tcb::Thread;
use x86_64::instructions::interrupts::enable_and_hlt;

unsafe extern "C" {
    pub unsafe fn context_restore(old_rsp: *mut *mut u8, new_rsp: *mut *mut u8);
}

/// This function is used to trampoline into a lamda capture
extern "C" fn trampoline(arg: u64) {
    unsafe {
        Box::from_raw((arg as *mut u64).cast::<Box<dyn FnOnce() + Send>>())();
    }
}

pub enum ThreadError {
    NoPlaceInQueue,
    InitFailed,
    EmptyStack,
}

#[derive(Debug)]
pub struct ThreadHandle {
    id: TaskId,
}

#[inline]
fn schedule(entry: *const (), options: &ThreadOptions) -> ThreadHandle {
    let stack = stack::Stack::new(KERNEL_THREAD_STACK_SIZE);
    let tcb = Thread::spawn(stack, entry, options);
    x86_64::instructions::interrupts::disable();
    let id = SCHEDULER.lock().schedule(tcb);
    x86_64::instructions::interrupts::enable();
    ThreadHandle { id }
}

pub fn spawn(f: impl FnOnce() + 'static + Send) -> ThreadHandle {
    let closure =
        alloc::boxed::Box::new(alloc::boxed::Box::new(f) as alloc::boxed::Box<dyn FnOnce() + Send>);
    let capture = alloc::boxed::Box::into_raw(closure);
    schedule(
        trampoline as *const (),
        &ThreadOptions {
            argument_0: capture as usize,
            ..Default::default()
        },
    )
}

pub fn thread(entry: fn()) -> ThreadHandle {
    schedule(entry as *const (), &ThreadOptions::default())
}

//gets called when a thread has ended
pub extern "C" fn thread_exit() -> ! {
    SCHEDULER.lock().kill_current_task();
    loop {
        enable_and_hlt();
    }
}
