use std::fmt;
use std::mem::MaybeUninit;
use std::sync::atomic::{fence, AtomicUsize, Ordering};
use std::sync::Arc;

use super::error::SpscError;
use super::receiver::Receiver;
use super::sender::Sender;

pub struct SpscQueue<T> {
    buffer: *mut MaybeUninit<T>,
    cap: usize,
    head: AtomicUsize,
    tail: AtomicUsize,
}

unsafe impl<T: Send> Send for SpscQueue<T> {}
unsafe impl<T: Send> Sync for SpscQueue<T> {}

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

    pub fn is_full(&self) -> bool {
        self.len() >= self.cap
    }

    pub const fn capacity(&self) -> usize {
        self.cap
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
