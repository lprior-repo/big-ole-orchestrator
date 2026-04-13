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

pub mod connector;
pub mod port;
pub mod retry;
pub mod storage;
pub mod supervisor;

use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, HashMap, HashSet};
use thiserror::Error;
use tokio::time::Duration;

pub use port::LockManager;
pub use retry::{LockManagerRetryWrapper, RetryConfig};

pub use connector::{
    CommitOutcome, Connector, ConnectorError, ConnectorRegistry, HttpConnector, PreparedEffect,
    ReconcileOutcome,
};

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
        fn connector_type(&self) -> &str { "noop" }
        fn connector_version(&self) -> &str { "0.1.0" }
        fn supports_compensation(&self) -> bool { false }
        async fn prepare(
            &self, _intent: serde_json::Value, effect_id: String, fence: u64,
        ) -> Result<PreparedEffect, ConnectorError> {
            Ok(PreparedEffect { effect_id, payload: serde_json::json!({}), fence })
        }
        async fn commit(
            &self, _prepared: PreparedEffect,
        ) -> Result<CommitOutcome, ConnectorError> {
            Ok(CommitOutcome::Committed { receipt: "noop".into() })
        }
        async fn reconcile(
            &self, _effect_id: &str,
        ) -> Result<ReconcileOutcome, ConnectorError> {
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
        let _ = CommitOutcome::Committed { receipt: "r".into() };
        let _ = CommitOutcome::Failed;
        let _ = CommitOutcome::Ambiguous;
    }

    #[test]
    fn reconcile_outcome_maps_to_reconcile_action() {
        use vo_types::ReconcileAction;
        assert_eq!(
            ReconcileAction::from(ReconcileOutcome::Committed { receipt: "r".into() }),
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
        let pe = c.prepare(serde_json::json!({"url": "https://example.com"}), "fx-1".into(), 1).await.unwrap();
        assert_eq!(pe.effect_id, "fx-1");
        let outcome = c.commit(pe).await.unwrap();
        assert_eq!(outcome, CommitOutcome::Committed { receipt: "noop".into() });
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
        let pe = c.prepare(
            serde_json::json!({"method": "POST", "path": "/charges"}),
            "fx-42".into(), 7,
        ).await.unwrap();
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
    fn test_lock_entry_new_with_ttl() {
        let owner = OwnerId::new("owner1".into());
        let lock_id = LockId::new("test-lock");
        let entry = LockEntry::new(lock_id, owner, LockMode::Exclusive, 5000);

        assert_eq!(entry.lock_id.0, "test-lock");
        assert_eq!(entry.owner.0, "owner1");
        assert_eq!(entry.mode, LockMode::Exclusive);
        assert_eq!(entry.status, LockStatus::Held);
        assert!(entry.expires_at > entry.acquired_at);
        assert!(!entry.hold_token.is_empty());
    }

    #[test]
    fn test_lock_entry_remaining_ttl_expired() {
        let owner = OwnerId::new("owner1".into());
        let lock_id = LockId::new("expired-lock");
        let expired_entry = LockEntry {
            lock_id: lock_id.clone(),
            owner: owner.clone(),
            mode: LockMode::Exclusive,
            status: LockStatus::Expired,
            acquired_at: now() - chrono::Duration::seconds(10),
            expires_at: now() - chrono::Duration::seconds(5),
            hold_token: "token".to_string(),
        };

        assert!(expired_entry.is_expired());
        assert!(expired_entry.remaining_ttl().is_none());
    }

    #[test]
    fn test_lock_entry_remaining_ttl_valid() {
        let owner = OwnerId::new("owner1".into());
        let lock_id = LockId::new("valid-lock");
        let entry = LockEntry {
            lock_id: lock_id.clone(),
            owner: owner.clone(),
            mode: LockMode::Exclusive,
            status: LockStatus::Held,
            acquired_at: now() - chrono::Duration::seconds(1),
            expires_at: now() + chrono::Duration::seconds(100),
            hold_token: "token".to_string(),
        };

        assert!(!entry.is_expired());
        assert!(entry.remaining_ttl().is_some());
    }

    #[test]
    fn test_lock_request_fields() {
        let request = LockRequest {
            lock_id: LockId::new("my-lock"),
            owner: OwnerId::new("owner-123".into()),
            mode: LockMode::Shared,
            ttl_ms: 30000,
            request_id: "req-abc".to_string(),
        };

        assert_eq!(request.lock_id.0, "my-lock");
        assert_eq!(request.owner.0, "owner-123");
        assert_eq!(request.mode, LockMode::Shared);
        assert_eq!(request.ttl_ms, 30000);
        assert_eq!(request.request_id, "req-abc");
    }

    #[test]
    fn test_lock_response_fields() {
        let response = LockResponse {
            request_id: "req-1".to_string(),
            lock_id: LockId::new("lock-1"),
            owner: OwnerId::new("owner-1".into()),
            granted: true,
            hold_token: Some("token-xyz".to_string()),
            expires_at: Some(now() + chrono::Duration::seconds(60)),
            error: None,
        };

        assert!(response.granted);
        assert_eq!(response.lock_id.as_str(), "lock-1");
        assert_eq!(response.hold_token, Some("token-xyz".to_string()));
    }

    #[test]
    fn test_lock_response_denied() {
        let response = LockResponse {
            request_id: "req-2".to_string(),
            lock_id: LockId::new("lock-2"),
            owner: OwnerId::new("owner-2".into()),
            granted: false,
            hold_token: None,
            expires_at: None,
            error: Some("lock held by another".to_string()),
        };

        assert!(!response.granted);
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap(), "lock held by another");
    }

    #[test]
    fn test_lock_release_fields() {
        let release = LockRelease {
            lock_id: LockId::new("release-lock"),
            owner: OwnerId::new("release-owner".into()),
            hold_token: "release-token".to_string(),
        };

        assert_eq!(release.lock_id.0, "release-lock");
        assert_eq!(release.owner.0, "release-owner");
        assert_eq!(release.hold_token, "release-token");
    }

    #[test]
    fn test_lock_query_fields() {
        let query = LockQuery {
            lock_id: Some(LockId::new("query-lock")),
            owner: Some(OwnerId::new("query-owner".into())),
        };

        assert!(query.lock_id.is_some());
        assert!(query.owner.is_some());
    }

    #[test]
    fn test_lock_query_empty() {
        let query = LockQuery {
            lock_id: None,
            owner: None,
        };

        assert!(query.lock_id.is_none());
        assert!(query.owner.is_none());
    }

    #[test]
    fn test_lock_promote_fields() {
        let promote = LockPromote {
            lock_id: LockId::new("promote-lock"),
            owner: OwnerId::new("promote-owner".into()),
            hold_token: "promote-token".to_string(),
            new_mode: LockMode::Exclusive,
        };

        assert_eq!(promote.lock_id.as_str(), "promote-lock");
        assert_eq!(promote.new_mode, LockMode::Exclusive);
    }

    #[test]
    fn test_lock_promote_response_granted() {
        let response = LockPromoteResponse {
            request_id: "promote-req".to_string(),
            lock_id: LockId::new("promote-lock"),
            granted: true,
            new_mode: Some(LockMode::Exclusive),
            error: None,
        };

        assert!(response.granted);
        assert_eq!(response.new_mode, Some(LockMode::Exclusive));
    }

    #[test]
    fn test_lock_promote_response_denied() {
        let response = LockPromoteResponse {
            request_id: "promote-req-2".to_string(),
            lock_id: LockId::new("promote-lock-2"),
            granted: false,
            new_mode: None,
            error: Some("cannot upgrade from exclusive".to_string()),
        };

        assert!(!response.granted);
        assert!(response.error.is_some());
    }

    #[test]
    fn test_lock_query_response_empty() {
        let response = LockQueryResponse { locks: vec![] };
        assert!(response.locks.is_empty());
    }

    #[test]
    fn test_lock_query_response_with_locks() {
        let owner = OwnerId::new("owner".into());
        let lock_id = LockId::new("q-lock");
        let entry = LockEntry::new(lock_id, owner, LockMode::Shared, 10000);

        let response = LockQueryResponse { locks: vec![entry] };
        assert_eq!(response.locks.len(), 1);
        assert_eq!(response.locks[0].lock_id.as_str(), "q-lock");
    }

    #[test]
    fn test_wait_edge_fields() {
        let edge = WaitEdge {
            waiter: OwnerId::new("waiter-1".into()),
            lock_id: LockId::new("wait-lock"),
            requested_mode: LockMode::Exclusive,
        };

        assert_eq!(edge.waiter.0, "waiter-1");
        assert_eq!(edge.lock_id.0, "wait-lock");
        assert_eq!(edge.requested_mode, LockMode::Exclusive);
    }

    #[test]
    fn test_wait_for_graph_add_edge() {
        let mut graph = WaitForGraph::default();
        let waiter = OwnerId::new("new-waiter".into());
        let lock = LockId::new("new-lock");

        graph.add_edge(WaitEdge {
            waiter: waiter.clone(),
            lock_id: lock.clone(),
            requested_mode: LockMode::Exclusive,
        });

        let waiters = graph.get_waiters(&lock);
        assert_eq!(waiters.len(), 1);
        assert_eq!(waiters[0].0, "new-waiter");
    }

    #[test]
    fn test_wait_for_graph_remove_edges_for_owner() {
        let mut graph = WaitForGraph::default();
        let owner1 = OwnerId::new("owner1".into());
        let owner2 = OwnerId::new("owner2".into());
        let lock = LockId::new("multi-lock");

        graph.add_edge(WaitEdge {
            waiter: owner1.clone(),
            lock_id: lock.clone(),
            requested_mode: LockMode::Exclusive,
        });
        graph.add_edge(WaitEdge {
            waiter: owner2.clone(),
            lock_id: lock.clone(),
            requested_mode: LockMode::Shared,
        });

        graph.remove_edges_for_owner(&owner1);

        let waiters = graph.get_waiters(&lock);
        assert_eq!(waiters.len(), 1);
        assert_eq!(waiters[0].0, "owner2");
    }

    #[test]
    fn test_wait_for_graph_remove_edges_for_lock() {
        let mut graph = WaitForGraph::default();
        let owner = OwnerId::new("owner".into());
        let lock1 = LockId::new("graph-lock-1");
        let lock2 = LockId::new("graph-lock-2");

        graph.add_edge(WaitEdge {
            waiter: owner.clone(),
            lock_id: lock1.clone(),
            requested_mode: LockMode::Exclusive,
        });
        graph.add_edge(WaitEdge {
            waiter: owner.clone(),
            lock_id: lock2.clone(),
            requested_mode: LockMode::Exclusive,
        });

        graph.remove_edges_for_lock(&lock1);

        let waiters = graph.get_waiters(&lock1);
        assert!(waiters.is_empty());

        let waiters = graph.get_waiters(&lock2);
        assert_eq!(waiters.len(), 1);
    }

    #[test]
    fn test_wait_for_graph_no_cycle() {
        let mut graph = WaitForGraph::default();
        let owner1 = OwnerId::new("owner1".into());
        let owner2 = OwnerId::new("owner2".into());
        let lock = LockId::new("no-cycle-lock");

        graph.set_lock_holder(lock.clone(), owner1.clone());
        graph.add_edge(WaitEdge {
            waiter: owner2.clone(),
            lock_id: lock.clone(),
            requested_mode: LockMode::Exclusive,
        });

        let cycle = graph.detect_cycle();
        assert!(cycle.is_none());
    }

    #[test]
    fn test_wait_for_graph_self_wait() {
        let mut graph = WaitForGraph::default();
        let owner = OwnerId::new("self-owner".into());
        let lock = LockId::new("self-lock");

        graph.set_lock_holder(lock.clone(), owner.clone());
        graph.add_edge(WaitEdge {
            waiter: owner.clone(),
            lock_id: lock.clone(),
            requested_mode: LockMode::Exclusive,
        });

        // Self-wait should be ignored in cycle detection
        let cycle = graph.detect_cycle();
        assert!(cycle.is_none());
    }

    #[test]
    fn test_lock_error_not_found() {
        let lock_id = LockId::new("missing");
        let err = LockError::NotFound(lock_id.clone());
        assert!(err.to_string().contains("missing"));
    }

    #[test]
    fn test_lock_error_not_owner() {
        let expected = OwnerId::new("expected".into());
        let got = OwnerId::new("got".into());
        let err = LockError::NotOwner { expected, got };
        assert!(err.to_string().contains("expected"));
        assert!(err.to_string().contains("got"));
    }

    #[test]
    fn test_lock_error_invalid_token() {
        let err = LockError::InvalidToken;
        assert!(err.to_string().contains("invalid"));
        assert!(err.to_string().contains("token"));
    }

    #[test]
    fn test_lock_error_deadlock() {
        let err = LockError::DeadlockDetected;
        assert!(err.to_string().contains("deadlock"));
    }

    #[test]
    fn test_lock_error_incompatible_mode() {
        let err = LockError::IncompatibleMode;
        assert!(err.to_string().contains("incompatible"));
        assert!(err.to_string().contains("mode"));
    }

    #[test]
    fn test_lock_error_invalid_ttl() {
        let err = LockError::InvalidTtl(0);
        assert!(err.to_string().contains("0"));
    }

    #[test]
    fn test_lock_error_nats() {
        let err = LockError::Nats("connection failed".to_string());
        assert!(err.to_string().contains("NATS"));
        assert!(err.to_string().contains("connection"));
    }

    #[test]
    fn test_lock_error_storage() {
        let err = LockError::Storage("disk full".to_string());
        assert!(err.to_string().contains("storage"));
        assert!(err.to_string().contains("disk"));
    }

    #[test]
    fn test_lock_error_timeout() {
        let err = LockError::Timeout;
        assert!(err.to_string().contains("timeout"));
    }

    #[test]
    fn test_lock_error_upgrade_would_deadlock() {
        let err = LockError::UpgradeWouldDeadlock;
        assert!(err.to_string().contains("upgrade"));
        assert!(err.to_string().contains("shared"));
    }
}
