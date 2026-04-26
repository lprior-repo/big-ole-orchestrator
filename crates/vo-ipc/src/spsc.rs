use std::fmt;
use std::mem::MaybeUninit;
use std::sync::atomic::{fence, AtomicUsize, Ordering};
use std::sync::Arc;

use thiserror::Error;

pub struct SpscQueue<T> {
    buffer: *mut MaybeUninit<T>,
    cap: usize,
    head: AtomicUsize,
    tail: AtomicUsize,
}

unsafe impl<T: Send> Send for SpscQueue<T> {}
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

    pub const fn capacity(&self) -> usize {
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
        drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(self.buffer, self.cap)) });
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SpscError {
    #[error("queue is full")]
    Full,
    #[error("queue is empty")]
    Empty,
}

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

    #[test]
    fn spsc_queue_capacity_rounds_to_power_of_two() {
        let queue = Arc::new(SpscQueue::<i32>::new(5));
        assert_eq!(queue.capacity(), 8);
    }

    #[test]
    fn spsc_queue_capacity_already_power_of_two() {
        let queue = Arc::new(SpscQueue::<i32>::new(16));
        assert_eq!(queue.capacity(), 16);
    }

    #[test]
    fn spsc_queue_capacity_one() {
        let queue = Arc::new(SpscQueue::<i32>::new(1));
        assert_eq!(queue.capacity(), 1);
        let (tx, rx) = queue.sender();
        tx.send(42).unwrap();
        assert_eq!(rx.recv().unwrap(), 42);
    }

    #[test]
    fn spsc_queue_is_empty_after_drain() {
        let queue = Arc::new(SpscQueue::<String>::new(4));
        let (tx, rx) = queue.sender();
        tx.send("a".into()).unwrap();
        tx.send("b".into()).unwrap();
        rx.recv().unwrap();
        rx.recv().unwrap();
        assert!(queue.is_empty());
    }

    #[test]
    fn spsc_error_display() {
        assert_eq!(SpscError::Full.to_string(), "queue is full");
        assert_eq!(SpscError::Empty.to_string(), "queue is empty");
    }

    #[test]
    fn spsc_sender_is_full() {
        let queue = Arc::new(SpscQueue::<i32>::new(2));
        let (tx, _rx) = queue.sender();
        tx.send(1).unwrap();
        tx.send(2).unwrap();
        assert!(tx.is_full());
    }

    #[test]
    fn spsc_receiver_is_empty() {
        let queue = Arc::new(SpscQueue::<i32>::new(4));
        let (_tx, rx) = queue.sender();
        assert!(rx.is_empty());
    }

    #[test]
    fn spsc_sender_debug() {
        let queue = Arc::new(SpscQueue::<i32>::new(4));
        let (tx, _) = queue.sender();
        assert!(format!("{:?}", tx).contains("Sender"));
    }

    #[test]
    fn spsc_receiver_debug() {
        let queue = Arc::new(SpscQueue::<i32>::new(4));
        let (_, rx) = queue.sender();
        assert!(format!("{:?}", rx).contains("Receiver"));
    }
}
