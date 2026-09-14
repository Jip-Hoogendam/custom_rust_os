use core::mem::MaybeUninit;

use alloc::boxed::Box;

#[repr(C)]
#[derive(Debug, Clone)]
pub struct Stack {
    inner: Option<Box<[MaybeUninit<usize>]>>,
}

unsafe impl Send for Stack {}

impl Stack {
    #[must_use]
    pub fn new(size: usize) -> Self {
        Self {
            inner: Some(Box::new_uninit_slice(size)),
        }
    }

    /// # Option
    /// Returns nothign if the stack is empty
    #[must_use]
    pub fn get_base(&mut self) -> Option<&mut MaybeUninit<usize>> {
        if let Some(inner) = &mut self.inner {
            Some(inner.last_mut()?)
        } else {
            None
        }
    }

    /// Used when creating a thread with no stack
    #[must_use]
    pub const fn null_stack() -> Self {
        Self { inner: None }
    }
}
