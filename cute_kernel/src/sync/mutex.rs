use core::sync::atomic::{AtomicBool, Ordering};

use crate::{multi_tasking::scheduler::SchedulerResult, syscall::syscall};

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

#[inline]
pub fn await_lock(mtx: &'static SysMutex) {
    use core::mem::transmute;
    while {
        let lock = unsafe {
            syscall(
                crate::syscall::SysCalls::MtxLock,
                transmute::<&'static SysMutex, usize>(mtx),
                0,
                0,
            )
            .0
        };
        !matches!(
            unsafe { transmute::<usize, SchedulerResult>(lock) },
            SchedulerResult::MutexLock
        )
    } {}
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
            await_lock(self);
        }
    }

    #[inline]
    pub fn unlock(&self) {
        self.locked.store(false, Ordering::SeqCst);
    }
}
