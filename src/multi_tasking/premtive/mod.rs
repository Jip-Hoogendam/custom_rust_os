use crate::{
    multi_tasking::scheduler::{MAX_THREAD_COUNT, TASK_QUEUE},
    println,
};

pub mod stack;
pub mod tcb;
use tcb::TCB;

unsafe extern "C" {
    pub unsafe fn context_restore(old_rsp: *mut *mut u8, new_rsp: *mut *mut u8);
}

pub enum ThreadError {
    NoPlaceInQueue,
}

pub struct ThreadHandle {
    tcb: TCB,
}

pub fn thread(entry: fn() -> !) -> Result<ThreadHandle, ThreadError> {
    let stack = stack::Stack::new(1024 * 16);
    let tcb = TCB::init(stack, entry);

    x86_64::instructions::interrupts::disable();
    'outer: {
        let mut task_queue = TASK_QUEUE.lock();
        for task_queue_ptr in 0..MAX_THREAD_COUNT {
            if task_queue[task_queue_ptr] == 0 {
                task_queue[task_queue_ptr] = tcb.rsp as usize;
                break 'outer;
            }
        }
        return Err(ThreadError::NoPlaceInQueue);
    }
    x86_64::instructions::interrupts::enable();
    Ok(ThreadHandle { tcb })
}

pub fn thread_exit() {
    println!("Thread ended!");
}
