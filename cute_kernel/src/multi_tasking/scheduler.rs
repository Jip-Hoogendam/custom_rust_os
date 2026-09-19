use core::{
    cmp::min,
    mem::{MaybeUninit, transmute},
    num::{NonZeroU64, NonZeroUsize},
    ops::Bound,
    time::Duration,
};

use alloc::{boxed::Box, collections::BTreeMap};

use crate::{
    acpi::apic::LOCAL_APIC,
    interrupts::{
        call_kill, call_switch_task,
        interupt_gaurd::{
            BlockInterupts, InteruptAlreadyBlocked, InteruptCell, InteruptController, InteruptGaurd,
        },
    },
    multi_tasking::{TaskId, premtive::tcb::Thread},
    time::Instant,
    tools::{CPUId, cpu_id},
    variables::{
        SCHEDULER_TASK_COUNT_MAX, SCHEDULER_TASK_CYCLES_MAX, SCHEDULER_TASK_CYCLES_MIN,
        SCHEDULER_TASKS_SLOPE_FACTOR,
    },
};

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum ProcState {
    Resource {
        resource: NonZeroUsize,
        time: Instant,
    }, //The thread is blocked and waiting on a resource.
    Running,        //A prosessor is currently running this task
    Paused,         //This program is paused for whatever reason
    Sleep(Instant), //No processor is currently running this program
    Killed,         //The thread was killed for whatever reason.
    Stopped,        //The thread is done running.
    // A special "state" for the task. By loading one or more of these tasks types into the shceduler
    // then the fallback will run like any other task. The state actual state of a fallback task is
    // maintained by each cpu.
    Fallback,
}

#[derive(Debug, Clone)]
struct TaskInfo {
    task: Thread,
    state: ProcState,
}

pub static SCHEDULER: Scheduler = Scheduler::new();
//Maintans what tasks need to be scheduled.
static SCHEDULER_TABLE: spin::Mutex<BTreeMap<TaskId, TaskInfo>> = spin::Mutex::new(BTreeMap::new());

//A struct to hold the current state of the proseesor and the fallback task
//If the curr_task is none then it is currently running the fallback task.
#[derive(Debug)]
struct CPUInfo {
    fallback: TaskInfo,
    curr_task: Option<TaskId>,
}

pub struct Scheduler {
    /// Scheduler should only change its own prosess while interuts are blocked.
    current_tasks: InteruptCell<MaybeUninit<Box<[CPUInfo]>>>,
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
        cpu_index: CPUId,
        interupt_gaurd: &InteruptGaurd<T>,
    ) -> Option<TaskId> {
        unsafe {
            self.current_tasks.get(interupt_gaurd).assume_init_ref()[cpu_index as usize].curr_task
        }
    }

    //To set the current task to the fallback just run the fallback
    #[inline]
    fn set_current_task<T: InteruptController>(
        &self,
        cpu_index: CPUId,
        task: Option<TaskId>,
        interupt_gaurd: &InteruptGaurd<T>,
    ) {
        unsafe {
            self.current_tasks.get(interupt_gaurd).assume_init_mut()[cpu_index as usize]
                .curr_task = task;
        }
    }

    pub fn is_running<T: InteruptController>(
        &self,
        task_id: TaskId,
        _interupt_gaurd: &InteruptGaurd<T>,
    ) -> bool {
        if let Some(task) = SCHEDULER_TABLE.lock().get(&task_id)
            && task.state != ProcState::Killed
        {
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn init<T: InteruptController>(
        &self,
        processor_count: usize,
        interupt_gaurd: &InteruptGaurd<T>,
    ) {
        let fallback = TaskInfo {
            task: Thread::init_root(),
            state: ProcState::Stopped,
        };

        let mut tasks = Box::new_uninit_slice(processor_count);
        for task in &mut tasks {
            task.write(CPUInfo {
                fallback: fallback.clone(),
                curr_task: None,
            });
        }
        let tasks = unsafe { transmute::<Box<[MaybeUninit<CPUInfo>]>, Box<[CPUInfo]>>(tasks) };
        unsafe { self.current_tasks.get(interupt_gaurd).write(tasks) };
    }

    ///Schedulers a new task
    pub fn schedule<T>(&self, thread: Thread, interupt_gaurd: &InteruptGaurd<T>) -> TaskId
    where
        T: InteruptController,
    {
        let mut task_table = SCHEDULER_TABLE.lock();
        let id = TaskId::new();
        let now = Instant::now();
        let task_info = TaskInfo {
            task: thread,
            state: ProcState::Sleep(now),
        };
        if task_table.len() <= 1 {
            let time = Self::compute_task_time(task_table.len(), &task_info);
            LOCAL_APIC.set_timer_intial_count(time, interupt_gaurd);
        }
        task_table.insert(id, task_info);
        id
    }

    ///Awaits on a resource
    pub fn await_resource<T>(&self, cpu_index: CPUId, resource: &T) {
        {
            let interupt_gaurd = InteruptGaurd::<BlockInterupts>::block();
            let resource = NonZeroUsize::try_from(&raw const *resource as usize)
                .unwrap_or(unsafe { NonZeroUsize::new(1).unwrap_unchecked() });
            if let Some(task) = self.get_current_task(cpu_index, &interupt_gaurd)
                && let Some(task) = SCHEDULER_TABLE.lock().get_mut(&task)
            {
                task.state = ProcState::Resource {
                    resource,
                    time: Instant::now(),
                };
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
                if let ProcState::Resource {
                    resource: blocked_resource,
                    time,
                } = task_info.state
                    && resource == blocked_resource
                {
                    task_info.state = ProcState::Sleep(time);
                    drop(interupt_gaurd);
                    call_switch_task();
                    return;
                }
            }
        }
    }

    ///Kills the current tas
    pub fn kill_current_task(&self, cpu_index: CPUId) -> ! {
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
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_precision_loss)]
    #[allow(clippy::cast_possible_truncation)]
    fn compute_task_time(task_count: usize, _task: &TaskInfo) -> u32 {
        const {
            debug_assert!(SCHEDULER_TASK_COUNT_MAX > 1);
            debug_assert!(SCHEDULER_TASK_CYCLES_MIN < SCHEDULER_TASK_CYCLES_MAX);
        }
        let task_count = min(task_count as u32, SCHEDULER_TASK_COUNT_MAX);
        let n = (-(task_count as f32 / SCHEDULER_TASK_COUNT_MAX as f32)) + 1.;
        let pow_n = [n; SCHEDULER_TASKS_SLOPE_FACTOR]
            .iter()
            .fold(1f32, |a, b| a * b);
        (pow_n * (SCHEDULER_TASK_CYCLES_MAX - SCHEDULER_TASK_CYCLES_MIN) as f32) as u32
            + SCHEDULER_TASK_CYCLES_MIN
    }

    //Gives the next task that needs to be scheduled
    pub fn next<T>(
        &self,
        cpu_index: u16,
        curr_stack: *mut u8,
        interupt_gaurd: &InteruptGaurd<T>,
    ) -> NonZeroU64
    where
        T: InteruptController,
    {
        let schedule_time = Instant::now();
        let mut task_table = SCHEDULER_TABLE.lock();
        let mut new_task = None;
        let mut tasks = unsafe { self.current_tasks.get(interupt_gaurd) };
        let cpu_state = unsafe { &mut tasks.assume_init_mut()[cpu_index as usize] };

        let mut min_age = match cpu_state.fallback.state {
            ProcState::Stopped => {
                cpu_state.fallback.state = ProcState::Running;
                Duration::ZERO
            }
            ProcState::Sleep(min_age) => min_age.elapsed(),
            _ => Duration::from_millis(10),
        };

        //Get the next task
        for (id, task_info) in cpu_state.curr_task.map_or_else(
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
            if let ProcState::Sleep(age) = task_info.state
                && age.elapsed() > min_age
            {
                new_task = Some(*id);
                min_age = age.elapsed();
            }
        }

        let task_count = task_table.len();

        if let Some(new_task) = new_task {
            //Unsets the previous task
            if let Some(task) = cpu_state.curr_task
                && let Some(thread) = task_table.get_mut(&task)
            {
                thread.task.rsp = curr_stack as u64;
                thread.state = ProcState::Sleep(schedule_time);
            } else if cpu_state.curr_task.is_none() {
                cpu_state.fallback.task.rsp = curr_stack as u64;
                cpu_state.fallback.state = ProcState::Sleep(schedule_time);
            }

            //Load next task
            let task_info = unsafe { task_table.get_mut(&new_task).unwrap_unchecked() };
            task_info.state = ProcState::Running;
            self.set_current_task(cpu_index, Some(new_task), interupt_gaurd);
            let time = Self::compute_task_time(task_count, task_info);
            LOCAL_APIC.set_timer_intial_count(time, interupt_gaurd);
            unsafe { task_info.task.rsp.try_into().unwrap_unchecked() }
        } else {
            if let Some(id) = cpu_state.curr_task
                && let Some(thread) = task_table.get_mut(&id)
            {
                thread.state = ProcState::Sleep(schedule_time);
                thread.task.rsp = curr_stack as u64;
                cpu_state.curr_task = None;
            }
            //Load fallback task
            let time = Self::compute_task_time(task_count, &cpu_state.fallback);
            LOCAL_APIC.set_timer_intial_count(time, interupt_gaurd);
            match cpu_state.fallback.state {
                ProcState::Stopped | ProcState::Running => unsafe {
                    (curr_stack as u64).try_into().unwrap_unchecked()
                },
                _ => {
                    cpu_state.fallback.state = ProcState::Running;
                    unsafe { (cpu_state.fallback.task.rsp).try_into().unwrap_unchecked() }
                }
            }
        }
    }
}

//A simple custom scheduler for the task switch
#[unsafe(no_mangle)]
extern "C" fn schedule(stack: *mut u8) -> *mut u8 {
    let gaurd = InteruptGaurd::<InteruptAlreadyBlocked>::block();
    let next = SCHEDULER.next(cpu_id(), stack, &gaurd);
    let next_stack: u64 = next.into();
    LOCAL_APIC.end_iterupts(&gaurd);
    next_stack as *mut u8
}
