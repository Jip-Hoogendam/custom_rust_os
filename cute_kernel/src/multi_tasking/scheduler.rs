use core::{
    num::NonZeroU64,
    ops::Bound,
    ptr::null_mut,
    sync::atomic::{AtomicU64, Ordering},
};

use alloc::collections::{BTreeMap, LinkedList, btree_set::Range};

use crate::{
    multi_tasking::{TaskId, cooprative::Task, premtive::tcb::Thread},
    println,
};

#[derive(PartialEq, Eq, Debug)]
pub enum Resource {} //stub for the resource a process might be waiting for

#[derive(PartialEq, Eq, Debug)]
pub enum ProcState {
    Blocked(Resource), //The thread is blocked and waiting on a resource
    Running,           //A prosessor is currently running this task
    Paused,            //This program is paused for whatever reason
    Sleep,             //No processor is currently running this program
    Killed,            //The thread was killed for whatever reason.
    Stopped,           //The thread is done running.
}

#[derive(Debug)]
struct TaskInfo {
    parent: Option<TaskId>,
    task: Thread,
    state: ProcState,
}

pub static SCHEDULER: spin::Mutex<Scheduler> = spin::Mutex::new(Scheduler::new());
//Maintans what tasks need to be scheduled.
static SCHEDULER_TABLE: spin::Mutex<BTreeMap<TaskId, TaskInfo>> = spin::Mutex::new(BTreeMap::new());

#[derive(Default)]
pub struct Scheduler {
    current_task: Option<TaskId>,
}

unsafe impl Send for Scheduler {}
unsafe impl Sync for Scheduler {}

impl Scheduler {
    pub const fn new() -> Scheduler {
        Scheduler { current_task: None }
    }

    #[inline]
    pub fn init(&mut self) {
        let thread = Thread::init_root();
        let mut task_table = SCHEDULER_TABLE.lock();
        let id = TaskId::new();
        task_table.insert(
            id,
            TaskInfo {
                parent: self.current_task,
                task: thread,
                state: ProcState::Running,
            },
        );
        self.current_task = Some(id)
    }

    pub fn schedule(&mut self, thread: Thread) -> TaskId {
        let mut task_table = SCHEDULER_TABLE.lock();
        let id = TaskId::new();
        task_table.insert(
            id,
            TaskInfo {
                parent: self.current_task,
                task: thread,
                state: ProcState::Sleep,
            },
        );
        id
    }

    pub fn kill_current_task(&mut self) {
        //!Todo schedule the reaper to run
        if let Some(task) = self.current_task {
            if let Some(task_info) = SCHEDULER_TABLE.lock().get_mut(&task) {
                task_info.state = ProcState::Killed;
            }
            self.current_task = None;
        }
    }

    //Gives the next task that needs to be scheduled
    pub fn next(&mut self, curr_stack: *mut u8) -> Option<NonZeroU64> {
        let mut task_table = SCHEDULER_TABLE.lock();
        if task_table.is_empty() {
            return Some(unsafe { (curr_stack as u64).try_into().unwrap_unchecked() });
        }

        let mut new_task = None;

        for (id, task_info) in if let Some(task) = self.current_task {
            task_table
                .range((Bound::Excluded(task), Bound::Unbounded))
                .chain(task_table.range((Bound::Unbounded, Bound::Excluded(task))))
        } else {
            task_table
                .range(..)
                .chain(task_table.range((Bound::Unbounded, Bound::Excluded(TaskId(0)))))
        } {
            if matches!(task_info.state, ProcState::Sleep) {
                new_task = Some(*id);
                break;
            }
        }

        if let Some(new_task) = new_task {
            if let Some(task) = self.current_task
                && let Some(thread) = task_table.get_mut(&task)
            {
                thread.task.rsp = curr_stack as u64;
                thread.state = ProcState::Sleep;
            }
            let task_info = unsafe { task_table.get_mut(&new_task).unwrap_unchecked() };
            task_info.state = ProcState::Running;
            self.current_task = Some(new_task);
        }

        self.current_task.map(|task| unsafe {
            task_table
                .get(&task)
                .unwrap_unchecked()
                .task
                .rsp
                .try_into()
                .unwrap_unchecked()
        })
    }
}

pub fn init() {}

//A simple custom scheduler for the task switch
#[unsafe(no_mangle)]
extern "C" fn schedule(stack: *mut u8) -> *mut u8 {
    loop {
        if let Some(next) = SCHEDULER.lock().next(stack) {
            let next_stack: u64 = next.into();
            return next_stack as *mut u8;
        }
    }
}
