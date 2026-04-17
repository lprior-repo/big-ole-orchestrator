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

mod connector;
pub use connector::{
    CommitOutcome, Connector, ConnectorError, ConnectorRegistry, HttpConnector, PreparedEffect,
    ReconcileOutcome,
};
pub mod executor;
pub use executor::{
    ExecutionOutcome, ManagedEffectError, ManagedEffectExecutor, ManagedEffectTask,
};
pub mod pool;
mod port;
pub mod retry;
mod storage;
pub mod supervisor;

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
mod connector_tests {
    use super::connector::*;
    use crate::connector::ConnectorRegistry;

    struct NoopConnector;

    #[async_trait::async_trait]
    impl Connector for NoopConnector {
        fn connector_type(&self) -> &str {
            "noop"
        }
        fn connector_version(&self) -> &str {
            "0.1.0"
        }
        fn supports_compensation(&self) -> bool {
            false
        }
        async fn prepare(
            &self,
            _intent: serde_json::Value,
            effect_id: String,
            fence: u64,
        ) -> Result<PreparedEffect, ConnectorError> {
            Ok(PreparedEffect {
                effect_id,
                payload: serde_json::json!({}),
                fence,
            })
        }
        async fn commit(&self, _prepared: PreparedEffect) -> Result<CommitOutcome, ConnectorError> {
            Ok(CommitOutcome::Committed {
                receipt: "noop".into(),
            })
        }
        async fn reconcile(&self, _effect_id: &str) -> Result<ReconcileOutcome, ConnectorError> {
            Ok(ReconcileOutcome::NotCommitted)
        }
    }

    #[test]
    fn connector_error_retryable_is_retryable() {
        let err = ConnectorError::retryable("timeout");
        assert!(err.is_retryable());
    }

    #[test]
    fn connector_error_terminal_is_not_retryable() {
        let err = ConnectorError::terminal("auth failed");
        assert!(!err.is_retryable());
    }

    #[test]
    fn connector_error_compensation_not_supported() {
        let err = ConnectorError::compensation_not_supported("http");
        assert!(err.is_retryable());
        assert!(err.to_string().contains("http"));
    }

    #[test]
    fn prepared_effect_carries_fields() {
        let pe = PreparedEffect {
            effect_id: "fx-1".to_string(),
            payload: serde_json::json!({"method": "POST"}),
            fence: 42,
        };
        assert_eq!(pe.effect_id, "fx-1");
        assert_eq!(pe.fence, 42);
    }

    #[test]
    fn prepared_effect_serde_round_trip() {
        let pe = PreparedEffect {
            effect_id: "fx-2".to_string(),
            payload: serde_json::json!({"key": "val"}),
            fence: 7,
        };
        let s = serde_json::to_string(&pe).unwrap();
        let recovered: PreparedEffect = serde_json::from_str(&s).unwrap();
        assert_eq!(recovered.effect_id, pe.effect_id);
        assert_eq!(recovered.fence, pe.fence);
    }

    #[test]
    fn commit_outcome_variants() {
        let _ = CommitOutcome::Committed {
            receipt: "r".into(),
        };
        let _ = CommitOutcome::Failed;
        let _ = CommitOutcome::Ambiguous;
    }

    #[test]
    fn reconcile_outcome_maps_to_reconcile_action() {
        use vo_types::ReconcileAction;
        assert_eq!(
            ReconcileAction::from(ReconcileOutcome::Committed {
                receipt: "r".into()
            }),
            ReconcileAction::Commit,
        );
        assert_eq!(
            ReconcileAction::from(ReconcileOutcome::NotCommitted),
            ReconcileAction::Rollback,
        );
        assert_eq!(
            ReconcileAction::from(ReconcileOutcome::StillAmbiguous),
            ReconcileAction::Retry,
        );
    }

    #[tokio::test]
    async fn default_compensate_returns_not_supported() {
        let c = NoopConnector;
        let result = c.compensate(serde_json::json!({}), "cx-1".into(), 1).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("noop"));
    }

    #[tokio::test]
    async fn noop_connector_prepare_commit_cycle() {
        let c = NoopConnector;
        let pe = c
            .prepare(
                serde_json::json!({"url": "https://example.com"}),
                "fx-1".into(),
                1,
            )
            .await
            .unwrap();
        assert_eq!(pe.effect_id, "fx-1");
        let outcome = c.commit(pe).await.unwrap();
        assert_eq!(
            outcome,
            CommitOutcome::Committed {
                receipt: "noop".into()
            }
        );
    }

    #[tokio::test]
    async fn registry_register_and_get() {
        let mut reg = ConnectorRegistry::new();
        assert!(reg.get("noop").is_none());
        reg.register("noop".to_string(), Box::new(NoopConnector));
        assert!(reg.get("noop").is_some());
    }

    #[tokio::test]
    async fn registry_list() {
        let mut reg = ConnectorRegistry::new();
        assert!(reg.list().is_empty());
        reg.register("noop".to_string(), Box::new(NoopConnector));
        assert_eq!(reg.list(), vec!["noop"]);
    }

    #[tokio::test]
    async fn http_connector_type_and_version() {
        let c = crate::connector::HttpConnector::new("https://api.example.com");
        assert_eq!(c.connector_type(), "http");
        assert_eq!(c.connector_version(), "1.0.0");
        assert!(!c.supports_compensation());
    }

    #[tokio::test]
    async fn http_connector_prepare_includes_idempotency_key() {
        let c = crate::connector::HttpConnector::new("https://api.example.com");
        let pe = c
            .prepare(
                serde_json::json!({"method": "POST", "path": "/charges"}),
                "fx-42".into(),
                7,
            )
            .await
            .unwrap();
        assert_eq!(pe.effect_id, "fx-42");
        assert_eq!(pe.fence, 7);
        assert_eq!(pe.payload["idempotency_key"], "fx-42:7");
        assert_eq!(pe.payload["base_url"], "https://api.example.com");
    }

    #[test]
    fn connector_error_kind_classification() {
        let retryable = ConnectorError::retryable("timeout");
        assert!(retryable.is_retryable());
        let terminal = ConnectorError::terminal("bad request");
        assert!(!terminal.is_retryable());
    }
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

    #[test]
    fn test_wait_for_graph_no_cycle() {
        let mut graph = WaitForGraph::default();
        let owner1 = OwnerId::new("owner1".into());
        let owner2 = OwnerId::new("owner2".into());
        let owner3 = OwnerId::new("owner3".into());
        let lock1 = LockId::new("lock1");
        let lock2 = LockId::new("lock2");

        graph.set_lock_holder(lock1.clone(), owner1.clone());
        graph.set_lock_holder(lock2.clone(), owner2.clone());

        graph.add_edge(WaitEdge {
            waiter: owner3.clone(),
            lock_id: lock1.clone(),
            requested_mode: LockMode::Shared,
        });

        let cycle = graph.detect_cycle();
        assert!(cycle.is_none());
    }

    #[test]
    fn test_wait_for_graph_empty() {
        let graph = WaitForGraph::default();
        assert!(graph.detect_cycle().is_none());
    }

    #[test]
    fn test_wait_for_graph_self_loop() {
        let mut graph = WaitForGraph::default();
        let owner = OwnerId::new("owner1".into());
        let lock = LockId::new("lock1");

        graph.set_lock_holder(lock.clone(), owner.clone());

        graph.add_edge(WaitEdge {
            waiter: owner.clone(),
            lock_id: lock.clone(),
            requested_mode: LockMode::Exclusive,
        });

        let cycle = graph.detect_cycle();
        assert!(cycle.is_none());
    }

    #[test]
    fn test_wait_for_graph_three_way_cycle() {
        let mut graph = WaitForGraph::default();
        let o1 = OwnerId::new("o1".into());
        let o2 = OwnerId::new("o2".into());
        let o3 = OwnerId::new("o3".into());
        let l1 = LockId::new("l1");
        let l2 = LockId::new("l2");
        let l3 = LockId::new("l3");

        graph.set_lock_holder(l1.clone(), o1.clone());
        graph.set_lock_holder(l2.clone(), o2.clone());
        graph.set_lock_holder(l3.clone(), o3.clone());

        graph.add_edge(WaitEdge {
            waiter: o1.clone(),
            lock_id: l2.clone(),
            requested_mode: LockMode::Exclusive,
        });
        graph.add_edge(WaitEdge {
            waiter: o2.clone(),
            lock_id: l3.clone(),
            requested_mode: LockMode::Exclusive,
        });
        graph.add_edge(WaitEdge {
            waiter: o3.clone(),
            lock_id: l1.clone(),
            requested_mode: LockMode::Exclusive,
        });

        let cycle = graph.detect_cycle();
        assert!(cycle.is_some());
        assert!(cycle.unwrap().len() == 3);
    }

    #[test]
    fn test_wait_for_graph_get_waiters() {
        let mut graph = WaitForGraph::default();
        let o1 = OwnerId::new("o1".into());
        let o2 = OwnerId::new("o2".into());
        let lock = LockId::new("lock1");

        graph.add_edge(WaitEdge {
            waiter: o1.clone(),
            lock_id: lock.clone(),
            requested_mode: LockMode::Shared,
        });
        graph.add_edge(WaitEdge {
            waiter: o2.clone(),
            lock_id: lock.clone(),
            requested_mode: LockMode::Exclusive,
        });

        let waiters = graph.get_waiters(&lock);
        assert_eq!(waiters.len(), 2);
    }

    #[test]
    fn test_wait_for_graph_remove_edges_for_owner() {
        let mut graph = WaitForGraph::default();
        let o1 = OwnerId::new("o1".into());
        let o2 = OwnerId::new("o2".into());
        let lock = LockId::new("lock1");

        graph.add_edge(WaitEdge {
            waiter: o1.clone(),
            lock_id: lock.clone(),
            requested_mode: LockMode::Shared,
        });
        graph.add_edge(WaitEdge {
            waiter: o2.clone(),
            lock_id: lock.clone(),
            requested_mode: LockMode::Exclusive,
        });

        graph.remove_edges_for_owner(&o1);
        let waiters = graph.get_waiters(&lock);
        assert_eq!(waiters.len(), 1);
        assert_eq!(waiters[0], o2);
    }

    #[test]
    fn test_wait_for_graph_remove_edges_for_lock() {
        let mut graph = WaitForGraph::default();
        let o1 = OwnerId::new("o1".into());
        let lock1 = LockId::new("lock1");
        let lock2 = LockId::new("lock2");

        graph.add_edge(WaitEdge {
            waiter: o1.clone(),
            lock_id: lock1.clone(),
            requested_mode: LockMode::Shared,
        });
        graph.add_edge(WaitEdge {
            waiter: o1.clone(),
            lock_id: lock2.clone(),
            requested_mode: LockMode::Exclusive,
        });

        graph.remove_edges_for_lock(&lock1);
        assert!(graph.get_waiters(&lock1).is_empty());
        assert_eq!(graph.get_waiters(&lock2).len(), 1);
    }

    #[test]
    fn test_wait_for_graph_add_edge_deduplicates() {
        let mut graph = WaitForGraph::default();
        let o1 = OwnerId::new("o1".into());
        let lock = LockId::new("lock1");

        graph.add_edge(WaitEdge {
            waiter: o1.clone(),
            lock_id: lock.clone(),
            requested_mode: LockMode::Shared,
        });
        graph.add_edge(WaitEdge {
            waiter: o1.clone(),
            lock_id: lock.clone(),
            requested_mode: LockMode::Exclusive,
        });

        let waiters = graph.get_waiters(&lock);
        assert_eq!(waiters.len(), 1);
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

    #[test]
    fn test_lock_mode_shared_shared_compatible() {
        assert!(!LockMode::Shared.can_upgrade_to(LockMode::Shared));
        assert!(!LockMode::Exclusive.can_upgrade_to(LockMode::Exclusive));
    }
}
