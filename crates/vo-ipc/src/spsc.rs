use std::fmt;
use std::mem::MaybeUninit;
use std::sync::atomic::{fence, AtomicUsize, Ordering};
use std::sync::Arc;

use thiserror::Error;
const _CACHE_LINE: usize = 64;

pub struct SpscQueue<T> {
    buffer: *mut MaybeUninit<T>,
    cap: usize,
    head: AtomicUsize,
    tail: AtomicUsize,
}

/// # Safety
///
/// `SpscQueue<T>` can be sent across threads if `T: Send` because all access
/// to the buffer is through the atomic head/tail indices which ensure only
/// one thread writes and one thread reads. The queue is designed for
/// Single-Producer Single-Consumer use only.
unsafe impl<T: Send> Send for SpscQueue<T> {}

/// # Safety
///
/// `SpscQueue<T>` can be shared between threads if `T: Send` because:
/// 1. All buffer access is synchronized via atomic head/tail operations
/// 2. The SPSC discipline ensures only one producer and one consumer
/// 3. Proper memory ordering fences prevent data races
/// 4. Arc<SpscQueue<T>> is the intended usage pattern for multi-threaded access
unsafe impl<T: Send> Sync for SpscQueue<T> {}

pub struct Sender<T> {
    queue: *const SpscQueue<T>,
}

pub struct Receiver<T> {
    queue: *const SpscQueue<T>,
}

impl<T> SpscQueue<T> {
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.next_power_of_two();
        let buffer = Box::into_raw(
            std::iter::repeat_with(MaybeUninit::<T>::uninit)
                .take(cap)
                .collect::<Box<[MaybeUninit<T>]>>(),
        )
        .cast::<MaybeUninit<T>>();
        Self {
            buffer,
            cap,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    pub fn sender(self: &Arc<Self>) -> (Sender<T>, Receiver<T>) {
        (
            Sender {
                queue: Arc::as_ptr(self),
            },
            Receiver {
                queue: Arc::as_ptr(self),
            },
        )
    }

    const fn mask(&self, idx: usize) -> usize {
        idx & (self.cap - 1)
    }

    /// # Errors
    /// Returns `SpscError::Full` if the queue is full.
    pub fn send(&self, msg: T) -> Result<(), SpscError> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);

        if head.wrapping_sub(tail) >= self.cap {
            return Err(SpscError::Full);
        }

        let slot = unsafe { &mut *self.buffer.add(self.mask(head)) };
        slot.write(msg);
        fence(Ordering::Release);
        self.head.store(head.wrapping_add(1), Ordering::Relaxed);
        Ok(())
    }

    /// # Errors
    /// Returns `SpscError::Empty` if the queue is empty.
    pub fn recv(&self) -> Result<T, SpscError> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);

        if head == tail {
            return Err(SpscError::Empty);
        }

        let slot = unsafe { &mut *self.buffer.add(self.mask(tail)) };
        let msg = unsafe { slot.assume_init_read() };
        fence(Ordering::Release);
        self.tail.store(tail.wrapping_add(1), Ordering::Relaxed);
        Ok(msg)
    }

    pub fn len(&self) -> usize {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);
        head.wrapping_sub(tail)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn capacity(&self) -> usize {
        self.cap
    }
}

impl<T> Sender<T> {
    /// # Errors
    /// Returns `SpscError::Full` if the queue is full.
    pub fn send(&self, msg: T) -> Result<(), SpscError> {
        unsafe { (*self.queue).send(msg) }
    }

    #[must_use]
    pub fn is_full(&self) -> bool {
        let q = unsafe { &*self.queue };
        let head = q.head.load(Ordering::Relaxed);
        let tail = q.tail.load(Ordering::Acquire);
        head.wrapping_sub(tail) >= q.cap
    }
}

impl<T> Receiver<T> {
    /// # Errors
    /// Returns `SpscError::Empty` if the queue is empty.
    pub fn recv(&self) -> Result<T, SpscError> {
        unsafe { (*self.queue).recv() }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        let q = unsafe { &*self.queue };
        q.head.load(Ordering::Acquire) == q.tail.load(Ordering::Relaxed)
    }
}

impl<T> Drop for SpscQueue<T> {
    fn drop(&mut self) {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Relaxed);
        let mut idx = tail;
        while idx != head {
            let slot = unsafe { &mut *self.buffer.add(self.mask(idx)) };
            unsafe { slot.assume_init_drop() };
            idx = idx.wrapping_add(1);
        }
        unsafe {
            let slice = std::slice::from_raw_parts_mut(self.buffer, self.cap);
            let _ = Box::from_raw(slice);
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for SpscQueue<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SpscQueue")
            .field("capacity", &self.cap)
            .field("len", &self.len())
            .finish_non_exhaustive()
    }
}

impl<T> fmt::Debug for Sender<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sender").finish()
    }
}

impl<T> fmt::Debug for Receiver<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Receiver").finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpscError {
    Full,
    Empty,
}

impl SpscError {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Full => "queue is full",
            Self::Empty => "queue is empty",
        }
    }
}

impl std::fmt::Display for SpscError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full => write!(f, "queue is full"),
            Self::Empty => write!(f, "queue is empty"),
        }
    }
}

#[allow(clippy::missing_errors_doc)]
impl std::error::Error for SpscError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spsc_queue_basic_send_recv() {
        let queue = Arc::new(SpscQueue::<i32>::new(8));
        let (tx, rx) = queue.sender();

        tx.send(1).unwrap();
        tx.send(2).unwrap();
        tx.send(3).unwrap();

        assert_eq!(rx.recv().unwrap(), 1);
        assert_eq!(rx.recv().unwrap(), 2);
        assert_eq!(rx.recv().unwrap(), 3);
    }

    #[test]
    fn spsc_queue_full_error() {
        let queue = Arc::new(SpscQueue::<i32>::new(2));
        let (tx, _rx) = queue.sender();

        tx.send(1).unwrap();
        tx.send(2).unwrap();
        assert_eq!(tx.send(3), Err(SpscError::Full));
    }

    #[test]
    fn spsc_queue_empty_error() {
        let queue = Arc::new(SpscQueue::<i32>::new(8));
        let (_tx, rx) = queue.sender();

        assert_eq!(rx.recv(), Err(SpscError::Empty));
    }

    #[test]
    fn spsc_queue_len() {
        let queue = Arc::new(SpscQueue::<i32>::new(8));
        let (tx, rx) = queue.sender();

        assert_eq!(queue.len(), 0);
        tx.send(1).unwrap();
        assert_eq!(queue.len(), 1);
        tx.send(2).unwrap();
        assert_eq!(queue.len(), 2);
        rx.recv().unwrap();
        assert_eq!(queue.len(), 1);
        rx.recv().unwrap();
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn spsc_queue_wraparound() {
        let queue = Arc::new(SpscQueue::<i32>::new(4));
        let (tx, rx) = queue.sender();

        for i in 0..4 {
            tx.send(i).unwrap();
        }
        assert_eq!(queue.len(), 4);

        rx.recv().unwrap();
        rx.recv().unwrap();
        assert_eq!(queue.len(), 2);

        tx.send(100).unwrap();
        tx.send(101).unwrap();
        assert_eq!(queue.len(), 4);

        for i in 0..4 {
            let val = rx.recv().unwrap();
            if i < 2 {
                assert_eq!(val, i + 2);
            } else {
                assert_eq!(val, i - 2 + 100);
            }
        }
    }

    #[test]
    fn spsc_queue_debug() {
        let queue = SpscQueue::<i32>::new(8);
        assert!(format!("{:?}", queue).contains("SpscQueue"));
    }
}

#[test]
fn spsc_queue_sync_thread_safe() {
    let queue = Arc::new(SpscQueue::<i32>::new(8));
    let queue_clone = Arc::clone(&queue);

    let handle = std::thread::spawn(move || {
        for i in 0..100 {
            loop {
                if queue_clone.send(i).is_ok() {
                    break;
                }
                std::hint::spin_loop();
            }
        }
    });

    let mut received = 0;
    while received < 100 {
        while let Ok(_) = queue.recv() {
            received += 1;
        }
        if received < 100 {
            std::hint::spin_loop();
        }
    }

    handle.join().unwrap();
}
