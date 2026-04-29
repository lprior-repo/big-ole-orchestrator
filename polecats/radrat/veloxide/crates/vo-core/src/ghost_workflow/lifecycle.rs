//! GhostLifecycle — in-memory lifecycle state machine

use std::collections::HashMap;

use vo_types::{RegistrationStatus, TimestampMs, WorkflowName};

use super::{GhostWorkflowError, WorkflowRegistration};

#[derive(Debug, Clone)]
pub struct GhostLifecycle {
    registrations: HashMap<WorkflowName, WorkflowRegistration>,
}

impl GhostLifecycle {
    #[must_use]
    pub fn new() -> Self {
        Self {
            registrations: HashMap::new(),
        }
    }

    pub fn register(&mut self, registration: WorkflowRegistration) {
        self.registrations
            .insert(registration.name().clone(), registration);
    }

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

        match reg.status() {
            RegistrationStatus::Active | RegistrationStatus::Quarantined => {
                reg.set_status(RegistrationStatus::Deactivated);
                reg.set_deactivated_at(deactivated_at);
                Ok(())
            }
            current => Err(GhostWorkflowError::InvalidTransition {
                workflow: name.as_str().to_string(),
                from: current,
                to: RegistrationStatus::Deactivated,
            }),
        }
    }

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
                status: reg.status(),
            })
        }
    }

    pub fn instance_started(&mut self, name: &WorkflowName) {
        if let Some(reg) = self.registrations.get_mut(name) {
            reg.increment_instances();
        }
    }

    pub fn instance_completed(&mut self, name: &WorkflowName) {
        if let Some(reg) = self.registrations.get_mut(name) {
            reg.decrement_instances();
        }
    }

    pub fn reap(&mut self) -> Vec<WorkflowName> {
        let reaped: Vec<WorkflowName> = self
            .registrations
            .iter()
            .filter(|(_, reg)| reg.is_reapable())
            .map(|(name, _)| name.clone())
            .collect();

        for name in &reaped {
            if let Some(reg) = self.registrations.get_mut(name) {
                reg.set_status(RegistrationStatus::Deleted);
            }
        }

        reaped
    }

    #[must_use]
    pub fn get(&self, name: &WorkflowName) -> Option<&WorkflowRegistration> {
        self.registrations.get(name)
    }

    pub fn get_mut(&mut self, name: &WorkflowName) -> Option<&mut WorkflowRegistration> {
        self.registrations.get_mut(name)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.registrations.len()
    }

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
