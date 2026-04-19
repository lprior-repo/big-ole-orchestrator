//! Ghost workflow lifecycle: Active → Deactivated → Deleted (ADR-021).
//!
//! When a file watcher detects binary deletion, the workflow transitions to
//! `Deactivated`. In-flight instances continue against the pinned version.
//! A background reaper sweeps Deactivated workflows with zero running
//! instances and transitions them to `Deleted` (terminal).

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use vo_types::{BinaryHash, RegistrationStatus, TimestampMs, WorkflowName};

// ─────────────────────────────────────────────────────────────────────────────
// Error Types
// ─────────────────────────────────────────────────────────────────────────────

/// Errors for ghost workflow lifecycle operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GhostWorkflowError {
    /// Transition not allowed from current status.
    #[error("invalid transition: cannot go from {from:?} to {to:?} for workflow {workflow}")]
    InvalidTransition {
        workflow: String,
        from: RegistrationStatus,
        to: RegistrationStatus,
    },

    /// Trigger rejected because workflow is not Active.
    #[error("trigger rejected: workflow {workflow} is {status:?} (HTTP 404)")]
    TriggerRejected {
        workflow: String,
        status: RegistrationStatus,
    },

    /// Cannot reactivate a Deleted workflow.
    #[error("cannot reactivate deleted workflow: {workflow}")]
    CannotReactivateDeleted { workflow: String },

    /// Reaper found a workflow that is not Deactivated.
    #[error("reaper: workflow {workflow} is {status:?}, expected Deactivated")]
    ReaperNotDeactivated {
        workflow: String,
        status: RegistrationStatus,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// WorkflowRegistration — persisted registration record
// ─────────────────────────────────────────────────────────────────────────────

/// Persisted workflow registration with lifecycle metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRegistration {
    name: WorkflowName,
    version_hash: BinaryHash,
    status: RegistrationStatus,
    registered_at: TimestampMs,
    deactivated_at: Option<TimestampMs>,
    running_instance_count: u64,
}

impl WorkflowRegistration {
    /// Create a new Active registration.
    pub fn new(name: WorkflowName, version_hash: BinaryHash, registered_at: TimestampMs) -> Self {
        Self {
            name,
            version_hash,
            status: RegistrationStatus::Active,
            registered_at,
            deactivated_at: None,
            running_instance_count: 0,
        }
    }

    #[must_use]
    pub fn name(&self) -> &WorkflowName {
        &self.name
    }

    #[must_use]
    pub fn version_hash(&self) -> &BinaryHash {
        &self.version_hash
    }

    #[must_use]
    pub fn status(&self) -> RegistrationStatus {
        self.status
    }

    #[must_use]
    pub fn registered_at(&self) -> TimestampMs {
        self.registered_at
    }

    #[must_use]
    pub fn deactivated_at(&self) -> Option<TimestampMs> {
        self.deactivated_at
    }

    #[must_use]
    pub fn running_instance_count(&self) -> u64 {
        self.running_instance_count
    }

    /// Returns true if this workflow accepts new triggers.
    #[must_use]
    pub fn accepts_triggers(&self) -> bool {
        self.status == RegistrationStatus::Active
    }

    /// Increment the running instance count.
    pub fn increment_instances(&mut self) {
        self.running_instance_count = self.running_instance_count.saturating_add(1);
    }

    /// Decrement the running instance count.
    pub fn decrement_instances(&mut self) {
        self.running_instance_count = self.running_instance_count.saturating_sub(1);
    }

    /// Returns true if the reaper can garbage-collect this workflow.
    #[must_use]
    pub fn is_reapable(&self) -> bool {
        self.status == RegistrationStatus::Deactivated && self.running_instance_count == 0
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GhostLifecycle — in-memory lifecycle state machine
// ─────────────────────────────────────────────────────────────────────────────

/// In-memory ghost workflow lifecycle manager.
///
/// Tracks workflow registrations and enforces ADR-021 transitions:
/// - Active → Deactivated (binary deletion)
/// - Deactivated → Deleted (reaper GC when 0 running instances)
/// - Deleted is terminal (no transitions out)
#[derive(Debug, Clone)]
pub struct GhostLifecycle {
    registrations: HashMap<WorkflowName, WorkflowRegistration>,
}

impl GhostLifecycle {
    /// Create an empty lifecycle manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            registrations: HashMap::new(),
        }
    }

    /// Register a new workflow as Active.
    pub fn register(&mut self, registration: WorkflowRegistration) {
        self.registrations
            .insert(registration.name.clone(), registration);
    }

    /// Deactivate a workflow (file watcher detected binary deletion).
    ///
    /// Stops new triggers but allows in-flight instances to complete.
    pub fn deactivate(
        &mut self,
        name: &WorkflowName,
        deactivated_at: TimestampMs,
    ) -> Result<(), GhostWorkflowError> {
        let reg = self.registrations.get_mut(name).ok_or_else(|| {
            GhostWorkflowError::InvalidTransition {
                workflow: name.as_str().to_string(),
                from: RegistrationStatus::Deleted,
                to: RegistrationStatus::Deactivated,
            }
        })?;

        match reg.status {
            RegistrationStatus::Active | RegistrationStatus::Quarantined => {
                reg.status = RegistrationStatus::Deactivated;
                reg.deactivated_at = Some(deactivated_at);
                Ok(())
            }
            current => Err(GhostWorkflowError::InvalidTransition {
                workflow: name.as_str().to_string(),
                from: current,
                to: RegistrationStatus::Deactivated,
            }),
        }
    }

    /// Check if a trigger should be accepted for the given workflow.
    /// Returns Ok(()) if Active, Err with 404 context if not.
    pub fn check_trigger(&self, name: &WorkflowName) -> Result<(), GhostWorkflowError> {
        let reg =
            self.registrations
                .get(name)
                .ok_or_else(|| GhostWorkflowError::TriggerRejected {
                    workflow: name.as_str().to_string(),
                    status: RegistrationStatus::Deleted,
                })?;

        if reg.accepts_triggers() {
            Ok(())
        } else {
            Err(GhostWorkflowError::TriggerRejected {
                workflow: name.as_str().to_string(),
                status: reg.status,
            })
        }
    }

    /// Increment running instance count for a workflow.
    pub fn instance_started(&mut self, name: &WorkflowName) {
        if let Some(reg) = self.registrations.get_mut(name) {
            reg.increment_instances();
        }
    }

    /// Decrement running instance count for a workflow.
    pub fn instance_completed(&mut self, name: &WorkflowName) {
        if let Some(reg) = self.registrations.get_mut(name) {
            reg.decrement_instances();
        }
    }

    /// Run one reaper sweep: reap all Deactivated workflows with 0 running instances.
    ///
    /// Returns the names of workflows that were reaped (transitioned to Deleted).
    pub fn reap(&mut self) -> Vec<WorkflowName> {
        let reaped: Vec<WorkflowName> = self
            .registrations
            .iter()
            .filter(|(_, reg)| reg.is_reapable())
            .map(|(name, _)| name.clone())
            .collect();

        for name in &reaped {
            if let Some(reg) = self.registrations.get_mut(name) {
                reg.status = RegistrationStatus::Deleted;
            }
        }

        reaped
    }

    /// Get a registration by name.
    #[must_use]
    pub fn get(&self, name: &WorkflowName) -> Option<&WorkflowRegistration> {
        self.registrations.get(name)
    }

    /// Get a mutable registration by name.
    pub fn get_mut(&mut self, name: &WorkflowName) -> Option<&mut WorkflowRegistration> {
        self.registrations.get_mut(name)
    }

    /// Returns the number of tracked registrations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.registrations.len()
    }

    /// Returns true if no registrations are tracked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.registrations.is_empty()
    }
}

impl Default for GhostLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ReaperConfig — configuration for the background reaper loop
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for the ghost workflow reaper background task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReaperConfig {
    sweep_interval: Duration,
}

impl ReaperConfig {
    /// Create a new reaper config with the given sweep interval.
    #[must_use]
    pub fn new(sweep_interval: Duration) -> Self {
        Self { sweep_interval }
    }

    #[must_use]
    pub fn sweep_interval(&self) -> Duration {
        self.sweep_interval
    }
}

impl Default for ReaperConfig {
    fn default() -> Self {
        Self {
            sweep_interval: Duration::from_secs(60),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use vo_types::{BinaryHash, TimestampMs, WorkflowName};

    fn make_hash() -> BinaryHash {
        BinaryHash::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .unwrap()
    }

    fn make_name(s: &str) -> WorkflowName {
        WorkflowName::parse(s).unwrap()
    }

    fn make_ts(ms: u64) -> TimestampMs {
        TimestampMs::try_from(ms).unwrap()
    }

    fn make_registration(name: &str) -> WorkflowRegistration {
        WorkflowRegistration::new(make_name(name), make_hash(), make_ts(1000))
    }

    #[test]
    fn new_registration_is_active() {
        let reg = make_registration("test-wf");
        assert_eq!(reg.status(), RegistrationStatus::Active);
        assert!(reg.accepts_triggers());
        assert_eq!(reg.running_instance_count(), 0);
        assert!(reg.deactivated_at().is_none());
    }

    #[test]
    fn deactivate_active_workflow() {
        let mut lc = GhostLifecycle::new();
        let name = make_name("test-wf");
        lc.register(make_registration("test-wf"));

        lc.deactivate(&name, make_ts(2000)).unwrap();

        let reg = lc.get(&name).unwrap();
        assert_eq!(reg.status(), RegistrationStatus::Deactivated);
        assert_eq!(reg.deactivated_at(), Some(make_ts(2000)));
        assert!(!reg.accepts_triggers());
    }

    #[test]
    fn deactivate_quarantined_workflow() {
        let mut lc = GhostLifecycle::new();
        let name = make_name("test-wf");
        let mut reg = make_registration("test-wf");
        reg.status = RegistrationStatus::Quarantined;
        lc.register(reg);

        lc.deactivate(&name, make_ts(2000)).unwrap();

        let reg = lc.get(&name).unwrap();
        assert_eq!(reg.status(), RegistrationStatus::Deactivated);
    }

    #[test]
    fn deactivate_already_deactivated_is_error() {
        let mut lc = GhostLifecycle::new();
        let name = make_name("test-wf");
        let mut reg = make_registration("test-wf");
        reg.status = RegistrationStatus::Deactivated;
        lc.register(reg);

        let result = lc.deactivate(&name, make_ts(3000));
        assert!(result.is_err());
    }

    #[test]
    fn deactivate_deleted_is_error() {
        let mut lc = GhostLifecycle::new();
        let name = make_name("test-wf");
        let mut reg = make_registration("test-wf");
        reg.status = RegistrationStatus::Deleted;
        lc.register(reg);

        let result = lc.deactivate(&name, make_ts(3000));
        assert!(result.is_err());
    }

    // ── Adversarial: trigger on Deactivated workflow → 404 ──

    #[test]
    fn trigger_on_deactivated_returns_404() {
        let mut lc = GhostLifecycle::new();
        let name = make_name("test-wf");
        let mut reg = make_registration("test-wf");
        reg.status = RegistrationStatus::Deactivated;
        lc.register(reg);

        let result = lc.check_trigger(&name);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, GhostWorkflowError::TriggerRejected { .. }));
    }

    #[test]
    fn trigger_on_active_succeeds() {
        let mut lc = GhostLifecycle::new();
        let name = make_name("test-wf");
        lc.register(make_registration("test-wf"));

        assert!(lc.check_trigger(&name).is_ok());
    }

    #[test]
    fn trigger_on_deleted_returns_404() {
        let mut lc = GhostLifecycle::new();
        let name = make_name("test-wf");
        let mut reg = make_registration("test-wf");
        reg.status = RegistrationStatus::Deleted;
        lc.register(reg);

        let result = lc.check_trigger(&name);
        assert!(result.is_err());
    }

    #[test]
    fn trigger_on_unknown_workflow_returns_404() {
        let lc = GhostLifecycle::new();
        let name = make_name("nonexistent");

        let result = lc.check_trigger(&name);
        assert!(result.is_err());
    }

    // ── Reaper tests ──

    #[test]
    fn reap_deactivated_with_zero_instances() {
        let mut lc = GhostLifecycle::new();
        let name = make_name("test-wf");
        let mut reg = make_registration("test-wf");
        reg.status = RegistrationStatus::Deactivated;
        lc.register(reg);

        let reaped = lc.reap();

        assert_eq!(reaped.len(), 1);
        assert_eq!(reaped[0], name);
        assert_eq!(lc.get(&name).unwrap().status(), RegistrationStatus::Deleted);
    }

    #[test]
    fn reap_skips_deactivated_with_running_instances() {
        let mut lc = GhostLifecycle::new();
        let name = make_name("test-wf");
        let mut reg = make_registration("test-wf");
        reg.status = RegistrationStatus::Deactivated;
        reg.running_instance_count = 3;
        lc.register(reg);

        let reaped = lc.reap();

        assert!(reaped.is_empty());
        assert_eq!(
            lc.get(&name).unwrap().status(),
            RegistrationStatus::Deactivated
        );
    }

    #[test]
    fn reap_skips_active_workflows() {
        let mut lc = GhostLifecycle::new();
        lc.register(make_registration("test-wf"));

        let reaped = lc.reap();

        assert!(reaped.is_empty());
        assert_eq!(
            lc.get(&make_name("test-wf")).unwrap().status(),
            RegistrationStatus::Active
        );
    }

    // ── Adversarial: in-flight instance completes after deactivation → reaper cleans up ──

    #[test]
    fn in_flight_completes_then_reaper_cleans_up() {
        let mut lc = GhostLifecycle::new();
        let name = make_name("test-wf");

        let mut reg = make_registration("test-wf");
        reg.increment_instances();
        reg.increment_instances();
        lc.register(reg);

        lc.deactivate(&name, make_ts(2000)).unwrap();
        assert_eq!(lc.get(&name).unwrap().running_instance_count(), 2);

        let reaped = lc.reap();
        assert!(reaped.is_empty());

        lc.instance_completed(&name);
        lc.instance_completed(&name);
        assert_eq!(lc.get(&name).unwrap().running_instance_count(), 0);

        let reaped = lc.reap();
        assert_eq!(reaped.len(), 1);
        assert_eq!(lc.get(&name).unwrap().status(), RegistrationStatus::Deleted);
    }

    #[test]
    fn full_lifecycle_active_deactivate_reap() {
        let mut lc = GhostLifecycle::new();
        let name = make_name("my-workflow");
        lc.register(make_registration("my-workflow"));

        assert!(lc.check_trigger(&name).is_ok());

        lc.instance_started(&name);
        lc.deactivate(&name, make_ts(2000)).unwrap();
        assert!(lc.check_trigger(&name).is_err());

        let reaped = lc.reap();
        assert!(reaped.is_empty());

        lc.instance_completed(&name);
        let reaped = lc.reap();
        assert_eq!(reaped.len(), 1);
        assert_eq!(lc.get(&name).unwrap().status(), RegistrationStatus::Deleted);
    }

    #[test]
    fn reap_multiple_workflows() {
        let mut lc = GhostLifecycle::new();

        let mut reg1 = make_registration("wf-a");
        reg1.status = RegistrationStatus::Deactivated;
        lc.register(reg1);

        let mut reg2 = make_registration("wf-b");
        reg2.status = RegistrationStatus::Deactivated;
        reg2.running_instance_count = 1;
        lc.register(reg2);

        lc.register(make_registration("wf-c"));

        let reaped = lc.reap();
        assert_eq!(reaped.len(), 1);
        assert_eq!(
            lc.get(&make_name("wf-a")).unwrap().status(),
            RegistrationStatus::Deleted
        );
        assert_eq!(
            lc.get(&make_name("wf-b")).unwrap().status(),
            RegistrationStatus::Deactivated
        );
        assert_eq!(
            lc.get(&make_name("wf-c")).unwrap().status(),
            RegistrationStatus::Active
        );
    }

    // ── Instance counting ──

    #[test]
    fn instance_count_saturates_at_zero() {
        let mut reg = make_registration("test-wf");
        assert_eq!(reg.running_instance_count(), 0);
        reg.decrement_instances();
        assert_eq!(reg.running_instance_count(), 0);
    }

    #[test]
    fn instance_count_increments_and_decrements() {
        let mut reg = make_registration("test-wf");
        reg.increment_instances();
        reg.increment_instances();
        assert_eq!(reg.running_instance_count(), 2);
        reg.decrement_instances();
        assert_eq!(reg.running_instance_count(), 1);
    }

    // ── ReaperConfig defaults ──

    #[test]
    fn reaper_config_default_is_60_seconds() {
        let config = ReaperConfig::default();
        assert_eq!(config.sweep_interval(), Duration::from_secs(60));
    }

    // ── Serde roundtrip ──

    #[test]
    fn workflow_registration_serde_roundtrip() {
        let mut reg = make_registration("serde-wf");
        reg.status = RegistrationStatus::Deactivated;
        reg.deactivated_at = Some(make_ts(5000));
        reg.increment_instances();

        let json = serde_json::to_string(&reg).unwrap();
        let parsed: WorkflowRegistration = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, reg);
    }

    #[test]
    fn registration_status_deleted_serde_roundtrip() {
        let status = RegistrationStatus::Deleted;
        let json = serde_json::to_string(&status).unwrap();
        let parsed: RegistrationStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, RegistrationStatus::Deleted);
    }
}
