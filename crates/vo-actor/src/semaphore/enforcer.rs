//! Action Layer — Actor Invariant Enforcement (ADR-015)
//!
//! Combines execution semaphore with instance registry for full invariant enforcement.

use std::sync::Arc;

use vo_types::InstanceId;

use crate::semaphore::execution::ExecutionSemaphore;
use crate::semaphore::types::BackpressureStatus;

/// Errors from actor invariant operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvariantError {
    #[error("Instance already active: {instance_id}")]
    InstanceAlreadyActive { instance_id: InstanceId },
    #[error("Registry error: {reason}")]
    RegistryError { reason: String },
    #[error("Instance not found: {instance_id}")]
    InstanceNotFound { instance_id: InstanceId },
}

/// Result of checking actor invariants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvariantCheck {
    /// Whether the instance is allowed to proceed.
    pub allowed: bool,
    /// Current status of the invariant.
    pub status: BackpressureStatus,
    /// Error if not allowed.
    pub error: Option<InvariantError>,
}

impl InvariantCheck {
    /// Returns true if the check passed.
    #[must_use]
    pub fn is_allowed(&self) -> bool {
        self.allowed
    }
}

/// Trait for Instance Registry Interface.
pub trait InstanceRegistryInterface {
    fn is_active(&self, instance_id: &InstanceId) -> bool;
}

/// Combines execution semaphore with instance registry for full invariant enforcement.
///
/// This provides the complete ADR-015 invariant enforcement:
/// - Single-writer invariant (from instance_registry)
/// - Resource admission control (from execution semaphore)
pub struct InvariantEnforcer<S> {
    execution_semaphore: Arc<ExecutionSemaphore>,
    instance_registry: Arc<S>,
}

impl<S: std::fmt::Debug> std::fmt::Debug for InvariantEnforcer<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InvariantEnforcer")
            .field("execution_semaphore", &self.execution_semaphore)
            .field("instance_registry", &"<instance_registry>")
            .finish()
    }
}

impl<S> InvariantEnforcer<S> {
    /// Creates a new invariant enforcer.
    #[must_use]
    pub fn new(execution_semaphore: Arc<ExecutionSemaphore>, instance_registry: Arc<S>) -> Self {
        Self {
            execution_semaphore,
            instance_registry,
        }
    }
}

impl<S: InstanceRegistryInterface + Send + Sync> InvariantEnforcer<S> {
    /// Checks if an instance can be activated.
    ///
    /// Returns `Ok(InvariantCheck)` with admission details if allowed.
    /// Returns `Err(InvariantError)` if the invariant is violated.
    pub fn check_activation(
        &self,
        instance_id: &InstanceId,
    ) -> Result<InvariantCheck, InvariantError> {
        if self.instance_registry.is_active(instance_id) {
            return Ok(InvariantCheck {
                allowed: false,
                status: BackpressureStatus::Healthy,
                error: Some(InvariantError::InstanceAlreadyActive {
                    instance_id: instance_id.clone(),
                }),
            });
        }

        Ok(InvariantCheck {
            allowed: true,
            status: self.execution_semaphore.current_status(),
            error: None,
        })
    }

    /// Returns the current backpressure status.
    #[must_use]
    pub fn backpressure_status(&self) -> BackpressureStatus {
        self.execution_semaphore.current_status()
    }

    /// Returns the execution semaphore for permit acquisition.
    #[must_use]
    pub fn execution_semaphore(&self) -> &Arc<ExecutionSemaphore> {
        &self.execution_semaphore
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invariant_check_is_allowed_true() {
        let check = InvariantCheck {
            allowed: true,
            status: BackpressureStatus::Healthy,
            error: None,
        };
        assert!(check.is_allowed());
    }

    #[test]
    fn invariant_check_is_allowed_false() {
        let check = InvariantCheck {
            allowed: false,
            status: BackpressureStatus::Heavy,
            error: Some(InvariantError::InstanceAlreadyActive {
                instance_id: InstanceId::from_bytes([0u8; 16]),
            }),
        };
        assert!(!check.is_allowed());
    }
}
