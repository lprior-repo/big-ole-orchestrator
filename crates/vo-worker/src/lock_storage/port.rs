//! Distributed Lock Storage Port
//!
//! Defines the interface for distributed lock state storage.
//! Implementors must be Send + Sync.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{LockEntry, LockId, LockMode, LockQuery, LockQueryResponse, LockRelease, OwnerId};

#[async_trait]
pub trait LockStorage: Send + Sync {
    async fn acquire(&self, entry: LockEntry) -> Result<(), LockStorageError>;

    async fn release(&self, release: LockRelease) -> Result<(), LockStorageError>;

    async fn query(&self, query: LockQuery) -> Result<LockQueryResponse, LockStorageError>;

    async fn cleanup_expired(&self, now: DateTime<Utc>) -> Result<u64, LockStorageError>;

    async fn get(&self, lock_id: &LockId) -> Result<Option<LockEntry>, LockStorageError>;

    async fn update_status(
        &self,
        lock_id: &LockId,
        owner: &OwnerId,
        hold_token: &str,
    ) -> Result<(), LockStorageError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockStorageError {
    LockNotFound(LockId),
    NotLockOwner {
        lock_id: LockId,
        expected: OwnerId,
        got: OwnerId,
    },
    InvalidHoldToken {
        lock_id: LockId,
        hold_token: String,
    },
    DuplicateLock(LockId),
    IncompatibleMode,
    Storage(String),
    SerializationFailed(String),
    DeserializationFailed(String),
    InvalidArgument(String),
}

impl LockStorageError {
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::Storage(_) | Self::DuplicateLock(_))
    }

    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            Self::InvalidHoldToken { .. }
                | Self::InvalidArgument(_)
                | Self::SerializationFailed(_)
                | Self::DeserializationFailed(_)
        )
    }
}

impl std::fmt::Display for LockStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LockNotFound(lock_id) => write!(f, "Lock not found: {}", lock_id),
            Self::NotLockOwner {
                lock_id,
                expected,
                got,
            } => {
                write!(
                    f,
                    "Not lock owner of {}: expected {}, got {}",
                    lock_id, expected, got
                )
            }
            Self::InvalidHoldToken {
                lock_id,
                hold_token,
            } => {
                write!(
                    f,
                    "Invalid hold token '{}' for lock {}",
                    hold_token, lock_id
                )
            }
            Self::DuplicateLock(lock_id) => write!(f, "Duplicate lock: {}", lock_id),
            Self::IncompatibleMode => write!(f, "Lock held in incompatible mode"),
            Self::Storage(s) => write!(f, "Storage error: {}", s),
            Self::SerializationFailed(s) => write!(f, "Serialization failed: {}", s),
            Self::DeserializationFailed(s) => write!(f, "Deserialization failed: {}", s),
            Self::InvalidArgument(s) => write!(f, "Invalid argument: {}", s),
        }
    }
}

impl std::error::Error for LockStorageError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_storage_error_is_transient() {
        assert!(LockStorageError::Storage("test".to_string()).is_transient());
        assert!(LockStorageError::DuplicateLock(LockId::new("test")).is_transient());
        assert!(!LockStorageError::InvalidArgument("test".to_string()).is_transient());
    }

    #[test]
    fn lock_storage_error_is_fatal() {
        assert!(LockStorageError::InvalidHoldToken {
            lock_id: LockId::new("test"),
            hold_token: "bad".to_string()
        }
        .is_fatal());
        assert!(LockStorageError::InvalidArgument("test".to_string()).is_fatal());
        assert!(!LockStorageError::Storage("test".to_string()).is_fatal());
    }
}
