use core::{
    arch::asm,
    ops::{Add, AddAssign, BitOr, Sub, SubAssign},
};

use crate::{
    arch::x86::registers::Register,
    memory::{
        addresses::{Address, PhysAddr, PhysMemOffset, VirtAddr},
        frames::PhysFrame,
        pageing::frame_allocator::FrameAllocator,
    },
};

/// A struct to construnct from via multiple options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageEntryOptions(u64);

impl BitOr<PageEntryOption> for PageEntryOptions {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: PageEntryOption) -> Self::Output {
        Self(self.0 | rhs as u64)
    }
}

impl BitOr for PageEntryOptions {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl Default for PageEntryOptions {
    #[inline]
    fn default() -> Self {
        PageEntryOption::ReadWrite | PageEntryOption::Present
    }
}

impl PageEntryOptions {
    #[inline]
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

/// A struct to construct to set the `PageEntryOptions`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
#[non_exhaustive]
pub enum PageEntryOption {
    Present = 0x00_00_00_00_00_00_00_01,
    ReadWrite = 0x00_00_00_00_00_00_00_02,
    UserSuperVisor = 0x00_00_00_00_00_00_00_04,
    WriteTrough = 0x00_00_00_00_00_00_00_08,
    CacheDisable = 0x00_00_00_00_00_00_00_10,
    Accessed = 0x00_00_00_00_00_00_00_20,
    Dirty = 0x00_00_00_00_00_00_00_40,
    PageSizeOrPat = 0x00_00_00_00_00_00_00_80,
    Global = 0x00_00_00_00_00_00_01_00,
    PAT = 0x00_00_00_00_00_00_08_00,
    ExecuteDisable = 0x80_00_00_00_00_00_00_00,
}

impl BitOr for PageEntryOption {
    type Output = PageEntryOptions;
    fn bitor(self, rhs: Self) -> Self::Output {
        PageEntryOptions(self as u64 | rhs as u64)
    }
}

/// A entry in the Page Tables or diretory
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    const ADDRESS_MASK: u64 = 0x00_0F_FF_FF_FF_FF_F0_00;
    pub fn write(&mut self, frame: &PhysFrame, options: PageEntryOptions) {
        let addr = frame.get_base().to_usize() as u64 & Self::ADDRESS_MASK;
        self.0 = addr | options.as_u64();
    }

    pub const fn options(&mut self) -> PageEntryOptions {
        PageEntryOptions(self.0 & !Self::ADDRESS_MASK)
    }

    pub const fn free(&mut self) {
        self.0 = self.0 & !(PageEntryOption::Present as u64);
    }

    pub const fn null(&mut self) {
        self.0 = 0;
    }

    //Returns weather it is present or not.
    #[inline]
    #[must_use]
    pub const fn present(&self) -> bool {
        self.0 & PageEntryOption::Present as u64 != 0
    }

    #[inline]
    #[must_use]
    pub fn frame(&self) -> PhysFrame {
        PhysFrame::new(PhysAddr::new((self.0 & Self::ADDRESS_MASK) as usize))
    }
}

/// A simple structure to index into page tables
#[derive(Debug, Clone, Copy)]
pub struct PageTableIndex(u16);

impl PageTableIndex {
    //This creates a new value truncating if outside of bounds
    #[inline]
    #[must_use]
    pub const fn new_truncate(index: usize) -> Self {
        #[allow(clippy::cast_possible_truncation)]
        let index = if index > 0x1FF {
            0x1FFu16
        } else {
            index as u16
        };

        Self(index)
    }

    #[inline]
    #[must_use]
    pub const fn index(&self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Page(usize);

impl Page {
    #[inline]
    pub fn flush(&self) {
        unsafe {
            asm!("invlpg [{}]", in(reg) self.0, options(nostack, preserves_flags));
        }
    }

    const PAGE_INDEX_MASK: usize = 0x1FF;

    #[inline]
    #[must_use]
    pub fn from_containing(addr: VirtAddr) -> Self {
        const OFFSET_MASK: usize = 0xFFF;
        Self(addr.to_usize() & !OFFSET_MASK)
    }

    #[inline]
    #[must_use]
    pub const fn l1_index(&self) -> PageTableIndex {
        const L1_INDEX: usize = 12;
        PageTableIndex::new_truncate(self.0 >> L1_INDEX & Self::PAGE_INDEX_MASK)
    }

    #[inline]
    #[must_use]
    pub const fn l2_index(&self) -> PageTableIndex {
        const L2_INDEX: usize = 21;
        PageTableIndex::new_truncate(self.0 >> L2_INDEX & Self::PAGE_INDEX_MASK)
    }

    #[inline]
    #[must_use]
    pub const fn l3_index(&self) -> PageTableIndex {
        const L3_INDEX: usize = 30;
        PageTableIndex::new_truncate(self.0 >> L3_INDEX & Self::PAGE_INDEX_MASK)
    }

    #[inline]
    #[must_use]
    pub const fn l4_index(&self) -> PageTableIndex {
        const L4_INDEX: usize = 39;
        PageTableIndex::new_truncate(self.0 >> L4_INDEX & Self::PAGE_INDEX_MASK)
    }

    /// unused but for future refrence
    #[allow(dead_code)]
    #[inline]
    #[must_use]
    pub const fn l5_index(&self) -> PageTableIndex {
        const L5_INDEX: usize = 48;
        PageTableIndex::new_truncate(self.0 >> L5_INDEX & Self::PAGE_INDEX_MASK)
    }
}

impl Add<usize> for Page {
    type Output = Self;

    #[inline]
    fn add(self, rhs: usize) -> Self::Output {
        const MEMORY_ADDRESS_START: usize = 0x1000;
        Self(self.0 + rhs * MEMORY_ADDRESS_START)
    }
}

impl AddAssign<usize> for Page {
    #[inline]
    fn add_assign(&mut self, rhs: usize) {
        const MEMORY_ADDRESS_START: usize = 0x1000;
        self.0 += rhs * MEMORY_ADDRESS_START;
    }
}

impl Sub<usize> for Page {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: usize) -> Self::Output {
        const MEMORY_ADDRESS_START: usize = 0x1000;
        Self(self.0 - rhs * MEMORY_ADDRESS_START)
    }
}

impl SubAssign<usize> for Page {
    #[inline]
    fn sub_assign(&mut self, rhs: usize) {
        const MEMORY_ADDRESS_START: usize = 0x1000;
        self.0 -= rhs * MEMORY_ADDRESS_START;
    }
}

#[derive(Debug)]
pub enum PageTableError {
    CouldNotCreatePageTable,
    PageAlreadyExists,
    IncompatiblePageOptions,
}

#[repr(align(4096))]
pub struct PageTable {
    entries: [PageTableEntry; 512],
}

impl PageTable {
    /// Loads the l4 page from the cr3 register
    #[must_use]
    pub fn load(offset: PhysMemOffset) -> &'static mut Self {
        use crate::arch::x86::registers::Cr3;
        let cr3 = Cr3::read();
        let address = cr3.l4_table_frame().get_base() + offset;
        unsafe { address.to_mut_ref() }
    }

    #[must_use]
    pub fn new(offset: PhysMemOffset, frame: &PhysFrame) -> &'static mut Self {
        let begin = frame.get_base() + offset;
        let ptr = begin.to_mut_ptr::<u8>();

        //Zero out all the pages
        for i in 0..4096 {
            unsafe { *ptr.add(i) = 0u8 }
        }
        unsafe { begin.to_mut_ref() }
    }

    #[inline]
    #[must_use]
    pub const fn entry(&self, index: usize) -> &PageTableEntry {
        &self.entries[index]
    }

    #[inline]
    #[must_use]
    pub const fn entry_mut(&mut self, index: PageTableIndex) -> &mut PageTableEntry {
        &mut self.entries[index.index()]
    }

    pub fn iter_from(
        &mut self,
        index: PageTableIndex,
    ) -> impl Iterator<Item = &mut PageTableEntry> {
        self.entries.iter_mut().skip(index.index())
    }

    pub fn is_empty(&mut self) -> bool {
        self.entries.iter().all(|entry| !entry.present())
    }

    /// # Errors
    ///  This function errors if it fails to allocate a page due to insuficcant memory or a page has
    ///  a incompatible type
    pub fn parse_or_alloc_entry(
        &mut self,
        index: PageTableIndex,
        offset: PhysMemOffset,
        options: PageEntryOptions,
        frame_allocator: &mut FrameAllocator,
    ) -> Result<&'static mut Self, PageTableError> {
        let entry = self.entry_mut(index);

        let phys_frame = if entry.present() {
            if entry.options() != options {
                Err(PageTableError::IncompatiblePageOptions)?;
            }
            entry.frame()
        } else {
            let mut physframe = frame_allocator
                .allocate()
                .ok_or(PageTableError::CouldNotCreatePageTable)?;
            physframe.zero(offset);
            entry.write(&physframe, options);
            physframe
        };

        unsafe { Ok((offset + phys_frame.get_base()).to_mut_ref()) }
    }

    /// attempst to parse the given page
    pub fn parse_entry(
        &mut self,
        index: PageTableIndex,
        offset: PhysMemOffset,
    ) -> Option<&'static mut Self> {
        let entry = self.entry_mut(index);

        if entry.present() {
            unsafe { Some((offset + entry.frame().get_base()).to_mut_ref()) }
        } else {
            None
        }
    }

    /// Looks in the given Page and frees all page tables that are unused
    pub fn free_unused(
        &mut self,
        page: Page,
        offset: PhysMemOffset,
        frame_allocator: &mut FrameAllocator,
    ) {
        let l4_index = page.l4_index();
        let Some(l3_table) = self.parse_entry(l4_index, offset) else {
            return;
        };
        let l3_index = page.l3_index();
        let Some(l2_table) = l3_table.parse_entry(l3_index, offset) else {
            if l3_table.is_empty() {
                let l3_entry = self.entry_mut(l4_index);
                frame_allocator.dealloc(&l3_entry.frame());
                l3_entry.free();
            }
            return;
        };
        let l2_index = page.l2_index();
        let Some(l1_table) = l2_table.parse_entry(l2_index, offset) else {
            if l2_table.is_empty() {
                let l2_entry = l3_table.entry_mut(l3_index);
                frame_allocator.dealloc(&l2_entry.frame());
                l2_entry.free();
            }
            if l3_table.is_empty() {
                let l3_entry = self.entry_mut(l4_index);
                frame_allocator.dealloc(&l3_entry.frame());
                l3_entry.free();
            }
            return;
        };
        if l1_table.is_empty() {
            let l1_entry = l2_table.entry_mut(l2_index);
            frame_allocator.dealloc(&l1_entry.frame());
            l1_entry.free();
        }
        if l2_table.is_empty() {
            let l2_entry = l3_table.entry_mut(l3_index);
            frame_allocator.dealloc(&l2_entry.frame());
            l2_entry.free();
        }
        if l3_table.is_empty() {
            let l3_entry = self.entry_mut(l4_index);
            frame_allocator.dealloc(&l3_entry.frame());
            l3_entry.free();
        }
    }

    /// Gets a mutable refrence to a page table
    pub fn page_table_mut(&mut self, page: Page, offset: PhysMemOffset) -> Option<&mut Self> {
        let l3_table = self.parse_entry(page.l4_index(), offset)?;
        let l2_table = l3_table.parse_entry(page.l3_index(), offset)?;
        l2_table.parse_entry(page.l2_index(), offset)
    }

    /// Allocates a new page table. This may exist already.
    ///
    /// # Errors
    /// This function will return a error if there is insufficant memory to allocate the page or if
    /// the direcotrys have a incompatible type
    pub fn allocate_page_table(
        &mut self,
        page: Page,
        options: PageEntryOptions,
        offset: PhysMemOffset,
        frame_allocator: &mut FrameAllocator,
    ) -> Result<&mut Self, PageTableError> {
        let l3_table =
            self.parse_or_alloc_entry(page.l4_index(), offset, options, frame_allocator)?;
        let l2_table =
            l3_table.parse_or_alloc_entry(page.l3_index(), offset, options, frame_allocator)?;
        let l1_table =
            l2_table.parse_or_alloc_entry(page.l2_index(), offset, options, frame_allocator)?;
        Ok(l1_table)
    }
}
