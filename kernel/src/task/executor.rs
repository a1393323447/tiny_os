use super::{Task, TaskId, SPAWN_TASKS_QUEUE};

use alloc::{collections::BTreeMap, sync::Arc, task::Wake};
use core::task::{Waker, Context, Poll};
use crossbeam_queue::ArrayQueue;

pub struct Executor {
    /// store [`Task`]
    tasks: BTreeMap<TaskId, Task>,
    /// shared between the executor and wakers
    task_queue: Arc<ArrayQueue<TaskId>>,
    /// reuse the same waker for multiple wake-ups of the same task and
    /// ensure that reference-counted wakers are not deallocated inside interrupt handlers 
    /// because it could lead to deadlocks
    waker_cache: BTreeMap<TaskId, Waker>,
}

impl Executor {
    pub fn spawn(&mut self, task: Task) {
        let task_id = task.id;
        if self.tasks.insert(task.id, task).is_some() {
            panic!("task with same {:#?} already in tasks", task_id);
        }
        self.task_queue.push(task_id).expect("queue full");
    }
}

impl Executor {
    pub fn new() -> Executor {
        Executor { 
            tasks: BTreeMap::new(),
            task_queue: Arc::new(ArrayQueue::new(100)), 
            waker_cache: BTreeMap::new(),
        }
    }

    pub fn run(&mut self) -> ! {
        loop {
            self.get_spawn_task();
            self.run_ready_tasks();
            // interrupt can happen here
            self.sleep_if_idle();
        }
    }

    fn get_spawn_task(&mut self) {
        let queue = SPAWN_TASKS_QUEUE.get_or_init(|| { ArrayQueue::new(100) });
        while let Some(raw_task) = queue.pop() {
            let task = unsafe { raw_task.into_task() };
            self.task_queue.push(task.id).expect("queue full");
            self.tasks.insert(task.id, task);
        }
    }

    fn sleep_if_idle(&self) {
        use x86_64::instructions::interrupts::{self, enable_and_hlt};
        // 防止在检查 task_queue 是否为 empty 后, 又发生中断, 这时候 task_queue 就不是真的空的
        // hlt 会让 cpu 休息到下一个中断发生, 如果 task_queue 不是空就 hlt 就会导致这个任务可能很久都不会被处理
        interrupts::disable();
        if self.task_queue.is_empty() {
            enable_and_hlt();
        } else {
            interrupts::enable();
        }
    }

    fn run_ready_tasks(&mut self) {
        // destructure `self` to avoid borrow checker errors
        let Self {
            tasks,
            task_queue,
            waker_cache,
        } = self;

        while let Some(task_id) = task_queue.pop() {
            let task = match tasks.get_mut(&task_id) {
                Some(task) => task,
                None => continue, // task no longer exists
            };
            let waker = waker_cache
                .entry(task_id)
                .or_insert_with(|| TaskWaker::new(task_id, task_queue.clone()));
            let mut context = Context::from_waker(waker);
            match task.poll(&mut context) {
                Poll::Ready(()) => {
                    // task done -> remove it and its cached waker
                    tasks.remove(&task_id);
                    waker_cache.remove(&task_id);
                }
                Poll::Pending => {}
            }
        }
    }

}


struct TaskWaker {
    task_id: TaskId,
    task_queue: Arc<ArrayQueue<TaskId>>,
}

impl TaskWaker {
    fn new(task_id: TaskId, task_queue: Arc<ArrayQueue<TaskId>>) -> Waker {
        // the Waker type supports From conversions for all Arc-wrapped values that implement the Wake trait
        Waker::from(Arc::new(TaskWaker {
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