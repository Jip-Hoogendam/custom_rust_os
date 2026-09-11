use core::{
    arch::asm,
    cell::UnsafeCell,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

/// This struct is used to control the interupts. Works basicallly like a lock gaurd but then for
/// interutps.
pub trait InteruptController {
    // Blocks the interupts
    fn block();
    // Unblocks the interupts
    fn unblock();
}

/// This is a struct to be used in the interupt gaurd to temporaraly block the interupts
pub struct BlockInterupts;

impl InteruptController for BlockInterupts {
    #[inline]
    fn block() {
        unsafe {
            asm!("cli", options(preserves_flags, nostack));
        }
    }

    #[inline]
    fn unblock() {
        unsafe {
            asm!("sti", options(preserves_flags, nostack));
        }
    }
}

/// This is a struct to be used in the interupt gaurd when the interutps are blocked by something
/// else.
pub struct InteruptAlreadyBlocked;
impl InteruptController for InteruptAlreadyBlocked {
    #[inline]
    fn block() {}

    #[inline]
    fn unblock() {}
}

pub struct InteruptGaurd<T>
where
    T: InteruptController,
{
    controller: PhantomData<T>,
}

impl<T: InteruptController> Drop for InteruptGaurd<T> {
    #[inline]
    fn drop(&mut self) {
        T::unblock();
    }
}

impl<T: InteruptController> InteruptGaurd<T> {
    #[inline]
    #[must_use]
    pub fn block() -> Self {
        T::block();
        Self {
            controller: PhantomData,
        }
    }

    /// Unblocks the inteupts then halts the processor checking if there are no interupts scheduled
    #[inline]
    fn unblock_hlt() {
        unsafe {
            asm!("sti; hlt", options(nomem, nostack, preserves_flags));
            asm!("cli", options(nomem, nostack, preserves_flags));
        }
    }

    #[inline]
    pub fn block_halt_if<F>(condition: F)
    where
        F: FnOnce() -> bool,
    {
        T::block();
        if condition() {
            Self::unblock_hlt();
        } else {
            T::unblock();
        }
    }

    #[inline]
    pub fn scope<F, R>(f: F) -> R
    where
        F: FnOnce(&Self) -> R,
    {
        let gaurd = Self::block();
        f(&gaurd)
    }
}

pub struct InteruptCellGaurd<'a, T: ?Sized + 'a, C: InteruptController> {
    inner: &'a mut T,
    _gaurd: &'a InteruptGaurd<C>,
}

impl<T: ?Sized, C: InteruptController> Deref for InteruptCellGaurd<'_, T, C> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.inner
    }
}

impl<T: ?Sized, C: InteruptController> DerefMut for InteruptCellGaurd<'_, T, C> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner
    }
}

/// Like the `UnsafeCell` allows interiour mutibility but with the only check that the interupts are
/// disabled
pub struct InteruptCell<T> {
    inner: UnsafeCell<T>,
}

impl<T> InteruptCell<T> {
    #[inline]
    pub const fn new(inner: T) -> Self {
        Self {
            inner: UnsafeCell::new(inner),
        }
    }

    /// # Safety
    /// This function is unsafe as it allows interiour mutability without passing mut
    #[inline]
    pub const unsafe fn get<'a, C: InteruptController>(
        &'a self,
        interupt_gaurd: &'a InteruptGaurd<C>,
    ) -> InteruptCellGaurd<'a, T, C> {
        InteruptCellGaurd {
            inner: unsafe { &mut *self.inner.get() },
            _gaurd: interupt_gaurd,
        }
    }

    #[inline]
    pub const fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }
}
