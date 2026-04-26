//! Lock error types.

use std::fmt;
use thiserror::Error;

use crate::lock_types::{LockId, OwnerId};

#[derive(Debug, Error)]
pub enum LockError {
    #[error("lock not found: {0}")]
    NotFound(LockId),
    #[error("not lock owner: expected {expected}, got {got}")]
    NotOwner { expected: OwnerId, got: OwnerId },
    #[error("invalid hold token")]
    InvalidToken,
    #[error("deadlock detected")]
    DeadlockDetected,
    #[error("lock held in incompatible mode")]
    IncompatibleMode,
    #[error("TTL must be positive, got {0}")]
    InvalidTtl(u64),
    #[error("NATS communication error: {0}")]
    Nats(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("timeout waiting for lock")]
    Timeout,
    #[error("already holds lock in shared mode, cannot upgrade")]
    UpgradeWouldDeadlock,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_error_variants() {
        let _ = LockError::NotFound(LockId::new("x"));
        let _ = LockError::NotOwner {
            expected: OwnerId::new("a".into()),
            got: OwnerId::new("b".into()),
        };
        let _ = LockError::InvalidToken;
        let _ = LockError::DeadlockDetected;
        let _ = LockError::IncompatibleMode;
        let _ = LockError::InvalidTtl(0);
        let _ = LockError::Nats("conn err".into());
        let _ = LockError::Storage("io err".into());
        let _ = LockError::Timeout;
        let _ = LockError::UpgradeWouldDeadlock;
    }
}
