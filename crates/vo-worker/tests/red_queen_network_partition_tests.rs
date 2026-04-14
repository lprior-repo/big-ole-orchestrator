//! Red Queen adversarial tests for network partition scenarios in vo-worker.
//!
//! This module implements adversarial testing for network partition scenarios:
//! - NATS communication failures during lock operations
//! - Lock acquisition/release timeout during partition
//! - System recovery after network heals
//! - Deadlock detection during partition
//! - TTL expiration during network failure
//!
//! These tests attack the contracts from the other side — they verify that
//! the system fails (or succeeds) correctly under adversarial network conditions.

use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use vo_worker::{
    CommitOutcome, Connector, ConnectorError, PreparedEffect, ReconcileOutcome,
    LockError, LockId, LockMode, LockQuery, LockQueryResponse, LockRelease,
    LockRequest, LockResponse, LockPromote, LockPromoteResponse, LockManager,
    OwnerId, RetryConfig, LockManagerRetryWrapper,
};

static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn state_guard() -> MutexGuard<'static, ()> {
    let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    guard
}

// ============================================================================
// Mock Connectors for Network Partition Simulation
// ============================================================================

struct NetworkPartitionConnector {
    fail_mode: std::sync::atomic::AtomicBool,
    call_count: std::sync::atomic::AtomicUsize,
}

impl NetworkPartitionConnector {
    fn new() -> Self {
        Self {
            fail_mode: std::sync::atomic::AtomicBool::new(false),
            call_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    fn induce_partition(&self) {
        self.fail_mode.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    fn heal_partition(&self) {
        self.fail_mode.store(false, std::sync::atomic::Ordering::SeqCst);
    }

    fn is_partitioned(&self) -> bool {
        self.fail_mode.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn call_count(&self) -> usize {
        self.call_count.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Clone for NetworkPartitionConnector {
    fn clone(&self) -> Self {
        Self {
            fail_mode: std::sync::atomic::AtomicBool::new(self.fail_mode.load(std::sync::atomic::Ordering::SeqCst)),
            call_count: std::sync::atomic::AtomicUsize::new(self.call_count.load(std::sync::atomic::Ordering::SeqCst)),
        }
    }
}

#[async_trait]
impl Connector for NetworkPartitionConnector {
    fn connector_type(&self) -> &str { "network-partition" }
    fn connector_version(&self) -> &str { "0.1.0" }
    fn supports_compensation(&self) -> bool { true }

    async fn prepare(
        &self, _intent: serde_json::Value, effect_id: String, fence: u64,
    ) -> Result<PreparedEffect, ConnectorError> {
        self.call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.is_partitioned() {
            Err(ConnectorError::retryable("NATS connection timeout: network partition"))
        } else {
            Ok(PreparedEffect {
                effect_id,
                payload: serde_json::json!({}),
                fence,
            })
        }
    }

    async fn commit(
        &self, _prepared: PreparedEffect,
    ) -> Result<CommitOutcome, ConnectorError> {
        self.call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.is_partitioned() {
            Err(ConnectorError::retryable("NATS send failed: network unreachable"))
        } else {
            Ok(CommitOutcome::Committed { receipt: "ok".into() })
        }
    }

    async fn reconcile(
        &self, _effect_id: &str,
    ) -> Result<ReconcileOutcome, ConnectorError> {
        self.call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.is_partitioned() {
            Err(ConnectorError::retryable("NATS subscription failed: network unreachable"))
        } else {
            Ok(ReconcileOutcome::NotCommitted)
        }
    }
}

// ============================================================================
// Mock LockManager for Network Partition Testing
// ============================================================================

struct PartitionableLockManager {
    locked_locks: Arc<Mutex<std::collections::HashMap<String, (OwnerId, String)>>>,
    is_partitioned: std::sync::atomic::AtomicBool,
    partition_call_count: std::sync::atomic::AtomicUsize,
}

impl PartitionableLockManager {
    fn new() -> Self {
        Self {
            locked_locks: Arc::new(Mutex::new(std::collections::HashMap::new())),
            is_partitioned: std::sync::atomic::AtomicBool::new(false),
            partition_call_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    fn induce_partition(&self) {
        self.is_partitioned.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    fn heal_partition(&self) {
        self.is_partitioned.store(false, std::sync::atomic::Ordering::SeqCst);
    }

    fn is_partitioned(&self) -> bool {
        self.is_partitioned.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn partition_call_count(&self) -> usize {
        self.partition_call_count.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Default for PartitionableLockManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for PartitionableLockManager {
    fn clone(&self) -> Self {
        Self {
            locked_locks: self.locked_locks.clone(),
            is_partitioned: std::sync::atomic::AtomicBool::new(self.is_partitioned.load(std::sync::atomic::Ordering::SeqCst)),
            partition_call_count: std::sync::atomic::AtomicUsize::new(self.partition_call_count.load(std::sync::atomic::Ordering::SeqCst)),
        }
    }
}

#[async_trait]
impl LockManager for PartitionableLockManager {
    async fn acquire(&self, request: LockRequest) -> LockResponse {
        self.partition_call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        if self.is_partitioned() {
            return LockResponse {
                request_id: request.request_id,
                lock_id: request.lock_id.clone(),
                owner: request.owner.clone(),
                granted: false,
                hold_token: None,
                expires_at: None,
                error: Some("NATS communication error: network partition".to_string()),
            };
        }

        let lock_key = request.lock_id.as_str().to_string();
        let mut locks = self.locked_locks.lock().unwrap();

        if let Some((existing_owner, _)) = locks.get(&lock_key) {
            if existing_owner != &request.owner {
                return LockResponse {
                    request_id: request.request_id,
                    lock_id: request.lock_id,
                    owner: request.owner,
                    granted: false,
                    hold_token: None,
                    expires_at: None,
                    error: Some("lock held by another owner".to_string()),
                };
            }
        }

        let hold_token = format!("token-{}-{}", request.owner, request.lock_id.as_str());
        locks.insert(lock_key, (request.owner.clone(), hold_token.clone()));

        LockResponse {
            request_id: request.request_id,
            lock_id: request.lock_id,
            owner: request.owner,
            granted: true,
            hold_token: Some(hold_token),
            expires_at: None,
            error: None,
        }
    }

    async fn release(&self, release: LockRelease) -> Result<(), LockError> {
        self.partition_call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        if self.is_partitioned() {
            return Err(LockError::Nats("network partition: cannot release lock".to_string()));
        }

        let lock_key = release.lock_id.as_str().to_string();
        let mut locks = self.locked_locks.lock().unwrap();

        if let Some((owner, _)) = locks.get(&lock_key) {
            if &release.owner != owner {
                return Err(LockError::NotOwner {
                    expected: owner.clone(),
                    got: release.owner,
                });
            }
        }

        locks.remove(&lock_key);
        Ok(())
    }

    async fn query(&self, _query: LockQuery) -> LockQueryResponse {
        self.partition_call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        LockQueryResponse { locks: vec![] }
    }

    async fn promote(&self, _promote: LockPromote) -> LockPromoteResponse {
        self.partition_call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        LockPromoteResponse {
            request_id: String::new(),
            lock_id: _promote.lock_id,
            granted: false,
            new_mode: None,
            error: Some("not implemented".to_string()),
        }
    }

    async fn demote(&self, lock_id: LockId, _owner: OwnerId, _hold_token: String) -> Result<LockMode, LockError> {
        self.partition_call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err(LockError::NotFound(lock_id))
    }

    async fn extend_ttl(
        &self, lock_id: LockId, _owner: OwnerId, _hold_token: String, _ttl_ms: u64,
    ) -> Result<chrono::DateTime<chrono::Utc>, LockError> {
        self.partition_call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.is_partitioned() {
            return Err(LockError::Nats("network partition: cannot extend TTL".to_string()));
        }
        Err(LockError::NotFound(lock_id))
    }

    async fn is_locked(&self, lock_id: &LockId) -> bool {
        self.partition_call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let locks = self.locked_locks.lock().unwrap();
        locks.contains_key(lock_id.as_str())
    }

    async fn get_holder(&self, lock_id: &LockId) -> Option<(OwnerId, LockMode)> {
        self.partition_call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let locks = self.locked_locks.lock().unwrap();
        locks.get(lock_id.as_str()).map(|(o, _)| (o.clone(), LockMode::Exclusive))
    }
}

// ============================================================================
// Network Partition Tests: Lock Acquisition
// ============================================================================

#[cfg(test)]
mod red_queen_network_partition_acquire_tests {
    use super::*;

    #[tokio::test]
    async fn lock_acquire_fails_during_network_partition() {
        let _guard = state_guard();
        let manager = PartitionableLockManager::new();

        manager.induce_partition();

        let request = LockRequest {
            lock_id: LockId::new("test-lock"),
            owner: OwnerId::new("owner1".to_string()),
            mode: LockMode::Exclusive,
            ttl_ms: 1000,
            request_id: "req1".to_string(),
        };

        let response = manager.acquire(request).await;
        assert!(!response.granted, "lock should not be granted during partition");
        assert!(response.error.is_some());
        let error_msg = response.error.unwrap();
        assert!(error_msg.contains("NATS") || error_msg.contains("network"));
        assert_eq!(manager.partition_call_count(), 1);
    }

    #[tokio::test]
    async fn lock_acquire_succeeds_after_network_heals() {
        let _guard = state_guard();
        let manager = PartitionableLockManager::new();

        manager.induce_partition();

        let request = LockRequest {
            lock_id: LockId::new("test-lock"),
            owner: OwnerId::new("owner1".to_string()),
            mode: LockMode::Exclusive,
            ttl_ms: 1000,
            request_id: "req1".to_string(),
        };

        let response_partitioned = manager.acquire(request.clone()).await;
        assert!(!response_partitioned.granted);

        manager.heal_partition();

        let response_healed = manager.acquire(request).await;
        assert!(response_healed.granted, "lock should be granted after partition heals");
        assert!(response_healed.hold_token.is_some());
    }

    #[tokio::test]
    async fn concurrent_acquires_during_partition_all_fail() {
        let _guard = state_guard();
        let manager = Arc::new(PartitionableLockManager::new());
        manager.induce_partition();

        let handles: Vec<_> = (0..5)
            .map(|i| {
                let manager = manager.clone();
                let request = LockRequest {
                    lock_id: LockId::new("shared-lock"),
                    owner: OwnerId::new(format!("owner{}", i)),
                    mode: LockMode::Exclusive,
                    ttl_ms: 1000,
                    request_id: format!("req{}", i),
                };
                tokio::spawn(async move { manager.acquire(request).await })
            })
            .collect();

        let mut failure_count = 0;
        for handle in handles {
            let response = handle.await.unwrap();
            if !response.granted {
                failure_count += 1;
            }
        }

        assert_eq!(failure_count, 5, "all acquires should fail during partition");
    }

    #[tokio::test]
    async fn retry_wrapper_retries_during_partition() {
        let _guard = state_guard();
        let manager = PartitionableLockManager::new();
        let config = RetryConfig::new(10, 2.0, 5);
        let wrapper = LockManagerRetryWrapper::new(&manager, config);

        manager.induce_partition();

        let request = LockRequest {
            lock_id: LockId::new("test-lock"),
            owner: OwnerId::new("owner1".to_string()),
            mode: LockMode::Exclusive,
            ttl_ms: 1000,
            request_id: "req1".to_string(),
        };

        let response = wrapper.acquire(request).await;
        assert!(!response.granted, "lock should not be granted after retries exhausted");
        assert!(response.error.is_some());

        manager.heal_partition();

        let response_after_heal = wrapper.acquire(LockRequest {
            lock_id: LockId::new("test-lock-2"),
            owner: OwnerId::new("owner1".to_string()),
            mode: LockMode::Exclusive,
            ttl_ms: 1000,
            request_id: "req2".to_string(),
        }).await;

        assert!(response_after_heal.granted, "lock should succeed after partition heals");
    }
}

// ============================================================================
// Network Partition Tests: Lock Release
// ============================================================================

#[cfg(test)]
mod red_queen_network_partition_release_tests {
    use super::*;

    #[tokio::test]
    async fn lock_release_fails_during_network_partition() {
        let _guard = state_guard();
        let manager = PartitionableLockManager::new();

        manager.heal_partition();
        let request = LockRequest {
            lock_id: LockId::new("test-lock"),
            owner: OwnerId::new("owner1".to_string()),
            mode: LockMode::Exclusive,
            ttl_ms: 1000,
            request_id: "req1".to_string(),
        };
        let acquire_response = manager.acquire(request).await;
        assert!(acquire_response.granted);

        manager.induce_partition();

        let release = LockRelease {
            lock_id: LockId::new("test-lock"),
            owner: OwnerId::new("owner1".to_string()),
            hold_token: acquire_response.hold_token.unwrap(),
        };

        let result = manager.release(release).await;
        assert!(result.is_err(), "release should fail during partition");
        let err = result.unwrap_err();
        assert!(matches!(err, LockError::Nats(_)), "error should be NATS error");
    }

    #[tokio::test]
    async fn lock_release_succeeds_after_network_heals() {
        let _guard = state_guard();
        let manager = PartitionableLockManager::new();

        manager.heal_partition();
        let request = LockRequest {
            lock_id: LockId::new("test-lock"),
            owner: OwnerId::new("owner1".to_string()),
            mode: LockMode::Exclusive,
            ttl_ms: 1000,
            request_id: "req1".to_string(),
        };
        let acquire_response = manager.acquire(request).await;
        assert!(acquire_response.granted);

        manager.induce_partition();

        let release_partitioned = manager.release(LockRelease {
            lock_id: LockId::new("test-lock"),
            owner: OwnerId::new("owner1".to_string()),
            hold_token: acquire_response.hold_token.clone().unwrap(),
        }).await;
        assert!(release_partitioned.is_err());

        manager.heal_partition();

        let release_healed = manager.release(LockRelease {
            lock_id: LockId::new("test-lock"),
            owner: OwnerId::new("owner1".to_string()),
            hold_token: acquire_response.hold_token.unwrap(),
        }).await;
        assert!(release_healed.is_ok(), "release should succeed after partition heals");
    }

    #[tokio::test]
    async fn lock_remains_held_after_release_fails_due_to_partition() {
        let _guard = state_guard();
        let manager = PartitionableLockManager::new();

        manager.heal_partition();
        let request = LockRequest {
            lock_id: LockId::new("test-lock"),
            owner: OwnerId::new("owner1".to_string()),
            mode: LockMode::Exclusive,
            ttl_ms: 10000,
            request_id: "req1".to_string(),
        };
        let acquire_response = manager.acquire(request).await;
        assert!(acquire_response.granted);

        manager.induce_partition();

        let release = LockRelease {
            lock_id: LockId::new("test-lock"),
            owner: OwnerId::new("owner1".to_string()),
            hold_token: acquire_response.hold_token.unwrap(),
        };
        let _ = manager.release(release).await;

        let is_locked = manager.is_locked(&LockId::new("test-lock")).await;
        assert!(is_locked, "lock should still be held after failed release during partition");
    }
}

// ============================================================================
// Network Partition Tests: TTL Expiration
// ============================================================================

#[cfg(test)]
mod red_queen_network_partition_ttl_tests {
    use super::*;

    #[tokio::test]
    async fn lock_ttl_extend_fails_during_partition() {
        let _guard = state_guard();
        let manager = PartitionableLockManager::new();

        manager.heal_partition();
        let request = LockRequest {
            lock_id: LockId::new("test-lock"),
            owner: OwnerId::new("owner1".to_string()),
            mode: LockMode::Exclusive,
            ttl_ms: 1000,
            request_id: "req1".to_string(),
        };
        let acquire_response = manager.acquire(request).await;
        assert!(acquire_response.granted);

        manager.induce_partition();

        let result = manager.extend_ttl(
            LockId::new("test-lock"),
            OwnerId::new("owner1".to_string()),
            acquire_response.hold_token.unwrap(),
            5000,
        ).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, LockError::Nats(_)), "TTL extend should fail with NATS error during partition");
    }

    #[tokio::test]
    async fn is_locked_returns_correct_value_during_partition() {
        let _guard = state_guard();
        let manager = PartitionableLockManager::new();

        manager.heal_partition();
        let request = LockRequest {
            lock_id: LockId::new("test-lock"),
            owner: OwnerId::new("owner1".to_string()),
            mode: LockMode::Exclusive,
            ttl_ms: 1000,
            request_id: "req1".to_string(),
        };
        let acquire_response = manager.acquire(request).await;
        assert!(acquire_response.granted);

        let is_locked_before = manager.is_locked(&LockId::new("test-lock")).await;
        assert!(is_locked_before);

        manager.induce_partition();

        let is_locked_during = manager.is_locked(&LockId::new("test-lock")).await;
        assert!(is_locked_during, "is_locked should still return true during partition");
    }

    #[tokio::test]
    async fn get_holder_returns_correct_value_during_partition() {
        let _guard = state_guard();
        let manager = PartitionableLockManager::new();

        manager.heal_partition();
        let request = LockRequest {
            lock_id: LockId::new("test-lock"),
            owner: OwnerId::new("owner1".to_string()),
            mode: LockMode::Exclusive,
            ttl_ms: 1000,
            request_id: "req1".to_string(),
        };
        let acquire_response = manager.acquire(request).await;
        assert!(acquire_response.granted);

        manager.induce_partition();

        let holder = manager.get_holder(&LockId::new("test-lock")).await;
        assert!(holder.is_some(), "get_holder should return holder during partition");
        let (owner, mode) = holder.unwrap();
        assert_eq!(owner, OwnerId::new("owner1".to_string()));
        assert_eq!(mode, LockMode::Exclusive);
    }
}

// ============================================================================
// Network Partition Tests: Query Operations
// ============================================================================

#[cfg(test)]
mod red_queen_network_partition_query_tests {
    use super::*;

    #[tokio::test]
    async fn query_succeeds_during_partition() {
        let _guard = state_guard();
        let manager = PartitionableLockManager::new();

        manager.heal_partition();
        let request = LockRequest {
            lock_id: LockId::new("test-lock"),
            owner: OwnerId::new("owner1".to_string()),
            mode: LockMode::Exclusive,
            ttl_ms: 1000,
            request_id: "req1".to_string(),
        };
        let _ = manager.acquire(request).await;

        manager.induce_partition();

        let query_response = manager.query(LockQuery {
            lock_id: None,
            owner: None,
        }).await;

        assert!(query_response.locks.is_empty());
        assert_eq!(manager.partition_call_count(), 2);
    }
}

// ============================================================================
// Network Partition Tests: Deadlock Detection
// ============================================================================

#[cfg(test)]
mod red_queen_network_partition_deadlock_tests {
    use super::*;

    #[tokio::test]
    async fn deadlock_detection_works_during_partition() {
        let _guard = state_guard();
        let mut graph = vo_worker::WaitForGraph::default();

        let owner1 = OwnerId::new("owner1".into());
        let owner2 = OwnerId::new("owner2".into());
        let lock1 = LockId::new("lock1");
        let lock2 = LockId::new("lock2");

        graph.set_lock_holder(lock1.clone(), owner1.clone());
        graph.set_lock_holder(lock2.clone(), owner2.clone());

        graph.add_edge(vo_worker::WaitEdge {
            waiter: owner1.clone(),
            lock_id: lock2.clone(),
            requested_mode: LockMode::Exclusive,
        });

        graph.add_edge(vo_worker::WaitEdge {
            waiter: owner2.clone(),
            lock_id: lock1.clone(),
            requested_mode: LockMode::Exclusive,
        });

        let cycle = graph.detect_cycle();
        assert!(cycle.is_some(), "deadlock should be detected even during partition simulation");
        let cycle_owners = cycle.unwrap();
        assert!(cycle_owners.len() == 2);
    }

    #[tokio::test]
    async fn no_deadlock_when_no_cycle() {
        let _guard = state_guard();
        let mut graph = vo_worker::WaitForGraph::default();

        let owner1 = OwnerId::new("owner1".into());
        let owner2 = OwnerId::new("owner2".into());
        let lock1 = LockId::new("lock1");
        let lock2 = LockId::new("lock2");

        graph.set_lock_holder(lock1.clone(), owner1.clone());
        graph.set_lock_holder(lock2.clone(), owner2.clone());

        graph.add_edge(vo_worker::WaitEdge {
            waiter: owner1.clone(),
            lock_id: lock2.clone(),
            requested_mode: LockMode::Exclusive,
        });

        let cycle = graph.detect_cycle();
        assert!(cycle.is_none(), "no deadlock when only one wait edge");
    }
}

// ============================================================================
// Network Partition Tests: Recovery
// ============================================================================

#[cfg(test)]
mod red_queen_network_partition_recovery_tests {
    use super::*;

    #[tokio::test]
    async fn system_recovers_after_prolonged_partition() {
        let _guard = state_guard();
        let manager = PartitionableLockManager::new();

        manager.induce_partition();

        let request = LockRequest {
            lock_id: LockId::new("test-lock"),
            owner: OwnerId::new("owner1".to_string()),
            mode: LockMode::Exclusive,
            ttl_ms: 1000,
            request_id: "req1".to_string(),
        };

        let response_partitioned = manager.acquire(request.clone()).await;
        assert!(!response_partitioned.granted);

        tokio::time::sleep(Duration::from_millis(50)).await;

        manager.heal_partition();

        let response_healed = manager.acquire(request).await;
        assert!(response_healed.granted, "system should recover and grant lock");
        assert_eq!(manager.partition_call_count(), 2);
    }

    #[tokio::test]
    async fn multiple_partition_heal_cycles() {
        let _guard = state_guard();
        let manager = PartitionableLockManager::new();

        for i in 0..3 {
            manager.induce_partition();

            let request = LockRequest {
                lock_id: LockId::new(format!("lock-{}", i)),
                owner: OwnerId::new("owner1".to_string()),
                mode: LockMode::Exclusive,
                ttl_ms: 1000,
                request_id: format!("req{}", i),
            };

            let response_partitioned = manager.acquire(request.clone()).await;
            assert!(!response_partitioned.granted, "cycle {}: acquire should fail during partition", i);

            manager.heal_partition();

            let response_healed = manager.acquire(request).await;
            assert!(response_healed.granted, "cycle {}: acquire should succeed after heal", i);
        }
    }

    #[tokio::test]
    async fn partition_heals_during_retry_wait() {
        let _guard = state_guard();
        let manager = PartitionableLockManager::new();
        let config = RetryConfig::new(10, 2.0, 10);

        let request = LockRequest {
            lock_id: LockId::new("test-lock"),
            owner: OwnerId::new("owner1".to_string()),
            mode: LockMode::Exclusive,
            ttl_ms: 1000,
            request_id: "req1".to_string(),
        };

        manager.induce_partition();

        let response = manager.acquire(request.clone()).await;
        assert!(!response.granted, "acquire should fail during partition");

        manager.heal_partition();

        let wrapper = LockManagerRetryWrapper::new(&manager, config);
        let response_healed = wrapper.acquire(request).await;
        assert!(response_healed.granted, "lock should be granted after partition heals");
    }
}

// ============================================================================
// Network Partition Tests: Connector Integration
// ============================================================================

#[cfg(test)]
mod red_queen_network_partition_connector_tests {
    use super::*;

    #[tokio::test]
    async fn connector_prepare_fails_during_partition() {
        let _guard = state_guard();
        let connector = NetworkPartitionConnector::new();

        connector.induce_partition();

        let result = connector
            .prepare(serde_json::json!({}), "fx-1".into(), 1)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_retryable());
        assert_eq!(connector.call_count(), 1);
    }

    #[tokio::test]
    async fn connector_commit_fails_during_partition() {
        let _guard = state_guard();
        let connector = NetworkPartitionConnector::new();

        connector.induce_partition();

        let prepared = PreparedEffect {
            effect_id: "fx-1".into(),
            payload: serde_json::json!({}),
            fence: 1,
        };

        let result = connector.commit(prepared).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_retryable());
    }

    #[tokio::test]
    async fn connector_reconcile_fails_during_partition() {
        let _guard = state_guard();
        let connector = NetworkPartitionConnector::new();

        connector.induce_partition();

        let result = connector.reconcile("fx-1").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_retryable());
    }

    #[tokio::test]
    async fn connector_succeeds_after_partition_heals() {
        let _guard = state_guard();
        let connector = NetworkPartitionConnector::new();

        connector.induce_partition();
        let result_partitioned = connector.prepare(serde_json::json!({}), "fx-1".into(), 1).await;
        assert!(result_partitioned.is_err());

        connector.heal_partition();

        let result_healed = connector.prepare(serde_json::json!({}), "fx-2".into(), 2).await;
        assert!(result_healed.is_ok());
        assert_eq!(connector.call_count(), 2);
    }

    #[tokio::test]
    async fn connector_multiple_calls_during_partition() {
        let _guard = state_guard();
        let connector = NetworkPartitionConnector::new();
        connector.induce_partition();

        for i in 0..5 {
            let result = connector.prepare(serde_json::json!({}), format!("fx-{}", i), i as u64 + 1).await;
            assert!(result.is_err(), "call {} should fail", i);
        }

        assert_eq!(connector.call_count(), 5);
    }
}

// ============================================================================
// Network Partition Tests: State Transition Correctness
// ============================================================================

#[cfg(test)]
mod red_queen_network_partition_state_tests {
    use super::*;

    #[tokio::test]
    async fn lock_state_consistency_after_partition_heals() {
        let _guard = state_guard();
        let manager = PartitionableLockManager::new();

        manager.heal_partition();
        let request1 = LockRequest {
            lock_id: LockId::new("lock-1"),
            owner: OwnerId::new("owner1".to_string()),
            mode: LockMode::Exclusive,
            ttl_ms: 1000,
            request_id: "req1".to_string(),
        };
        let response1 = manager.acquire(request1).await;
        assert!(response1.granted);

        manager.induce_partition();

        manager.heal_partition();

        let request2 = LockRequest {
            lock_id: LockId::new("lock-2"),
            owner: OwnerId::new("owner2".to_string()),
            mode: LockMode::Exclusive,
            ttl_ms: 1000,
            request_id: "req2".to_string(),
        };
        let response2 = manager.acquire(request2).await;
        assert!(response2.granted);

        let holder1 = manager.get_holder(&LockId::new("lock-1")).await;
        assert!(holder1.is_some());

        let holder2 = manager.get_holder(&LockId::new("lock-2")).await;
        assert!(holder2.is_some());
    }

    #[tokio::test]
    async fn concurrent_lock_operations_after_recovery() {
        let _guard = state_guard();
        let manager = Arc::new(PartitionableLockManager::new());

        manager.induce_partition();
        manager.heal_partition();

        let handles: Vec<_> = (0..5)
            .map(|i| {
                let mgr = manager.clone();
                tokio::spawn(async move {
                    let request = LockRequest {
                        lock_id: LockId::new(format!("concurrent-lock-{}", i)),
                        owner: OwnerId::new(format!("owner{}", i)),
                        mode: LockMode::Exclusive,
                        ttl_ms: 1000,
                        request_id: format!("req{}", i),
                    };
                    mgr.acquire(request).await
                })
            })
            .collect();

        let mut success_count = 0;
        for handle in handles {
            let response = handle.await.unwrap();
            if response.granted {
                success_count += 1;
            }
        }

        assert_eq!(success_count, 5, "all concurrent locks should succeed after recovery");
    }
}
