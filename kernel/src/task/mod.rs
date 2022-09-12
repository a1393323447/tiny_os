pub mod executor;
pub mod keyboard;

use core::{
    future::Future, 
    pin::Pin, task::{Context, Poll},
    sync::atomic::{AtomicU64, Ordering}, ptr::NonNull,
};
use alloc::boxed::Box;
use crossbeam_queue::ArrayQueue;
use conquer_once::spin::OnceCell;

pub(super) static SPAWN_TASKS_QUEUE: OnceCell<ArrayQueue<RawTask>> = OnceCell::uninit();

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TaskId(u64);

impl TaskId {
    fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);

        // atomically increases the value and returns the previous value in one atomic operation.
        // the compiler is allowed to reorder the fetch_add operation in the instructions stream.
        TaskId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}
pub struct Task {
    id: TaskId,
    future: Pin<Box<dyn Future<Output = ()>>>,
}

impl Task {
    pub fn new(future: impl Future<Output = ()> + 'static) -> Task {
        Task {
            id: TaskId::new(),
            future: Box::pin(future),
        }
    }

    fn poll(&mut self, cx: &mut Context) -> Poll<()> {
        self.future.as_mut().poll(cx)
    }
}

#[derive(Debug)]
pub(super) struct RawTask {
    pointer: NonNull<dyn Future<Output = ()> + 'static>,
}

impl RawTask {
    pub(super) fn new(future: impl Future<Output = ()> + 'static) -> RawTask {
        let pointer = NonNull::new(Box::leak(Box::new(future)))
            .expect("Failed to allocate memory for future.");
        RawTask { pointer }
    }

    pub(super) unsafe fn into_task(self) -> Task {
        let future = Box::from_raw(self.pointer.as_ptr());
        Task { id: TaskId::new(), future: Pin::new_unchecked(future) }
    }
}

unsafe impl Send for RawTask {}

pub fn spawn(future: impl Future<Output = ()> + 'static) {
    let raw_task = RawTask::new(future);
    SPAWN_TASKS_QUEUE
        .get_or_init(|| {ArrayQueue::new(100)})
        .push(raw_task)
        .expect("queue full");
}