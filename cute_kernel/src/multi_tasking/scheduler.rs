use core::{array, cell::UnsafeCell, cmp::min, num::NonZeroU64, ops::Bound};

use alloc::{boxed::Box, collections::BTreeMap};

use crate::{
    acpi::apic::LOCAL_APIC,
    interrupts::interupt_gaurd::{
        self, InteruptAlreadyBlocked, InteruptCell, InteruptController, InteruptGaurd,
    },
    multi_tasking::{TaskId, premtive::tcb::Thread},
    serial_print,
    variables::{TASK_COUNT_MAX, TASK_CYCLES_MAX, TASK_CYCLES_MIN, TASKS_SLOPE_FACTOR},
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
    /// Scheduler should only change its own prosess while interuts are blocked.
    current_tasks: InteruptCell<Box<[TaskId]>>,
}

unsafe impl Send for Scheduler {}
unsafe impl Sync for Scheduler {}

impl Scheduler {
    #[must_use]
    pub const fn new() -> Self {
        Self { current_task: None }
    }

    #[inline]
    pub const fn current_task(&self) -> TaskId {}

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
        self.current_task = Some(id);
    }

    pub fn schedule<T>(&self, thread: Thread, interupt_gaurd: &InteruptGaurd<T>) -> TaskId
    where
        T: InteruptController,
    {
        let mut task_table = SCHEDULER_TABLE.lock();
        let id = TaskId::new();
        let task_info = TaskInfo {
            parent: self.current_task,
            task: thread,
            state: ProcState::Sleep,
        };
        if task_table.len() <= 1 {
            let time = Self::compute_task_time(task_table.len(), &task_info);
            LOCAL_APIC.set_timer_intial_count(time, interupt_gaurd);
        }
        task_table.insert(id, task_info);
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

    /// Computes how much time the next thread gets. This does not yet allow giving more priority to
    /// certain tasks
    ///
    /// # Function
    /// Currently it computes the time based off the following functions where N is the number of
    /// tasks, P is the number of prosessors  and f the amount of cycles to wait.
    ///
    /// f = (-(N/(`TASK_COUNT_MAX` * P)) + 1)^`TASKS_SLOPE_FACTOR` * (`TASK_CYCLES_MAN` - `TASK_CYCLES_MIX`) + `TASK_CYCLES_MIN`
    fn compute_task_time(task_count: usize, _task: &TaskInfo) -> u32 {
        const {
            debug_assert!(TASK_COUNT_MAX > 1);
            debug_assert!(TASK_CYCLES_MIN < TASK_CYCLES_MAX);
        }
        let task_count = min(task_count as u32, TASK_COUNT_MAX);
        let n = (-(task_count as f32 / TASK_COUNT_MAX as f32)) + 1.;
        let pow_n = [n; TASKS_SLOPE_FACTOR].iter().fold(1f32, |a, b| a * b);
        (pow_n * (TASK_CYCLES_MAX - TASK_CYCLES_MIN) as f32) as u32 + TASK_CYCLES_MIN
    }

    //Gives the next task that needs to be scheduled
    pub fn next<T>(
        &mut self,
        curr_stack: *mut u8,
        interupt_gaurd: &InteruptGaurd<T>,
    ) -> Option<NonZeroU64>
    where
        T: InteruptController,
    {
        let mut task_table = SCHEDULER_TABLE.lock();
        if task_table.is_empty() {
            return Some(unsafe { (curr_stack as u64).try_into().unwrap_unchecked() });
        }

        let mut new_task = None;

        for (id, task_info) in self.current_task.map_or_else(
            || {
                task_table
                    .range(..)
                    .chain(task_table.range((Bound::Unbounded, Bound::Excluded(TaskId(0)))))
            },
            |task| {
                task_table
                    .range((Bound::Excluded(task), Bound::Unbounded))
                    .chain(task_table.range((Bound::Unbounded, Bound::Excluded(task))))
            },
        ) {
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
            let task_info = task_table.get(&task).unwrap_unchecked();
            let time = Self::compute_task_time(task_table.len(), task_info);
            LOCAL_APIC.set_timer_intial_count(time, interupt_gaurd);
            task_info.task.rsp.try_into().unwrap_unchecked()
        })
    }
}

pub const fn init() {}

//A simple custom scheduler for the task switch
#[unsafe(no_mangle)]
extern "C" fn schedule(stack: *mut u8) -> *mut u8 {
    loop {
        let gaurd = InteruptGaurd::<InteruptAlreadyBlocked>::block();
        if let Some(next) = SCHEDULER.lock().next(stack, &gaurd) {
            let next_stack: u64 = next.into();
            LOCAL_APIC.end_iterupts(&gaurd);
            break next_stack as *mut u8;
        }
    }
}
