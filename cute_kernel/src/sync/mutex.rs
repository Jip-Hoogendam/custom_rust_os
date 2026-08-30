use core::sync::atomic::{AtomicBool, Ordering};

use crate::{multi_tasking::scheduler::SCHEDULER, tools::cpu_id};

/// This mutex is more close to the `sys::Mutex` then the `sync::Mutex` as it does not store
/// information.
pub struct Mutex {
    semaphore: AtomicBool,
}

impl Mutex {
    pub const fn new() -> Self {
        Self {
            semaphore: AtomicBool::new(false),
        }
    }

    pub fn try_lock(&self) -> bool {
        !self.semaphore.fetch_or(true, Ordering::Relaxed)
    }

    pub fn lock(&self) {
        while self.semaphore.fetch_or(true, Ordering::Relaxed) {
            SCHEDULER.await_resource(cpu_id(), self);
        }
    }

    pub fn unlock(&self) {
        self.semaphore.store(false, Ordering::Relaxed);
        SCHEDULER.notify_one_resource(self);
    }
}
