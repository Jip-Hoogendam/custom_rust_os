use core::{error::Error, fmt::Display, marker::PhantomData};

use alloc::slice;
use bootloader_api::info::{MemoryRegionKind, MemoryRegions};

use crate::memory::{
    addresses::{Address, PhysAddr, PhysMemOffset, VirtAddr},
    frames::{Frame4Kb, FrameSize, PhysFrame},
};

pub struct FrameBitMap<T: FrameSize = Frame4Kb> {
    inner: &'static mut [usize],
    pages: usize,
    page_free: usize,
    last_allocation_index: usize,
    size: PhantomData<T>,
}

impl<T: FrameSize> FrameBitMap<T> {
    #[must_use]
    pub fn new(mem_bytes_total: u64, start: VirtAddr) -> Self {
        let bitmap = unsafe {
            slice::from_raw_parts_mut(
                start.to_mut_ptr(),
                Self::reqired_size_bytes(mem_bytes_total).div_ceil(size_of::<usize>()),
            )
        };

        #[allow(clippy::cast_possible_truncation)]
        let pages = (mem_bytes_total / T::SIZE as u64).div_ceil(u64::from(u8::BITS)) as usize;

        for i in &mut *bitmap {
            *i = 0;
        }

        Self {
            inner: bitmap,
            pages,
            page_free: pages,
            last_allocation_index: usize::MIN,
            size: PhantomData,
        }
    }

    #[inline]
    #[must_use]
    pub const fn free(&self) -> usize {
        self.page_free
    }

    //Returns its size in pages
    #[inline]
    #[must_use]
    pub const fn byte_size(&self) -> usize {
        size_of_val(self.inner)
    }

    //Returns the amount of bytes needed to store the given memory padded to the nearest size of
    //usize;
    #[must_use]
    pub const fn reqired_size_bytes(mem_bytes_total: u64) -> usize {
        #[allow(clippy::cast_possible_truncation)]
        let bytes_reqired = (mem_bytes_total / T::SIZE as u64).div_ceil(u8::BITS as u64) as usize;
        let fmt_padding = bytes_reqired % size_of::<usize>();
        bytes_reqired + fmt_padding
    }

    ///Checks if a frame is used or not. Returns Nothing if it does not exist
    #[inline]
    #[must_use]
    fn is_used(&mut self, frame: &PhysFrame) -> Option<bool> {
        let (block, inner) = self.get_frame(frame)?;
        Some(*block & (1 << inner) != 0)
    }

    #[inline]
    #[must_use]
    fn get_frame(&mut self, frame: &PhysFrame) -> Option<(&mut usize, usize)> {
        let frame_index = frame.index();
        let index = frame_index / usize::BITS as usize;
        let inner = frame_index % usize::BITS as usize;
        Some((self.inner.get_mut(index)?, inner))
    }

    fn seek_free_frame_block(&mut self) -> Option<(usize, usize)> {
        for index in self.last_allocation_index..self.inner.len() {
            if self.inner[index] != usize::MAX {
                self.last_allocation_index = index;
                return Some((
                    self.last_allocation_index,
                    self.inner[self.last_allocation_index].count_zeros() as usize,
                ));
            }
        }
        for index in self.last_allocation_index..self.inner.len() {
            if self.inner[index] != usize::MAX {
                self.last_allocation_index = index;
                return Some((
                    self.last_allocation_index,
                    self.inner[self.last_allocation_index].count_zeros() as usize,
                ));
            }
        }
        None
    }

    ///Sets wether a frame is used or unused
    #[inline]
    pub fn set_frame(&mut self, frame: &PhysFrame, used: bool) {
        let (frame_usize, bit) = self.get_frame(frame).expect("Frame out of range!");
        let prev = *frame_usize;
        *frame_usize = (*frame_usize & !(1 << bit)) | usize::from(used) << bit;
        if prev < *frame_usize {
            self.page_free -= 1;
        } else if prev > *frame_usize {
            self.page_free += 1;
        }
    }
}

//Frame allocator that gives usable frames from the bootloader memory_map
pub struct FrameAllocator {
    bitmap: FrameBitMap<Frame4Kb>,
}

#[derive(Debug, Clone, Copy)]
pub enum FrameAllocatorError {
    PhysMemExausted,
    NoRegionFreeToAllocateAllocator,
}

impl Error for FrameAllocatorError {}
impl Display for FrameAllocatorError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl FrameAllocator {
    #[inline]
    pub fn dealloc(&mut self, frame: &PhysFrame) {
        self.bitmap.set_frame(frame, false);
    }

    #[must_use]
    #[inline]
    pub fn allocate(&mut self) -> Option<PhysFrame<Frame4Kb>> {
        let (block, index) = self.bitmap.seek_free_frame_block()?;
        let frame = PhysFrame::<Frame4Kb>::new(PhysAddr::new((block + index) * Frame4Kb::SIZE));
        self.bitmap.set_frame(&frame, true);
        Some(frame)
    }

    /// Allocates a specific requested frame
    /// Returns false if it was already allocated.
    #[must_use = "You should check that the allocation actually could be completed"]
    #[inline]
    pub fn allocate_exact(&mut self, frame: &PhysFrame) -> bool {
        if self.bitmap.is_used(frame).unwrap_or(true) {
            false
        } else {
            self.bitmap.set_frame(frame, true);
            true
        }
    }

    ///create the Frame allocator from the passed `memory_map`
    ///# Safety
    ///Caller must pass a valid `memory_map` where all USABLE frames are unused
    #[must_use]
    pub unsafe fn init(
        memory_map: &'static MemoryRegions,
        offset: PhysMemOffset,
    ) -> Result<Self, FrameAllocatorError> {
        let mem_total = memory_map
            .iter()
            .filter(|region| region.kind == MemoryRegionKind::Usable)
            .fold(0, |_c, region| region.end);

        let mut first_free = PhysAddr::new(0);
        let mut region_size = 0;

        for region in memory_map.iter() {
            if region.kind == MemoryRegionKind::Usable {
                region_size += region.end - region.start;
                if region_size >= FrameBitMap::<Frame4Kb>::reqired_size_bytes(mem_total) as u64 {
                    break;
                }
            } else {
                first_free = PhysAddr::from(region.end);
            }
        }

        if region_size < FrameBitMap::<Frame4Kb>::reqired_size_bytes(mem_total) as u64 {
            Err(FrameAllocatorError::NoRegionFreeToAllocateAllocator)?;
        }

        let mut bitmap = FrameBitMap::new(mem_total, first_free + offset);

        let mut last_usable = 0;
        for region in memory_map.iter() {
            if region.kind == MemoryRegionKind::Usable {
                for frame_start in (last_usable..region.start).step_by(Frame4Kb::SIZE) {
                    let frame = PhysFrame::<Frame4Kb>::new(PhysAddr::from(frame_start));
                    bitmap.set_frame(&frame, true);
                }
                last_usable = region.end;
            }
        }

        let bitmap_page_size = bitmap.byte_size() + (bitmap.byte_size() % Frame4Kb::SIZE);
        for region in (0..bitmap_page_size).step_by(Frame4Kb::SIZE) {
            let frame = PhysFrame::<Frame4Kb>::new(first_free + region);
            bitmap.set_frame(&frame, true);
        }

        Ok(Self { bitmap })
    }
}
