pub mod executor;
pub mod keyboard;

use core::{
    future::Future, 
    pin::Pin, task::{Context, Poll},
    sync::atomic::{AtomicU64, Ordering},
};
use alloc::boxed::Box;

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
