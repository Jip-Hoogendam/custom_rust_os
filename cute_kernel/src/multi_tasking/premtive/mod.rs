use core::time::Duration;

use crate::{
    interrupts::interupt_gaurd::{self, BlockInterupts, InteruptGaurd},
    multi_tasking::{
        TaskId,
        premtive::tcb::ThreadOptions,
        scheduler::{SCHEDULER, Scheduler},
    },
    tools::cpu_id,
    variables::KERNEL_THREAD_STACK_SIZE,
};

pub mod stack;
pub mod tcb;
use alloc::boxed::Box;
use tcb::Thread;

unsafe extern "C" {
    pub unsafe fn context_restore(old_rsp: *mut *mut u8, new_rsp: *mut *mut u8);
}

/// This function is used to trampoline into a lamda capture
extern "C" fn trampoline(arg: u64) {
    unsafe {
        Box::from_raw((arg as *mut u64).cast::<Box<dyn FnOnce() + Send>>())();
    }
}

pub enum ThreadError {
    NoPlaceInQueue,
    InitFailed,
    EmptyStack,
}

#[derive(Debug)]
pub struct ThreadHandle {
    id: TaskId,
}

impl ThreadHandle {
    #[inline]
    pub fn join(&self) {
        InteruptGaurd::<BlockInterupts>::block_halt_while(|g| SCHEDULER.is_running(self.id, &g));
    }
}

#[test_case]
fn join_task() {
    use crate::time::Instant;
    const WAIT_TIME: Duration = Duration::from_secs(3);

    let start = Instant::now();
    spawn(|| {
        use crate::tools::sleep;
        use core::time::Duration;
        sleep(Duration::from_secs(3));
    })
    .join();
    if start.elapsed() < WAIT_TIME {
        panic!("Did not properly join thread");
    }
}

#[inline]
fn schedule(entry: *const (), options: &ThreadOptions) -> ThreadHandle {
    let stack = stack::Stack::new(KERNEL_THREAD_STACK_SIZE);
    let tcb = Thread::spawn(stack, entry, options);
    let id = InteruptGaurd::<BlockInterupts>::scope(|interupt_gaurd| {
        SCHEDULER.schedule(tcb, interupt_gaurd)
    });
    ThreadHandle { id }
}

pub fn spawn(f: impl FnOnce() + 'static + Send) -> ThreadHandle {
    let closure =
        alloc::boxed::Box::new(alloc::boxed::Box::new(f) as alloc::boxed::Box<dyn FnOnce() + Send>);
    let capture = alloc::boxed::Box::into_raw(closure);
    schedule(
        trampoline as *const (),
        &ThreadOptions {
            argument_0: capture as usize,
            ..Default::default()
        },
    )
}

pub fn thread(entry: fn()) -> ThreadHandle {
    schedule(entry as *const (), &ThreadOptions::default())
}

//gets called when a thread has ended
pub extern "C" fn thread_exit() -> ! {
    SCHEDULER.kill_current_task(cpu_id());
}
