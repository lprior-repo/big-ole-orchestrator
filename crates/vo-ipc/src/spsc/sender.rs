use std::fmt;

use super::error::SpscError;
use super::queue::SpscQueue;

pub struct Sender<T> {
    pub(crate) queue: *const SpscQueue<T>,
}

impl<T> Sender<T> {
    /// # Errors
    /// Returns `SpscError::Full` if the queue is full.
    pub fn send(&self, msg: T) -> Result<(), SpscError> {
        unsafe { (*self.queue).send(msg) }
    }

    #[must_use]
    pub fn is_full(&self) -> bool {
        unsafe { (*self.queue).is_full() }
    }
}

impl<T> fmt::Debug for Sender<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sender").finish()
    }
}
