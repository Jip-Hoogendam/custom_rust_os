use core::{
    cmp::min,
    mem::{MaybeUninit, transmute},
    sync::atomic::AtomicBool,
    time::Duration,
};

use alloc::{boxed::Box, collections::BTreeMap};
use spin::MutexGuard;

use crate::{
    acpi::apic::LOCAL_APIC,
    arch::instructions::int3,
    halt_loop,
    interrupts::{
        call_kill, call_mutex_await,
        interupt_gaurd::{
            BlockInterupts, InteruptAlreadyBlocked, InteruptCell, InteruptCellGaurd,
            InteruptController, InteruptGaurd,
        },
    },
    multi_tasking::{ScheduleStackCall, TaskId, premtive::tcb::Thread},
    println, serial_print, serial_println,
    sync::mutex::SysMutex,
    tasks,
    time::Instant,
    tools::{CPUId, cpu_id},
    variables::{
        SCHEDULER_TASK_COUNT_MAX, SCHEDULER_TASK_CYCLES_MAX, SCHEDULER_TASK_CYCLES_MIN,
        SCHEDULER_TASKS_SLOPE_FACTOR,
    },
};

#[derive(Debug, Clone, Copy)]
pub enum ProcState {
    PromoteMutex {
        resource: &'static SysMutex,
    },
    Mutex {
        resource: &'static SysMutex,
        time: Instant,
    }, //The thread is blocked and waiting on a resource.
    Running,        //A prosessor is currently running this task
    Paused,         //This program is paused for whatever reason
    Sleep(Instant), //No processor is currently running this program
    Init,
}

//Returns the result of the schedule
#[derive(Debug, Clone, Copy)]
#[repr(u64)]
pub enum SchedulerResult {
    Resuming,
    MutexLock,
    ResumingNoMutexLock = 501,
    CannotPause = 502,
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
    current_tasks: InteruptCell<Option<Box<[CPUInfo]>>>,
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
            current_tasks: InteruptCell::new(None),
        }
    }

    #[inline]
    pub fn get_current_task_id<T: InteruptController>(
        &self,
        cpu_index: CPUId,
        interupt_gaurd: &InteruptGaurd<T>,
    ) -> Option<TaskId> {
        unsafe { self.current_tasks.get(interupt_gaurd) }
            .as_ref()
            .and_then(|tasks| tasks[cpu_index as usize].curr_task)
    }

    pub fn is_running<T: InteruptController>(
        &self,
        task_id: TaskId,
        _interupt_gaurd: &InteruptGaurd<T>,
    ) -> bool {
        SCHEDULER_TABLE.lock().get(&task_id).is_some()
    }

    #[inline]
    pub fn init<T: InteruptController>(
        &self,
        processor_count: usize,
        interupt_gaurd: &InteruptGaurd<T>,
    ) {
        let fallback = TaskInfo {
            task: Thread::init_root(),
            state: ProcState::Init,
        };

        let mut tasks = Box::new_uninit_slice(processor_count);
        for task in &mut tasks {
            task.write(CPUInfo {
                fallback: fallback.clone(),
                curr_task: None,
            });
        }
        let tasks = unsafe { transmute::<Box<[MaybeUninit<CPUInfo>]>, Box<[CPUInfo]>>(tasks) };
        unsafe { *self.current_tasks.get(interupt_gaurd) = Some(tasks) };
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
    pub fn await_resource<T: InteruptController>(
        &self,
        cpu_index: CPUId,
        resource: &'static SysMutex,
        interupt_gaurd: &InteruptGaurd<T>,
    ) {
        let mut scheduler_table = SCHEDULER_TABLE.lock();
        if let Some(task) = self.get_current_task_id(cpu_index, interupt_gaurd)
            && let Some(task) = scheduler_table.get_mut(&task)
        {
            task.state = ProcState::PromoteMutex { resource };
        } else {
            let mut tasks = unsafe { self.current_tasks.get(interupt_gaurd) };
            let tasks = tasks.as_mut().expect("Needs to have a task to await");
            let cpu_info = &mut tasks[cpu_index as usize];
            cpu_info.fallback.state = ProcState::PromoteMutex { resource };
        }
    }

    ///Kills the current tas
    ///
    /// # Panics
    /// if the task no longer exists
    pub fn kill_current_task(&self, cpu_index: CPUId) -> ! {
        InteruptGaurd::<BlockInterupts>::scope(|g| {
            if let Some(task) = self.get_current_task_id(cpu_index, g) {
                SCHEDULER_TABLE.lock().remove(&task);
            } else {
                panic!("Cannot kill non existant task");
            }

            call_kill()
        })
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

    ///Saves the state of the current task and returns the old task id
    fn save_state(
        &self,
        curr_stack: *mut ScheduleStackCall,
        task_table: &mut MutexGuard<'_, BTreeMap<TaskId, TaskInfo>>,
        cpu_info: &mut CPUInfo,
    ) -> Option<TaskId> {
        let old_task = cpu_info.curr_task.take();

        let curr_task = if let Some(task_id) = old_task {
            task_table.get_mut(&task_id)?
        } else {
            &mut cpu_info.fallback
        };

        let now = Instant::now();

        curr_task.task.rsp = curr_stack as u64;
        match curr_task.state {
            ProcState::Init | ProcState::Running => {
                curr_task.state = ProcState::Sleep(now);
            }
            ProcState::PromoteMutex { resource } => {
                curr_task.state = ProcState::Mutex {
                    resource,
                    time: now,
                };
            }
            ProcState::Mutex { .. } | ProcState::Paused | ProcState::Sleep(_) => {}
        }

        old_task
    }

    #[inline]
    fn load_task<T: InteruptController>(
        &self,
        task_count: usize,
        task: &mut TaskInfo,
        interupt_gaurd: &InteruptGaurd<T>,
        held_lock: Option<&SysMutex>,
    ) -> *mut ScheduleStackCall {
        let time = Self::compute_task_time(task_count, task);
        LOCAL_APIC.set_timer_intial_count(time, interupt_gaurd);

        let rsp = task.task.rsp as *mut ScheduleStackCall;

        match task.state {
            ProcState::Running | ProcState::Sleep(_) | ProcState::Init => {
                assert!(held_lock.is_none());

                task.state = ProcState::Running;
            }
            ProcState::PromoteMutex { .. } => {
                panic!("Cannot run a unpromoted mutex");
            }
            ProcState::Paused => {
                assert!(held_lock.is_none());
                unsafe { (*rsp).rdi = SchedulerResult::CannotPause as u64 };
            }
            ProcState::Mutex { .. } => {
                if held_lock.is_some() {
                    unsafe { (*rsp).rdi = SchedulerResult::MutexLock as u64 };
                    task.state = ProcState::Running;
                } else {
                    unsafe { (*rsp).rdi = SchedulerResult::ResumingNoMutexLock as u64 };
                }
            }
        }

        rsp
    }

    //Gives the next task that needs to be scheduled
    pub fn next<T>(
        &self,
        curr_stack: *mut ScheduleStackCall,
        interupt_gaurd: &InteruptGaurd<T>,
    ) -> *mut ScheduleStackCall
    where
        T: InteruptController,
    {
        let cpu_index = cpu_id();
        let schedule_time = Instant::now();
        let mut task_table = SCHEDULER_TABLE.lock();

        let mut tasks = unsafe { self.current_tasks.get(interupt_gaurd) };
        let Some(tasks) = tasks.as_mut() else {
            return curr_stack;
        };

        let cpu_state = &mut tasks[cpu_index as usize];
        let old_task = self.save_state(curr_stack, &mut task_table, cpu_state);

        let mut held_lock: Option<&SysMutex> = None;

        let mut min_age = match cpu_state.fallback.state {
            ProcState::Sleep(min_age) => min_age.since(schedule_time),
            ProcState::Mutex { resource, time } if resource.try_lock() => {
                held_lock = Some(resource);
                time.since(schedule_time)
            }
            _ => Duration::from_millis(10),
        };

        //Get the next task
        for (id, task_info) in task_table.iter() {
            match task_info.state {
                ProcState::Sleep(age) if age.since(schedule_time) > min_age => {
                    if let Some(lock) = held_lock.take() {
                        lock.unlock();
                    }
                    cpu_state.curr_task = Some(*id);
                    min_age = age.since(schedule_time);
                }
                ProcState::Mutex { resource, time }
                    if time.since(schedule_time) > min_age && resource.try_lock() =>
                {
                    if let Some(lock) = held_lock.take() {
                        lock.unlock();
                    }
                    cpu_state.curr_task = Some(*id);
                    min_age = time.since(schedule_time);
                    held_lock = Some(resource);
                }
                _ => (),
            }
        }

        let task_count = task_table.len();

        if let Some(new_task) = cpu_state.curr_task
            && let Some(task_info) = task_table.get_mut(&new_task)
        {
            //Load next task
            self.load_task(task_count, task_info, interupt_gaurd, held_lock)
        } else {
            //Load fallback task
            self.load_task(
                task_count,
                &mut cpu_state.fallback,
                interupt_gaurd,
                held_lock,
            )
        }
    }
}

//A simple custom scheduler for the task switch
#[unsafe(no_mangle)]
extern "C" fn schedule(stack: *mut ScheduleStackCall) -> *mut ScheduleStackCall {
    let gaurd = InteruptGaurd::<InteruptAlreadyBlocked>::block();
    let stack = SCHEDULER.next(stack, &gaurd);
    LOCAL_APIC.end_iterupts(&gaurd);
    stack
}
