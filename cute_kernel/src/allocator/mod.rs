use crate::{
    allocator::binary_buddy::ALLOCATOR,
    memory::{
        addresses::VirtAddr,
        pageing::{
            MemoryMapper, MemoryMapperError,
            page_table::PageEntryOption,
        },
    },
};

pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Self {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<'_, A> {
        self.inner.lock()
    }
}

pub mod binary_buddy;

pub const HEAP_START: VirtAddr = VirtAddr::new(0x6969_dead_0000);
pub const HEAP_SIZE: usize = 1 << 24;

/// Initlizazes the heap
///
/// # Errors
/// Returns a `MapToError` if it fails to allocate the HEAP
pub fn init_heap(memory_mapper: &mut MemoryMapper) -> Result<(), MemoryMapperError> {
    let region_promise = memory_mapper.allocate_bytes(
        HEAP_START,
        PageEntryOption::Present | PageEntryOption::ReadWrite,
        HEAP_SIZE,
    )?;

    unsafe {
        #[allow(clippy::cast_possible_truncation)]
        ALLOCATOR.lock().init(region_promise.offset(), HEAP_SIZE);
    }
    Ok(())
}
