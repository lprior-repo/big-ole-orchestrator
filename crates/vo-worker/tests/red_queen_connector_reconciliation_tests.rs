//! Red Queen adversarial tests for Connector reconciliation semantics (ADR-041).
//!
//! This module implements adversarial testing for connector reconciliation:
//! - Ambiguous result after timeout
//! - Reconciliation query failure
//! - Idempotency key collision
//! - Concurrent reconciliation
//!
//! These tests attack the contracts from the other side — they verify that
//! the system fails (or succeeds) correctly under adversarial conditions.

use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::MutexGuard;
use async_trait::async_trait;
use vo_worker::{
    connector::{
        CommitOutcome, Connector, ConnectorError, PreparedEffect, ReconcileOutcome,
    },
    ConnectorRegistry,
};

static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn state_guard() -> MutexGuard<'static, ()> {
    let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    guard
}

struct AmbiguousAfterTimeoutConnector {
    call_count: std::sync::atomic::AtomicUsize,
    timeout_threshold: usize,
}

impl AmbiguousAfterTimeoutConnector {
    fn new(timeout_threshold: usize) -> Self {
        Self {
            call_count: std::sync::atomic::AtomicUsize::new(0),
            timeout_threshold,
        }
    }
}

#[async_trait]
impl Connector for AmbiguousAfterTimeoutConnector {
    fn connector_type(&self) -> &str { "ambiguous-timeout" }
    fn connector_version(&self) -> &str { "0.1.0" }
    fn supports_compensation(&self) -> bool { false }

    async fn prepare(
        &self, _intent: serde_json::Value, effect_id: String, fence: u64,
    ) -> Result<PreparedEffect, ConnectorError> {
        Ok(PreparedEffect {
            effect_id,
            payload: serde_json::json!({}),
            fence,
        })
    }

    async fn commit(
        &self, _prepared: PreparedEffect,
    ) -> Result<CommitOutcome, ConnectorError> {
        let count = self.call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if count >= self.timeout_threshold {
            Ok(CommitOutcome::Ambiguous)
        } else {
            Ok(CommitOutcome::Committed { receipt: format!("receipt-{}", count) })
        }
    }

    async fn reconcile(
        &self, effect_id: &str,
    ) -> Result<ReconcileOutcome, ConnectorError> {
        let count = self.call_count.load(std::sync::atomic::Ordering::SeqCst);
        if count > self.timeout_threshold {
            Ok(ReconcileOutcome::StillAmbiguous)
        } else {
            Ok(ReconcileOutcome::Committed {
                receipt: format!("reconcile-receipt-{}", effect_id),
            })
        }
    }
}

struct ReconciliationFailingConnector {
    fail_reconcile: bool,
}

#[async_trait]
impl Connector for ReconciliationFailingConnector {
    fn connector_type(&self) -> &str { "fail-reconcile" }
    fn connector_version(&self) -> &str { "0.1.0" }
    fn supports_compensation(&self) -> bool { false }

    async fn prepare(
        &self, _intent: serde_json::Value, effect_id: String, fence: u64,
    ) -> Result<PreparedEffect, ConnectorError> {
        Ok(PreparedEffect {
            effect_id,
            payload: serde_json::json!({}),
            fence,
        })
    }

    async fn commit(
        &self, _prepared: PreparedEffect,
    ) -> Result<CommitOutcome, ConnectorError> {
        Ok(CommitOutcome::Committed { receipt: "ok".into() })
    }

    async fn reconcile(
        &self, _effect_id: &str,
    ) -> Result<ReconcileOutcome, ConnectorError> {
        if self.fail_reconcile {
            Err(ConnectorError::retryable("reconciliation query failed"))
        } else {
            Ok(ReconcileOutcome::NotCommitted)
        }
    }
}

struct IdempotencyKeyCollisionConnector {
    seen_keys: std::sync::Mutex<std::collections::HashSet<String>>,
}

impl IdempotencyKeyCollisionConnector {
    fn new() -> Self {
        Self {
            seen_keys: std::sync::Mutex::new(std::collections::HashSet::new()),
        }
    }
}

#[async_trait]
impl Connector for IdempotencyKeyCollisionConnector {
    fn connector_type(&self) -> &str { "idempotency-collision" }
    fn connector_version(&self) -> &str { "0.1.0" }
    fn supports_compensation(&self) -> bool { false }

    async fn prepare(
        &self, _intent: serde_json::Value, effect_id: String, fence: u64,
    ) -> Result<PreparedEffect, ConnectorError> {
        let key = format!("{}:{}", effect_id, fence);
        let mut keys = self.seen_keys.lock().unwrap();
        if keys.contains(&key) {
            Ok(PreparedEffect {
                effect_id,
                payload: serde_json::json!({"collision": true, "key": key}),
                fence,
            })
        } else {
            keys.insert(key);
            Ok(PreparedEffect {
                effect_id,
                payload: serde_json::json!({"collision": false, "key": key}),
                fence,
            })
        }
    }

    async fn commit(
        &self, prepared: PreparedEffect,
    ) -> Result<CommitOutcome, ConnectorError> {
        if prepared.payload.get("collision").and_then(|v| v.as_bool()).unwrap_or(false) {
            Ok(CommitOutcome::Ambiguous)
        } else {
            Ok(CommitOutcome::Committed {
                receipt: prepared.payload["key"].as_str().unwrap_or("ok").into(),
            })
        }
    }

    async fn reconcile(
        &self, effect_id: &str,
    ) -> Result<ReconcileOutcome, ConnectorError> {
        Ok(ReconcileOutcome::StillAmbiguous)
    }
}

struct ConcurrentReconciliationConnector {
    active_reconciles: std::sync::atomic::AtomicUsize,
    max_concurrent: usize,
}

impl ConcurrentReconciliationConnector {
    fn new(max_concurrent: usize) -> Self {
        Self {
            active_reconciles: std::sync::atomic::AtomicUsize::new(0),
            max_concurrent,
        }
    }
}

#[async_trait]
impl Connector for ConcurrentReconciliationConnector {
    fn connector_type(&self) -> &str { "concurrent-reconcile" }
    fn connector_version(&self) -> &str { "0.1.0" }
    fn supports_compensation(&self) -> bool { false }

    async fn prepare(
        &self, _intent: serde_json::Value, effect_id: String, fence: u64,
    ) -> Result<PreparedEffect, ConnectorError> {
        Ok(PreparedEffect {
            effect_id,
            payload: serde_json::json!({}),
            fence,
        })
    }

    async fn commit(
        &self, _prepared: PreparedEffect,
    ) -> Result<CommitOutcome, ConnectorError> {
        Ok(CommitOutcome::Committed { receipt: "ok".into() })
    }

    async fn reconcile(
        &self, effect_id: &str,
    ) -> Result<ReconcileOutcome, ConnectorError> {
        let current = self.active_reconciles.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        tokio::task::yield_now().await;
        self.active_reconciles.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);

        if current >= self.max_concurrent {
            Ok(ReconcileOutcome::StillAmbiguous)
        } else {
            Ok(ReconcileOutcome::Committed {
                receipt: format!("reconcile-{}", effect_id),
            })
        }
    }
}

#[cfg(test)]
mod red_queen_ambiguous_timeout_tests {
    use super::*;

    #[tokio::test]
    async fn ambiguous_after_timeout_subsequent_commit_is_ambiguous() {
        let _guard = state_guard();
        let connector = AmbiguousAfterTimeoutConnector::new(2);

        let pe1 = connector
            .prepare(serde_json::json!({}), "fx-1".into(), 1)
            .await
            .unwrap();
        let outcome1 = connector.commit(pe1).await.unwrap();
        assert!(matches!(outcome1, CommitOutcome::Committed { .. }));

        let pe2 = connector
            .prepare(serde_json::json!({}), "fx-2".into(), 2)
            .await
            .unwrap();
        let outcome2 = connector.commit(pe2).await.unwrap();
        assert!(matches!(outcome2, CommitOutcome::Committed { .. }));

        let pe3 = connector
            .prepare(serde_json::json!({}), "fx-3".into(), 3)
            .await
            .unwrap();
        let outcome3 = connector.commit(pe3).await.unwrap();
        assert!(matches!(outcome3, CommitOutcome::Ambiguous));
    }

    #[tokio::test]
    async fn reconcile_after_ambiguous_returns_still_ambiguous() {
        let _guard = state_guard();
        let connector = AmbiguousAfterTimeoutConnector::new(1);

        let pe = connector
            .prepare(serde_json::json!({}), "fx-1".into(), 1)
            .await
            .unwrap();
        let _ = connector.commit(pe).await.unwrap();

        let reconcile_outcome = connector.reconcile("fx-1").await.unwrap();
        assert!(matches!(reconcile_outcome, ReconcileOutcome::StillAmbiguous));
    }

    #[tokio::test]
    async fn timeout_threshold_zero_immediately_ambiguous() {
        let _guard = state_guard();
        let connector = AmbiguousAfterTimeoutConnector::new(0);

        let pe = connector
            .prepare(serde_json::json!({}), "fx-1".into(), 1)
            .await
            .unwrap();
        let outcome = connector.commit(pe).await.unwrap();
        assert!(matches!(outcome, CommitOutcome::Ambiguous));
    }

    #[tokio::test]
    async fn recovery_after_ambiguous_with_higher_threshold() {
        let _guard = state_guard();
        let connector = AmbiguousAfterTimeoutConnector::new(5);

        for i in 0..4 {
            let pe = connector
                .prepare(serde_json::json!({}), format!("fx-{}", i), i as u64 + 1)
                .await
                .unwrap();
            let outcome = connector.commit(pe).await.unwrap();
            assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        }

        let pe5 = connector
            .prepare(serde_json::json!({}), "fx-5".into(), 5)
            .await
            .unwrap();
        let outcome5 = connector.commit(pe5).await.unwrap();
        assert!(matches!(outcome5, CommitOutcome::Ambiguous));
    }
}

#[cfg(test)]
mod red_queen_reconciliation_failure_tests {
    use super::*;

    #[tokio::test]
    async fn reconcile_failure_returns_retryable_error() {
        let _guard = state_guard();
        let connector = ReconciliationFailingConnector { fail_reconcile: true };

        let result = connector.reconcile("fx-1").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_retryable());
        assert!(err.to_string().contains("reconciliation query failed"));
    }

    #[tokio::test]
    async fn reconcile_success_returns_not_committed() {
        let _guard = state_guard();
        let connector = ReconciliationFailingConnector { fail_reconcile: false };

        let outcome = connector.reconcile("fx-1").await.unwrap();
        assert!(matches!(outcome, ReconcileOutcome::NotCommitted));
    }

    #[tokio::test]
    async fn commit_still_works_after_reconcile_failure() {
        let _guard = state_guard();
        let connector = ReconciliationFailingConnector { fail_reconcile: true };

        let pe = connector
            .prepare(serde_json::json!({}), "fx-1".into(), 1)
            .await
            .unwrap();
        let commit_outcome = connector.commit(pe).await.unwrap();
        assert!(matches!(commit_outcome, CommitOutcome::Committed { .. }));

        let reconcile_result = connector.reconcile("fx-1").await;
        assert!(reconcile_result.is_err());
    }

    #[tokio::test]
    async fn multiple_reconcile_failures_are_retryable() {
        let _guard = state_guard();
        let connector = ReconciliationFailingConnector { fail_reconcile: true };

        for _ in 0..5 {
            let result = connector.reconcile("fx-1").await;
            assert!(result.is_err());
            assert!(result.unwrap_err().is_retryable());
        }
    }

    #[tokio::test]
    async fn terminal_error_during_reconcile_is_not_retryable() {
        let _guard = state_guard();

        struct TerminalReconcileConnector;
        #[async_trait]
        impl Connector for TerminalReconcileConnector {
            fn connector_type(&self) -> &str { "terminal" }
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
                Ok(CommitOutcome::Committed { receipt: "ok".into() })
            }

            async fn reconcile(
                &self, _effect_id: &str,
            ) -> Result<ReconcileOutcome, ConnectorError> {
                Err(ConnectorError::terminal("database corrupted"))
            }
        }

        let connector = TerminalReconcileConnector;
        let result = connector.reconcile("fx-1").await;
        assert!(result.is_err());
        assert!(!result.unwrap_err().is_retryable());
    }
}

#[cfg(test)]
mod red_queen_idempotency_collision_tests {
    use super::*;

    #[tokio::test]
    async fn first_prepare_with_new_key_succeeds() {
        let _guard = state_guard();
        let connector = IdempotencyKeyCollisionConnector::new();

        let pe = connector
            .prepare(serde_json::json!({}), "fx-1".into(), 1)
            .await
            .unwrap();
        assert!(!pe.payload.get("collision").and_then(|v| v.as_bool()).unwrap_or(false));

        let outcome = connector.commit(pe).await.unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn second_prepare_with_same_key_returns_collision() {
        let _guard = state_guard();
        let connector = IdempotencyKeyCollisionConnector::new();

        let pe1 = connector
            .prepare(serde_json::json!({}), "fx-1".into(), 1)
            .await
            .unwrap();
        let _ = connector.commit(pe1).await.unwrap();

        let pe2 = connector
            .prepare(serde_json::json!({}), "fx-1".into(), 1)
            .await
            .unwrap();
        assert!(pe2.payload.get("collision").and_then(|v| v.as_bool()).unwrap_or(false));
    }

    #[tokio::test]
    async fn commit_after_collision_returns_ambiguous() {
        let _guard = state_guard();
        let connector = IdempotencyKeyCollisionConnector::new();

        let pe1 = connector
            .prepare(serde_json::json!({}), "fx-1".into(), 1)
            .await
            .unwrap();
        let _ = connector.commit(pe1).await.unwrap();

        let pe2 = connector
            .prepare(serde_json::json!({}), "fx-1".into(), 1)
            .await
            .unwrap();
        let outcome2 = connector.commit(pe2).await.unwrap();
        assert!(matches!(outcome2, CommitOutcome::Ambiguous));
    }

    #[tokio::test]
    async fn different_effect_ids_do_not_collision() {
        let _guard = state_guard();
        let connector = IdempotencyKeyCollisionConnector::new();

        let pe1 = connector
            .prepare(serde_json::json!({}), "fx-1".into(), 1)
            .await
            .unwrap();
        let _ = connector.commit(pe1).await.unwrap();

        let pe2 = connector
            .prepare(serde_json::json!({}), "fx-2".into(), 1)
            .await
            .unwrap();
        assert!(!pe2.payload.get("collision").and_then(|v| v.as_bool()).unwrap_or(false));

        let outcome2 = connector.commit(pe2).await.unwrap();
        assert!(matches!(outcome2, CommitOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn different_fences_do_not_collision() {
        let _guard = state_guard();
        let connector = IdempotencyKeyCollisionConnector::new();

        let pe1 = connector
            .prepare(serde_json::json!({}), "fx-1".into(), 1)
            .await
            .unwrap();
        let _ = connector.commit(pe1).await.unwrap();

        let pe2 = connector
            .prepare(serde_json::json!({}), "fx-1".into(), 2)
            .await
            .unwrap();
        assert!(!pe2.payload.get("collision").and_then(|v| v.as_bool()).unwrap_or(false));
    }

    #[tokio::test]
    async fn reconciliation_still_ambiguous_after_collision() {
        let _guard = state_guard();
        let connector = IdempotencyKeyCollisionConnector::new();

        let pe1 = connector
            .prepare(serde_json::json!({}), "fx-1".into(), 1)
            .await
            .unwrap();
        let _ = connector.commit(pe1).await.unwrap();

        let _pe2 = connector
            .prepare(serde_json::json!({}), "fx-1".into(), 1)
            .await
            .unwrap();

        let reconcile_outcome = connector.reconcile("fx-1").await.unwrap();
        assert!(matches!(reconcile_outcome, ReconcileOutcome::StillAmbiguous));
    }
}

#[cfg(test)]
mod red_queen_concurrent_reconciliation_tests {
    use super::*;

    #[tokio::test]
    async fn sequential_reconciliation_succeeds() {
        let _guard = state_guard();
        let connector = ConcurrentReconciliationConnector::new(1);

        for i in 0..5 {
            let outcome = connector.reconcile(&format!("fx-{}", i)).await.unwrap();
            assert!(matches!(outcome, ReconcileOutcome::Committed { .. }));
        }
    }

    #[tokio::test]
    async fn concurrent_reconciliations_exceed_threshold() {
        let _guard = state_guard();
        let connector = ConcurrentReconciliationConnector::new(3);

        let handles: Vec<_> = (0..10)
            .map(|i| {
                let connector = std::sync::Arc::new(connector.clone());
                let effect_id = format!("fx-{}", i);
                tokio::spawn(async move {
                    connector.reconcile(&effect_id).await
                })
            })
            .collect();

        let mut ambiguous_count = 0;
        let mut committed_count = 0;

        for handle in handles {
            match handle.await.unwrap() {
                Ok(ReconcileOutcome::Committed { .. }) => committed_count += 1,
                Ok(ReconcileOutcome::StillAmbiguous) => ambiguous_count += 1,
                _ => {}
            }
        }

        assert!(ambiguous_count > 0, "Expected some ambiguous results under contention");
    }

    #[tokio::test]
    async fn high_concurrency_all_become_ambiguous() {
        let _guard = state_guard();
        let connector = ConcurrentReconciliationConnector::new(0);

        let handles: Vec<_> = (0..20)
            .map(|i| {
                let connector = std::sync::Arc::new(connector.clone());
                let effect_id = format!("fx-{}", i);
                tokio::spawn(async move {
                    connector.reconcile(&effect_id).await
                })
            })
            .collect();

        let mut ambiguous_count = 0;

        for handle in handles {
            match handle.await.unwrap() {
                Ok(ReconcileOutcome::StillAmbiguous) => ambiguous_count += 1,
                Ok(ReconcileOutcome::Committed { .. }) => {}
                _ => {}
            }
        }

        assert_eq!(ambiguous_count, 20);
    }

    #[tokio::test]
    async fn concurrent_with_prepare_commit_still_works() {
        let _guard = state_guard();
        let connector = ConcurrentReconciliationConnector::new(5);

        let pe = connector
            .prepare(serde_json::json!({}), "fx-1".into(), 1)
            .await
            .unwrap();
        let commit_outcome = connector.commit(pe).await.unwrap();
        assert!(matches!(commit_outcome, CommitOutcome::Committed { .. }));

        let reconcile_outcome = connector.reconcile("fx-1").await.unwrap();
        assert!(matches!(reconcile_outcome, ReconcileOutcome::Committed { .. }));
    }
}

#[cfg(test)]
mod red_queen_registry_reconciliation_tests {
    use super::*;

    #[tokio::test]
    async fn registry_stores_multiple_connectors() {
        let _guard = state_guard();
        let mut registry = ConnectorRegistry::new();

        registry.register(
            "ambiguous".to_string(),
            Box::new(AmbiguousAfterTimeoutConnector::new(0)),
        );
        registry.register(
            "fail-reconcile".to_string(),
            Box::new(ReconciliationFailingConnector { fail_reconcile: false }),
        );

        assert_eq!(registry.len(), 2);
        assert!(registry.get("ambiguous").is_some());
        assert!(registry.get("fail-reconcile").is_some());
    }

    #[tokio::test]
    async fn registry_get_returns_correct_connector() {
        let _guard = state_guard();
        let mut registry = ConnectorRegistry::new();

        registry.register(
            "test".to_string(),
            Box::new(AmbiguousAfterTimeoutConnector::new(10)),
        );

        let connector = registry.get("test").unwrap();
        let pe = connector
            .prepare(serde_json::json!({}), "fx-1".into(), 1)
            .await
            .unwrap();
        let outcome = connector.commit(pe).await.unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn registry_list_returns_all_names() {
        let _guard = state_guard();
        let mut registry = ConnectorRegistry::new();

        registry.register("a".to_string(), Box::new(AmbiguousAfterTimeoutConnector::new(0)));
        registry.register("b".to_string(), Box::new(ReconciliationFailingConnector { fail_reconcile: false }));

        let names = registry.list();
        assert!(names.contains(&"a"));
        assert!(names.contains(&"b"));
        assert_eq!(names.len(), 2);
    }

    #[tokio::test]
    async fn registry_get_nonexistent_returns_none() {
        let _guard = state_guard();
        let registry = ConnectorRegistry::new();

        assert!(registry.get("nonexistent").is_none());
    }
}

impl Clone for AmbiguousAfterTimeoutConnector {
    fn clone(&self) -> Self {
        Self {
            call_count: std::sync::atomic::AtomicUsize::new(
                self.call_count.load(std::sync::atomic::Ordering::SeqCst)
            ),
            timeout_threshold: self.timeout_threshold,
        }
    }
}

impl Clone for ReconciliationFailingConnector {
    fn clone(&self) -> Self {
        Self {
            fail_reconcile: self.fail_reconcile,
        }
    }
}

impl Clone for IdempotencyKeyCollisionConnector {
    fn clone(&self) -> Self {
        Self {
            seen_keys: std::sync::Mutex::new(
                self.seen_keys.lock().unwrap().clone()
            ),
        }
    }
}

impl Clone for ConcurrentReconciliationConnector {
    fn clone(&self) -> Self {
        Self {
            active_reconciles: std::sync::atomic::AtomicUsize::new(0),
            max_concurrent: self.max_concurrent,
        }
    }
}

#[cfg(test)]
mod red_queen_integration_tests {
    use super::*;

    #[tokio::test]
    async fn full_reconciliation_cycle_ambiguous_to_resolved() {
        let _guard = state_guard();
        let connector = AmbiguousAfterTimeoutConnector::new(1);

        let pe = connector
            .prepare(serde_json::json!({}), "fx-critical".into(), 1)
            .await
            .unwrap();
        let commit_outcome = connector.commit(pe).await.unwrap();
        assert!(matches!(commit_outcome, CommitOutcome::Committed { .. }));

        let reconcile_outcome_1 = connector.reconcile("fx-critical").await.unwrap();
        assert!(matches!(reconcile_outcome_1, ReconcileOutcome::StillAmbiguous));

        let pe2 = connector
            .prepare(serde_json::json!({}), "fx-critical-2".into(), 2)
            .await
            .unwrap();
        let commit_outcome_2 = connector.commit(pe2).await.unwrap();
        assert!(matches!(commit_outcome_2, CommitOutcome::Ambiguous));
    }

    #[tokio::test]
    async fn error_classification_preserved_through_reconciliation() {
        let _guard = state_guard();

        struct MixedErrorConnector;
        #[async_trait]
        impl Connector for MixedErrorConnector {
            fn connector_type(&self) -> &str { "mixed-error" }
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
                Ok(CommitOutcome::Committed { receipt: "ok".into() })
            }

            async fn reconcile(
                &self, effect_id: &str,
            ) -> Result<ReconcileOutcome, ConnectorError> {
                if effect_id.contains("retryable") {
                    Err(ConnectorError::retryable("transient failure"))
                } else if effect_id.contains("terminal") {
                    Err(ConnectorError::terminal("permanent failure"))
                } else {
                    Ok(ReconcileOutcome::NotCommitted)
                }
            }
        }

        let connector = MixedErrorConnector;

        let result_retryable = connector.reconcile("fx-retryable").await;
        assert!(result_retryable.is_err());
        assert!(result_retryable.unwrap_err().is_retryable());

        let result_terminal = connector.reconcile("fx-terminal").await;
        assert!(result_terminal.is_err());
        assert!(!result_terminal.unwrap_err().is_retryable());

        let result_ok = connector.reconcile("fx-ok").await;
        assert!(result_ok.is_ok());
        assert!(matches!(result_ok.unwrap(), ReconcileOutcome::NotCommitted));
    }

    #[tokio::test]
    async fn all_reconcile_outcome_variants_are_exercised() {
        let _guard = state_guard();

        struct AllOutcomesConnector;
        #[async_trait]
        impl Connector for AllOutcomesConnector {
            fn connector_type(&self) -> &str { "all-outcomes" }
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
                Ok(CommitOutcome::Committed { receipt: "ok".into() })
            }

            async fn reconcile(
                &self, effect_id: &str,
            ) -> Result<ReconcileOutcome, ConnectorError> {
                match effect_id {
                    "committed" => Ok(ReconcileOutcome::Committed { receipt: "found".into() }),
                    "not-committed" => Ok(ReconcileOutcome::NotCommitted),
                    "still-ambiguous" => Ok(ReconcileOutcome::StillAmbiguous),
                    _ => Err(ConnectorError::retryable("unknown")),
                }
            }
        }

        let connector = AllOutcomesConnector;

        assert!(matches!(
            connector.reconcile("committed").await.unwrap(),
            ReconcileOutcome::Committed { receipt: _ }
        ));
        assert!(matches!(
            connector.reconcile("not-committed").await.unwrap(),
            ReconcileOutcome::NotCommitted
        ));
        assert!(matches!(
            connector.reconcile("still-ambiguous").await.unwrap(),
            ReconcileOutcome::StillAmbiguous
        ));
    }
}
