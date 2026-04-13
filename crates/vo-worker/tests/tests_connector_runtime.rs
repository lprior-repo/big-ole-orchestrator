//! Connector runtime tests for idempotency-key HTTP connector and
//! mock SQL connector with unique constraints (ADR-041).

use std::sync::Mutex;

use async_trait::async_trait;
use vo_worker::{
    CommitOutcome, Connector, ConnectorError, ConnectorRegistry, HttpConnector,
    PreparedEffect, ReconcileOutcome,
};

// ========================================================================
// Mock SQL Connector with unique constraint simulation
// ========================================================================

struct MockSqlConnector {
    committed_keys: Mutex<Vec<String>>,
}

impl MockSqlConnector {
    fn new() -> Self {
        Self {
            committed_keys: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl Connector for MockSqlConnector {
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
        effect_intent: serde_json::Value,
        effect_id: String,
        fence: u64,
    ) -> Result<PreparedEffect, ConnectorError> {
        let unique_key = format!("{}:{}", effect_id, fence);
        let query = effect_intent["query"]
            .as_str()
            .unwrap_or("INSERT INTO effects (id) VALUES (?)");

        let payload = serde_json::json!({
            "unique_key": unique_key,
            "query": query,
            "effect_id": effect_id,
        });
        Ok(PreparedEffect {
            effect_id,
            payload,
            fence,
        })
    }

    async fn commit(
        &self,
        prepared: PreparedEffect,
    ) -> Result<CommitOutcome, ConnectorError> {
        let unique_key = prepared.payload["unique_key"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let mut committed = self.committed_keys.lock().unwrap();

        if committed.contains(&unique_key) {
            return Ok(CommitOutcome::Ambiguous);
        }

        committed.push(unique_key);
        Ok(CommitOutcome::Committed {
            receipt: format!("sql:{}", prepared.effect_id),
        })
    }

    async fn reconcile(
        &self,
        effect_id: &str,
    ) -> Result<ReconcileOutcome, ConnectorError> {
        let committed = self.committed_keys.lock().unwrap();
        let key = committed.iter().find(|k| k.contains(effect_id));
        if key.is_some() {
            Ok(ReconcileOutcome::Committed {
                receipt: format!("sql-reconcile:{}", effect_id),
            })
        } else {
            Ok(ReconcileOutcome::NotCommitted)
        }
    }

    async fn compensate(
        &self,
        compensation_intent: serde_json::Value,
        compensation_effect_id: String,
        fence: u64,
    ) -> Result<CommitOutcome, ConnectorError> {
        let unique_key = format!("{}:{}", compensation_effect_id, fence);
        let mut committed = self.committed_keys.lock().unwrap();

        if committed.contains(&unique_key) {
            return Ok(CommitOutcome::Ambiguous);
        }

        committed.push(unique_key);
        Ok(CommitOutcome::Committed {
            receipt: format!("sql-compensate:{}", compensation_intent),
        })
    }
}

// ========================================================================
// HTTP Connector Tests
// ========================================================================

#[tokio::test]
async fn http_connector_prepare_generates_idempotency_key_from_effect_id_and_fence() {
    let connector = HttpConnector::new("https://api.stripe.com");

    let pe = connector
        .prepare(
            serde_json::json!({"method": "POST", "path": "/v1/charges"}),
            "fx-stripe-charge".into(),
            1,
        )
        .await
        .unwrap();

    assert_eq!(pe.effect_id, "fx-stripe-charge");
    assert_eq!(pe.fence, 1);
    assert_eq!(pe.payload["idempotency_key"], "fx-stripe-charge:1");
    assert_eq!(pe.payload["base_url"], "https://api.stripe.com");
}

#[tokio::test]
async fn http_connector_fence_advancement_changes_idempotency_key() {
    let connector = HttpConnector::new("https://api.example.com");

    let pe1 = connector
        .prepare(serde_json::json!({}), "fx-1".into(), 1)
        .await
        .unwrap();
    let pe2 = connector
        .prepare(serde_json::json!({}), "fx-1".into(), 2)
        .await
        .unwrap();

    assert_eq!(pe1.payload["idempotency_key"], "fx-1:1");
    assert_eq!(pe2.payload["idempotency_key"], "fx-1:2");
    assert_ne!(
        pe1.payload["idempotency_key"],
        pe2.payload["idempotency_key"],
        "fence advancement must produce different idempotency keys"
    );
}

#[tokio::test]
async fn http_connector_reconcile_always_returns_still_ambiguous() {
    let connector = HttpConnector::new("https://api.example.com");

    let result = connector.reconcile("fx-any").await.unwrap();
    assert_eq!(result, ReconcileOutcome::StillAmbiguous);
}

#[tokio::test]
async fn http_connector_supports_compensation_false() {
    let connector = HttpConnector::new("https://api.example.com");
    assert!(!connector.supports_compensation());
}

#[tokio::test]
async fn http_connector_compensate_returns_not_supported() {
    let connector = HttpConnector::new("https://api.example.com");
    let result = connector
        .compensate(serde_json::json!({}), "comp-1".into(), 1)
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("http"));
}

#[tokio::test]
async fn http_connector_commit_invalid_method_returns_terminal_error() {
    let connector = HttpConnector::new("https://api.example.com");

    let pe = connector
        .prepare(
            serde_json::json!({"method": "INVALID", "path": "/test"}),
            "fx-bad-method".into(),
            1,
        )
        .await
        .unwrap();

    let result = connector.commit(pe).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(!err.is_retryable());
}

// ========================================================================
// Mock SQL Connector Tests
// ========================================================================

#[tokio::test]
async fn sql_connector_prepare_generates_unique_key() {
    let connector = MockSqlConnector::new();

    let pe = connector
        .prepare(
            serde_json::json!({"query": "INSERT INTO effects (id) VALUES (?)"}),
            "fx-sql-1".into(),
            1,
        )
        .await
        .unwrap();

    assert_eq!(pe.effect_id, "fx-sql-1");
    assert_eq!(pe.payload["unique_key"], "fx-sql-1:1");
}

#[tokio::test]
async fn sql_connector_commit_succeeds_first_time() {
    let connector = MockSqlConnector::new();

    let pe = connector
        .prepare(serde_json::json!({"query": "INSERT INTO t VALUES (1)"}), "fx-sql-2".into(), 1)
        .await
        .unwrap();

    let outcome = connector.commit(pe).await.unwrap();
    assert!(matches!(outcome, CommitOutcome::Committed { .. }));
}

#[tokio::test]
async fn sql_connector_commit_returns_ambiguous_on_duplicate_key() {
    let connector = MockSqlConnector::new();

    let pe1 = connector
        .prepare(serde_json::json!({}), "fx-sql-dup".into(), 1)
        .await
        .unwrap();
    let _ = connector.commit(pe1).await.unwrap();

    // Second commit with same key returns Ambiguous (unique constraint violation)
    let pe2 = connector
        .prepare(serde_json::json!({}), "fx-sql-dup".into(), 1)
        .await
        .unwrap();
    let outcome = connector.commit(pe2).await.unwrap();
    assert_eq!(outcome, CommitOutcome::Ambiguous);
}

#[tokio::test]
async fn sql_connector_reconcile_committed_returns_committed() {
    let connector = MockSqlConnector::new();

    let pe = connector
        .prepare(serde_json::json!({}), "fx-sql-reconcile".into(), 1)
        .await
        .unwrap();
    let _ = connector.commit(pe).await.unwrap();

    let outcome = connector.reconcile("fx-sql-reconcile").await.unwrap();
    assert!(matches!(outcome, ReconcileOutcome::Committed { .. }));
}

#[tokio::test]
async fn sql_connector_reconcile_not_committed_returns_not_committed() {
    let connector = MockSqlConnector::new();

    let outcome = connector.reconcile("fx-nonexistent").await.unwrap();
    assert_eq!(outcome, ReconcileOutcome::NotCommitted);
}

#[tokio::test]
async fn sql_connector_supports_compensation() {
    let connector = MockSqlConnector::new();
    assert!(connector.supports_compensation());
}

#[tokio::test]
async fn sql_connector_compensate_succeeds() {
    let connector = MockSqlConnector::new();

    let outcome = connector
        .compensate(
            serde_json::json!({"query": "DELETE FROM t WHERE id = ?"}),
            "comp-sql-1".into(),
            1,
        )
        .await
        .unwrap();
    assert!(matches!(outcome, CommitOutcome::Committed { .. }));
}

#[tokio::test]
async fn sql_connector_compensate_idempotent() {
    let connector = MockSqlConnector::new();

    let outcome1 = connector
        .compensate(serde_json::json!({}), "comp-sql-dup".into(), 1)
        .await
        .unwrap();
    assert!(matches!(outcome1, CommitOutcome::Committed { .. }));

    // Duplicate compensation returns Ambiguous
    let outcome2 = connector
        .compensate(serde_json::json!({}), "comp-sql-dup".into(), 1)
        .await
        .unwrap();
    assert_eq!(outcome2, CommitOutcome::Ambiguous);
}

// ========================================================================
// Registry Tests
// ========================================================================

#[tokio::test]
async fn registry_stores_http_and_sql_connectors() {
    let mut reg = ConnectorRegistry::new();

    reg.register(
        "http-stripe".to_string(),
        Box::new(HttpConnector::new("https://api.stripe.com")),
    );
    reg.register(
        "sql-primary".to_string(),
        Box::new(MockSqlConnector::new()),
    );

    assert_eq!(reg.len(), 2);

    let http = reg.get("http-stripe").unwrap();
    assert_eq!(http.connector_type(), "http");

    let sql = reg.get("sql-primary").unwrap();
    assert_eq!(sql.connector_type(), "sql");
    assert!(sql.supports_compensation());
}

#[tokio::test]
async fn registry_connector_lifecycle() {
    let mut reg = ConnectorRegistry::new();

    reg.register("sql".to_string(), Box::new(MockSqlConnector::new()));

    let connector = reg.get("sql").unwrap();

    let pe = connector
        .prepare(
            serde_json::json!({"query": "INSERT INTO t VALUES (1)"}),
            "fx-reg-1".into(),
            1,
        )
        .await
        .unwrap();

    let outcome = connector.commit(pe).await.unwrap();
    assert!(matches!(outcome, CommitOutcome::Committed { .. }));

    let reconcile = connector.reconcile("fx-reg-1").await.unwrap();
    assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
}

// ========================================================================
// Full Lifecycle: HTTP Connector prepare/commit/reconcile
// ========================================================================

#[tokio::test]
async fn http_connector_full_lifecycle_prepare_commit() {
    let connector = HttpConnector::new("https://api.example.com");

    let pe = connector
        .prepare(
            serde_json::json!({"method": "POST", "path": "/api/effects", "body": {"key": "val"}}),
            "fx-lifecycle".into(),
            1,
        )
        .await
        .unwrap();

    assert_eq!(pe.payload["idempotency_key"], "fx-lifecycle:1");
    assert_eq!(pe.payload["request"]["method"], "POST");
    assert_eq!(pe.payload["request"]["path"], "/api/effects");

    // Commit would make HTTP request; reconcile returns StillAmbiguous
    let reconcile = connector.reconcile("fx-lifecycle").await.unwrap();
    assert_eq!(reconcile, ReconcileOutcome::StillAmbiguous);
}

// ========================================================================
// Full Lifecycle: SQL Connector with crash simulation
// ========================================================================

#[tokio::test]
async fn sql_connector_crash_after_prepare_recover_and_commit() {
    let connector = MockSqlConnector::new();

    // Step 1: Prepare (journal the effect)
    let pe = connector
        .prepare(
            serde_json::json!({"query": "INSERT INTO t VALUES (1)"}),
            "fx-crash-sql".into(),
            1,
        )
        .await
        .unwrap();

    // Simulate crash: effect is prepared but not committed
    // Recovery: reconcile first
    let reconcile = connector.reconcile("fx-crash-sql").await.unwrap();
    assert_eq!(
        reconcile,
        ReconcileOutcome::NotCommitted,
        "effect was not committed before crash"
    );

    // Re-commit
    let outcome = connector.commit(pe).await.unwrap();
    assert!(matches!(outcome, CommitOutcome::Committed { .. }));

    // Verify reconciliation now returns Committed
    let reconcile_after = connector.reconcile("fx-crash-sql").await.unwrap();
    assert!(matches!(reconcile_after, ReconcileOutcome::Committed { .. }));
}

#[tokio::test]
async fn sql_connector_duplicate_commit_returns_ambiguous_then_reconcile_resolves() {
    let connector = MockSqlConnector::new();

    // First commit succeeds
    let pe1 = connector
        .prepare(serde_json::json!({}), "fx-dup-resolve".into(), 1)
        .await
        .unwrap();
    let _ = connector.commit(pe1).await.unwrap();

    // Second commit returns Ambiguous (unique constraint)
    let pe2 = connector
        .prepare(serde_json::json!({}), "fx-dup-resolve".into(), 1)
        .await
        .unwrap();
    let outcome = connector.commit(pe2).await.unwrap();
    assert_eq!(outcome, CommitOutcome::Ambiguous);

    // Reconciliation resolves the ambiguity
    let reconcile = connector.reconcile("fx-dup-resolve").await.unwrap();
    assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
}
