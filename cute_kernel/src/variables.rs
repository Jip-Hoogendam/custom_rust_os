// This module contains global variables to set the values in the kernal. This should reduce the
// usage of magic vairables.
// These should only contain variables that a res at compile time

//---------------- GLOBAL SETTINGS

/// System timer frequency
pub const DESIRED_CLOCK_FREAKUENCY: usize = 1500; //hz

///The amount of segment discriptors the GDT will hold
pub const GDT_TABLE_SIZE: usize = 32; //Discriptors

///The allocated size of the interupt stacks
pub type InteruptStack = [u8; 4096 * 64];

pub const KERNEL_THREAD_STACK_SIZE: usize = 1024 * 16; //Bytes

//---------------- SCHEDULER OPTIONS
// The current scheduler schedules tasks in a simple logorithmic curve between the `TASK_CYCLES_MIN`
// to the `TASK_CYCLES_MAX` based of the slope factor.
/// The minimum amount of cycles a task can be scheduled for.
pub const SCHEDULER_TASK_CYCLES_MIN: u32 = 5000; //Cycles
/// The max amount of cycless a task gets when only one task is scheduled.
pub const SCHEDULER_TASK_CYCLES_MAX: u32 = 50000; //Cycles
/// How many tasks need to be scheduled before the `TASK_CYCLES_MIN` is hit
pub const SCHEDULER_TASK_COUNT_MAX: u32 = 100; //Theads
/// How fast the `clock_count` reaches min.
pub const SCHEDULER_TASKS_SLOPE_FACTOR: usize = 3;

//---------------- FRAME ALLOCATOR OPTIONS
///At what offset the frame allocator should start to keep lower regions safe for trampolines or
///other allocations.
pub const FRAME_ALLOCATOR_BEGIN_FRAME_OFFSET: usize = 0x28_0000; //640 frames
