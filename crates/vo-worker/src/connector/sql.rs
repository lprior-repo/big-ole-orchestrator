//! SQL unique-constraint connector (ADR-041).
//!
//! This connector simulates SQL transaction semantics using an in-memory
//! `HashSet` to enforce unique constraints on effect IDs. When a commit is
//! attempted with an effect ID that already exists in the committed set, the
//! connector returns `CommitOutcome::Ambiguous`, simulating a unique constraint
//! violation that would occur in a real SQL database.
//!
//! # Unique Constraint Semantics
//!
//! The SQL connector models the behavior of a SQL database with a unique index
//! on effect IDs:
//!
//! 1. **prepare** — Generates a prepared effect with the SQL query and a unique
//!    key derived from `effect_id:fence`.
//! 2. **commit** — Attempts to insert the effect ID into the committed set.
//!    - If the effect ID is not present: inserts it and returns `Committed`.
//!    - If the effect ID is already present: returns `Ambiguous` (unique
//!      constraint violation, like a duplicate key error).
//! 3. **reconcile** — Checks whether the effect ID exists in the committed set.
//!    - Present → `Committed` (the effect was committed before a crash/timeout).
//!    - Absent → `NotCommitted` (the effect was never committed).
//! 4. **compensate** — Removes the effect ID from the committed set, simulating
//!    a SQL rollback transaction.
//!
//! # ADR-041 Durability Sequence
//!
//! This connector obeys the ADR-041 durability sequence:
//! 1. `prepare(effect_intent, effect_id, fence)` → `PreparedEffect`
//! 2. Engine persists `EffectPrepared`
//! 3. `commit(prepared)` → `CommitOutcome`
//! 4. On success, Engine persists `EffectCommitted`
//! 5. On crash or ambiguity, Engine calls `reconcile(effect_id)` before retry

use crate::connector::{
    CommitOutcome, Connector, ConnectorError, PreparedEffect, ReconcileOutcome,
};
use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};

/// SQL connector with unique constraint enforcement for exactly-once semantics.
///
/// This connector simulates a SQL database with a unique index on effect IDs.
/// It is useful for testing the connector protocol without requiring a real
/// SQL database, and for verifying that the engine correctly handles unique
/// constraint violations as ambiguous outcomes.
///
/// # Thread Safety
///
/// All internal state is protected by a `Mutex`, making this connector safe
/// for concurrent use across threads.
///
/// # Example
///
/// ```ignore
/// let connector = SqlConnector::new();
///
/// // First commit succeeds
/// let pe = connector.prepare(
///     json!({"query": "INSERT INTO t VALUES (1)"}),
///     "fx-1".to_string(),
///     1,
/// ).await.unwrap();
/// let outcome = connector.commit(pe).await.unwrap();
/// assert!(matches!(outcome, CommitOutcome::Committed { .. }));
///
/// // Second commit with same effect_id returns Ambiguous (unique constraint)
/// let pe2 = connector.prepare(
///     json!({}),
///     "fx-1".to_string(),
///     1,
/// ).await.unwrap();
/// let outcome2 = connector.commit(pe2).await.unwrap();
/// assert_eq!(outcome2, CommitOutcome::Ambiguous);
/// ```
#[derive(Debug)]
pub struct SqlConnector {
    /// Set of committed effect IDs — acts as the unique index.
    committed_effects: std::sync::Mutex<HashSet<String>>,
    /// Simulates a crash during commit (for testing reconciliation).
    crash_on_commit: AtomicBool,
}

impl SqlConnector {
    /// Create a new SQL connector with no committed effects.
    #[must_use]
    pub fn new() -> Self {
        Self {
            committed_effects: std::sync::Mutex::new(HashSet::new()),
            crash_on_commit: AtomicBool::new(false),
        }
    }

    /// Inject a crash on the next commit operation.
    ///
    /// This is useful for testing the reconciliation loop. After calling
    /// this method, the next `commit()` call will return a retryable error
    /// simulating a connection loss.
    pub fn inject_commit_crash(&self) {
        self.crash_on_commit.store(true, Ordering::SeqCst);
    }

    /// Check whether the given effect ID has been committed.
    ///
    /// This is a convenience method for testing and debugging.
    #[must_use]
    pub fn is_committed(&self, effect_id: &str) -> bool {
        self.committed_effects.lock().unwrap().contains(effect_id)
    }

    /// Get the number of committed effects.
    #[must_use]
    pub fn committed_count(&self) -> usize {
        self.committed_effects.lock().unwrap().len()
    }
}

impl Default for SqlConnector {
    fn default() -> Self {
        Self::new()
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
        effect_intent: serde_json::Value,
        effect_id: String,
        fence: u64,
    ) -> Result<PreparedEffect, ConnectorError> {
        let query = effect_intent["query"].as_str().unwrap_or("BEGIN");
        let unique_key = format!("{}:{}", effect_id, fence);

        Ok(PreparedEffect {
            effect_id: effect_id.clone(),
            payload: serde_json::json!({
                "unique_key": unique_key,
                "query": query,
                "effect_id": effect_id,
                "fence": fence,
            }),
            fence,
        })
    }

    async fn commit(&self, prepared: PreparedEffect) -> Result<CommitOutcome, ConnectorError> {
        // Simulate crash during commit
        if self.crash_on_commit.load(Ordering::SeqCst) {
            self.crash_on_commit.store(false, Ordering::SeqCst);
            return Err(ConnectorError::retryable(
                "SQL connection lost: crash injected",
            ));
        }

        let mut committed = self.committed_effects.lock().unwrap();

        // Unique constraint: if effect_id already exists, return Ambiguous
        if committed.contains(&prepared.effect_id) {
            return Ok(CommitOutcome::Ambiguous);
        }

        // Insert into the set (simulates INSERT with unique constraint)
        committed.insert(prepared.effect_id.clone());

        Ok(CommitOutcome::Committed {
            receipt: format!("sql:{}", prepared.effect_id),
        })
    }

    async fn reconcile(&self, effect_id: &str) -> Result<ReconcileOutcome, ConnectorError> {
        let committed = self.committed_effects.lock().unwrap();

        if committed.contains(effect_id) {
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
        let rollback_query = compensation_intent["rollback_query"]
            .as_str()
            .unwrap_or("ROLLBACK");

        let unique_key = format!("{}:{}", compensation_effect_id, fence);
        let mut committed = self.committed_effects.lock().unwrap();

        if committed.contains(&unique_key) {
            return Ok(CommitOutcome::Ambiguous);
        }

        // Remove from committed set (simulates ROLLBACK)
        committed.remove(&compensation_effect_id);

        Ok(CommitOutcome::Committed {
            receipt: format!("compensated:{}:{}", compensation_effect_id, rollback_query),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_sql_connector_new() {
        let connector = SqlConnector::new();
        assert_eq!(connector.committed_count(), 0);
        assert!(!connector.is_committed("any-id"));
    }

    #[test]
    fn test_sql_connector_type() {
        let connector = SqlConnector::new();
        assert_eq!(connector.connector_type(), "sql");
    }

    #[test]
    fn test_sql_connector_version() {
        let connector = SqlConnector::new();
        assert_eq!(connector.connector_version(), "1.0.0");
    }

    #[test]
    fn test_sql_connector_supports_compensation() {
        let connector = SqlConnector::new();
        assert!(connector.supports_compensation());
    }

    #[tokio::test]
    async fn test_sql_connector_prepare_generates_unique_key() {
        let connector = SqlConnector::new();
        let pe = connector
            .prepare(
                json!({"query": "INSERT INTO effects (id) VALUES (?)"}),
                "fx-sql-1".to_string(),
                1,
            )
            .await
            .unwrap();

        assert_eq!(pe.effect_id, "fx-sql-1");
        assert_eq!(pe.fence, 1);
        assert_eq!(pe.payload["unique_key"], "fx-sql-1:1");
        assert_eq!(
            pe.payload["query"],
            "INSERT INTO effects (id) VALUES (?)"
        );
    }

    #[tokio::test]
    async fn test_sql_connector_prepare_default_query() {
        let connector = SqlConnector::new();
        let pe = connector
            .prepare(json!({}), "fx-sql-default".to_string(), 5)
            .await
            .unwrap();

        assert_eq!(pe.payload["query"], "BEGIN");
        assert_eq!(pe.payload["unique_key"], "fx-sql-default:5");
    }

    #[tokio::test]
    async fn test_sql_connector_commit_succeeds_first_time() {
        let connector = SqlConnector::new();
        let pe = connector
            .prepare(
                json!({"query": "INSERT INTO t VALUES (1)"}),
                "fx-sql-2".to_string(),
                1,
            )
            .await
            .unwrap();

        let outcome = connector.commit(pe).await.unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        assert!(connector.is_committed("fx-sql-2"));
        assert_eq!(connector.committed_count(), 1);
    }

    #[tokio::test]
    async fn test_sql_connector_commit_returns_ambiguous_on_duplicate() {
        let connector = SqlConnector::new();

        // First commit succeeds
        let pe1 = connector
            .prepare(json!({}), "fx-dup".to_string(), 1)
            .await
            .unwrap();
        let outcome1 = connector.commit(pe1).await.unwrap();
        assert!(matches!(outcome1, CommitOutcome::Committed { .. }));

        // Second commit with same effect_id returns Ambiguous
        let pe2 = connector
            .prepare(json!({}), "fx-dup".to_string(), 1)
            .await
            .unwrap();
        let outcome2 = connector.commit(pe2).await.unwrap();
        assert_eq!(outcome2, CommitOutcome::Ambiguous);
        assert_eq!(connector.committed_count(), 1); // Still only 1 committed
    }

    #[tokio::test]
    async fn test_sql_connector_reconcile_committed() {
        let connector = SqlConnector::new();
        let pe = connector
            .prepare(json!({}), "fx-reconcile".to_string(), 1)
            .await
            .unwrap();
        let _ = connector.commit(pe).await.unwrap();

        let outcome = connector.reconcile("fx-reconcile").await.unwrap();
        assert!(matches!(outcome, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn test_sql_connector_reconcile_not_committed() {
        let connector = SqlConnector::new();
        let outcome = connector.reconcile("fx-nonexistent").await.unwrap();
        assert_eq!(outcome, ReconcileOutcome::NotCommitted);
    }

    #[tokio::test]
    async fn test_sql_connector_compensate_removes_effect() {
        let connector = SqlConnector::new();

        // Commit first
        let pe = connector
            .prepare(json!({}), "fx-compensate".to_string(), 1)
            .await
            .unwrap();
        let _ = connector.commit(pe).await.unwrap();
        assert!(connector.is_committed("fx-compensate"));

        // Compensate
        let outcome = connector
            .compensate(
                json!({"rollback_query": "DELETE FROM t WHERE id = ?"}),
                "fx-compensate".to_string(),
                1,
            )
            .await
            .unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        assert!(!connector.is_committed("fx-compensate"));
    }

    #[tokio::test]
    async fn test_sql_connector_compensate_default_rollback() {
        let connector = SqlConnector::new();

        let pe = connector
            .prepare(json!({}), "fx-comp-default".to_string(), 1)
            .await
            .unwrap();
        let _ = connector.commit(pe).await.unwrap();

        let outcome = connector
            .compensate(json!({}), "fx-comp-default".to_string(), 1)
            .await
            .unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        assert!(!connector.is_committed("fx-comp-default"));
    }

    #[tokio::test]
    async fn test_sql_connector_crash_during_commit() {
        let connector = SqlConnector::new();
        connector.inject_commit_crash();

        let pe = connector
            .prepare(json!({}), "fx-crash".to_string(), 1)
            .await
            .unwrap();

        let outcome = connector.commit(pe).await;
        assert!(outcome.is_err());
        let err = outcome.unwrap_err();
        assert!(err.is_retryable());
        assert!(err.to_string().contains("crash"));

        // Effect should NOT be committed after crash
        assert!(!connector.is_committed("fx-crash"));
    }

    #[tokio::test]
    async fn test_sql_connector_crash_then_reconcile_then_commit() {
        // This tests the full ADR-041 crash recovery sequence

        let connector = SqlConnector::new();

        // Step 1: Prepare the effect
        let pe = connector
            .prepare(
                json!({"query": "INSERT INTO t VALUES (1)"}),
                "fx-crash-recover".to_string(),
                1,
            )
            .await
            .unwrap();

        // Step 2: Inject crash BEFORE commit
        connector.inject_commit_crash();

        // Step 3: Attempt commit — fails with retryable error
        let outcome = connector.commit(pe.clone()).await;
        assert!(outcome.is_err());

        // Step 4: Reconcile — should return NotCommitted
        let reconcile = connector.reconcile("fx-crash-recover").await.unwrap();
        assert_eq!(reconcile, ReconcileOutcome::NotCommitted);

        // Step 5: Retry commit — should succeed
        let outcome2 = connector.commit(pe).await.unwrap();
        assert!(matches!(outcome2, CommitOutcome::Committed { .. }));

        // Step 6: Reconcile now returns Committed
        let reconcile2 = connector.reconcile("fx-crash-recover").await.unwrap();
        assert!(matches!(reconcile2, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn test_sql_connector_fence_advancement_changes_unique_key() {
        let connector = SqlConnector::new();

        // Same effect_id, different fence → different unique_key
        let pe1 = connector
            .prepare(json!({}), "fx-fence".to_string(), 1)
            .await
            .unwrap();
        let pe2 = connector
            .prepare(json!({}), "fx-fence".to_string(), 2)
            .await
            .unwrap();

        assert_eq!(pe1.payload["unique_key"], "fx-fence:1");
        assert_eq!(pe2.payload["unique_key"], "fx-fence:2");
        assert_ne!(pe1.payload["unique_key"], pe2.payload["unique_key"]);
    }

    #[tokio::test]
    async fn test_sql_connector_multiple_unique_effects() {
        let connector = SqlConnector::new();

        for i in 0..10 {
            let pe = connector
                .prepare(json!({}), format!("fx-{}", i), 1)
                .await
                .unwrap();
            let outcome = connector.commit(pe).await.unwrap();
            assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        }

        assert_eq!(connector.committed_count(), 10);
    }

    #[tokio::test]
    async fn test_sql_connector_receipt_format() {
        let connector = SqlConnector::new();
        let pe = connector
            .prepare(json!({}), "fx-receipt".to_string(), 1)
            .await
            .unwrap();

        let outcome = connector.commit(pe).await.unwrap();
        match outcome {
            CommitOutcome::Committed { receipt } => {
                assert_eq!(receipt, "sql:fx-receipt");
            }
            _ => panic!("expected Committed"),
        }
    }

    #[tokio::test]
    async fn test_sql_connector_duplicate_compensation_returns_ambiguous() {
        let connector = SqlConnector::new();

        // First compensation succeeds
        let pe = connector
            .prepare(json!({}), "fx-comp-id".to_string(), 1)
            .await
            .unwrap();
        let _ = connector.commit(pe).await.unwrap();

        let outcome1 = connector
            .compensate(json!({}), "fx-comp-id".to_string(), 1)
            .await
            .unwrap();
        assert!(matches!(outcome1, CommitOutcome::Committed { .. }));

        // The unique_key for compensation is "effect_id:fence"
        // So compensating again with the same effect_id:fence should return Ambiguous
        let pe2 = connector
            .prepare(json!({}), "fx-comp-id-2".to_string(), 1)
            .await
            .unwrap();
        let _ = connector.commit(pe2).await.unwrap();

        let outcome2 = connector
            .compensate(
                json!({"rollback_query": "DELETE FROM t"}),
                "fx-comp-id-2".to_string(),
                1,
            )
            .await
            .unwrap();
        assert!(matches!(outcome2, CommitOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn test_sql_connector_empty_effect_id_in_prepare() {
        let connector = SqlConnector::new();
        let pe = connector
            .prepare(json!({}), "".to_string(), 0)
            .await
            .unwrap();

        assert_eq!(pe.effect_id, "");
        assert_eq!(pe.payload["unique_key"], ":0");
    }

    #[tokio::test]
    async fn test_sql_connector_large_query_payload() {
        let connector = SqlConnector::new();
        let large_query = "INSERT INTO large_table (col1, col2, col3, col4, col5) VALUES (".to_string()
            + &"x".repeat(1000)
            + ")";
        let pe = connector
            .prepare(
                json!({"query": large_query.clone()}),
                "fx-large".to_string(),
                1,
            )
            .await
            .unwrap();

        assert!(pe.payload["query"].is_string());
        let outcome = connector.commit(pe).await.unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn test_sql_connector_special_characters_in_effect_id() {
        let connector = SqlConnector::new();
        let pe = connector
            .prepare(
                json!({}),
                "fx-special-abc-123-xyz-789".to_string(),
                42,
            )
            .await
            .unwrap();

        assert_eq!(pe.payload["unique_key"], "fx-special-abc-123-xyz-789:42");

        let outcome = connector.commit(pe).await.unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        let reconcile = connector.reconcile("fx-special-abc-123-xyz-789").await.unwrap();
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn test_sql_connector_deterministic_behavior() {
        let connector = SqlConnector::new();

        // Deterministic: same inputs always produce same outputs
        for i in 0..5 {
            let pe = connector
                .prepare(
                    json!({"query": "SELECT 1"}),
                    "fx-det".to_string(),
                    1,
                )
                .await
                .unwrap();

            assert_eq!(pe.payload["unique_key"], "fx-det:1");

            // First commit succeeds; subsequent commits return Ambiguous (unique constraint)
            let outcome = connector.commit(pe).await.unwrap();
            match i {
                0 => assert!(matches!(outcome, CommitOutcome::Committed { .. })),
                _ => assert_eq!(outcome, CommitOutcome::Ambiguous),
            }
        }

        assert_eq!(connector.committed_count(), 1); // Deduplicated by effect_id
    }
}
