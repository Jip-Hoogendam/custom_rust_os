use crate::multi_tasking::{TaskId, scheduler::SCHEDULER};

pub mod stack;
pub mod tcb;
use tcb::Thread;
use x86_64::instructions::interrupts::enable_and_hlt;

unsafe extern "C" {
    pub unsafe fn context_restore(old_rsp: *mut *mut u8, new_rsp: *mut *mut u8);
}

pub enum ThreadError {
    NoPlaceInQueue,
    InitFailed,
}

#[derive(Debug)]
pub struct ThreadHandle {
    id: TaskId,
}

pub fn spawn(f: impl FnOnce() + 'static + Send) -> ThreadHandle {
    let closure =
        alloc::boxed::Box::new(alloc::boxed::Box::new(f) as alloc::boxed::Box<dyn FnOnce() + Send>);
    let capture = alloc::boxed::Box::into_raw(closure);
    let stack = stack::Stack::new(1024 * 16);
    let tcb = Thread::spawn(stack, capture);
    x86_64::instructions::interrupts::disable();
    let id = SCHEDULER.lock().schedule(tcb);
    x86_64::instructions::interrupts::enable();
    ThreadHandle { id }
}

//Initilizes a new thread.
pub fn thread(entry: fn()) -> ThreadHandle {
    let stack = stack::Stack::new(1024 * 16);
    let tcb = Thread::init_fn(stack, entry);
    x86_64::instructions::interrupts::disable();
    let id = SCHEDULER.lock().schedule(tcb);
    x86_64::instructions::interrupts::enable();
    ThreadHandle { id }
}

//gets called when a thread has ended
pub extern "C" fn thread_exit() -> ! {
    SCHEDULER.lock().kill_current_task();
    loop {
        enable_and_hlt();
    }
}
