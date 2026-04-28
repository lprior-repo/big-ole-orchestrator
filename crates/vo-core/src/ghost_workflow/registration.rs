//! WorkflowRegistration — persisted registration record

use serde::{Deserialize, Serialize};
use vo_types::{BinaryHash, RegistrationStatus, TimestampMs, WorkflowName};

use crate::ghost_workflow::GhostWorkflowError;

/// Domain event emitted when a workflow is reaped (Deactivated → Deleted).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowReaped {
    pub workflow: WorkflowName,
    pub version_hash: BinaryHash,
}

/// Persisted workflow registration with lifecycle metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRegistration {
    pub(crate) name: WorkflowName,
    pub(crate) version_hash: BinaryHash,
    pub(crate) status: RegistrationStatus,
    pub(crate) registered_at: TimestampMs,
    pub(crate) deactivated_at: Option<TimestampMs>,
    pub(crate) running_instance_count: u64,
}

impl WorkflowRegistration {
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

    #[must_use]
    pub fn accepts_triggers(&self) -> bool {
        self.status == RegistrationStatus::Active
    }

    pub fn increment_instances(&mut self) {
        self.running_instance_count = self.running_instance_count.saturating_add(1);
    }

    pub fn decrement_instances(&mut self) {
        self.running_instance_count = self.running_instance_count.saturating_sub(1);
    }

    #[must_use]
    pub fn is_reapable(&self) -> bool {
        self.status == RegistrationStatus::Deactivated && self.running_instance_count == 0
    }

    /// Transition to Deleted with validation.
    ///
    /// Only allowed from `Deactivated` with zero running instances.
    /// Returns the domain event on success.
    pub fn transition_to_deleted(&mut self) -> Result<WorkflowReaped, GhostWorkflowError> {
        match self.status {
            RegistrationStatus::Deactivated if self.running_instance_count == 0 => {
                self.status = RegistrationStatus::Deleted;
                Ok(WorkflowReaped {
                    workflow: self.name.clone(),
                    version_hash: self.version_hash.clone(),
                })
            }
            RegistrationStatus::Deactivated => Err(GhostWorkflowError::ReaperNotDeactivated {
                workflow: self.name.as_str().to_string(),
                status: RegistrationStatus::Deactivated,
            }),
            current => Err(GhostWorkflowError::InvalidTransition {
                workflow: self.name.as_str().to_string(),
                from: current,
                to: RegistrationStatus::Deleted,
            }),
        }
    }

    pub(crate) fn set_status(&mut self, status: RegistrationStatus) {
        self.status = status;
    }

    pub(crate) fn set_deactivated_at(&mut self, deactivated_at: TimestampMs) {
        self.deactivated_at = Some(deactivated_at);
    }
}

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
