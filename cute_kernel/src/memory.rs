use bootloader_api::info::{MemoryRegionKind, MemoryRegions};
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{FrameAllocator, OffsetPageTable, PageTable, PhysFrame, Size4KiB},
};

//Frame allocator that gives usable frames from the bootloader memory_map
pub struct BootFrameAllocator {
    memory_map: &'static MemoryRegions,
    reigion_index: usize,
    next_addr: u64,
}

impl BootFrameAllocator {
    ///create the Frame allocator from the passed `memory_map`
    ///
    ///# Safety
    ///Caller must pass a valid `memory_map` where all USABLE frames are unused
    #[must_use]
    pub const unsafe fn init(memory_map: &'static MemoryRegions) -> Self {
        Self {
            memory_map,
            reigion_index: 0,
            next_addr: 0,
        }
    }

    //returns a itirator over all the usable frames in the memory_map
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        self.memory_map
            .iter()
            .filter(|region| region.kind == MemoryRegionKind::Usable)
            .map(|region| region.start..region.end)
            .flat_map(|region| region.step_by(4096))
            .map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        let mut reigion = self.memory_map.get(self.reigion_index)?;
        if reigion.kind != MemoryRegionKind::Usable {
            reigion = loop {
                self.reigion_index += 1;
                let reigion = self.memory_map.get(self.reigion_index)?;
                if reigion.kind == MemoryRegionKind::Usable {
                    self.next_addr = reigion.start;
                    break reigion;
                }
            }
        }

        let frame = PhysFrame::containing_address(PhysAddr::new(self.next_addr));
        self.next_addr += 4096;
        if self.next_addr >= reigion.end {
            self.reigion_index += 1;
        }
        Some(frame)
    }
}

///Initilize a new `PageOffsetTable`
///
///# Safety
///
///Caller must garuntee that the entire physical memory is mapped.
///Caller must garuntee this function is only called once.
#[must_use]
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    unsafe {
        let l4_table_frame = activate_l4_table(physical_memory_offset);
        OffsetPageTable::new(l4_table_frame, physical_memory_offset)
    }
}

///gets the current active level 4 page table.
///
///# Safety
///
///Caller must garuntee that the entire physical memory is mapped.
///Caller must garuntee that this function is only called once.
unsafe fn activate_l4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    let (l4_table_frame, _) = Cr3::read();

    let phys = l4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    unsafe { &mut *page_table_ptr }
}
