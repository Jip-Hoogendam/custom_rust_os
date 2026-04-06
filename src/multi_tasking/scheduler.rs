use core::sync::atomic::{AtomicU64, Ordering};

use crate::println;

pub static MAX_THREAD_COUNT: usize = 30;
pub static TASK_QUEUE: spin::Mutex<[usize; MAX_THREAD_COUNT]> = spin::Mutex::new([0; 30]);

#[unsafe(no_mangle)]
extern "C" fn scheduler(stack: *mut u8) -> *mut u8 {
    static TASK_HEAD: AtomicU64 = AtomicU64::new(0);
    let mut queue = TASK_QUEUE.lock();

    let current = TASK_HEAD.load(Ordering::SeqCst) as usize % MAX_THREAD_COUNT;

    queue[current] = stack as usize;

    for i in 1..=MAX_THREAD_COUNT {
        let next = (current + i) % MAX_THREAD_COUNT;
        let next_stack = queue[next];

        if next_stack != 0 {
            // println!("task index {}, stack {:#x}", next, next_stack);
            TASK_HEAD.store(next as u64, Ordering::SeqCst);
            return next_stack as *mut u8;
        }
    }

    // fallback: stay on current task
    stack
}
