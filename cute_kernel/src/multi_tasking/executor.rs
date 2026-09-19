use core::{
    error::Error,
    fmt::Display,
    task::{Context, Poll, Waker},
};

use alloc::{collections::BTreeMap, sync::Arc, task::Wake};
use crossbeam_queue::ArrayQueue;

use super::{TaskId, cooprative::Task};

pub struct Executor {
    tasks: BTreeMap<TaskId, (Task, Waker)>,
    task_queue: Arc<ArrayQueue<TaskId>>,
}

impl Default for Executor {
    fn default() -> Self {
        Self {
            tasks: BTreeMap::new(),
            task_queue: Arc::new(ArrayQueue::new(100)),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ExecutorError {
    TaskQueueFull,
    InvalidTaskId,
}
impl Display for ExecutorError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl Error for ExecutorError {}

impl Executor {
    /// Spawns a new async task on the executor
    ///
    /// # Errors
    /// - Returns a `ExecutorError::InvalidTaskId` if the task id gets reused
    /// - Returns a `ExecutorError::TaskQueueFull` if the task queue is full
    pub fn spawn(&mut self, task: Task) -> Result<(), ExecutorError> {
        let task_id = task.get_id();

        let Self { tasks, task_queue } = self;

        if tasks
            .insert(
                task_id,
                (task, TaskWaker::new_waker(task_id, task_queue.clone())),
            )
            .is_some()
        {
            Err(ExecutorError::InvalidTaskId)?;
        }

        if self.task_queue.push(task_id).is_ok() {
            Ok(())
        } else {
            Err(ExecutorError::TaskQueueFull)
        }
    }

    pub fn run_ready_tasks(&mut self) {
        while let Some(task_id) = self.task_queue.pop() {
            let Some((task, waker)) = self.tasks.get_mut(&task_id) else {
                continue;
            };

            let mut context = Context::from_waker(waker);
            match task.poll(&mut context) {
                Poll::Ready(()) => {
                    self.tasks.remove(&task_id);
                }
                Poll::Pending => {}
            }
        }
    }

    pub fn run(&mut self) -> ! {
        loop {
            self.run_ready_tasks();
            self.sleep_if_idle();
        }
    }

    fn sleep_if_idle(&self) {
        use crate::interrupts::interupt_gaurd::{BlockInterupts, InteruptGaurd};
        InteruptGaurd::<BlockInterupts>::block_halt_if(|_| self.task_queue.is_empty());
    }
}

struct TaskWaker {
    task_id: TaskId,
    task_queue: Arc<ArrayQueue<TaskId>>,
}

impl TaskWaker {
    fn new_waker(task_id: TaskId, task_queue: Arc<ArrayQueue<TaskId>>) -> Waker {
        Waker::from(Arc::new(Self {
            task_id,
            task_queue,
        }))
    }

    fn wake_task(&self) {
        self.task_queue.push(self.task_id).expect("task_queue full");
    }
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_task();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_task();
    }
}
