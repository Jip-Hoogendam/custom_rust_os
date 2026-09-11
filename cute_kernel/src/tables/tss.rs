/// This struct is to define a IST to use in the idt.
#[repr(packed, C)]
#[derive(Clone, Copy, Debug)]
pub struct InteruptStack(u8);

/// This is the bitmap to control access to the ports.
#[derive(Debug, Clone, Copy)]
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

/// The global type of a interupt stack table
type InteruptStackTable = [u64; 7];

/// The stack state segment is mostly used to point to stacks that should be
/// stack table being used to point to stacks to use in case of an interupt.
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
    interupt_stack_table: InteruptStackTable,
    reserved_3: u32,
    reserved_4: u16,
    iopb: u16,
    pub io_bitmap: IOBitmap,
}

impl Default for TaskStateSegment {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskStateSegment {
    #[inline]
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
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

    /// Pushes a stack to be used for a IST
    #[inline]
    pub fn push_ist(&mut self, ptr: u64) -> Option<InteruptStack> {
        // Hacky way to get the length since i cant use .len() on a packed field....
        let interupt_stack_table_length = size_of::<InteruptStackTable>() / size_of::<u64>();
        for index in 0..interupt_stack_table_length {
            if self.interupt_stack_table[index] == 0 {
                self.interupt_stack_table[index] = ptr;
                return Some(InteruptStack(u8::try_from(index).ok()? + 1));
            }
        }
        None
    }
}
