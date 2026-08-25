pub const DOUBLE_FAULT_IST_INDEX: usize = 0;

/// This is the bitmap to control access to the ports.
pub struct IOBitmap {
    bitmap: [u8; 8193],
}

impl Default for IOBitmap {
    fn default() -> Self {
        Self::new()
    }
}

impl IOBitmap {
    /// Creates a new bitmap disssallowing all
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bitmap: [u8::MAX; 8193],
        }
    }

    /// Disallows the usage of a port
    #[inline]
    pub const fn disallow(&mut self, index: u16) {
        let byte = index / 8;
        let port = index % 8;
        self.bitmap[byte as usize] |= 1 << port;
    }

    /// Allows the usage of a port
    #[inline]
    pub const fn allow(&mut self, index: u16) {
        let byte = index / 8;
        let port = index % 8;
        self.bitmap[byte as usize] &= !(1 << port);
    }

    /// Reads the current permissions of a port
    #[inline]
    #[must_use]
    pub const fn is_allowed(&self, index: u16) -> bool {
        let byte = index / 8;
        let port = index % 8;
        self.bitmap[byte as usize] & 1 << port != 0
    }
}

/// The stack state segment is mostly used to point to stacks that should be used. With the interupt
/// stack table being used to point to stacks to use in case of an interupt. And the stack to use
/// when escalating to diffrent privalage levels.
#[repr(C, packed(4))]
pub struct TaskStateSegment {
    reserved: u32,
    /// These pointers are used during privalage escalation to point the stack to.
    /// These should probebly be set before going to ring 3
    privalage_stack_pointer: [u64; 3],
    reserved_2: [u32; 2],
    /// These are used to describe what stack to use during a interupt. Set the ist in the
    /// interupt descriptor to anything higher then 0 to use one of these stacks during those interupts.
    interupt_stack_table: [u64; 7],
    reserved_3: u32,
    reserved_4: u16,
    iopb: u16,
    io_bitmap: IOBitmap,
}

impl Default for TaskStateSegment {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskStateSegment {
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            reserved: 0,
            privalage_stack_pointer: [0; 3],
            reserved_2: [0; 2],
            interupt_stack_table: [0; 7],
            reserved_3: 0,
            reserved_4: 0,
            iopb: core::mem::offset_of!(Self, io_bitmap) as u16,
            io_bitmap: IOBitmap::new(),
        }
    }

    /// Gets the io bitmap from this `TasktStateSegment`
    #[inline]
    pub const fn get_bitmapp(&mut self) -> &mut IOBitmap {
        &mut self.io_bitmap
    }

    /// Sets a the stack for an interupt discriptor.
    #[inline]
    pub const fn set_interupt_stack(&mut self, index: usize, ptr: u64) {
        self.interupt_stack_table[index] = ptr;
    }
}
