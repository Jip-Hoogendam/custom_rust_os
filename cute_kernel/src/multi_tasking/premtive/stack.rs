use core::alloc::Layout;

use alloc::alloc::{alloc, dealloc};

#[repr(C)]
#[derive(Debug)]
pub struct Stack {
    pub begin: *mut u8,
    pub end: *mut u8,
}

unsafe impl Send for Stack {}

impl Drop for Stack {
    fn drop(&mut self) {
        unsafe {
            let layout =
                Layout::from_size_align_unchecked(self.end as usize - self.begin as usize, 16);
            dealloc(self.begin, layout);
        }
    }
}

impl Stack {
    //Createss a new stack where the size is the amount of bytes that should be allocated.
    pub fn new(size: usize) -> Stack {
        unsafe {
            let layout = Layout::from_size_align_unchecked(size, 16);
            let ptr = alloc(layout);
            Stack {
                begin: ptr,
                end: ptr.byte_add(size),
            }
        }
    }
}
