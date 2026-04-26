//! Lock domain types for distributed lock management.

use chrono::{DateTime, Utc};
use std::fmt;
use tokio::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LockId(String);

impl LockId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct OwnerId(String);

impl OwnerId {
    pub fn new(id: String) -> Self {
        Self(id)
    }
}

impl fmt::Display for OwnerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockMode {
    Shared,
    Exclusive,
}

impl LockMode {
    pub fn can_upgrade_to(&self, other: LockMode) -> bool {
        matches!((self, other), (LockMode::Shared, LockMode::Exclusive))
    }

    pub fn can_downgrade_to(&self, other: LockMode) -> bool {
        matches!((self, other), (LockMode::Exclusive, LockMode::Shared))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockStatus {
    Held,
    Pending,
    Expired,
}

#[derive(Debug, Clone)]
pub struct LockEntry {
    pub lock_id: LockId,
    pub owner: OwnerId,
    pub mode: LockMode,
    pub status: LockStatus,
    pub acquired_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub hold_token: String,
}

impl LockEntry {
    pub fn new(lock_id: LockId, owner: OwnerId, mode: LockMode, ttl_ms: u64) -> Self {
        let now = Utc::now();
        let expires_at = now + chrono::Duration::milliseconds(ttl_ms as i64);
        Self {
            lock_id,
            owner,
            mode,
            status: LockStatus::Held,
            acquired_at: now,
            expires_at,
            hold_token: ulid::Ulid::new().to_string(),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at < Utc::now()
    }

    pub fn remaining_ttl(&self) -> Option<Duration> {
        let remaining = self.expires_at - Utc::now();
        if remaining.num_milliseconds() <= 0 {
            None
        } else {
            Some(Duration::from_millis(remaining.num_milliseconds() as u64))
        }
    }
}

#[derive(Debug, Clone)]
pub struct LockRequest {
    pub lock_id: LockId,
    pub owner: OwnerId,
    pub mode: LockMode,
    pub ttl_ms: u64,
    pub request_id: String,
}

#[derive(Debug, Clone)]
pub struct LockResponse {
    pub request_id: String,
    pub lock_id: LockId,
    pub owner: OwnerId,
    pub granted: bool,
    pub hold_token: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LockRelease {
    pub lock_id: LockId,
    pub owner: OwnerId,
    pub hold_token: String,
}

#[derive(Debug, Clone)]
pub struct LockQuery {
    pub lock_id: Option<LockId>,
    pub owner: Option<OwnerId>,
}

#[derive(Debug, Clone)]
pub struct LockQueryResponse {
    pub locks: Vec<LockEntry>,
}

#[derive(Debug, Clone)]
pub struct LockPromote {
    pub lock_id: LockId,
    pub owner: OwnerId,
    pub hold_token: String,
    pub new_mode: LockMode,
}

#[derive(Debug, Clone)]
pub struct LockPromoteResponse {
    pub request_id: String,
    pub lock_id: LockId,
    pub granted: bool,
    pub new_mode: Option<LockMode>,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_mode_upgrade() {
        assert!(LockMode::Shared.can_upgrade_to(LockMode::Exclusive));
        assert!(!LockMode::Exclusive.can_upgrade_to(LockMode::Shared));
    }

    #[test]
    fn test_lock_mode_downgrade() {
        assert!(LockMode::Exclusive.can_downgrade_to(LockMode::Shared));
        assert!(!LockMode::Shared.can_downgrade_to(LockMode::Exclusive));
    }

    #[test]
    fn test_lock_entry_expiry() {
        let owner = OwnerId::new("owner1".into());
        let lock_id = LockId::new("test");
        let entry = LockEntry::new(lock_id, owner, LockMode::Exclusive, 1000);

        assert!(!entry.is_expired());
        assert!(entry.remaining_ttl().is_some());

        let expired_entry = LockEntry {
            expires_at: Utc::now() - chrono::Duration::seconds(1),
            ..entry
        };
        assert!(expired_entry.is_expired());
        assert!(expired_entry.remaining_ttl().is_none());
    }

    #[test]
    fn test_lock_id_display() {
        let id = LockId::new("my-lock");
        assert_eq!(format!("{}", id), "my-lock");
    }

    #[test]
    fn test_owner_id_display() {
        let id = OwnerId::new("owner-1".into());
        assert_eq!(format!("{}", id), "owner-1");
    }

    #[test]
    fn test_lock_mode_shared_shared_compatible() {
        assert!(!LockMode::Shared.can_upgrade_to(LockMode::Shared));
        assert!(!LockMode::Exclusive.can_upgrade_to(LockMode::Exclusive));
    }
}
