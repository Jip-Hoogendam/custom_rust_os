use core::{cell::UnsafeCell, sync::atomic::AtomicU64};

pub struct Clock {
    ticks: AtomicU64,
    ticks_per_second: UnsafeCell<usize>,
}

unsafe impl Send for Clock {}
unsafe impl Sync for Clock {}

impl Default for Clock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock {
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ticks: AtomicU64::new(0),
            ticks_per_second: UnsafeCell::new(1),
        }
    }

    /// # Safety
    /// This function is not thread safe
    pub unsafe fn set_tps(&self, scale: usize) {
        unsafe { *(self.ticks_per_second.get()) = scale }
    }

    /// # Safety
    /// This function expects to be incremented at the specified ticks per seconds to be correct.
    pub unsafe fn tick(&self) {
        self.ticks
            .fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    }

    pub fn progress(&self) -> u64 {
        self.ticks
            .fetch_or(0, core::sync::atomic::Ordering::Relaxed)
    }

    pub const fn tps(&self) -> usize {
        unsafe { *self.ticks_per_second.get() }
    }
}

pub static SYSTEM_CLOCK: Clock = Clock::new();
