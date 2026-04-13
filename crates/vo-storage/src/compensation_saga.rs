//! Generic compensation saga model for Managed Effect nodes (ADR-034).
//!
//! This module implements the compensation/saga model for managed effects. It provides:
//!
//! - **Compensation Registration**: Register compensators when effects are committed
//! - **Execution Ordering**: Compensation follows reverse dependency order of committed effects
//! - **Ambiguity Routing**: Route ambiguous compensation outcomes through reconciliation
//! - **Compensation Timeout**: Timeout handling for compensation execution
//!
//! ## Saga States
//!
//! Each compensation goes through:
//! 1. `Registered` - Compensation registered, awaiting execution
//! 2. `Pending` - Queued for execution (Automatic policy)
//! 3. `InProgress` - Compensation executing
//! 4. `Succeeded` - Compensation completed successfully (terminal)
//! 5. `Failed` - Compensation failed (terminal)
//! 6. `Ambiguous` - Outcome unclear, requires reconciliation
//!
//! ## Architecture
//!
//! ```text
//! Workflow commits effect E1 → Compensation C1 registered
//! Workflow commits effect E2 → Compensation C2 registered
//! Workflow fails → Compensations execute in reverse order: C2 → C1
//! ```

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use vo_types::{
    CompensationPolicy, CompensationRecord, CompensationStatus, EffectIntent, TimestampMs,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SagaCompensationStatus {
    Registered,
    Pending,
    InProgress,
    Succeeded,
    Failed,
    Ambiguous,
    TimedOut,
}

impl From<CompensationStatus> for SagaCompensationStatus {
    fn from(status: CompensationStatus) -> Self {
        match status {
            CompensationStatus::NotNeeded => SagaCompensationStatus::Succeeded,
            CompensationStatus::Pending => SagaCompensationStatus::Pending,
            CompensationStatus::InProgress => SagaCompensationStatus::InProgress,
            CompensationStatus::Succeeded => SagaCompensationStatus::Succeeded,
            CompensationStatus::Failed => SagaCompensationStatus::Failed,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompensationEntry {
    pub effect_id: String,
    pub compensation_effect_id: Option<String>,
    pub policy: CompensationPolicy,
    pub status: SagaCompensationStatus,
    pub registered_at: TimestampMs,
    pub started_at: Option<TimestampMs>,
    pub completed_at: Option<TimestampMs>,
    pub timeout_ms: Option<u64>,
    pub dependencies: Vec<String>,
}

impl CompensationEntry {
    pub fn new(effect_id: String, policy: CompensationPolicy, dependencies: Vec<String>) -> Self {
        Self {
            effect_id,
            compensation_effect_id: None,
            policy,
            status: SagaCompensationStatus::Registered,
            registered_at: TimestampMs::now(),
            started_at: None,
            completed_at: None,
            timeout_ms: None,
            dependencies,
        }
    }

    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    pub fn with_compensation_effect_id(mut self, compensation_effect_id: String) -> Self {
        self.compensation_effect_id = Some(compensation_effect_id);
        self
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            SagaCompensationStatus::Succeeded
                | SagaCompensationStatus::Failed
                | SagaCompensationStatus::TimedOut
        )
    }

    pub fn is_timed_out(&self, now: TimestampMs) -> bool {
        if let (Some(started), Some(timeout)) = (self.started_at, self.timeout_ms) {
            let elapsed = now.as_u64().saturating_sub(started.as_u64());
            return elapsed > timeout;
        }
        false
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompensationManifest {
    entries: HashMap<String, CompensationEntry>,
    registration_order: Vec<String>,
    version: u64,
}

impl Default for CompensationManifest {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            registration_order: Vec::new(),
            version: 0,
        }
    }
}

impl CompensationManifest {
    pub fn register(
        &mut self,
        effect_id: String,
        policy: CompensationPolicy,
        dependencies: Vec<String>,
    ) -> Result<(), CompensationError> {
        if self.entries.contains_key(&effect_id) {
            return Err(CompensationError::AlreadyRegistered(effect_id));
        }
        let entry = CompensationEntry::new(effect_id.clone(), policy, dependencies);
        self.entries.insert(effect_id.clone(), entry);
        self.registration_order.push(effect_id);
        self.version += 1;
        Ok(())
    }

    pub fn get(&self, effect_id: &str) -> Option<&CompensationEntry> {
        self.entries.get(effect_id)
    }

    pub fn get_mut(&mut self, effect_id: &str) -> Option<&mut CompensationEntry> {
        self.entries.get_mut(effect_id)
    }

    pub fn transition_to(
        &mut self,
        effect_id: &str,
        new_status: SagaCompensationStatus,
    ) -> Result<(), CompensationError> {
        let entry = self
            .entries
            .get_mut(effect_id)
            .ok_or_else(|| CompensationError::NotFound(effect_id.to_string()))?;

        if entry.is_terminal() {
            return Err(CompensationError::TerminalState {
                effect_id: effect_id.to_string(),
                status: entry.status,
            });
        }

        entry.status = new_status;

        if new_status == SagaCompensationStatus::InProgress {
            entry.started_at = Some(TimestampMs::now());
        }

        if matches!(
            new_status,
            SagaCompensationStatus::Succeeded | SagaCompensationStatus::Failed
        ) {
            entry.completed_at = Some(TimestampMs::now());
        }

        self.version += 1;
        Ok(())
    }

    pub fn set_ambiguous(&mut self, effect_id: &str) -> Result<(), CompensationError> {
        self.transition_to(effect_id, SagaCompensationStatus::Ambiguous)
    }

    pub fn set_timed_out(&mut self, effect_id: &str) -> Result<(), CompensationError> {
        self.transition_to(effect_id, SagaCompensationStatus::TimedOut)
    }

    pub fn pending_compensations(&self) -> impl Iterator<Item = &CompensationEntry> {
        self.entries
            .values()
            .filter(|e| e.status == SagaCompensationStatus::Pending)
    }

    pub fn in_progress_compensations(&self) -> impl Iterator<Item = &CompensationEntry> {
        self.entries
            .values()
            .filter(|e| e.status == SagaCompensationStatus::InProgress)
    }

    pub fn ambiguous_compensations(&self) -> impl Iterator<Item = &CompensationEntry> {
        self.entries
            .values()
            .filter(|e| e.status == SagaCompensationStatus::Ambiguous)
    }

    pub fn timed_out_compensations(&self) -> impl Iterator<Item = &CompensationEntry> {
        self.entries
            .values()
            .filter(|e| e.status == SagaCompensationStatus::TimedOut)
    }

    pub fn compensations_awaiting_execution(&self) -> Vec<&CompensationEntry> {
        self.registration_order
            .iter()
            .rev()
            .filter_map(|id| {
                let entry = self.entries.get(id)?;
                if entry.status == SagaCompensationStatus::Pending {
                    Some(entry)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn all_entries(&self) -> impl Iterator<Item = &CompensationEntry> {
        self.entries.values()
    }

    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    pub fn can_execute(&self, effect_id: &str) -> bool {
        if let Some(entry) = self.entries.get(effect_id) {
            if entry.status != SagaCompensationStatus::Pending {
                return false;
            }
            for dep in &entry.dependencies {
                if let Some(dep_entry) = self.entries.get(dep) {
                    if !dep_entry.is_terminal() {
                        return false;
                    }
                }
            }
            return true;
        }
        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompensationError {
    AlreadyRegistered(String),
    NotFound(String),
    TerminalState {
        effect_id: String,
        status: SagaCompensationStatus,
    },
    InvalidTransition {
        effect_id: String,
        from: SagaCompensationStatus,
        to: SagaCompensationStatus,
    },
    PolicyViolation {
        effect_id: String,
        policy: CompensationPolicy,
    },
    Timeout {
        effect_id: String,
    },
    ReconciliationRequired {
        effect_id: String,
    },
}

impl std::fmt::Display for CompensationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyRegistered(id) => write!(f, "compensation already registered for: {id}"),
            Self::NotFound(id) => write!(f, "compensation not found for: {id}"),
            Self::TerminalState { effect_id, status } => {
                write!(f, "effect {effect_id} is in terminal state: {status:?}")
            }
            Self::InvalidTransition {
                effect_id,
                from,
                to,
            } => {
                write!(f, "invalid transition for {effect_id}: {from:?} -> {to:?}")
            }
            Self::PolicyViolation { effect_id, policy } => {
                write!(f, "policy violation for {effect_id}: {policy:?}")
            }
            Self::Timeout { effect_id } => write!(f, "compensation timeout for: {effect_id}"),
            Self::ReconciliationRequired { effect_id } => {
                write!(f, "reconciliation required for ambiguous: {effect_id}")
            }
        }
    }
}

impl std::error::Error for CompensationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReconciliationAction {
    CommitCompensation,
    RetryCompensation,
    EscalateToOperator,
    AbandonCompensation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationContext {
    pub effect_id: String,
    pub compensation_effect_id: Option<String>,
    pub last_known_outcome: Option<String>,
    pub attempts: u32,
    pub last_attempt_at: Option<TimestampMs>,
}

pub trait CompensationReconciler: Send + Sync {
    fn reconcile(&self, ctx: &ReconciliationContext) -> ReconciliationAction;
}

pub struct NoOpReconciler;

impl CompensationReconciler for NoOpReconciler {
    fn reconcile(&self, _ctx: &ReconciliationContext) -> ReconciliationAction {
        ReconciliationAction::EscalateToOperator
    }
}

pub struct RetryReconciler {
    max_attempts: u32,
}

impl RetryReconciler {
    #[must_use]
    pub fn new(max_attempts: u32) -> Self {
        Self { max_attempts }
    }
}

impl CompensationReconciler for RetryReconciler {
    fn reconcile(&self, ctx: &ReconciliationContext) -> ReconciliationAction {
        if ctx.attempts < self.max_attempts {
            ReconciliationAction::RetryCompensation
        } else {
            ReconciliationAction::EscalateToOperator
        }
    }
}

pub struct CompensationSaga {
    manifest: Arc<Mutex<CompensationManifest>>,
    reconciler: Box<dyn CompensationReconciler>,
}

impl CompensationSaga {
    #[must_use]
    pub fn new() -> Self {
        Self::with_reconciler(NoOpReconciler)
    }

    #[must_use]
    pub fn with_reconciler<R: CompensationReconciler + 'static>(reconciler: R) -> Self {
        Self {
            manifest: Arc::new(Mutex::new(CompensationManifest::default())),
            reconciler: Box::new(reconciler),
        }
    }

    pub fn register(
        &self,
        effect_id: String,
        policy: CompensationPolicy,
        dependencies: Vec<String>,
    ) -> Result<(), CompensationError> {
        #[expect(clippy::unwrap_used)]
        let mut manifest = self.manifest.lock().unwrap();
        manifest.register(effect_id, policy, dependencies)
    }

    pub fn register_with_timeout(
        &self,
        effect_id: String,
        policy: CompensationPolicy,
        dependencies: Vec<String>,
        timeout_ms: u64,
    ) -> Result<(), CompensationError> {
        #[expect(clippy::unwrap_used)]
        let mut manifest = self.manifest.lock().unwrap();
        manifest.register(effect_id.clone(), policy, dependencies)?;
        manifest
            .get_mut(&effect_id)
            .expect("entry just inserted")
            .timeout_ms = Some(timeout_ms);
        Ok(())
    }

    pub fn queue_pending(&self, effect_id: &str) -> Result<(), CompensationError> {
        #[expect(clippy::unwrap_used)]
        let mut manifest = self.manifest.lock().unwrap();
        let entry = manifest
            .get_mut(effect_id)
            .ok_or_else(|| CompensationError::NotFound(effect_id.to_string()))?;

        if entry.policy == CompensationPolicy::None {
            return Err(CompensationError::PolicyViolation {
                effect_id: effect_id.to_string(),
                policy: entry.policy,
            });
        }

        manifest.transition_to(effect_id, SagaCompensationStatus::Pending)
    }

    pub fn start_compensation(&self, effect_id: &str) -> Result<(), CompensationError> {
        #[expect(clippy::unwrap_used)]
        let manifest = self.manifest.lock().unwrap();
        if !manifest.can_execute(effect_id) {
            return Err(CompensationError::PolicyViolation {
                effect_id: effect_id.to_string(),
                policy: manifest
                    .get(effect_id)
                    .map(|e| e.policy)
                    .unwrap_or(CompensationPolicy::None),
            });
        }
        drop(manifest);

        #[expect(clippy::unwrap_used)]
        let mut manifest = self.manifest.lock().unwrap();
        manifest.transition_to(effect_id, SagaCompensationStatus::InProgress)
    }

    pub fn succeed(&self, effect_id: &str) -> Result<(), CompensationError> {
        #[expect(clippy::unwrap_used)]
        let mut manifest = self.manifest.lock().unwrap();
        manifest.transition_to(effect_id, SagaCompensationStatus::Succeeded)
    }

    pub fn fail(&self, effect_id: &str) -> Result<(), CompensationError> {
        #[expect(clippy::unwrap_used)]
        let mut manifest = self.manifest.lock().unwrap();
        manifest.transition_to(effect_id, SagaCompensationStatus::Failed)
    }

    pub fn mark_ambiguous(
        &self,
        effect_id: &str,
    ) -> Result<ReconciliationAction, CompensationError> {
        #[expect(clippy::unwrap_used)]
        let mut manifest = self.manifest.lock().unwrap();
        manifest.set_ambiguous(effect_id)?;

        let entry = manifest.get(effect_id).expect("just set ambiguous");
        let ctx = ReconciliationContext {
            effect_id: effect_id.to_string(),
            compensation_effect_id: entry.compensation_effect_id.clone(),
            last_known_outcome: None,
            attempts: 0,
            last_attempt_at: entry.started_at,
        };

        Ok(self.reconciler.reconcile(&ctx))
    }

    pub fn handle_reconciliation(
        &self,
        effect_id: &str,
        action: ReconciliationAction,
    ) -> Result<(), CompensationError> {
        match action {
            ReconciliationAction::CommitCompensation => {
                #[expect(clippy::unwrap_used)]
                let mut manifest = self.manifest.lock().unwrap();
                manifest.transition_to(effect_id, SagaCompensationStatus::Succeeded)
            }
            ReconciliationAction::RetryCompensation => {
                #[expect(clippy::unwrap_used)]
                let mut manifest = self.manifest.lock().unwrap();
                manifest.transition_to(effect_id, SagaCompensationStatus::Pending)
            }
            ReconciliationAction::EscalateToOperator => {
                #[expect(clippy::unwrap_used)]
                let mut manifest = self.manifest.lock().unwrap();
                manifest.transition_to(effect_id, SagaCompensationStatus::Failed)
            }
            ReconciliationAction::AbandonCompensation => {
                #[expect(clippy::unwrap_used)]
                let mut manifest = self.manifest.lock().unwrap();
                manifest.transition_to(effect_id, SagaCompensationStatus::Failed)
            }
        }
    }

    pub fn check_timeouts(&self) -> Vec<String> {
        #[expect(clippy::unwrap_used)]
        let manifest = self.manifest.lock().unwrap();
        let now = TimestampMs::now();
        let mut timed_out = Vec::new();

        for entry in manifest.in_progress_compensations() {
            if entry.is_timed_out(now) {
                timed_out.push(entry.effect_id.clone());
            }
        }

        timed_out
    }

    pub fn expire_timed_out(&self) -> Result<(), CompensationError> {
        let timed_out = self.check_timeouts();
        #[expect(clippy::unwrap_used)]
        let mut manifest = self.manifest.lock().unwrap();
        for effect_id in timed_out {
            manifest.set_timed_out(&effect_id)?;
        }
        Ok(())
    }

    pub fn get_compensation_order(&self) -> Vec<String> {
        #[expect(clippy::unwrap_used)]
        let manifest = self.manifest.lock().unwrap();
        manifest
            .registration_order
            .iter()
            .rev()
            .filter(|id| {
                manifest
                    .get(*id)
                    .map(|e| e.status == SagaCompensationStatus::Pending)
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    pub fn manifest(&self) -> Arc<Mutex<CompensationManifest>> {
        Arc::clone(&self.manifest)
    }
}

impl Default for CompensationSaga {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_saga() -> CompensationSaga {
        CompensationSaga::new()
    }

    #[test]
    fn register_single_compensation() {
        let saga = create_test_saga();
        saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
            .unwrap();

        let manifest = saga.manifest();
        let guard = manifest.lock().unwrap();
        let entry = guard.get("fx-1").expect("entry exists");
        assert_eq!(entry.status, SagaCompensationStatus::Registered);
        assert_eq!(entry.policy, CompensationPolicy::Automatic);
    }

    #[test]
    fn register_duplicate_fails() {
        let saga = create_test_saga();
        saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
            .unwrap();

        let result = saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![]);
        assert!(matches!(
            result,
            Err(CompensationError::AlreadyRegistered(_))
        ));
    }

    #[test]
    fn queue_pending_transitions_to_pending() {
        let saga = create_test_saga();
        saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
            .unwrap();
        saga.queue_pending("fx-1").unwrap();

        let manifest = saga.manifest();
        let guard = manifest.lock().unwrap();
        let entry = guard.get("fx-1").expect("entry exists");
        assert_eq!(entry.status, SagaCompensationStatus::Pending);
    }

    #[test]
    fn queue_pending_none_policy_fails() {
        let saga = create_test_saga();
        saga.register("fx-1".to_string(), CompensationPolicy::None, vec![])
            .unwrap();

        let result = saga.queue_pending("fx-1");
        assert!(matches!(
            result,
            Err(CompensationError::PolicyViolation { .. })
        ));
    }

    #[test]
    fn compensation_order_is_reverse_registration() {
        let saga = create_test_saga();
        saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
            .unwrap();
        saga.register("fx-2".to_string(), CompensationPolicy::Automatic, vec![])
            .unwrap();
        saga.register("fx-3".to_string(), CompensationPolicy::Automatic, vec![])
            .unwrap();

        saga.queue_pending("fx-1").unwrap();
        saga.queue_pending("fx-2").unwrap();
        saga.queue_pending("fx-3").unwrap();

        let order = saga.get_compensation_order();
        assert_eq!(order, vec!["fx-3", "fx-2", "fx-1"]);
    }

    #[test]
    fn dependencies_block_execution() {
        let saga = create_test_saga();
        saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
            .unwrap();
        saga.register(
            "fx-2".to_string(),
            CompensationPolicy::Automatic,
            vec!["fx-1".to_string()],
        )
        .unwrap();

        saga.queue_pending("fx-1").unwrap();
        saga.queue_pending("fx-2").unwrap();

        let manifest = saga.manifest();
        let guard = manifest.lock().unwrap();

        assert!(guard.can_execute("fx-1"));
        assert!(!guard.can_execute("fx-2"));
    }

    #[test]
    fn dependencies_satisfied_allows_execution() {
        let saga = create_test_saga();
        saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
            .unwrap();
        saga.register(
            "fx-2".to_string(),
            CompensationPolicy::Automatic,
            vec!["fx-1".to_string()],
        )
        .unwrap();

        saga.queue_pending("fx-1").unwrap();
        saga.queue_pending("fx-2").unwrap();

        saga.start_compensation("fx-1").unwrap();
        saga.succeed("fx-1").unwrap();

        let manifest = saga.manifest();
        let guard = manifest.lock().unwrap();
        assert!(guard.can_execute("fx-2"));
    }

    #[test]
    fn start_and_succeed_compensation() {
        let saga = create_test_saga();
        saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
            .unwrap();
        saga.queue_pending("fx-1").unwrap();
        saga.start_compensation("fx-1").unwrap();

        {
            let manifest = saga.manifest();
            let guard = manifest.lock().unwrap();
            let entry = guard.get("fx-1").expect("entry exists");
            assert_eq!(entry.status, SagaCompensationStatus::InProgress);
            assert!(entry.started_at.is_some());
        }

        saga.succeed("fx-1").unwrap();

        {
            let manifest = saga.manifest();
            let guard = manifest.lock().unwrap();
            let entry = guard.get("fx-1").expect("entry exists");
            assert_eq!(entry.status, SagaCompensationStatus::Succeeded);
            assert!(entry.completed_at.is_some());
        }
    }

    #[test]
    fn fail_compensation() {
        let saga = create_test_saga();
        saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
            .unwrap();
        saga.queue_pending("fx-1").unwrap();
        saga.start_compensation("fx-1").unwrap();
        saga.fail("fx-1").unwrap();

        let manifest = saga.manifest();
        let guard = manifest.lock().unwrap();
        let entry = guard.get("fx-1").expect("entry exists");
        assert_eq!(entry.status, SagaCompensationStatus::Failed);
    }

    #[test]
    fn ambiguous_requires_reconciliation() {
        let saga = create_test_saga();
        saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
            .unwrap();
        saga.queue_pending("fx-1").unwrap();
        saga.start_compensation("fx-1").unwrap();

        let action = saga.mark_ambiguous("fx-1").unwrap();

        let manifest = saga.manifest();
        let guard = manifest.lock().unwrap();
        let entry = guard.get("fx-1").expect("entry exists");
        assert_eq!(entry.status, SagaCompensationStatus::Ambiguous);
        drop(guard);

        saga.handle_reconciliation("fx-1", action).unwrap();

        let guard = manifest.lock().unwrap();
        let entry = guard.get("fx-1").expect("entry exists");
        assert_eq!(entry.status, SagaCompensationStatus::Failed);
    }

    #[test]
    fn timeout_detection() {
        let saga = CompensationSaga::with_reconciler(RetryReconciler::new(3));
        saga.register_with_timeout(
            "fx-1".to_string(),
            CompensationPolicy::Automatic,
            vec![],
            100,
        )
        .unwrap();
        saga.queue_pending("fx-1").unwrap();
        saga.start_compensation("fx-1").unwrap();

        std::thread::sleep(std::time::Duration::from_millis(150));

        let timed_out = saga.check_timeouts();
        assert_eq!(timed_out, vec!["fx-1"]);
    }

    #[test]
    fn manual_policy_requires_explicit_approval() {
        let saga = create_test_saga();
        saga.register("fx-1".to_string(), CompensationPolicy::Manual, vec![])
            .unwrap();

        let manifest = saga.manifest();
        let guard = manifest.lock().unwrap();
        let entry = guard.get("fx-1").expect("entry exists");
        assert_eq!(entry.status, SagaCompensationStatus::Registered);
        assert_eq!(entry.policy, CompensationPolicy::Manual);
    }

    #[test]
    fn none_policy_is_not_compensatable() {
        let saga = create_test_saga();
        saga.register("fx-1".to_string(), CompensationPolicy::None, vec![])
            .unwrap();

        let manifest = saga.manifest();
        let guard = manifest.lock().unwrap();
        let entry = guard.get("fx-1").expect("entry exists");
        assert_eq!(entry.status, SagaCompensationStatus::Registered);
        assert_eq!(entry.policy, CompensationPolicy::None);
    }

    #[test]
    fn compensation_with_dependencies_respects_order() {
        let saga = create_test_saga();
        saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
            .unwrap();
        saga.register("fx-2".to_string(), CompensationPolicy::Automatic, vec![])
            .unwrap();
        saga.register(
            "fx-3".to_string(),
            CompensationPolicy::Automatic,
            vec!["fx-1".to_string(), "fx-2".to_string()],
        )
        .unwrap();

        saga.queue_pending("fx-1").unwrap();
        saga.queue_pending("fx-2").unwrap();
        saga.queue_pending("fx-3").unwrap();

        let order = saga.get_compensation_order();
        assert_eq!(order, vec!["fx-3", "fx-2", "fx-1"]);
    }

    #[test]
    fn expire_timed_out_marks_entries() {
        let saga = CompensationSaga::with_reconciler(RetryReconciler::new(1));
        saga.register_with_timeout(
            "fx-1".to_string(),
            CompensationPolicy::Automatic,
            vec![],
            50,
        )
        .unwrap();
        saga.queue_pending("fx-1").unwrap();
        saga.start_compensation("fx-1").unwrap();

        std::thread::sleep(std::time::Duration::from_millis(100));

        saga.expire_timed_out().unwrap();

        let manifest = saga.manifest();
        let guard = manifest.lock().unwrap();
        let entry = guard.get("fx-1").expect("entry exists");
        assert_eq!(entry.status, SagaCompensationStatus::TimedOut);
    }

    #[test]
    fn reconciliation_with_retry_requeues() {
        let saga = CompensationSaga::with_reconciler(RetryReconciler::new(3));
        saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
            .unwrap();
        saga.queue_pending("fx-1").unwrap();
        saga.start_compensation("fx-1").unwrap();
        saga.mark_ambiguous("fx-1").unwrap();

        let action = saga
            .manifest()
            .lock()
            .unwrap()
            .get("fx-1")
            .map(|e| {
                let ctx = ReconciliationContext {
                    effect_id: e.effect_id.clone(),
                    compensation_effect_id: e.compensation_effect_id.clone(),
                    last_known_outcome: None,
                    attempts: 1,
                    last_attempt_at: e.started_at,
                };
                saga.reconciler.reconcile(&ctx)
            })
            .unwrap();

        saga.handle_reconciliation("fx-1", action).unwrap();

        let manifest = saga.manifest();
        let guard = manifest.lock().unwrap();
        let entry = guard.get("fx-1").expect("entry exists");
        assert_eq!(entry.status, SagaCompensationStatus::Pending);
    }

    #[test]
    fn forward_recovery_retry_requeues_ambiguous_compensation() {
        let saga = CompensationSaga::with_reconciler(RetryReconciler::new(3));
        saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
            .unwrap();
        saga.queue_pending("fx-1").unwrap();
        saga.start_compensation("fx-1").unwrap();

        // Simulate ambiguous outcome (e.g., network timeout)
        let action = saga.mark_ambiguous("fx-1").unwrap();
        assert_eq!(action, ReconciliationAction::RetryCompensation);

        // Forward-recovery: retry requeues for re-execution
        saga.handle_reconciliation("fx-1", action).unwrap();

        let manifest = saga.manifest.lock().unwrap();
        let entry = manifest.get("fx-1").expect("entry exists");
        assert_eq!(entry.status, SagaCompensationStatus::Pending);
    }
}
