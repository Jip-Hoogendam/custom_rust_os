use core::{arch::asm, marker::PhantomData};

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

impl<T> Drop for InteruptGaurd<T>
where
    T: InteruptController,
{
    fn drop(&mut self) {
        T::unblock();
    }
}

impl<T> InteruptGaurd<T>
where
    T: InteruptController,
{
    #[must_use]
    pub fn block() -> Self {
        T::block();
        Self {
            controller: PhantomData,
        }
    }

    pub fn scope<F, R>(f: F) -> R
    where
        F: FnOnce(&Self) -> R,
    {
        let gaurd = Self::block();
        f(&gaurd)
    }
}
