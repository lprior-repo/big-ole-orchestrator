//! In-Memory Lock Storage Backend
//!
//! An in-memory implementation of the `LockStorage` trait.
//! Suitable for testing and single-node deployments.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::time::{interval, Duration};

use crate::lock_storage::port::{LockStorage, LockStorageError};
use crate::{LockEntry, LockId, LockMode, LockQuery, LockQueryResponse, LockRelease, LockStatus, OwnerId};

pub struct InMemoryLockStorage {
    locks: Arc<RwLock<HashMap<LockId, Vec<LockEntry>>>>,
    cleanup_interval: Duration,
}

impl InMemoryLockStorage {
    pub fn new() -> Self {
        Self {
            locks: Arc::new(RwLock::new(HashMap::new())),
            cleanup_interval: Duration::from_secs(60),
        }
    }

    pub fn with_cleanup_interval(cleanup_interval: Duration) -> Self {
        Self {
            locks: Arc::new(RwLock::new(HashMap::new())),
            cleanup_interval,
        }
    }

    pub async fn start_cleanup_task(self: Arc<Self>) {
        let mut ticker = interval(self.cleanup_interval);
        loop {
            ticker.tick().await;
            let now = Utc::now();
            if let Err(e) = self.cleanup_expired(now).await {
                tracing::error!("lock cleanup failed: {}", e);
            }
        }
    }

    fn check_compatibility(
        existing: &[LockEntry],
        new_owner: &OwnerId,
        new_mode: LockMode,
    ) -> Result<(), LockStorageError> {
        for entry in existing {
            if entry.owner == *new_owner {
                return Err(LockStorageError::DuplicateLock(entry.lock_id.clone()));
            }

            match (entry.mode, new_mode) {
                (LockMode::Exclusive, _) => {
                    return Err(LockStorageError::NotLockOwner {
                        lock_id: entry.lock_id.clone(),
                        expected: entry.owner.clone(),
                        got: new_owner.clone(),
                    });
                }
                (LockMode::Shared, LockMode::Exclusive) => {
                    return Err(LockStorageError::IncompatibleMode);
                }
                (LockMode::Shared, LockMode::Shared) => {}
            }
        }
        Ok(())
    }

    fn find_entry<'a>(
        entries: &'a [LockEntry],
        owner: &OwnerId,
        hold_token: &str,
    ) -> Option<&'a LockEntry> {
        entries.iter().find(|e| e.owner == *owner && e.hold_token == hold_token)
    }
}

impl Default for InMemoryLockStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LockStorage for InMemoryLockStorage {
    async fn acquire(&self, entry: LockEntry) -> Result<(), LockStorageError> {
        let mut locks = self.locks.write().unwrap();

        let entries = locks.entry(entry.lock_id.clone()).or_insert_with(Vec::new);
        Self::check_compatibility(entries, &entry.owner, entry.mode)?;
        entries.push(entry);
        Ok(())
    }

    async fn release(&self, release: LockRelease) -> Result<(), LockStorageError> {
        let mut locks = self.locks.write().unwrap();

        let entries = locks
            .get_mut(&release.lock_id)
            .ok_or_else(|| LockStorageError::LockNotFound(release.lock_id.clone()))?;

        let pos = entries
            .iter()
            .position(|e| e.owner == release.owner && e.hold_token == release.hold_token)
            .ok_or_else(|| {
                if entries.iter().any(|e| e.owner == release.owner) {
                    LockStorageError::InvalidHoldToken {
                        lock_id: release.lock_id.clone(),
                        hold_token: release.hold_token,
                    }
                } else {
                    LockStorageError::NotLockOwner {
                        lock_id: release.lock_id.clone(),
                        expected: entries.first().map(|e| e.owner.clone()).unwrap_or_else(|| OwnerId::new("unknown".into())),
                        got: release.owner,
                    }
                }
            })?;

        entries.remove(pos);

        if entries.is_empty() {
            locks.remove(&release.lock_id);
        }

        Ok(())
    }

    async fn query(&self, query: LockQuery) -> Result<LockQueryResponse, LockStorageError> {
        let locks = self.locks.read().unwrap();

        let mut filtered: Vec<LockEntry> = Vec::new();
        for entries in locks.values() {
            for entry in entries {
                if let Some(ref lock_id) = query.lock_id {
                    if &entry.lock_id != lock_id {
                        continue;
                    }
                }
                if let Some(ref owner) = query.owner {
                    if &entry.owner != owner {
                        continue;
                    }
                }
                filtered.push(entry.clone());
            }
        }

        Ok(LockQueryResponse { locks: filtered })
    }

    async fn cleanup_expired(&self, now: DateTime<Utc>) -> Result<u64, LockStorageError> {
        let mut locks = self.locks.write().unwrap();
        let mut total_removed = 0u64;

        for entries in locks.values_mut() {
            let before = entries.len();
            entries.retain(|entry| entry.expires_at > now);
            let removed = before - entries.len();
            total_removed += removed as u64;
        }

        locks.retain(|_, entries| !entries.is_empty());

        Ok(total_removed)
    }

    async fn get(&self, lock_id: &LockId) -> Result<Option<LockEntry>, LockStorageError> {
        let locks = self.locks.read().unwrap();
        Ok(locks.get(lock_id).and_then(|entries| entries.first().cloned()))
    }

    async fn update_status(
        &self,
        lock_id: &LockId,
        owner: &OwnerId,
        hold_token: &str,
    ) -> Result<(), LockStorageError> {
        let locks = self.locks.read().unwrap();

        let entries = locks
            .get(lock_id)
            .ok_or_else(|| LockStorageError::LockNotFound(lock_id.clone()))?;

        Self::find_entry(entries, owner, hold_token)
            .ok_or_else(|| LockStorageError::InvalidHoldToken {
                lock_id: lock_id.clone(),
                hold_token: hold_token.to_string(),
            })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(lock_id: &str, owner: &str, mode: LockMode, ttl_ms: u64) -> LockEntry {
        LockEntry::new(
            LockId::new(lock_id),
            OwnerId::new(owner.into()),
            mode,
            ttl_ms,
        )
    }

    #[tokio::test]
    async fn acquire_and_release() {
        let storage = InMemoryLockStorage::new();
        let entry = make_entry("lock1", "owner1", LockMode::Exclusive, 1000);

        storage.acquire(entry.clone()).await.unwrap();
        let result = storage.get(&entry.lock_id).await.unwrap();
        assert_eq!(result.as_ref().map(|e| e.lock_id.clone()), Some(entry.lock_id.clone()));

        let release = LockRelease {
            lock_id: entry.lock_id.clone(),
            owner: entry.owner.clone(),
            hold_token: entry.hold_token.clone(),
        };
        storage.release(release).await.unwrap();

        let result = storage.get(&entry.lock_id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn acquire_duplicate_lock_fails() {
        let storage = InMemoryLockStorage::new();
        let entry = make_entry("lock1", "owner1", LockMode::Exclusive, 1000);

        storage.acquire(entry.clone()).await.unwrap();
        let entry2 = make_entry("lock1", "owner2", LockMode::Exclusive, 1000);
        let result = storage.acquire(entry2).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn release_wrong_owner_fails() {
        let storage = InMemoryLockStorage::new();
        let entry = make_entry("lock1", "owner1", LockMode::Exclusive, 1000);

        storage.acquire(entry.clone()).await.unwrap();

        let release = LockRelease {
            lock_id: entry.lock_id.clone(),
            owner: OwnerId::new("wrong-owner".into()),
            hold_token: entry.hold_token.clone(),
        };
        let result = storage.release(release).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn release_wrong_token_fails() {
        let storage = InMemoryLockStorage::new();
        let entry = make_entry("lock1", "owner1", LockMode::Exclusive, 1000);

        storage.acquire(entry.clone()).await.unwrap();

        let release = LockRelease {
            lock_id: entry.lock_id.clone(),
            owner: entry.owner.clone(),
            hold_token: "wrong-token".to_string(),
        };
        let result = storage.release(release).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn query_by_owner() {
        let storage = InMemoryLockStorage::new();
        let entry1 = make_entry("lock1", "owner1", LockMode::Exclusive, 1000);
        let entry2 = make_entry("lock2", "owner1", LockMode::Shared, 1000);
        let entry3 = make_entry("lock3", "owner2", LockMode::Exclusive, 1000);

        storage.acquire(entry1).await.unwrap();
        storage.acquire(entry2).await.unwrap();
        storage.acquire(entry3).await.unwrap();

        let query = LockQuery {
            lock_id: None,
            owner: Some(OwnerId::new("owner1".into())),
        };
        let result = storage.query(query).await.unwrap();
        assert_eq!(result.locks.len(), 2);
    }

    #[tokio::test]
    async fn query_by_lock_id() {
        let storage = InMemoryLockStorage::new();
        let entry1 = make_entry("lock1", "owner1", LockMode::Exclusive, 1000);
        let entry2 = make_entry("lock2", "owner2", LockMode::Exclusive, 1000);

        storage.acquire(entry1).await.unwrap();
        storage.acquire(entry2).await.unwrap();

        let query = LockQuery {
            lock_id: Some(LockId::new("lock1")),
            owner: None,
        };
        let result = storage.query(query).await.unwrap();
        assert_eq!(result.locks.len(), 1);
        assert_eq!(result.locks[0].lock_id.as_str(), "lock1");
    }

    #[tokio::test]
    async fn cleanup_expired() {
        let storage = InMemoryLockStorage::new();
        let mut entry = make_entry("lock1", "owner1", LockMode::Exclusive, 1000);
        entry.expires_at = Utc::now() - chrono::Duration::seconds(1);

        storage.acquire(entry).await.unwrap();

        let removed = storage.cleanup_expired(Utc::now()).await.unwrap();
        assert_eq!(removed, 1);

        let result = storage.get(&LockId::new("lock1")).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn shared_locks_compatible() {
        let storage = InMemoryLockStorage::new();
        let entry1 = make_entry("lock1", "owner1", LockMode::Shared, 1000);
        let entry2 = make_entry("lock1", "owner2", LockMode::Shared, 1000);

        storage.acquire(entry1).await.unwrap();
        let result = storage.acquire(entry2).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn shared_to_exclusive_incompatible() {
        let storage = InMemoryLockStorage::new();
        let entry1 = make_entry("lock1", "owner1", LockMode::Shared, 1000);
        let entry2 = make_entry("lock1", "owner2", LockMode::Exclusive, 1000);

        storage.acquire(entry1).await.unwrap();
        let result = storage.acquire(entry2).await;
        assert!(result.is_err());
    }
}
