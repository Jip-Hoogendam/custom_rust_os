use core::{
    cmp::min,
    mem::{MaybeUninit, transmute},
    num::{NonZeroU64, NonZeroUsize},
    ops::Bound,
};

use alloc::{boxed::Box, collections::BTreeMap};

use crate::{
    acpi::apic::LOCAL_APIC,
    interrupts::{
        call_kill, call_switch_task,
        interupt_gaurd::{
            BlockInterupts, InteruptAlreadyBlocked, InteruptCell, InteruptController,
            InteruptGaurd,
        },
    },
    multi_tasking::{TaskId, premtive::tcb::Thread},
    tools::cpu_id,
    variables::{TASK_COUNT_MAX, TASK_CYCLES_MAX, TASK_CYCLES_MIN, TASKS_SLOPE_FACTOR},
};

#[derive(PartialEq, Eq, Debug)]
pub enum ProcState {
    Resource(NonZeroUsize), //The thread is blocked and waiting on a resource.
    Running,                //A prosessor is currently running this task
    Paused,                 //This program is paused for whatever reason
    Sleep,                  //No processor is currently running this program
    Killed,                 //The thread was killed for whatever reason.
    Stopped,                //The thread is done running.
}

#[derive(Debug)]
struct TaskInfo {
    parent: Option<TaskId>,
    task: Thread,
    state: ProcState,
}

pub static SCHEDULER: Scheduler = Scheduler::new();
//Maintans what tasks need to be scheduled.
static SCHEDULER_TABLE: spin::Mutex<BTreeMap<TaskId, TaskInfo>> = spin::Mutex::new(BTreeMap::new());

pub struct Scheduler {
    /// Scheduler should only change its own prosess while interuts are blocked.
    current_tasks: InteruptCell<MaybeUninit<Box<[Option<TaskId>]>>>,
}

unsafe impl Send for Scheduler {}
unsafe impl Sync for Scheduler {}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            current_tasks: InteruptCell::new(MaybeUninit::uninit()),
        }
    }

    #[inline]
    fn get_current_task<T: InteruptController>(
        &self,
        cpu_index: usize,
        interupt_gaurd: &InteruptGaurd<T>,
    ) -> Option<TaskId> {
        unsafe { self.current_tasks.get(interupt_gaurd).assume_init_ref()[cpu_index] }
    }

    #[inline]
    fn set_current_task<T: InteruptController>(
        &self,
        cpu_index: usize,
        task: Option<TaskId>,
        interupt_gaurd: &InteruptGaurd<T>,
    ) {
        unsafe { self.current_tasks.get(interupt_gaurd).assume_init_mut()[cpu_index] = task }
    }

    #[inline]
    pub fn init<T: InteruptController>(
        &self,
        processor_count: usize,
        interupt_gaurd: &InteruptGaurd<T>,
    ) {
        let mut tasks = Box::new_uninit_slice(processor_count);
        for task in &mut tasks {
            task.write(None);
        }
        let tasks = unsafe {
            transmute::<Box<[MaybeUninit<Option<TaskId>>]>, Box<[Option<TaskId>]>>(tasks)
        };
        unsafe { self.current_tasks.get(interupt_gaurd).write(tasks) };

        let thread = Thread::init_root();
        let mut task_table = SCHEDULER_TABLE.lock();
        let id = TaskId::new();
        task_table.insert(
            id,
            TaskInfo {
                parent: None,
                task: thread,
                state: ProcState::Running,
            },
        );
        self.set_current_task(cpu_id(), Some(id), interupt_gaurd);
    }

    pub fn schedule<T>(
        &self,
        cpu_index: usize,
        thread: Thread,
        interupt_gaurd: &InteruptGaurd<T>,
    ) -> TaskId
    where
        T: InteruptController,
    {
        let mut task_table = SCHEDULER_TABLE.lock();
        let id = TaskId::new();
        let task_info = TaskInfo {
            parent: self.get_current_task(cpu_index, interupt_gaurd),
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

    pub fn await_resource<T>(&self, cpu_index: usize, resource: &T) {
        {
            let interupt_gaurd = InteruptGaurd::<BlockInterupts>::block();
            let resource = NonZeroUsize::try_from(&raw const *resource as usize)
                .unwrap_or(unsafe { NonZeroUsize::new(1).unwrap_unchecked() });
            if let Some(task) = self.get_current_task(cpu_index, &interupt_gaurd)
                && let Some(task) = SCHEDULER_TABLE.lock().get_mut(&task)
            {
                task.state = ProcState::Resource(resource);
            }
        }
        call_switch_task();
    }

    /// Notifys one thread that the resource is unblocked
    pub fn notify_one_resource<T>(&self, resource: &T) {
        {
            let interupt_gaurd = InteruptGaurd::<BlockInterupts>::block();
            let resource = NonZeroUsize::try_from(&raw const *resource as usize)
                .unwrap_or(unsafe { NonZeroUsize::new(1).unwrap_unchecked() });
            for task_info in SCHEDULER_TABLE.lock().values_mut() {
                if let ProcState::Resource(blocked_resource) = task_info.state
                    && resource == blocked_resource
                {
                    task_info.state = ProcState::Sleep;
                    drop(interupt_gaurd);
                    call_switch_task();
                    return;
                }
            }
        }
    }

    pub fn kill_current_task(&self, cpu_index: usize) -> ! {
        {
            let interupt_gaurd = InteruptGaurd::<BlockInterupts>::block();
            if let Some(task) = self.get_current_task(cpu_index, &interupt_gaurd) {
                SCHEDULER_TABLE.lock().remove(&task);
                self.set_current_task(cpu_index, None, &interupt_gaurd);
            }
        }
        call_kill()
    }

    /// Computes how much time the next thread gets. This does not yet allow giving more priority to
    /// certain tasks
    ///
    /// # Function
    /// Currently it computes the time based off the following functions where N is the number of
    /// tasks, P is the number of prosessors  and f the amount of cycles to wait.
    ///
    /// f = (-(N/(`TASK_COUNT_MAX` * P)) + 1)^`TASKS_SLOPE_FACTOR` * (`TASK_CYCLES_MAN` - `TASK_CYCLES_MIX`) + `TASK_CYCLES_MIN`
    /// FIXME: Create a scheduler that bases the time based off the taskinfo
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
        &self,
        cpu_index: usize,
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
        let current_task = self.get_current_task(cpu_index, interupt_gaurd);

        for (id, task_info) in current_task.map_or_else(
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

        let task_count = task_table.len();

        if let Some(new_task) = new_task {
            if let Some(task) = current_task
                && let Some(thread) = task_table.get_mut(&task)
            {
                thread.task.rsp = curr_stack as u64;
                thread.state = ProcState::Sleep;
            }
            let task_info = unsafe { task_table.get_mut(&new_task).unwrap_unchecked() };
            task_info.state = ProcState::Running;
            self.set_current_task(cpu_index, Some(new_task), interupt_gaurd);
            let time = Self::compute_task_time(task_count, task_info);
            LOCAL_APIC.set_timer_intial_count(time, interupt_gaurd);
            Some(unsafe { task_info.task.rsp.try_into().unwrap_unchecked() })
        } else if let Some(current_task) = current_task {
            let task_info = unsafe { task_table.get_mut(&current_task).unwrap_unchecked() };
            let time = Self::compute_task_time(task_count, task_info);
            LOCAL_APIC.set_timer_intial_count(time, interupt_gaurd);
            Some(unsafe { (curr_stack as u64).try_into().unwrap_unchecked() })
        } else {
            None
        }
    }
}

//A simple custom scheduler for the task switch
#[unsafe(no_mangle)]
extern "C" fn schedule(stack: *mut u8) -> *mut u8 {
    loop {
        let gaurd = InteruptGaurd::<InteruptAlreadyBlocked>::block();
        if let Some(next) = SCHEDULER.next(cpu_id(), stack, &gaurd) {
            let next_stack: u64 = next.into();
            LOCAL_APIC.end_iterupts(&gaurd);
            break next_stack as *mut u8;
        }
    }
}
