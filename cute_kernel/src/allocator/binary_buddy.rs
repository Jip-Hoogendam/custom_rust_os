use core::{alloc::GlobalAlloc, mem, ptr::null_mut};

use crate::allocator::Locked;

#[global_allocator]
pub static ALLOCATOR: Locked<BinaryBuddy> = Locked::new(BinaryBuddy::new());

#[derive(Clone, Copy)]
pub struct BinaryBuddy {
    start: *mut BuddyBlock,
    end: *mut BuddyBlock,
}

unsafe impl Send for BinaryBuddy {}

#[derive(Clone, Copy)]
struct BuddyBlock {
    pub size: usize,
    pub free: bool,
}

impl BuddyBlock {
    fn start_addr(&self) -> usize {
        core::ptr::from_ref::<Self>(self) as usize
    }

    pub fn get_addr(&self) -> usize {
        mem::size_of::<Self>() + self.start_addr()
    }
}

impl BuddyBlock {}

impl Default for BinaryBuddy {
    fn default() -> Self {
        Self::new()
    }
}

impl BinaryBuddy {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            start: null_mut(),
            end: null_mut(),
        }
    }

    /// Initilizes the allocator.
    ///
    /// # Safety
    ///
    /// Caller must garuntee that the allocator points to a valid virtual memory adress.
    ///
    /// # Panics
    /// Panics if the set size is not a power of 2
    pub unsafe fn init(&mut self, start: usize, size: usize) {
        assert!(size.is_power_of_two(), "allocator size must be power of 2 ");
        self.start = start as *mut BuddyBlock;
        unsafe { (*self.start).size = size };
        unsafe { (*self.start).free = true };
        self.end = Self::next_block(self.start);
    }

    //finds the best block for the given size value.
    fn find_best_block(&self, size: usize) -> Option<*mut BuddyBlock> {
        let mut res_block: *mut BuddyBlock = null_mut();
        let mut block = self.start;
        let mut buddy = Self::next_block(block);
        if buddy == self.end && unsafe { (*block).free } {
            return Some(Self::split_block(self.start, size));
        }

        while block < self.end && buddy < self.end {
            if unsafe { (*block).free && (*buddy).free && (*block).size == (*buddy).size } {
                unsafe { *block }.size <<= 1;
                if size <= unsafe { *block }.size
                    && (res_block.is_null() || unsafe { (*block).size <= (*res_block).size })
                {
                    res_block = block;
                }

                block = Self::next_block(buddy);
                if block < self.end {
                    buddy = Self::next_block(block);
                }
                continue;
            }

            if unsafe {
                (*block).free
                    && size <= (*block).size
                    && (res_block.is_null() || (*block).size <= (*res_block).size)
            } {
                res_block = block;
            }

            if unsafe {
                (*buddy).free
                    && size <= (*buddy).size
                    && (res_block.is_null() || (*buddy).size <= (*res_block).size)
            } {
                res_block = buddy;
            }

            if unsafe { (*block).size <= (*buddy).size } {
                block = Self::next_block(buddy);
                if block < self.end {
                    buddy = Self::next_block(block);
                }
            } else {
                block = buddy;
                buddy = Self::next_block(block);
            }
        }

        if res_block.is_null() {
            None
        } else {
            Some(Self::split_block(res_block, size))
        }
    }

    ///finds the next usable block.
    #[inline]
    fn next_block(block: *mut BuddyBlock) -> *mut BuddyBlock {
        unsafe { block.cast::<u8>().add((*block).size).cast::<BuddyBlock>() }
    }

    ///splits the block into the 2 smaller blocks
    fn split_block(block: *mut BuddyBlock, size: usize) -> *mut BuddyBlock {
        if !block.is_null() && size != 0 {
            unsafe {
                while size < (*block).size / 2 {
                    let new_size = (*block).size / 2;
                    (*block).size = new_size;
                    let new_block = Self::next_block(block);
                    (*new_block).size = new_size;
                    (*new_block).free = true;
                }
            }
            block
        } else {
            null_mut()
        }
    }
}

unsafe impl GlobalAlloc for Locked<BinaryBuddy> {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let allocator = self.lock();
        let size = layout.size() + mem::size_of::<BuddyBlock>();

        allocator.find_best_block(size).map_or(null_mut(), |block| {
            unsafe { (*block).free = false };
            unsafe { (*block).get_addr() as *mut u8 }
        })
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: core::alloc::Layout) {
        let block = (ptr as usize - mem::size_of::<BuddyBlock>()) as *mut BuddyBlock;
        unsafe { (*block).free = true }
    }
}
