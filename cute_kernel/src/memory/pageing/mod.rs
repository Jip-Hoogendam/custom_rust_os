use core::{error::Error, fmt::Display};

use bootloader_api::info::MemoryRegions;

use crate::memory::{
        addresses::{PhysMemOffset, VirtAddr},
        frames::{Frame4Kb, PhysFrame},
        pageing::{
            frame_allocator::FrameAllocator,
            page_table::{Page, PageEntryOption, PageTable, PageTableError},
        },
    };

pub mod frame_allocator;
pub mod page_table;

/// Describes the layout of the physical memory that needs to be allocated.
/// - `PageMemoryMap::Range{begin: PhysFrame, end: PhysFrame}` describes a exact physical mapping that
/// is requested. Only use this if you need a exact mapping to the physical memory.
/// - `PageMemoryMap::Bytes(usize)` requests a page where the size is known however where in
/// physical space it is is not needed. The size is in bytes
/// - `PageMemoryMap::Single` Request a single page to be created anywhere in memory. This one is
/// mostly used internaly to reserve more internal info.
pub enum PageMemoryMap {
    Range { begin: PhysFrame, end: PhysFrame },
    Bytes(usize),
    Single,
}

#[derive(Debug)]
pub struct RegionPromise {
    offset: PhysMemOffset,
    frames: usize,
}

impl RegionPromise {
    #[inline]
    #[must_use]
    pub const fn offset(&self) -> PhysMemOffset {
        self.offset
    }
}

#[derive(Debug)]
pub enum MemoryMapperError {
    PageError(PageTableError),
    OutOfMemory,
    OutOfMemoryIncomplete(RegionPromise),
}

impl Error for MemoryMapperError {}
impl Display for MemoryMapperError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// A interface to interact with the page tables and memory allocator.
/// Most interactions will be done via the `MemoryPager::page()` function
pub struct MemoryMapper {
    frame_alloc: frame_allocator::FrameAllocator,
    l4_page_table: &'static mut page_table::PageTable,
    offset: PhysMemOffset,
}

impl MemoryMapper {
    #[must_use]
    pub fn init(memory_map: &'static MemoryRegions, offset: PhysMemOffset) -> Option<Self> {
        let frame_alloc = unsafe { FrameAllocator::init(memory_map, offset).ok()? };
        let l4_page_table = PageTable::load(offset);
        Some(Self {
            frame_alloc,
            l4_page_table,
            offset,
        })
    }

    /// Deallocates all memory used by the promise
    /// Returns false if the deallocation failed. This function should never fail
    pub fn deallocate(&mut self, promise: RegionPromise) -> bool {
        let mut page = Page::from_containing(promise.offset.inner());
        let mut frames_to_dealloc = promise.frames;

        while frames_to_dealloc > 0 {
            let Some(l1_table) = self.l4_page_table.page_table_mut(page, self.offset) else {
                return false;
            };
            let l1_index = page.l1_index();
            for entry in l1_table.iter_from(l1_index) {
                if !entry.present() {
                    return false;
                }
                self.frame_alloc.dealloc(&entry.frame());
                entry.free();
                page.flush();
                page += 1;
                frames_to_dealloc -= 1;
            }
            page -= 1;
        }

        self.l4_page_table
            .free_unused(page, self.offset, &mut self.frame_alloc);

        true
    }

    /// Allocates a exact region of physical memory. This memory must be able to be free before it
    /// is allocated.
    ///
    /// # Errors
    /// This function fails if there is insuficcant memory to complete the operation. Some memory
    /// can be recoverd if a `MemoryMapperError::OutOfMemoryIncomplete`.
    pub fn allocate_range(
        &mut self,
        start: VirtAddr,
        options: PageEntryOption,
        begin: PhysFrame,
        end: PhysFrame,
    ) -> Result<RegionPromise, MemoryMapperError> {
        let mut page = Page::from_containing(start);
        let mut l1_table = self
            .l4_page_table
            .allocate_page_table(
                page,
                options | PageEntryOption::Present | PageEntryOption::ReadWrite,
                self.offset,
                &mut self.frame_alloc,
            )
            .map_err(MemoryMapperError::PageError)?;

        let mut frames = 0;
        let mut frame = begin;

        'outer: while frame != end {
            let index = page.l1_index();
            for entry in l1_table.iter_from(index) {
                if entry.present() || frame == end {
                    break 'outer;
                }

                if self.frame_alloc.allocate_exact(&frame) {
                    break 'outer;
                }

                entry.write(
                    &frame,
                    options | PageEntryOption::Present | PageEntryOption::ReadWrite,
                );
                page.flush();

                frame += 1;
                page += 1;
                frames += 1;
            }
            if let Ok(next_table) = self.l4_page_table.allocate_page_table(
                page,
                options | PageEntryOption::Present | PageEntryOption::ReadWrite,
                self.offset,
                &mut self.frame_alloc,
            ) {
                l1_table = next_table;
            } else {
                break 'outer;
            }
        }

        // if not all frame allocated return incomplete instead
        if frame == end {
            Ok(RegionPromise {
                offset: PhysMemOffset::from_virt(start),
                frames,
            })
        } else {
            Err(MemoryMapperError::OutOfMemoryIncomplete(RegionPromise {
                offset: PhysMemOffset::from_virt(start),
                frames,
            }))
        }
    }

    /// Allocates pages with a exact range that includes the specified amount of bytes
    ///
    /// # Errors
    /// This function fails if there is insuficcant memory to complete the operation. Some memory
    /// can be recoverd if a `MemoryMapperError::OutOfMemoryIncomplete`.
    pub fn allocate_bytes(
        &mut self,
        start: VirtAddr,
        options: PageEntryOption,
        bytes: usize,
    ) -> Result<RegionPromise, MemoryMapperError> {
        let mut page = Page::from_containing(start);
        let mut l1_table = self
            .l4_page_table
            .allocate_page_table(
                page,
                options | PageEntryOption::Present | PageEntryOption::ReadWrite,
                self.offset,
                &mut self.frame_alloc,
            )
            .map_err(MemoryMapperError::PageError)?;
        let size = bytes.div_ceil(PhysFrame::<Frame4Kb>::SIZE);
        let mut to_allocate = size;

        // allocate all to allocate
        'outer: while to_allocate > 0 {
            let index = page.l1_index();
            for entry in l1_table.iter_from(index) {
                if entry.present() || to_allocate == 0 {
                    break 'outer;
                }

                let Some(frame) = self.frame_alloc.allocate() else {
                    break 'outer;
                };

                entry.write(
                    &frame,
                    options | PageEntryOption::Present | PageEntryOption::ReadWrite,
                );

                page.flush();
                page += 1;
                to_allocate -= 1;
            }
            if let Ok(next_table) = self.l4_page_table.allocate_page_table(
                page,
                options | PageEntryOption::Present | PageEntryOption::ReadWrite,
                self.offset,
                &mut self.frame_alloc,
            ) {
                l1_table = next_table;
            } else {
                break 'outer;
            }
        }

        // if not all frame allocated return incomplete instead
        if to_allocate > 0 {
            Err(MemoryMapperError::OutOfMemoryIncomplete(RegionPromise {
                offset: PhysMemOffset::from_virt(start),
                frames: size - to_allocate,
            }))
        } else {
            Ok(RegionPromise {
                offset: PhysMemOffset::from_virt(start),
                frames: size - to_allocate,
            })
        }
    }

    /// Allocates a single frame in the specified page. Returns a promise of the region allocated
    ///
    /// # Errors
    /// This function fails if there is insuficcant memory to complete the operation. Some memory
    /// can be recoverd if a `MemoryMapperError::OutOfMemoryIncomplete`.
    pub fn allocate_page(
        &mut self,
        start: VirtAddr,
        options: PageEntryOption,
    ) -> Result<RegionPromise, MemoryMapperError> {
        let page = Page::from_containing(start);
        let l1_table = self
            .l4_page_table
            .allocate_page_table(
                page,
                options | PageEntryOption::Present | PageEntryOption::ReadWrite,
                self.offset,
                &mut self.frame_alloc,
            )
            .map_err(MemoryMapperError::PageError)?;

        let l1_index = page.l1_index();
        let frame = self
            .frame_alloc
            .allocate()
            .ok_or(MemoryMapperError::OutOfMemory)?;
        let entry = l1_table.entry_mut(l1_index);

        if entry.present() {
            Err(MemoryMapperError::PageError(
                PageTableError::PageAlreadyExists,
            ))
        } else {
            entry.write(
                &frame,
                options | PageEntryOption::Present | PageEntryOption::ReadWrite,
            );

            page.flush();
            Ok(RegionPromise {
                offset: PhysMemOffset::from_virt(start),
                frames: 1,
            })
        }
    }
}
