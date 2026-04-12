//! Distributed Lock Manager for concurrent workspace access.
//!
//! Provides distributed locking with:
//! - Acquire/release with TTL (time-to-live)
//! - Deadlock detection via wait-for graph
//! - Lock promotion (shared -> exclusive) and demotion
//! - Crash-safe lock recovery
//! - Automatic retry with exponential backoff for lock acquisition

#![allow(unused)]
#![allow(missing_docs)]

mod port;
mod retry;
mod supervisor;

use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, HashMap, HashSet};
use thiserror::Error;
use tokio::time::Duration;

pub use port::LockManager;
pub use retry::{LockManagerRetryWrapper, RetryConfig};

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

impl std::fmt::Display for LockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

impl std::fmt::Display for OwnerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

#[derive(Debug, Clone)]
pub struct WaitEdge {
    pub waiter: OwnerId,
    pub lock_id: LockId,
    pub requested_mode: LockMode,
}

#[derive(Debug, Clone, Default)]
pub struct WaitForGraph {
    wait_edges: Vec<WaitEdge>,
    lock_holders: HashMap<LockId, OwnerId>,
}

impl WaitForGraph {
    pub fn add_edge(&mut self, edge: WaitEdge) {
        self.wait_edges
            .retain(|e| !(e.waiter == edge.waiter && e.lock_id == edge.lock_id));
        self.wait_edges.push(edge);
    }

    pub fn set_lock_holder(&mut self, lock_id: LockId, owner: OwnerId) {
        self.lock_holders.insert(lock_id, owner);
    }

    pub fn remove_edges_for_owner(&mut self, owner: &OwnerId) {
        self.wait_edges.retain(|e| &e.waiter != owner);
    }

    pub fn remove_edges_for_lock(&mut self, lock_id: &LockId) {
        self.wait_edges.retain(|e| &e.lock_id != lock_id);
    }

    pub fn get_waiters(&self, lock_id: &LockId) -> Vec<OwnerId> {
        self.wait_edges
            .iter()
            .filter(|e| &e.lock_id == lock_id)
            .map(|e| e.waiter.clone())
            .collect()
    }

    pub fn detect_cycle(&self) -> Option<Vec<OwnerId>> {
        let mut in_degree: HashMap<OwnerId, usize> = HashMap::new();
        let mut adjacency: BTreeMap<OwnerId, Vec<OwnerId>> = BTreeMap::new();

        for edge in &self.wait_edges {
            if let Some(holder) = self.lock_holders.get(&edge.lock_id) {
                if holder == &edge.waiter {
                    continue;
                }
                *in_degree.entry(holder.clone()).or_insert(0) += 1;
                adjacency
                    .entry(edge.waiter.clone())
                    .or_default()
                    .push(holder.clone());
            }
        }

        let all_owners: HashSet<OwnerId> = adjacency.keys().cloned().collect();
        let mut queue: Vec<OwnerId> = all_owners
            .iter()
            .filter(|o| in_degree.get(o).copied().unwrap_or(0) == 0)
            .cloned()
            .collect();

        while let Some(owner) = queue.pop() {
            if let Some(waiters) = adjacency.get(&owner) {
                for waiter in waiters {
                    if let Some(deg) = in_degree.get_mut(waiter) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push(waiter.clone());
                        }
                    }
                }
            }
        }

        let remaining: Vec<OwnerId> = all_owners
            .into_iter()
            .filter(|o| in_degree.get(o).copied().unwrap_or(0) > 0)
            .collect();

        if remaining.is_empty() {
            None
        } else {
            Some(remaining)
        }
    }
}

pub const LOCK_SUBJECT_PREFIX: &str = "veloxide.locks";
pub const LOCK_REQUEST_SUBJECT: &str = "veloxide.locks.requests";
pub const LOCK_RESPONSE_SUBJECT: &str = "veloxide.locks.responses";
pub const LOCK_EVENT_SUBJECT: &str = "veloxide.locks.events";

fn now() -> DateTime<Utc> {
    Utc::now()
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
            expires_at: now() - chrono::Duration::seconds(1),
            ..entry
        };
        assert!(expired_entry.is_expired());
        assert!(expired_entry.remaining_ttl().is_none());
    }

    #[test]
    fn test_wait_for_graph_cycle_detection() {
        let mut graph = WaitForGraph::default();
        let owner1 = OwnerId::new("owner1".into());
        let owner2 = OwnerId::new("owner2".into());
        let lock1 = LockId::new("lock1");
        let lock2 = LockId::new("lock2");

        graph.set_lock_holder(lock1.clone(), owner1.clone());
        graph.set_lock_holder(lock2.clone(), owner2.clone());

        graph.add_edge(WaitEdge {
            waiter: owner1.clone(),
            lock_id: lock2.clone(),
            requested_mode: LockMode::Exclusive,
        });

        graph.add_edge(WaitEdge {
            waiter: owner2.clone(),
            lock_id: lock1.clone(),
            requested_mode: LockMode::Exclusive,
        });

        let cycle = graph.detect_cycle();
        assert!(cycle.is_some());
    }
}
