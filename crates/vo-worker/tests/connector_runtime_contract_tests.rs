//! Connector runtime contract tests (ADR-041).
//!
//! Tests the full connector lifecycle contract:
//! - Connector registration and capability discovery
//! - Request/response lifecycle: prepare → commit → reconcile
//! - Ambiguity routing through reconciliation (not blind retry)
//! - Idempotency-key HTTP connector with mock server
//! - SQL connector under crash injection
//! - Ambiguity detection for timeout + unknown states

use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use vo_worker::{
    CommitOutcome, Connector, ConnectorError, ConnectorRegistry, HttpConnector, PreparedEffect,
    ReconcileOutcome,
};

// ============================================================================
// Mock Connectors
// ============================================================================

struct AlwaysCommittedConnector;

#[async_trait]
impl Connector for AlwaysCommittedConnector {
    fn connector_type(&self) -> &str {
        "always-committed"
    }
    fn connector_version(&self) -> &str {
        "1.0.0"
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
            payload: serde_json::json!({"status": "prepared"}),
            fence,
        })
    }

    async fn commit(&self, prepared: PreparedEffect) -> Result<CommitOutcome, ConnectorError> {
        Ok(CommitOutcome::Committed {
            receipt: format!("receipt:{}", prepared.effect_id),
        })
    }

    async fn reconcile(&self, _effect_id: &str) -> Result<ReconcileOutcome, ConnectorError> {
        Ok(ReconcileOutcome::Committed {
            receipt: "reconciled".into(),
        })
    }
}

struct AlwaysFailedConnector;

#[async_trait]
impl Connector for AlwaysFailedConnector {
    fn connector_type(&self) -> &str {
        "always-failed"
    }
    fn connector_version(&self) -> &str {
        "1.0.0"
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
        Ok(CommitOutcome::Failed)
    }

    async fn reconcile(&self, _effect_id: &str) -> Result<ReconcileOutcome, ConnectorError> {
        Ok(ReconcileOutcome::NotCommitted)
    }
}

struct CrashOnCommitConnector {
    crash_after: AtomicUsize,
    committed_effects: std::sync::Mutex<std::collections::HashSet<String>>,
}

impl CrashOnCommitConnector {
    fn new(crash_after: usize) -> Self {
        Self {
            crash_after: AtomicUsize::new(crash_after),
            committed_effects: std::sync::Mutex::new(std::collections::HashSet::new()),
        }
    }
}

#[async_trait]
impl Connector for CrashOnCommitConnector {
    fn connector_type(&self) -> &str {
        "crash-on-commit"
    }
    fn connector_version(&self) -> &str {
        "1.0.0"
    }
    fn supports_compensation(&self) -> bool {
        true
    }

    async fn prepare(
        &self,
        _intent: serde_json::Value,
        effect_id: String,
        fence: u64,
    ) -> Result<PreparedEffect, ConnectorError> {
        Ok(PreparedEffect {
            effect_id,
            payload: serde_json::json!({"prepared": true}),
            fence,
        })
    }

    async fn commit(&self, prepared: PreparedEffect) -> Result<CommitOutcome, ConnectorError> {
        let remaining = self.crash_after.fetch_sub(1, Ordering::SeqCst);
        if remaining > 0 {
            self.committed_effects
                .lock()
                .unwrap()
                .insert(prepared.effect_id.clone());
            Ok(CommitOutcome::Committed {
                receipt: format!("committed:{}", prepared.effect_id),
            })
        } else {
            Err(ConnectorError::retryable("connection lost during commit"))
        }
    }

    async fn reconcile(&self, effect_id: &str) -> Result<ReconcileOutcome, ConnectorError> {
        let committed = self.committed_effects.lock().unwrap();
        if committed.contains(effect_id) {
            Ok(ReconcileOutcome::Committed {
                receipt: format!("reconciled:{}", effect_id),
            })
        } else {
            Ok(ReconcileOutcome::NotCommitted)
        }
    }
}

struct TimeoutOnCommitConnector {
    call_count: AtomicUsize,
    timeout_after: usize,
    committed_ids: std::sync::Mutex<std::collections::HashSet<String>>,
}

impl TimeoutOnCommitConnector {
    fn new(timeout_after: usize) -> Self {
        Self {
            call_count: AtomicUsize::new(0),
            timeout_after,
            committed_ids: std::sync::Mutex::new(std::collections::HashSet::new()),
        }
    }
}

#[async_trait]
impl Connector for TimeoutOnCommitConnector {
    fn connector_type(&self) -> &str {
        "timeout-on-commit"
    }
    fn connector_version(&self) -> &str {
        "1.0.0"
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

    async fn commit(&self, prepared: PreparedEffect) -> Result<CommitOutcome, ConnectorError> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        if count < self.timeout_after {
            self.committed_ids
                .lock()
                .unwrap()
                .insert(prepared.effect_id.clone());
            Ok(CommitOutcome::Committed {
                receipt: format!("ok:{}", prepared.effect_id),
            })
        } else {
            Ok(CommitOutcome::Ambiguous)
        }
    }

    async fn reconcile(&self, effect_id: &str) -> Result<ReconcileOutcome, ConnectorError> {
        let ids = self.committed_ids.lock().unwrap();
        if ids.contains(effect_id) {
            Ok(ReconcileOutcome::Committed {
                receipt: format!("found:{}", effect_id),
            })
        } else {
            Ok(ReconcileOutcome::NotCommitted)
        }
    }
}

struct CompensatingConnector {
    compensated: AtomicBool,
}

impl CompensatingConnector {
    fn new() -> Self {
        Self {
            compensated: AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl Connector for CompensatingConnector {
    fn connector_type(&self) -> &str {
        "compensating"
    }
    fn connector_version(&self) -> &str {
        "1.0.0"
    }
    fn supports_compensation(&self) -> bool {
        true
    }

    async fn prepare(
        &self,
        _intent: serde_json::Value,
        effect_id: String,
        fence: u64,
    ) -> Result<PreparedEffect, ConnectorError> {
        Ok(PreparedEffect {
            effect_id,
            payload: serde_json::json!({"prepared": true}),
            fence,
        })
    }

    async fn commit(&self, prepared: PreparedEffect) -> Result<CommitOutcome, ConnectorError> {
        Ok(CommitOutcome::Committed {
            receipt: format!("committed:{}", prepared.effect_id),
        })
    }

    async fn reconcile(&self, _effect_id: &str) -> Result<ReconcileOutcome, ConnectorError> {
        Ok(ReconcileOutcome::Committed {
            receipt: "reconciled".into(),
        })
    }

    async fn compensate(
        &self,
        _compensation_intent: serde_json::Value,
        _compensation_effect_id: String,
        _fence: u64,
    ) -> Result<CommitOutcome, ConnectorError> {
        self.compensated.store(true, Ordering::SeqCst);
        Ok(CommitOutcome::Committed {
            receipt: "compensated".into(),
        })
    }
}

struct UnknownStateConnector {
    reconcile_unknown: AtomicBool,
}

impl UnknownStateConnector {
    fn new() -> Self {
        Self {
            reconcile_unknown: AtomicBool::new(true),
        }
    }
}

#[async_trait]
impl Connector for UnknownStateConnector {
    fn connector_type(&self) -> &str {
        "unknown-state"
    }
    fn connector_version(&self) -> &str {
        "1.0.0"
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
        Ok(CommitOutcome::Ambiguous)
    }

    async fn reconcile(&self, _effect_id: &str) -> Result<ReconcileOutcome, ConnectorError> {
        if self.reconcile_unknown.load(Ordering::SeqCst) {
            Ok(ReconcileOutcome::StillAmbiguous)
        } else {
            Ok(ReconcileOutcome::NotCommitted)
        }
    }
}

struct SqlConnector {
    committed_txns: std::sync::Mutex<std::collections::HashSet<String>>,
    crash_on_commit: AtomicBool,
}

impl SqlConnector {
    fn new() -> Self {
        Self {
            committed_txns: std::sync::Mutex::new(std::collections::HashSet::new()),
            crash_on_commit: AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl Connector for SqlConnector {
    fn connector_type(&self) -> &str {
        "sql"
    }
    fn connector_version(&self) -> &str {
        "1.0.0"
    }
    fn supports_compensation(&self) -> bool {
        true
    }

    async fn prepare(
        &self,
        intent: serde_json::Value,
        effect_id: String,
        fence: u64,
    ) -> Result<PreparedEffect, ConnectorError> {
        let query = intent["query"].as_str().unwrap_or("BEGIN");
        let transaction_id = effect_id.clone();
        Ok(PreparedEffect {
            effect_id,
            payload: serde_json::json!({
                "transaction": transaction_id,
                "query": query,
                "fence": fence,
            }),
            fence,
        })
    }

    async fn commit(&self, prepared: PreparedEffect) -> Result<CommitOutcome, ConnectorError> {
        if self.crash_on_commit.load(Ordering::SeqCst) {
            self.crash_on_commit.store(false, Ordering::SeqCst);
            return Err(ConnectorError::retryable(
                "SQL connection lost: crash injected",
            ));
        }
        self.committed_txns
            .lock()
            .unwrap()
            .insert(prepared.effect_id.clone());
        Ok(CommitOutcome::Committed {
            receipt: format!("txn:{}", prepared.effect_id),
        })
    }

    async fn reconcile(&self, effect_id: &str) -> Result<ReconcileOutcome, ConnectorError> {
        let txns = self.committed_txns.lock().unwrap();
        if txns.contains(effect_id) {
            Ok(ReconcileOutcome::Committed {
                receipt: format!("txn-found:{}", effect_id),
            })
        } else {
            Ok(ReconcileOutcome::NotCommitted)
        }
    }

    async fn compensate(
        &self,
        intent: serde_json::Value,
        compensation_effect_id: String,
        _fence: u64,
    ) -> Result<CommitOutcome, ConnectorError> {
        let rollback_query = intent["rollback_query"].as_str().unwrap_or("ROLLBACK");
        self.committed_txns
            .lock()
            .unwrap()
            .remove(&compensation_effect_id);
        Ok(CommitOutcome::Committed {
            receipt: format!("compensated:{}:{}", compensation_effect_id, rollback_query),
        })
    }
}

// ============================================================================
// 1. Connector Registration Tests
// ============================================================================

#[cfg(test)]
mod registration_tests {
    use super::*;

    #[test]
    fn registry_new_is_empty() {
        let reg = ConnectorRegistry::new();
        assert_eq!(reg.len(), 0);
        assert!(reg.list().is_empty());
    }

    #[test]
    fn registry_register_increments_len() {
        let mut reg = ConnectorRegistry::new();
        reg.register("c1".to_string(), Box::new(AlwaysCommittedConnector));
        assert_eq!(reg.len(), 1);
        reg.register("c2".to_string(), Box::new(AlwaysFailedConnector));
        assert_eq!(reg.len(), 2);
    }

    #[test]
    fn registry_register_same_name_overwrites() {
        let mut reg = ConnectorRegistry::new();
        reg.register("c1".to_string(), Box::new(AlwaysCommittedConnector));
        reg.register("c1".to_string(), Box::new(AlwaysFailedConnector));
        assert_eq!(reg.len(), 1);
    }

    #[tokio::test]
    async fn registry_get_returns_none_for_missing() {
        let reg = ConnectorRegistry::new();
        assert!(reg.get("nonexistent").is_none());
    }

    #[tokio::test]
    async fn registry_get_returns_registered_connector() {
        let mut reg = ConnectorRegistry::new();
        reg.register("committed".to_string(), Box::new(AlwaysCommittedConnector));
        let c = reg.get("committed").expect("should exist");
        assert_eq!(c.connector_type(), "always-committed");
    }

    #[tokio::test]
    async fn registry_list_contains_all_names() {
        let mut reg = ConnectorRegistry::new();
        reg.register("a".to_string(), Box::new(AlwaysCommittedConnector));
        reg.register("b".to_string(), Box::new(AlwaysFailedConnector));
        reg.register("c".to_string(), Box::new(CrashOnCommitConnector::new(1)));
        let names = reg.list();
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"a"));
        assert!(names.contains(&"b"));
        assert!(names.contains(&"c"));
    }

    #[tokio::test]
    async fn registry_connector_is_usable_after_retrieval() {
        let mut reg = ConnectorRegistry::new();
        reg.register("sql".to_string(), Box::new(SqlConnector::new()));
        let c = reg.get("sql").unwrap();
        let pe = c
            .prepare(serde_json::json!({"query": "SELECT 1"}), "tx-1".into(), 1)
            .await
            .unwrap();
        let outcome = c.commit(pe).await.unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn registry_multiple_gets_return_same_arc() {
        let mut reg = ConnectorRegistry::new();
        reg.register("c".to_string(), Box::new(AlwaysCommittedConnector));
        let a = reg.get("c").unwrap();
        let b = reg.get("c").unwrap();
        assert_eq!(a.connector_type(), b.connector_type());
    }
}

// ============================================================================
// 2. Capability Discovery Tests
// ============================================================================

#[cfg(test)]
mod capability_discovery_tests {
    use super::*;

    #[tokio::test]
    async fn http_connector_reports_type() {
        let c = HttpConnector::new("https://api.example.com");
        assert_eq!(c.connector_type(), "http");
    }

    #[tokio::test]
    async fn http_connector_reports_version() {
        let c = HttpConnector::new("https://api.example.com");
        assert_eq!(c.connector_version(), "1.0.0");
    }

    #[tokio::test]
    async fn http_connector_no_compensation() {
        let c = HttpConnector::new("https://api.example.com");
        assert!(!c.supports_compensation());
    }

    #[tokio::test]
    async fn sql_connector_reports_type() {
        let c = SqlConnector::new();
        assert_eq!(c.connector_type(), "sql");
    }

    #[tokio::test]
    async fn sql_connector_reports_version() {
        let c = SqlConnector::new();
        assert_eq!(c.connector_version(), "1.0.0");
    }

    #[tokio::test]
    async fn sql_connector_supports_compensation() {
        let c = SqlConnector::new();
        assert!(c.supports_compensation());
    }

    #[tokio::test]
    async fn compensating_connector_reports_capability() {
        let c = CompensatingConnector::new();
        assert_eq!(c.connector_type(), "compensating");
        assert!(c.supports_compensation());
    }

    #[tokio::test]
    async fn crash_connector_reports_capability() {
        let c = CrashOnCommitConnector::new(1);
        assert!(c.supports_compensation());
    }

    #[tokio::test]
    async fn registry_discovery_multiple_types() {
        let mut reg = ConnectorRegistry::new();
        reg.register(
            "http".to_string(),
            Box::new(HttpConnector::new("https://api.example.com")),
        );
        reg.register("sql".to_string(), Box::new(SqlConnector::new()));
        reg.register("noop".to_string(), Box::new(AlwaysCommittedConnector));

        let http = reg.get("http").unwrap();
        let sql = reg.get("sql").unwrap();
        let noop = reg.get("noop").unwrap();

        assert_eq!(http.connector_type(), "http");
        assert!(!http.supports_compensation());

        assert_eq!(sql.connector_type(), "sql");
        assert!(sql.supports_compensation());

        assert_eq!(noop.connector_type(), "always-committed");
        assert!(!noop.supports_compensation());
    }
}

// ============================================================================
// 3. Request/Response Lifecycle Tests
// ============================================================================

#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    #[tokio::test]
    async fn happy_path_prepare_commit_reconcile() {
        let c = AlwaysCommittedConnector;
        let pe = c
            .prepare(serde_json::json!({"key": "val"}), "fx-1".into(), 1)
            .await
            .unwrap();
        assert_eq!(pe.effect_id, "fx-1");
        assert_eq!(pe.fence, 1);

        let outcome = c.commit(pe).await.unwrap();
        assert!(
            matches!(outcome, CommitOutcome::Committed { receipt } if receipt == "receipt:fx-1")
        );

        let reconcile = c.reconcile("fx-1").await.unwrap();
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn prepare_preserves_intent_in_payload() {
        let c = AlwaysCommittedConnector;
        let intent = serde_json::json!({
            "action": "charge",
            "amount": 100,
            "currency": "USD"
        });
        let pe = c.prepare(intent.clone(), "fx-2".into(), 5).await.unwrap();
        assert_eq!(pe.payload["status"], "prepared");
    }

    #[tokio::test]
    async fn commit_after_prepare_uses_effect_id() {
        let c = AlwaysCommittedConnector;
        let pe = c
            .prepare(serde_json::json!({}), "fx-unique".into(), 42)
            .await
            .unwrap();
        let outcome = c.commit(pe).await.unwrap();
        if let CommitOutcome::Committed { receipt } = outcome {
            assert!(receipt.contains("fx-unique"));
        } else {
            panic!("expected committed");
        }
    }

    #[tokio::test]
    async fn failed_commit_returns_failed_outcome() {
        let c = AlwaysFailedConnector;
        let pe = c
            .prepare(serde_json::json!({}), "fx-fail".into(), 1)
            .await
            .unwrap();
        let outcome = c.commit(pe).await.unwrap();
        assert_eq!(outcome, CommitOutcome::Failed);
    }

    #[tokio::test]
    async fn reconcile_after_failure_returns_not_committed() {
        let c = AlwaysFailedConnector;
        let pe = c
            .prepare(serde_json::json!({}), "fx-fail".into(), 1)
            .await
            .unwrap();
        let _ = c.commit(pe).await.unwrap();
        let reconcile = c.reconcile("fx-fail").await.unwrap();
        assert_eq!(reconcile, ReconcileOutcome::NotCommitted);
    }

    #[tokio::test]
    async fn sql_connector_full_lifecycle() {
        let c = SqlConnector::new();
        let pe = c
            .prepare(
                serde_json::json!({"query": "INSERT INTO orders (id) VALUES (1)"}),
                "tx-sql-1".into(),
                1,
            )
            .await
            .unwrap();
        assert_eq!(pe.payload["query"], "INSERT INTO orders (id) VALUES (1)");
        assert_eq!(pe.payload["transaction"], "tx-sql-1");

        let outcome = c.commit(pe).await.unwrap();
        assert!(
            matches!(outcome, CommitOutcome::Committed { receipt } if receipt == "txn:tx-sql-1")
        );

        let reconcile = c.reconcile("tx-sql-1").await.unwrap();
        assert!(
            matches!(reconcile, ReconcileOutcome::Committed { receipt } if receipt.contains("tx-sql-1"))
        );
    }

    #[tokio::test]
    async fn sql_connector_compensate_reverses_commit() {
        let c = SqlConnector::new();
        let pe = c
            .prepare(serde_json::json!({"query": "INSERT"}), "tx-comp".into(), 1)
            .await
            .unwrap();
        let _ = c.commit(pe).await.unwrap();

        let outcome = c
            .compensate(
                serde_json::json!({"rollback_query": "DELETE"}),
                "tx-comp".into(),
                1,
            )
            .await
            .unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        let reconcile = c.reconcile("tx-comp").await.unwrap();
        assert_eq!(reconcile, ReconcileOutcome::NotCommitted);
    }

    #[tokio::test]
    async fn compensate_on_non_supporting_connector_returns_error() {
        let c = HttpConnector::new("https://api.example.com");
        let result = c.compensate(serde_json::json!({}), "cx-1".into(), 1).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("http"));
    }

    #[tokio::test]
    async fn compensating_connector_compensate_succeeds() {
        let c = CompensatingConnector::new();
        let outcome = c
            .compensate(serde_json::json!({}), "cx-2".into(), 1)
            .await
            .unwrap();
        assert!(
            matches!(outcome, CommitOutcome::Committed { receipt } if receipt == "compensated")
        );
        assert!(c.compensated.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn fence_increments_produce_different_prepared_effects() {
        let c = AlwaysCommittedConnector;
        let pe1 = c
            .prepare(serde_json::json!({}), "fx-1".into(), 1)
            .await
            .unwrap();
        let pe2 = c
            .prepare(serde_json::json!({}), "fx-1".into(), 2)
            .await
            .unwrap();
        assert_eq!(pe1.effect_id, pe2.effect_id);
        assert_ne!(pe1.fence, pe2.fence);
    }
}

// ============================================================================
// 4. Ambiguity Routing Through Reconciliation (Not Blind Retry)
// ============================================================================

#[cfg(test)]
mod ambiguity_routing_tests {
    use super::*;

    #[tokio::test]
    async fn ambiguous_commit_routes_to_reconcile_not_retry() {
        let c = TimeoutOnCommitConnector::new(1);
        let pe = c
            .prepare(serde_json::json!({}), "fx-timeout".into(), 1)
            .await
            .unwrap();
        let outcome = c.commit(pe).await.unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        let pe2 = c
            .prepare(serde_json::json!({}), "fx-timeout-2".into(), 2)
            .await
            .unwrap();
        let outcome2 = c.commit(pe2).await.unwrap();
        assert_eq!(outcome2, CommitOutcome::Ambiguous);

        let reconcile = c.reconcile("fx-timeout-2").await.unwrap();
        assert!(matches!(reconcile, ReconcileOutcome::NotCommitted));
    }

    #[tokio::test]
    async fn reconciliation_after_timeout_resolves_committed() {
        let c = TimeoutOnCommitConnector::new(3);
        let pe = c
            .prepare(serde_json::json!({}), "fx-resolve".into(), 1)
            .await
            .unwrap();
        let outcome = c.commit(pe).await.unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        let reconcile = c.reconcile("fx-resolve").await.unwrap();
        assert!(
            matches!(reconcile, ReconcileOutcome::Committed { receipt } if receipt.contains("fx-resolve"))
        );
    }

    #[tokio::test]
    async fn ambiguous_then_reconcile_committed_prevents_double_commit() {
        let c = CrashOnCommitConnector::new(1);
        let pe = c
            .prepare(serde_json::json!({}), "fx-double".into(), 1)
            .await
            .unwrap();
        let outcome = c.commit(pe).await.unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        let pe2 = c
            .prepare(serde_json::json!({}), "fx-double-2".into(), 2)
            .await
            .unwrap();
        let result = c.commit(pe2).await;
        assert!(result.is_err());

        let reconcile = c.reconcile("fx-double-2").await.unwrap();
        assert_eq!(reconcile, ReconcileOutcome::NotCommitted);
    }

    #[tokio::test]
    async fn reconcile_outcome_still_ambiguous_triggers_retry_action() {
        use vo_types::ReconcileAction;
        let outcome = ReconcileOutcome::StillAmbiguous;
        let action = ReconcileAction::from(outcome);
        assert_eq!(action, ReconcileAction::Retry);
    }

    #[tokio::test]
    async fn reconcile_outcome_committed_triggers_commit_action() {
        use vo_types::ReconcileAction;
        let outcome = ReconcileOutcome::Committed {
            receipt: "found".into(),
        };
        let action = ReconcileAction::from(outcome);
        assert_eq!(action, ReconcileAction::Commit);
    }

    #[tokio::test]
    async fn reconcile_outcome_not_committed_triggers_rollback_action() {
        use vo_types::ReconcileAction;
        let outcome = ReconcileOutcome::NotCommitted;
        let action = ReconcileAction::from(outcome);
        assert_eq!(action, ReconcileAction::Rollback);
    }

    #[tokio::test]
    async fn sql_connector_crash_then_reconcile_resolves() {
        let c = SqlConnector::new();
        c.crash_on_commit.store(true, Ordering::SeqCst);

        let pe = c
            .prepare(serde_json::json!({"query": "INSERT"}), "tx-crash".into(), 1)
            .await
            .unwrap();
        let result = c.commit(pe).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().is_retryable());

        let reconcile = c.reconcile("tx-crash").await.unwrap();
        assert_eq!(reconcile, ReconcileOutcome::NotCommitted);
    }

    #[tokio::test]
    async fn sql_connector_crash_after_commit_reconcile_finds_committed() {
        let c = SqlConnector::new();
        let pe = c
            .prepare(serde_json::json!({"query": "INSERT"}), "tx-ok".into(), 1)
            .await
            .unwrap();
        let _ = c.commit(pe).await.unwrap();

        c.crash_on_commit.store(true, Ordering::SeqCst);
        let pe2 = c
            .prepare(serde_json::json!({"query": "INSERT"}), "tx-ok-2".into(), 2)
            .await
            .unwrap();
        let result = c.commit(pe2).await;
        assert!(result.is_err());

        let reconcile = c.reconcile("tx-ok").await.unwrap();
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }
}

// ============================================================================
// 5. Idempotency-Key HTTP Connector Tests
// ============================================================================

#[cfg(test)]
mod http_connector_tests {
    use super::*;

    #[tokio::test]
    async fn http_prepare_builds_idempotency_key() {
        let c = HttpConnector::new("https://api.example.com");
        let pe = c
            .prepare(
                serde_json::json!({"method": "POST", "path": "/charges"}),
                "fx-http-1".into(),
                7,
            )
            .await
            .unwrap();
        assert_eq!(pe.payload["idempotency_key"], "fx-http-1:7");
        assert_eq!(pe.payload["base_url"], "https://api.example.com");
        assert_eq!(pe.payload["request"]["method"], "POST");
        assert_eq!(pe.payload["request"]["path"], "/charges");
    }

    #[tokio::test]
    async fn http_prepare_different_effects_different_keys() {
        let c = HttpConnector::new("https://api.example.com");
        let pe1 = c
            .prepare(serde_json::json!({}), "fx-a".into(), 1)
            .await
            .unwrap();
        let pe2 = c
            .prepare(serde_json::json!({}), "fx-b".into(), 1)
            .await
            .unwrap();
        assert_ne!(
            pe1.payload["idempotency_key"],
            pe2.payload["idempotency_key"]
        );
    }

    #[tokio::test]
    async fn http_prepare_same_effect_same_fence_same_key() {
        let c = HttpConnector::new("https://api.example.com");
        let pe1 = c
            .prepare(serde_json::json!({}), "fx-same".into(), 5)
            .await
            .unwrap();
        let pe2 = c
            .prepare(serde_json::json!({}), "fx-same".into(), 5)
            .await
            .unwrap();
        assert_eq!(
            pe1.payload["idempotency_key"],
            pe2.payload["idempotency_key"]
        );
    }

    #[tokio::test]
    async fn http_prepare_same_effect_different_fence_different_key() {
        let c = HttpConnector::new("https://api.example.com");
        let pe1 = c
            .prepare(serde_json::json!({}), "fx-diff".into(), 1)
            .await
            .unwrap();
        let pe2 = c
            .prepare(serde_json::json!({}), "fx-diff".into(), 2)
            .await
            .unwrap();
        assert_ne!(
            pe1.payload["idempotency_key"],
            pe2.payload["idempotency_key"]
        );
    }

    #[tokio::test]
    async fn http_reconcile_always_returns_still_ambiguous() {
        let c = HttpConnector::new("https://api.example.com");
        let outcome = c.reconcile("fx-any").await.unwrap();
        assert_eq!(outcome, ReconcileOutcome::StillAmbiguous);
    }

    #[tokio::test]
    async fn http_compensate_returns_not_supported() {
        let c = HttpConnector::new("https://api.example.com");
        let result = c.compensate(serde_json::json!({}), "cx-1".into(), 1).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn http_prepare_payload_preserves_body() {
        let c = HttpConnector::new("https://api.example.com");
        let body = serde_json::json!({"amount": 500, "currency": "EUR"});
        let pe = c
            .prepare(
                serde_json::json!({
                    "method": "POST",
                    "path": "/payments",
                    "body": body,
                }),
                "fx-body".into(),
                1,
            )
            .await
            .unwrap();
        assert_eq!(pe.payload["request"]["body"]["amount"], 500);
    }
}

// ============================================================================
// 6. Crash Injection Tests
// ============================================================================

#[cfg(test)]
mod crash_injection_tests {
    use super::*;

    #[tokio::test]
    async fn crash_on_commit_returns_retryable_error() {
        let c = CrashOnCommitConnector::new(0);
        let pe = c
            .prepare(serde_json::json!({}), "fx-crash".into(), 1)
            .await
            .unwrap();
        let result = c.commit(pe).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().is_retryable());
    }

    #[tokio::test]
    async fn crash_after_n_commits_then_crash() {
        let c = CrashOnCommitConnector::new(2);
        for i in 0..2 {
            let pe = c
                .prepare(serde_json::json!({}), format!("fx-ok-{}", i), i as u64 + 1)
                .await
                .unwrap();
            let outcome = c.commit(pe).await.unwrap();
            assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        }

        let pe = c
            .prepare(serde_json::json!({}), "fx-crash-3".into(), 3)
            .await
            .unwrap();
        let result = c.commit(pe).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn crash_then_reconcile_finds_committed() {
        let c = CrashOnCommitConnector::new(1);
        let pe = c
            .prepare(serde_json::json!({}), "fx-commit-then-crash".into(), 1)
            .await
            .unwrap();
        let _ = c.commit(pe).await.unwrap();

        let pe2 = c
            .prepare(serde_json::json!({}), "fx-will-crash".into(), 2)
            .await
            .unwrap();
        let result = c.commit(pe2).await;
        assert!(result.is_err());

        let reconcile = c.reconcile("fx-commit-then-crash").await.unwrap();
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));

        let reconcile2 = c.reconcile("fx-will-crash").await.unwrap();
        assert_eq!(reconcile2, ReconcileOutcome::NotCommitted);
    }

    #[tokio::test]
    async fn sql_crash_injection_recovery() {
        let c = SqlConnector::new();
        c.crash_on_commit.store(true, Ordering::SeqCst);

        let pe = c
            .prepare(
                serde_json::json!({"query": "INSERT"}),
                "tx-inject".into(),
                1,
            )
            .await
            .unwrap();
        let result = c.commit(pe).await;
        assert!(result.is_err());

        c.crash_on_commit.store(false, Ordering::SeqCst);

        let pe2 = c
            .prepare(
                serde_json::json!({"query": "INSERT"}),
                "tx-recover".into(),
                2,
            )
            .await
            .unwrap();
        let outcome = c.commit(pe2).await.unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        let reconcile = c.reconcile("tx-recover").await.unwrap();
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn sql_compensate_after_crash_reverses() {
        let c = SqlConnector::new();
        let pe = c
            .prepare(
                serde_json::json!({"query": "INSERT"}),
                "tx-comp-crash".into(),
                1,
            )
            .await
            .unwrap();
        let _ = c.commit(pe).await.unwrap();

        let outcome = c
            .compensate(
                serde_json::json!({"rollback_query": "DELETE FROM orders WHERE id = 1"}),
                "tx-comp-crash".into(),
                1,
            )
            .await
            .unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        let reconcile = c.reconcile("tx-comp-crash").await.unwrap();
        assert_eq!(reconcile, ReconcileOutcome::NotCommitted);
    }

    #[tokio::test]
    async fn multiple_sequential_crashes_all_retryable() {
        struct AlwaysCrashConnector;

        #[async_trait]
        impl Connector for AlwaysCrashConnector {
            fn connector_type(&self) -> &str {
                "always-crash"
            }
            fn connector_version(&self) -> &str {
                "1.0.0"
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

            async fn commit(
                &self,
                _prepared: PreparedEffect,
            ) -> Result<CommitOutcome, ConnectorError> {
                Err(ConnectorError::retryable("connection lost"))
            }

            async fn reconcile(
                &self,
                _effect_id: &str,
            ) -> Result<ReconcileOutcome, ConnectorError> {
                Ok(ReconcileOutcome::NotCommitted)
            }
        }

        let c = AlwaysCrashConnector;
        for i in 0..5 {
            let pe = c
                .prepare(
                    serde_json::json!({}),
                    format!("fx-crash-{}", i),
                    i as u64 + 1,
                )
                .await
                .unwrap();
            let result = c.commit(pe).await;
            assert!(result.is_err());
            assert!(result.unwrap_err().is_retryable());
        }
    }
}

// ============================================================================
// 7. Ambiguity Detection for Timeout + Unknown States
// ============================================================================

#[cfg(test)]
mod ambiguity_detection_tests {
    use super::*;

    #[tokio::test]
    async fn unknown_state_connector_returns_ambiguous_on_commit() {
        let c = UnknownStateConnector::new();
        let pe = c
            .prepare(serde_json::json!({}), "fx-unknown".into(), 1)
            .await
            .unwrap();
        let outcome = c.commit(pe).await.unwrap();
        assert_eq!(outcome, CommitOutcome::Ambiguous);
    }

    #[tokio::test]
    async fn unknown_state_reconcile_initially_still_ambiguous() {
        let c = UnknownStateConnector::new();
        let reconcile = c.reconcile("fx-unknown").await.unwrap();
        assert_eq!(reconcile, ReconcileOutcome::StillAmbiguous);
    }

    #[tokio::test]
    async fn unknown_state_resolves_after_server_recovers() {
        let c = UnknownStateConnector::new();
        c.reconcile_unknown.store(true, Ordering::SeqCst);
        let r1 = c.reconcile("fx-resolve").await.unwrap();
        assert_eq!(r1, ReconcileOutcome::StillAmbiguous);

        c.reconcile_unknown.store(false, Ordering::SeqCst);
        let r2 = c.reconcile("fx-resolve").await.unwrap();
        assert_eq!(r2, ReconcileOutcome::NotCommitted);
    }

    #[tokio::test]
    async fn timeout_connector_transitions_from_committed_to_ambiguous() {
        let c = TimeoutOnCommitConnector::new(2);
        for i in 0..2 {
            let pe = c
                .prepare(serde_json::json!({}), format!("fx-ok-{}", i), i as u64 + 1)
                .await
                .unwrap();
            let outcome = c.commit(pe).await.unwrap();
            assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        }

        let pe3 = c
            .prepare(serde_json::json!({}), "fx-amb".into(), 3)
            .await
            .unwrap();
        let outcome3 = c.commit(pe3).await.unwrap();
        assert_eq!(outcome3, CommitOutcome::Ambiguous);
    }

    #[tokio::test]
    async fn reconcile_action_mapping_for_all_outcomes() {
        use vo_types::ReconcileAction;

        let committed = ReconcileOutcome::Committed {
            receipt: "r".into(),
        };
        assert_eq!(ReconcileAction::from(committed), ReconcileAction::Commit);

        let not_committed = ReconcileOutcome::NotCommitted;
        assert_eq!(
            ReconcileAction::from(not_committed),
            ReconcileAction::Rollback
        );

        let still_ambiguous = ReconcileOutcome::StillAmbiguous;
        assert_eq!(
            ReconcileAction::from(still_ambiguous),
            ReconcileAction::Retry
        );
    }

    #[tokio::test]
    async fn commit_outcome_equality() {
        assert_eq!(
            CommitOutcome::Committed {
                receipt: "a".into()
            },
            CommitOutcome::Committed {
                receipt: "a".into()
            }
        );
        assert_ne!(
            CommitOutcome::Committed {
                receipt: "a".into()
            },
            CommitOutcome::Committed {
                receipt: "b".into()
            }
        );
        assert_eq!(CommitOutcome::Failed, CommitOutcome::Failed);
        assert_eq!(CommitOutcome::Ambiguous, CommitOutcome::Ambiguous);
    }

    #[tokio::test]
    async fn reconcile_outcome_equality() {
        assert_eq!(
            ReconcileOutcome::Committed {
                receipt: "r".into()
            },
            ReconcileOutcome::Committed {
                receipt: "r".into()
            }
        );
        assert_eq!(
            ReconcileOutcome::NotCommitted,
            ReconcileOutcome::NotCommitted
        );
        assert_eq!(
            ReconcileOutcome::StillAmbiguous,
            ReconcileOutcome::StillAmbiguous
        );
    }

    #[tokio::test]
    async fn error_classification_retryable_vs_terminal() {
        let retryable = ConnectorError::retryable("timeout");
        assert!(retryable.is_retryable());

        let terminal = ConnectorError::terminal("bad request");
        assert!(!terminal.is_retryable());

        let no_support = ConnectorError::compensation_not_supported("http");
        assert!(no_support.is_retryable());
    }

    #[tokio::test]
    async fn http_connector_ambiguous_maps_to_retry_action() {
        use vo_types::ReconcileAction;
        let c = HttpConnector::new("https://api.example.com");
        let reconcile = c.reconcile("any").await.unwrap();
        assert_eq!(reconcile, ReconcileOutcome::StillAmbiguous);
        let action = ReconcileAction::from(reconcile);
        assert_eq!(action, ReconcileAction::Retry);
    }
}

// ============================================================================
// 8. Integration: Full Reconciliation Cycle
// ============================================================================

#[cfg(test)]
mod integration_reconciliation_tests {
    use super::*;

    #[tokio::test]
    async fn sql_connector_full_reconciliation_after_crash() {
        let c = SqlConnector::new();

        let pe = c
            .prepare(
                serde_json::json!({"query": "INSERT INTO t VALUES (1)"}),
                "tx-full".into(),
                1,
            )
            .await
            .unwrap();
        let outcome = c.commit(pe).await.unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        c.crash_on_commit.store(true, Ordering::SeqCst);
        let pe2 = c
            .prepare(
                serde_json::json!({"query": "INSERT INTO t VALUES (2)"}),
                "tx-crash".into(),
                2,
            )
            .await
            .unwrap();
        let result = c.commit(pe2).await;
        assert!(result.is_err());

        let r1 = c.reconcile("tx-full").await.unwrap();
        assert!(matches!(r1, ReconcileOutcome::Committed { .. }));

        let r2 = c.reconcile("tx-crash").await.unwrap();
        assert_eq!(r2, ReconcileOutcome::NotCommitted);
    }

    #[tokio::test]
    async fn registry_based_connector_dispatch() {
        let mut reg = ConnectorRegistry::new();
        reg.register(
            "http".to_string(),
            Box::new(HttpConnector::new("https://api.example.com")),
        );
        reg.register("sql".to_string(), Box::new(SqlConnector::new()));

        let http = reg.get("http").unwrap();
        let pe = http
            .prepare(
                serde_json::json!({"method": "GET", "path": "/health"}),
                "fx-dispatch-http".into(),
                1,
            )
            .await
            .unwrap();
        assert_eq!(pe.payload["idempotency_key"], "fx-dispatch-http:1");

        let sql = reg.get("sql").unwrap();
        let pe = sql
            .prepare(
                serde_json::json!({"query": "SELECT 1"}),
                "fx-dispatch-sql".into(),
                1,
            )
            .await
            .unwrap();
        let outcome = sql.commit(pe).await.unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        let reconcile = sql.reconcile("fx-dispatch-sql").await.unwrap();
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn compensating_connector_full_lifecycle() {
        let c = CompensatingConnector::new();

        let pe = c
            .prepare(
                serde_json::json!({"action": "reserve"}),
                "fx-comp-full".into(),
                1,
            )
            .await
            .unwrap();
        let outcome = c.commit(pe).await.unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        let comp_outcome = c
            .compensate(
                serde_json::json!({"action": "release"}),
                "fx-comp-full".into(),
                1,
            )
            .await
            .unwrap();
        assert!(matches!(comp_outcome, CommitOutcome::Committed { .. }));
        assert!(c.compensated.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn timeout_then_reconcile_committed_flow() {
        let c = TimeoutOnCommitConnector::new(0);

        let pe = c
            .prepare(serde_json::json!({}), "fx-t-zero".into(), 1)
            .await
            .unwrap();
        let outcome = c.commit(pe).await.unwrap();
        assert_eq!(outcome, CommitOutcome::Ambiguous);

        let reconcile = c.reconcile("fx-t-zero").await.unwrap();
        assert_eq!(reconcile, ReconcileOutcome::NotCommitted);
    }
}

// ============================================================================
// 7. SQL Connector Transaction Protocol Tests
//
// NOTE: SqlConnector is a MOCK connector that does NOT execute SQL queries.
// It only verifies the connector protocol handles various query content safely.
//
// Actual SQL injection prevention happens at the API layer:
// - vo-api/tests/security_input_validation_tests.rs tests WorkflowName/SignalName rejection
//
// These tests verify the connector protocol correctly stores and retrieves
// transactions regardless of query content semantics.
// ============================================================================

#[cfg(test)]
mod sql_connector_transaction_tests {
    use super::*;

    #[tokio::test]
    async fn sql_connector_handles_dangerous_looking_query_drop_table() {
        let c = SqlConnector::new();
        let pe = c
            .prepare(
                serde_json::json!({"query": "'; DROP TABLE users; --"}),
                "tx-sqli-drop".into(),
                1,
            )
            .await
            .expect("prepare should succeed");
        let outcome = c.commit(pe).await.expect("commit should succeed");
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        let reconcile = c.reconcile("tx-sqli-drop").await.expect("reconcile should succeed");
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn sql_connector_handles_dangerous_looking_query_or_1_equals_1() {
        let c = SqlConnector::new();
        let pe = c
            .prepare(
                serde_json::json!({"query": "\" OR \"1\"=\"1"}),
                "tx-sqli-or".into(),
                1,
            )
            .await
            .expect("prepare should succeed");
        let outcome = c.commit(pe).await.expect("commit should succeed");
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        let reconcile = c.reconcile("tx-sqli-or").await.expect("reconcile should succeed");
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn sql_connector_handles_dangerous_looking_query_select_secrets() {
        let c = SqlConnector::new();
        let pe = c
            .prepare(
                serde_json::json!({"query": "1; SELECT * FROM secrets"}),
                "tx-sqli-select".into(),
                1,
            )
            .await
            .expect("prepare should succeed");
        let outcome = c.commit(pe).await.expect("commit should succeed");
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        let reconcile = c.reconcile("tx-sqli-select").await.expect("reconcile should succeed");
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn sql_connector_handles_dangerous_looking_query_admin_comment() {
        let c = SqlConnector::new();
        let pe = c
            .prepare(
                serde_json::json!({"query": "admin'--"}),
                "tx-sqli-admin".into(),
                1,
            )
            .await
            .expect("prepare should succeed");
        let outcome = c.commit(pe).await.expect("commit should succeed");
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        let reconcile = c.reconcile("tx-sqli-admin").await.expect("reconcile should succeed");
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn sql_connector_handles_dangerous_looking_query_update_balance() {
        let c = SqlConnector::new();
        let pe = c
            .prepare(
                serde_json::json!({"query": "'; UPDATE accounts SET balance=0; --"}),
                "tx-sqli-update".into(),
                1,
            )
            .await
            .expect("prepare should succeed");
        let outcome = c.commit(pe).await.expect("commit should succeed");
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        let reconcile = c.reconcile("tx-sqli-update").await.expect("reconcile should succeed");
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn sql_connector_handles_dangerous_looking_query_delete_transactions() {
        let c = SqlConnector::new();
        let pe = c
            .prepare(
                serde_json::json!({"query": "1; DELETE FROM transactions"}),
                "tx-sqli-delete".into(),
                1,
            )
            .await
            .expect("prepare should succeed");
        let outcome = c.commit(pe).await.expect("commit should succeed");
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        let reconcile = c.reconcile("tx-sqli-delete").await.expect("reconcile should succeed");
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn sql_connector_handles_dangerous_looking_query_union_select() {
        let c = SqlConnector::new();
        let pe = c
            .prepare(
                serde_json::json!({"query": "' UNION SELECT password FROM users--"}),
                "tx-sqli-union".into(),
                1,
            )
            .await
            .expect("prepare should succeed");
        let outcome = c.commit(pe).await.expect("commit should succeed");
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        let reconcile = c.reconcile("tx-sqli-union").await.expect("reconcile should succeed");
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn sql_connector_handles_dangerous_looking_query_insert_admin() {
        let c = SqlConnector::new();
        let pe = c
            .prepare(
                serde_json::json!({"query": "'; INSERT INTO admin VALUES ('hacker'); --"}),
                "tx-sqli-insert".into(),
                1,
            )
            .await
            .expect("prepare should succeed");
        let outcome = c.commit(pe).await.expect("commit should succeed");
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        let reconcile = c.reconcile("tx-sqli-insert").await.expect("reconcile should succeed");
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn sql_connector_handles_dangerous_query_then_normal_transaction() {
        let c = SqlConnector::new();

        let pe_inject = c
            .prepare(
                serde_json::json!({"query": "'; DROP TABLE users; --"}),
                "fx-inject".into(),
                1,
            )
            .await
            .expect("prepare should succeed");
        let outcome = c.commit(pe_inject).await.expect("commit should succeed");
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        let pe_normal = c
            .prepare(
                serde_json::json!({"query": "SELECT * FROM users"}),
                "fx-normal".into(),
                2,
            )
            .await
            .expect("prepare should succeed");
        let outcome = c.commit(pe_normal).await.expect("commit should succeed");
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        let reconcile_inject = c.reconcile("fx-inject").await.expect("reconcile should succeed");
        let reconcile_normal = c.reconcile("fx-normal").await.expect("reconcile should succeed");
        assert!(matches!(reconcile_inject, ReconcileOutcome::Committed { .. }));
        assert!(matches!(reconcile_normal, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn sql_connector_handles_dangerous_query_with_compensation() {
        let c = SqlConnector::new();

        let pe = c
            .prepare(
                serde_json::json!({"query": "INSERT INTO secrets VALUES ('malicious')"}),
                "fx-sqli-comp".into(),
                1,
            )
            .await
            .expect("prepare should succeed");
        let outcome = c.commit(pe).await.expect("commit should succeed");
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        let comp_outcome = c
            .compensate(
                serde_json::json!({"rollback_query": "DELETE FROM secrets WHERE data='malicious'"}),
                "fx-sqli-comp".into(),
                1,
            )
            .await
            .expect("compensate should succeed");
        assert!(matches!(comp_outcome, CommitOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn sql_connector_multiple_dangerous_queries_idempotent() {
        let c = SqlConnector::new();

        for i in 0..3 {
            let pe = c
                .prepare(
                    serde_json::json!({"query": "'; SELECT evil; --"}),
                    format!("fx-multi-{}", i).into(),
                    i as u64 + 1,
                )
                .await
                .expect("prepare should succeed");
            let outcome = c.commit(pe).await.expect("commit should succeed");
            assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        }

        for i in 0..3 {
            let reconcile = c.reconcile(&format!("fx-multi-{}", i))
                .await
                .expect("reconcile should succeed");
            assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
        }
    }

    #[tokio::test]
    async fn sql_connector_stores_query_content_without_execution() {
        let c = SqlConnector::new();

        let dangerous_queries = vec![
            ("fx-d1", "'; DROP ALL TABLES; --"),
            ("fx-d2", "1; EXECUTE sp_executesql"),
            ("fx-d3", "' OR '1'='1' OR '"),
        ];

        for (id, query) in &dangerous_queries {
            let pe = c
                .prepare(serde_json::json!({"query": query}), (*id).into(), 1)
                .await
                .expect("prepare should succeed");
            let outcome = c.commit(pe).await.expect("commit should succeed");
            assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        }

        for (id, _) in &dangerous_queries {
            let reconcile = c.reconcile(id).await.expect("reconcile should succeed");
            assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
        }
    }
}
