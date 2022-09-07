use core::{pin::Pin, task::{Poll, Context}};

use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;
use futures_util::task::AtomicWaker;
use futures_util::stream::{Stream, StreamExt};
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};

static WAKER: AtomicWaker = AtomicWaker::new();
static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();

/// Called by the keyboard interrupt handler
///
/// Must not block or allocate.
pub(crate) fn add_scancode(scancode: u8) {
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        match queue.push(scancode) {
            Ok(()) => WAKER.wake(), // wake executor
            Err(_) => log::warn!("scancode queue full; dropping keyboard input"),
        }
    } else {
        // It’s important that we don’t try to initialize the queue in this function 
        // because it will be called by the interrupt handler, 
        // which should not perform heap allocations. 
        log::warn!("scancode queue uninitialized");
    }
}

pub struct ScancodeStream {
    /// prevent construction of [`ScancodeStream`] from outside of the module.
    _private: (),
}

impl ScancodeStream {
    pub fn new() -> Self {
        SCANCODE_QUEUE.try_init_once(|| ArrayQueue::new(100))
            .expect("ScancodeStream::new should only be called once");
        ScancodeStream { _private: () }
    }
}

impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<u8>> {
        let queue = SCANCODE_QUEUE.try_get().expect("not initialized");
        // the queue is potentially empty
        if let Some(scancode) = queue.pop() {
            return Poll::Ready(Some(scancode));
        }
        // register the Waker in the WAKER static before the second check
        // a wakeup might happen before we return Poll::Pending, 
        // but it is guaranteed that we get a wakeup for any scancodes pushed after the check
        WAKER.register(&cx.waker());
        // try to pop from the queue a second time
        match queue.pop() {
            Some(scancode) => {
                // remove the registered waker
                WAKER.take();
                Poll::Ready(Some(scancode))
            }
            None => Poll::Pending,
        }
    }
}

pub async fn print_keypresses() {
    let mut scancodes = ScancodeStream::new();
    let mut keyboard = Keyboard::new(layouts::Us104Key, ScancodeSet1,
        HandleControl::Ignore);

    while let Some(scancode) = scancodes.next().await {
        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::Unicode(character) => print!("{}", character),
                    DecodedKey::RawKey(key) => print!("{:?}", key),
                }
            }
        }
    }
}