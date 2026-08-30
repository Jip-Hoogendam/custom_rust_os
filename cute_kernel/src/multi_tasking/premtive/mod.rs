use crate::{
    interrupts::interupt_gaurd::{BlockInterupts, InteruptGaurd},
    multi_tasking::{TaskId, premtive::tcb::ThreadOptions, scheduler::SCHEDULER},
    println,
    tools::cpu_id,
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
    let id = InteruptGaurd::<BlockInterupts>::scope(|interupt_gaurd| {
        SCHEDULER.schedule(cpu_id(), tcb, interupt_gaurd)
    });
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
    SCHEDULER.kill_current_task(cpu_id());
}
