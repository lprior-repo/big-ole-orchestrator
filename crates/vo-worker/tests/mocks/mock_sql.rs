//! SqlConnector mock for SQL connector protocol tests.
//!
//! This mock does NOT execute SQL queries. It verifies the connector
//! protocol correctly stores and retrieves transactions regardless
//! of query content semantics.

use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use vo_worker::{
    CommitOutcome, Connector, ConnectorError, PreparedEffect, ReconcileOutcome,
};

pub struct SqlConnector {
    committed_txns: std::sync::Mutex<HashSet<String>>,
    crash_on_commit: AtomicBool,
}

impl SqlConnector {
    pub fn new() -> Self {
        Self {
            committed_txns: std::sync::Mutex::new(HashSet::new()),
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
