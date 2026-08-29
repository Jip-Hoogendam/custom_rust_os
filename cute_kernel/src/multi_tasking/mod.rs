pub mod cooprative;
pub mod executor;
pub mod premtive;
pub mod scheduler;

use core::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct TaskId(u64);

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
impl TaskId {
    fn new() -> Self {
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}
