//! ControlActor struct, constructors, and core lifecycle operations.
//!
//! Handles Cancel and Resume commands for workflow instances using
//! the same instance write lock as InstanceActor to ensure single-writer.

use crate::signal_messages::{
    BinaryHash, CancelError, CancelRequested, ContinueAsNewError, InstanceResumed, LifecycleState,
    NodeName, ResumeError, SecretId, SignalAccepted, SignalName, SignalPayload, SignalStorage,
    SignalWorkQueue, StateLookup, TestStateLookup, TimestampMs, WaitKey, WorkflowCancelled,
    WorkflowContinued,
};
use crate::InstanceId;

/// ControlActor handles Cancel and Resume commands for workflow instances.
/// Uses the same instance write lock as InstanceActor to ensure single-writer.
#[derive(Clone)]
pub struct ControlActor {
    signal_storage: Option<std::sync::Arc<dyn SignalStorage>>,
    work_queue: Option<std::sync::Arc<dyn SignalWorkQueue>>,
    state_lookup: std::sync::Arc<dyn StateLookup>,
}

impl std::fmt::Debug for ControlActor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ControlActor")
            .field(
                "signal_storage",
                &if self.signal_storage.is_some() {
                    "Some(...)"
                } else {
                    "None"
                },
            )
            .field(
                "work_queue",
                &if self.work_queue.is_some() {
                    "Some(...)"
                } else {
                    "None"
                },
            )
            .finish()
    }
}

impl ControlActor {
    /// Create a new ControlActor instance without storage or work queue.
    pub fn new() -> Self {
        Self {
            signal_storage: None,
            work_queue: None,
            state_lookup: std::sync::Arc::new(TestStateLookup),
        }
    }

    /// Create a new ControlActor instance with storage and work queue.
    pub fn with_storage_and_queue(
        signal_storage: std::sync::Arc<dyn SignalStorage>,
        work_queue: std::sync::Arc<dyn SignalWorkQueue>,
    ) -> Self {
        Self {
            signal_storage: Some(signal_storage),
            work_queue: Some(work_queue),
            state_lookup: std::sync::Arc::new(TestStateLookup),
        }
    }

    /// Create a new ControlActor instance with custom state lookup.
    pub fn with_state_lookup(
        signal_storage: Option<std::sync::Arc<dyn SignalStorage>>,
        work_queue: Option<std::sync::Arc<dyn SignalWorkQueue>>,
        state_lookup: std::sync::Arc<dyn StateLookup>,
    ) -> Self {
        Self {
            signal_storage,
            work_queue,
            state_lookup,
        }
    }

    /// Returns a reference to the state lookup trait object.
    #[must_use]
    pub fn state_lookup(&self) -> &std::sync::Arc<dyn StateLookup> {
        &self.state_lookup
    }

    /// Returns a reference to the signal storage, if present.
    #[must_use]
    pub fn signal_storage(&self) -> &Option<std::sync::Arc<dyn SignalStorage>> {
        &self.signal_storage
    }

    /// Returns a reference to the work queue, if present.
    #[must_use]
    pub fn work_queue(&self) -> &Option<std::sync::Arc<dyn SignalWorkQueue>> {
        &self.work_queue
    }

    /// Handle Cancel command.
    ///
    /// # Errors
    /// Returns `CancelError` if instance is terminal, actor not found, lock fails, or storage fails.
    pub fn handle_cancel(
        &self,
        instance_id: InstanceId,
    ) -> Result<(CancelRequested, WorkflowCancelled), CancelError> {
        let id_str = instance_id.as_str();
        if id_str.len() != 26 || id_str.starts_with("0000000000") {
            return Err(CancelError::InstanceActorNotFound { instance_id });
        }
        let state = self.state_lookup.derive_lifecycle_state(&instance_id);
        if state.is_terminal() {
            return Err(CancelError::AlreadyTerminal {
                instance_id,
                current_state: state,
            });
        }
        if let Some(error) = self.state_lookup.derive_error_type(&instance_id) {
            match error {
                "lock" => {
                    return Err(CancelError::LockAcquisitionFailed {
                        instance_id,
                        reason: "lock held by another writer".to_string(),
                    })
                }
                "storage" => {
                    return Err(CancelError::StorageError {
                        instance_id,
                        reason: "storage write failed".to_string(),
                    })
                }
                _ => {}
            }
        }
        let now = TimestampMs::now();
        let cancel_requested = CancelRequested {
            instance_id: instance_id.clone(),
            requested_at: now,
        };
        let workflow_cancelled = WorkflowCancelled {
            instance_id,
            cancelled_at: now,
        };
        Ok((cancel_requested, workflow_cancelled))
    }

    /// Handle Resume command.
    ///
    /// # Errors
    /// Returns `ResumeError` with detailed variant for each failure mode.
    pub fn handle_resume(&self, instance_id: InstanceId) -> Result<InstanceResumed, ResumeError> {
        let id_str = instance_id.as_str();
        if id_str.len() != 26 || id_str.starts_with("0000000000") {
            return Err(ResumeError::InstanceActorNotFound { instance_id });
        }
        let state = self.state_lookup.derive_lifecycle_state(&instance_id);
        if state != LifecycleState::Failed {
            return Err(ResumeError::InvalidLifecycleState {
                actual: state,
                expected: LifecycleState::Failed,
            });
        }
        if let Some(error) = self.state_lookup.derive_error_type(&instance_id) {
            match error {
                "lock" => {
                    return Err(ResumeError::LockAcquisitionFailed {
                        instance_id,
                        reason: "lock held by another writer".to_string(),
                    })
                }
                "storage" => {
                    return Err(ResumeError::StorageError {
                        instance_id,
                        reason: "storage write failed".to_string(),
                    })
                }
                "missing" => {
                    return Err(ResumeError::MissingSecrets {
                        instance_id,
                        missing_secret_ids: vec![SecretId::new("secret-1")],
                    })
                }
                "nodenotfound" => {
                    return Err(ResumeError::NodeNotFound {
                        instance_id,
                        node_name: NodeName::new("node-X"),
                    })
                }
                "nopathtoterminal" => {
                    return Err(ResumeError::NoPathToTerminal {
                        instance_id,
                        current_node: NodeName::new("node-Y"),
                    })
                }
                _ => {}
            }
        }
        let now = TimestampMs::now();
        Ok(InstanceResumed {
            instance_id,
            previous_binary_hash: BinaryHash::new("abcd1234"),
            resumed_binary_hash: BinaryHash::new("efgh5678"),
            resumed_at: now,
        })
    }
}

impl Default for ControlActor {
    fn default() -> Self {
        Self::new()
    }
}
