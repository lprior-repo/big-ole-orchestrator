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

// SAFETY: SpscQueue is accessed through &self by both Sender (producer) and
// Receiver (consumer) on separate threads. The producer exclusively writes
// `head` and the consumer exclusively writes `tail`; each reads the other's
// counter with Acquire ordering to synchronize access to the shared buffer.
// `T: Send` ensures values transferred via the buffer are safe to send
// between threads. No interior mutability on T is exposed — values move
// from producer to consumer via MaybeUninit slots.
unsafe impl<T: Send> Send for SpscQueue<T> {}
unsafe impl<T: Send> Sync for SpscQueue<T> {}

pub struct Sender<T> {
    queue: Arc<SpscQueue<T>>,
}

pub struct Receiver<T> {
    queue: Arc<SpscQueue<T>>,
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
                queue: self.clone(),
            },
            Receiver {
                queue: self.clone(),
            },
        )
    }

    const fn mask(&self, idx: usize) -> usize {
        // cap is always a power of two (guaranteed by `new`), so cap-1 is a
        // bitmask that maps any idx into [0, cap). This ensures pointer
        // arithmetic via buffer.add(mask(idx)) is always in-bounds.
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

        // SAFETY: `mask(head)` returns a value in [0, cap) because cap is a
        // power of two (see mask()). The caller verified head - tail < cap
        // (queue not full), so this slot is not concurrently being read by the
        // consumer. The write to the slot is followed by a Release fence
        // before head is advanced, ensuring the consumer sees the written
        // value when it loads head with Acquire.
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

        // SAFETY: `mask(tail)` returns a value in [0, cap). The caller
        // verified head != tail (queue not empty), so this slot contains a
        // value previously written by the producer. The producer's Release
        // fence (before storing head) synchronizes with our Acquire load of
        // head, so assume_init_read observes a fully initialized value.
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
        self.queue.send(msg)
    }

    #[must_use]
    pub fn is_full(&self) -> bool {
        let q = &self.queue;
        let head = q.head.load(Ordering::Relaxed);
        let tail = q.tail.load(Ordering::Acquire);
        head.wrapping_sub(tail) >= q.cap
    }
}

impl<T> Receiver<T> {
    /// # Errors
    /// Returns `SpscError::Empty` if the queue is empty.
    pub fn recv(&self) -> Result<T, SpscError> {
        self.queue.recv()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        let q = &self.queue;
        q.head.load(Ordering::Acquire) == q.tail.load(Ordering::Relaxed)
    }
}

impl<T> Drop for SpscQueue<T> {
    fn drop(&mut self) {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Relaxed);
        let mut idx = tail;
        while idx != head {
            // SAFETY: mask(idx) is in [0, cap). Each iteration advances idx
            // through the same indices that send() used to write values,
            // so each slot was previously initialized by the producer.
            let slot = unsafe { &mut *self.buffer.add(self.mask(idx)) };
            unsafe { slot.assume_init_drop() };
            idx = idx.wrapping_add(1);
        }
        // SAFETY: buffer was allocated via Box::into_raw with exactly cap
        // elements. Reconstructing the slice from the same pointer and length
        // and dropping it reclaims the original allocation.
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
        let (tx, rx) = queue.sender();

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
