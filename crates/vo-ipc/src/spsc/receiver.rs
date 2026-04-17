use std::fmt;

use super::error::SpscError;
use super::queue::SpscQueue;

pub struct Receiver<T> {
    pub(crate) queue: *const SpscQueue<T>,
}

impl<T> Receiver<T> {
    /// # Errors
    /// Returns `SpscError::Empty` if the queue is empty.
    pub fn recv(&self) -> Result<T, SpscError> {
        unsafe { (*self.queue).recv() }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        unsafe { (*self.queue).is_empty() }
    }
}

impl<T> fmt::Debug for Receiver<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Receiver").finish()
    }
}
