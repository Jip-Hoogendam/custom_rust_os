use core::{fmt::Debug, mem::MaybeUninit};

use alloc::boxed::Box;

#[repr(C)]
#[derive(Clone)]
pub struct Stack {
    inner: Option<Box<[MaybeUninit<usize>]>>,
}

impl Debug for Stack {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match &self.inner {
            Some(inner) => write!(f, "Stack{{Init: {{len:{}}}", inner.len()),
            None => write!(f, "Stack{{ptr: Uninit}}"),
        }
    }
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
