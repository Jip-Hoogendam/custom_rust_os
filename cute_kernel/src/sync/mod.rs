use core::{
    cell::UnsafeCell,
    error::Error,
    fmt::Display,
    ops::{Deref, DerefMut},
};

/// The condition variable must always be a pointer to the exact heap allocated data.
#[derive(PartialEq, Eq, Debug)]
#[repr(transparent)]
pub struct ConditionVariable(usize);

mod mutex;

/// A system mutex.
/// This mutex is based off the implemntation used by the `std::sync` in rust. found [here](https://doc.rust-lang.org/src/std/sync/poison/mutex.rs.html#227-231)
pub struct Mutex<T: ?Sized> {
    inner: mutex::Mutex,
    data: UnsafeCell<T>,
}

impl<T: Default> Default for Mutex<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: Sized> Mutex<T> {
    pub const fn new(data: T) -> Self {
        Self {
            inner: mutex::Mutex::new(),
            data: UnsafeCell::new(data),
        }
    }
}

impl<T: ?Sized> Mutex<T> {
    pub fn lock(&self) -> MutexGuard<'_, T> {
        self.inner.lock();
        MutexGuard::new(self)
    }

    /// # Errors
    /// This returns a `TryLockResult::WouldBlock` if the mutex is locked
    pub fn try_lock(&self) -> Result<MutexGuard<'_, T>, TryLockResult> {
        if self.inner.try_lock() {
            Ok(MutexGuard::new(self))
        } else {
            Err(TryLockResult::WouldBlock)
        }
    }
}

unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}

/// A gaurded mutex sytem
/// This mutex gaurd is based off the implemention used by the `std::sync` in rust. found [here](https://doc.rust-lang.org/src/std/sync/poison/mutex.rs.html#227-231)
#[must_use = "if unused the Mutex will immediately unlock"]
#[must_not_suspend = "holding a MutexGuard across suspend \
                      points can cause deadlocks, delays, \
                      and cause Futures to not implement `Send`"]
#[clippy::has_significant_drop]
pub struct MutexGuard<'a, T: ?Sized + 'a> {
    lock: &'a Mutex<T>,
}

impl<T: ?Sized + Display> Display for MutexGuard<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "MutexGaurd{{ {} }}", unsafe { &*self.lock.data.get() })
    }
}

impl<'a, T: ?Sized> MutexGuard<'a, T> {
    pub const fn new(mutex: &'a Mutex<T>) -> Self {
        Self { lock: mutex }
    }
}

unsafe impl<T: ?Sized + Sync> Sync for MutexGuard<'_, T> {}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.inner.unlock();
    }
}

impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TryLockResult {
    WouldBlock,
}

impl Display for TryLockResult {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Error for TryLockResult {}

#[cfg(test)]
use crate::{multi_tasking, println};

#[test_case]
fn mutex_test() {
    static TEST_MUTEX: Mutex<i32> = Mutex::new(0);

    for _i in 0..20 {
        multi_tasking::premtive::spawn(|| {
            for _ in 0..1000 {
                let mut value = TEST_MUTEX.lock();
                *value += 1;
            }
        });
    }

    loop {
        let gaurd = TEST_MUTEX.lock();
        if *gaurd == 20000 {
            println!("Mutex works !");
            break;
        }
    }
}
