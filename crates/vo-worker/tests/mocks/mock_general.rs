//! General-purpose mock connectors for connector contract tests.
//!
//! Covers: AlwaysCommitted, AlwaysFailed, CrashOnCommit, TimeoutOnCommit,
//! Compensating, UnknownState, and AlwaysCrash (inline in tests).

use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use vo_worker::{
    CommitOutcome, Connector, ConnectorError, PreparedEffect, ReconcileOutcome,
};

// -- AlwaysCommittedConnector ------------------------------------------------

pub struct AlwaysCommittedConnector;

#[async_trait]
impl Connector for AlwaysCommittedConnector {
    fn connector_type(&self) -> &str { "always-committed" }
    fn connector_version(&self) -> &str { "1.0.0" }
    fn supports_compensation(&self) -> bool { false }

    async fn prepare(
        &self, _intent: serde_json::Value, effect_id: String, fence: u64,
    ) -> Result<PreparedEffect, ConnectorError> {
        Ok(PreparedEffect { effect_id, payload: serde_json::json!({"status": "prepared"}), fence })
    }

    async fn commit(&self, prepared: PreparedEffect) -> Result<CommitOutcome, ConnectorError> {
        Ok(CommitOutcome::Committed { receipt: format!("receipt:{}", prepared.effect_id) })
    }

    async fn reconcile(&self, _effect_id: &str) -> Result<ReconcileOutcome, ConnectorError> {
        Ok(ReconcileOutcome::Committed { receipt: "reconciled".into() })
    }
}

// -- AlwaysFailedConnector ---------------------------------------------------

pub struct AlwaysFailedConnector;

#[async_trait]
impl Connector for AlwaysFailedConnector {
    fn connector_type(&self) -> &str { "always-failed" }
    fn connector_version(&self) -> &str { "1.0.0" }
    fn supports_compensation(&self) -> bool { false }

    async fn prepare(
        &self, _intent: serde_json::Value, effect_id: String, fence: u64,
    ) -> Result<PreparedEffect, ConnectorError> {
        Ok(PreparedEffect { effect_id, payload: serde_json::json!({}), fence })
    }

    async fn commit(&self, _prepared: PreparedEffect) -> Result<CommitOutcome, ConnectorError> {
        Ok(CommitOutcome::Failed)
    }

    async fn reconcile(&self, _effect_id: &str) -> Result<ReconcileOutcome, ConnectorError> {
        Ok(ReconcileOutcome::NotCommitted)
    }
}

// -- CrashOnCommitConnector --------------------------------------------------

pub struct CrashOnCommitConnector {
    crash_after: AtomicUsize,
    committed_effects: std::sync::Mutex<HashSet<String>>,
}

impl CrashOnCommitConnector {
    pub fn new(crash_after: usize) -> Self {
        Self { crash_after: AtomicUsize::new(crash_after), committed_effects: std::sync::Mutex::new(HashSet::new()) }
    }
}

#[async_trait]
impl Connector for CrashOnCommitConnector {
    fn connector_type(&self) -> &str { "crash-on-commit" }
    fn connector_version(&self) -> &str { "1.0.0" }
    fn supports_compensation(&self) -> bool { true }

    async fn prepare(
        &self, _intent: serde_json::Value, effect_id: String, fence: u64,
    ) -> Result<PreparedEffect, ConnectorError> {
        Ok(PreparedEffect { effect_id, payload: serde_json::json!({"prepared": true}), fence })
    }

    async fn commit(&self, prepared: PreparedEffect) -> Result<CommitOutcome, ConnectorError> {
        let remaining = self.crash_after.fetch_sub(1, Ordering::SeqCst);
        if remaining > 0 {
            self.committed_effects.lock().unwrap().insert(prepared.effect_id.clone());
            Ok(CommitOutcome::Committed { receipt: format!("committed:{}", prepared.effect_id) })
        } else {
            Err(ConnectorError::retryable("connection lost during commit"))
        }
    }

    async fn reconcile(&self, effect_id: &str) -> Result<ReconcileOutcome, ConnectorError> {
        let committed = self.committed_effects.lock().unwrap();
        if committed.contains(effect_id) {
            Ok(ReconcileOutcome::Committed { receipt: format!("reconciled:{}", effect_id) })
        } else {
            Ok(ReconcileOutcome::NotCommitted)
        }
    }
}

// -- TimeoutOnCommitConnector ------------------------------------------------

pub struct TimeoutOnCommitConnector {
    call_count: AtomicUsize,
    timeout_after: usize,
    committed_ids: std::sync::Mutex<HashSet<String>>,
}

impl TimeoutOnCommitConnector {
    pub fn new(timeout_after: usize) -> Self {
        Self { call_count: AtomicUsize::new(0), timeout_after, committed_ids: std::sync::Mutex::new(HashSet::new()) }
    }
}

#[async_trait]
impl Connector for TimeoutOnCommitConnector {
    fn connector_type(&self) -> &str { "timeout-on-commit" }
    fn connector_version(&self) -> &str { "1.0.0" }
    fn supports_compensation(&self) -> bool { false }

    async fn prepare(
        &self, _intent: serde_json::Value, effect_id: String, fence: u64,
    ) -> Result<PreparedEffect, ConnectorError> {
        Ok(PreparedEffect { effect_id, payload: serde_json::json!({}), fence })
    }

    async fn commit(&self, prepared: PreparedEffect) -> Result<CommitOutcome, ConnectorError> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        if count < self.timeout_after {
            self.committed_ids.lock().unwrap().insert(prepared.effect_id.clone());
            Ok(CommitOutcome::Committed { receipt: format!("ok:{}", prepared.effect_id) })
        } else {
            Ok(CommitOutcome::Ambiguous)
        }
    }

    async fn reconcile(&self, effect_id: &str) -> Result<ReconcileOutcome, ConnectorError> {
        let ids = self.committed_ids.lock().unwrap();
        if ids.contains(effect_id) {
            Ok(ReconcileOutcome::Committed { receipt: format!("found:{}", effect_id) })
        } else {
            Ok(ReconcileOutcome::NotCommitted)
        }
    }
}

// -- CompensatingConnector ---------------------------------------------------

pub struct CompensatingConnector {
    pub compensated: AtomicBool,
}

impl CompensatingConnector {
    pub fn new() -> Self {
        Self { compensated: AtomicBool::new(false) }
    }
}

#[async_trait]
impl Connector for CompensatingConnector {
    fn connector_type(&self) -> &str { "compensating" }
    fn connector_version(&self) -> &str { "1.0.0" }
    fn supports_compensation(&self) -> bool { true }

    async fn prepare(
        &self, _intent: serde_json::Value, effect_id: String, fence: u64,
    ) -> Result<PreparedEffect, ConnectorError> {
        Ok(PreparedEffect { effect_id, payload: serde_json::json!({"prepared": true}), fence })
    }

    async fn commit(&self, prepared: PreparedEffect) -> Result<CommitOutcome, ConnectorError> {
        Ok(CommitOutcome::Committed { receipt: format!("committed:{}", prepared.effect_id) })
    }

    async fn reconcile(&self, _effect_id: &str) -> Result<ReconcileOutcome, ConnectorError> {
        Ok(ReconcileOutcome::Committed { receipt: "reconciled".into() })
    }

    async fn compensate(
        &self, _compensation_intent: serde_json::Value, _compensation_effect_id: String, _fence: u64,
    ) -> Result<CommitOutcome, ConnectorError> {
        self.compensated.store(true, Ordering::SeqCst);
        Ok(CommitOutcome::Committed { receipt: "compensated".into() })
    }
}

// -- UnknownStateConnector ---------------------------------------------------

pub struct UnknownStateConnector {
    reconcile_unknown: AtomicBool,
}

impl UnknownStateConnector {
    pub fn new() -> Self {
        Self { reconcile_unknown: AtomicBool::new(true) }
    }
}

#[async_trait]
impl Connector for UnknownStateConnector {
    fn connector_type(&self) -> &str { "unknown-state" }
    fn connector_version(&self) -> &str { "1.0.0" }
    fn supports_compensation(&self) -> bool { false }

    async fn prepare(
        &self, _intent: serde_json::Value, effect_id: String, fence: u64,
    ) -> Result<PreparedEffect, ConnectorError> {
        Ok(PreparedEffect { effect_id, payload: serde_json::json!({}), fence })
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
