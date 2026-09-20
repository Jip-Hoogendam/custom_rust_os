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

///The state of the stack on a scheduler call
#[derive(Debug)]
#[repr(C, packed)]
struct ScheduleStackCall {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rbx: u64,
    rbp: u64,
    rdi: u64,
    rsi: u64,
    rdx: u64,
    rcx: u64,
    rax: u64,
}
