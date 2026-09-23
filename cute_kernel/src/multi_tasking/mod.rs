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
pub struct ScheduleStackCall {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rbx: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rax: u64,
}
