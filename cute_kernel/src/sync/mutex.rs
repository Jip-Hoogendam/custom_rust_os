use core::sync::atomic::{AtomicBool, Ordering};

use crate::{multi_tasking::scheduler::SCHEDULER, println, serial_println, tools::cpu_id};

/// This mutex is more close to the `sys::Mutex` then the `sync::Mutex` as it does not store
/// information.
#[derive(Debug)]
pub struct SysMutex {
    locked: AtomicBool,
}

impl Default for SysMutex {
    fn default() -> Self {
        Self::new()
    }
}

impl SysMutex {
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
        }
    }

    #[inline]
    #[must_use]
    pub fn locked(&self) -> bool {
        self.locked.fetch_or(false, Ordering::SeqCst)
    }

    #[inline]
    #[must_use]
    pub fn try_lock(&self) -> bool {
        !self.locked.fetch_or(true, Ordering::SeqCst)
    }

    #[inline]
    pub fn lock(&'static self) {
        if !self.try_lock() {
            SCHEDULER.await_resource(cpu_id(), self);
        }
    }

    #[inline]
    pub fn unlock(&self) {
        self.locked.store(false, Ordering::SeqCst);
    }
}
