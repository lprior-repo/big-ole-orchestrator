use crate::codec::StorageError;

/// Timer value: stores the duration of a timer entry in big-endian bytes.
///
/// Invariant: `duration_ms > 0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerValue([u8; 8]);

impl TimerValue {
    /// Create a new `TimerValue` from duration in milliseconds.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::InvalidArgument` if `duration_ms` is zero.
    pub fn new(duration_ms: u64) -> Result<Self, StorageError> {
        if duration_ms == 0 {
            return Err(StorageError::InvalidArgument);
        }
        Ok(Self(duration_ms.to_be_bytes()))
    }

    /// Return the big-endian byte representation.
    #[must_use]
    pub const fn as_be_bytes(&self) -> [u8; 8] {
        self.0
    }

    /// Decode a big-endian byte slice into a `TimerValue`.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::InvalidArgument` if duration is zero.
    pub fn from_be_bytes(bytes: [u8; 8]) -> Result<Self, StorageError> {
        let duration_ms = u64::from_be_bytes(bytes);
        Self::new(duration_ms)
    }
}
