//! Lock Manager Port
//!
//! Defines the interface for distributed lock management.
//! Implementors must be Send + Sync.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{LockId, LockMode, LockQueryResponse, LockRelease, LockRequest, LockResponse, LockPromote, LockPromoteResponse, OwnerId};

#[async_trait]
pub trait LockManager: Send + Sync {
    /// Acquire a lock with the given parameters.
    async fn acquire(&self, request: LockRequest) -> LockResponse;

    /// Release a held lock.
    async fn release(&self, release: LockRelease) -> Result<(), crate::LockError>;

    /// Query locks by the given filter.
    async fn query(&self, query: crate::LockQuery) -> LockQueryResponse;

    /// Promote a shared lock to exclusive mode.
    async fn promote(&self, promote: LockPromote) -> LockPromoteResponse;

    /// Demote an exclusive lock to shared mode.
    async fn demote(&self, lock_id: LockId, owner: OwnerId, hold_token: String) -> Result<LockMode, crate::LockError>;

    /// Extend the TTL of a held lock.
    async fn extend_ttl(&self, lock_id: LockId, owner: OwnerId, hold_token: String, ttl_ms: u64) -> Result<DateTime<Utc>, crate::LockError>;

    /// Check if a specific lock is held.
    async fn is_locked(&self, lock_id: &LockId) -> bool;

    /// Get the current holder of a lock, if any.
    async fn get_holder(&self, lock_id: &LockId) -> Option<(OwnerId, LockMode)>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lock_manager_trait_bounds() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<dyn LockManager>();
    }
}
