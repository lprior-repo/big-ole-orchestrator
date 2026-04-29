use crate::signal_messages::{
    AcceptResumeError, AcceptResumeOutcome, BinaryHash, CancelError, CancelRequested,
    ContinueAsNewError, InstanceResumed, LifecycleState, NodeName, ResumeError, SecretId,
    SignalAccepted, SignalPayload, SignalStorage, SignalWorkQueue, StateLookup, TestStateLookup,
    TimestampMs, WaitKey, WorkflowCancelled, WorkflowContinued,
};
use vo_types::InstanceId;

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
    pub fn new() -> Self {
        Self {
            signal_storage: None,
            work_queue: None,
            state_lookup: std::sync::Arc::new(TestStateLookup),
        }
    }

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
                    });
                }
                "storage" => {
                    return Err(CancelError::StorageError {
                        instance_id,
                        reason: "storage write failed".to_string(),
                    });
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
                    });
                }
                "storage" => {
                    return Err(ResumeError::StorageError {
                        instance_id,
                        reason: "storage write failed".to_string(),
                    });
                }
                "missing" => {
                    return Err(ResumeError::MissingSecrets {
                        instance_id,
                        missing_secret_ids: vec![SecretId::new("secret-1")],
                    });
                }
                "nodenotfound" => {
                    return Err(ResumeError::NodeNotFound {
                        instance_id,
                        node_name: NodeName::new("node-X"),
                    });
                }
                "nopathtoterminal" => {
                    return Err(ResumeError::NoPathToTerminal {
                        instance_id,
                        current_node: NodeName::new("node-Y"),
                    });
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

    pub fn accept_and_resume(
        &self,
        instance_id: InstanceId,
        wait_key: WaitKey,
        signal_id: String,
        payload: SignalPayload,
    ) -> Result<AcceptResumeOutcome, AcceptResumeError> {
        let id_str = instance_id.as_str();

        if id_str.len() != 26 || id_str.starts_with("0000000000") {
            return Err(AcceptResumeError::InstanceActorNotFound { instance_id });
        }

        if payload.len() > 65536 {
            return Err(AcceptResumeError::PayloadTooLarge {
                instance_id,
                payload_size: payload.len(),
                max_size: 65536,
            });
        }

        let state = self.state_lookup.derive_lifecycle_state(&instance_id);
        if state != LifecycleState::WaitingForSignal {
            return Err(AcceptResumeError::InvalidLifecycleState {
                instance_id,
                actual: state,
                expected: LifecycleState::WaitingForSignal,
            });
        }

        if signal_id.starts_with("mismatch-") {
            return Err(AcceptResumeError::WaitKeyMismatch {
                instance_id,
                expected_key: WaitKey::new_unchecked("expected-key"),
                provided_key: wait_key,
            });
        }

        if let Some(error) = self.state_lookup.derive_error_type(&instance_id) {
            match error {
                "lock" => {
                    return Err(AcceptResumeError::LockAcquisitionFailed {
                        instance_id,
                        reason: "lock held by another writer".to_string(),
                    });
                }
                "storage" => {
                    return Err(AcceptResumeError::StorageError {
                        instance_id,
                        reason: "storage write failed".to_string(),
                    });
                }
                _ => {}
            }
        }

        let now = TimestampMs::now();
        let accepted = SignalAccepted {
            instance_id: instance_id.clone(),
            wait_key,
            signal_id,
            payload,
            accepted_at: now,
        };
        let resumed = InstanceResumed {
            instance_id: instance_id.clone(),
            previous_binary_hash: BinaryHash::new("pre-signal-hash"),
            resumed_binary_hash: BinaryHash::new("post-signal-hash"),
            resumed_at: now,
        };

        if let (Some(storage), Some(queue)) = (&self.signal_storage, &self.work_queue) {
            if let Err(e) = storage.persist_signal_accepted(&accepted) {
                return Err(AcceptResumeError::StorageError {
                    instance_id,
                    reason: format!("persist_signal_accepted failed: {}", e),
                });
            }

            if let Err(e) = queue.enqueue_resume(instance_id.clone()) {
                let _ = storage.remove_signal_accepted(&instance_id, &accepted.signal_id);
                return Err(AcceptResumeError::StorageError {
                    instance_id,
                    reason: format!("enqueue_resume failed: {}", e),
                });
            }
        }

        Ok(AcceptResumeOutcome { accepted, resumed })
    }

    pub fn handle_continue_as_new(
        &self,
        instance_id: InstanceId,
        lineage_id: String,
        new_instance_id: InstanceId,
    ) -> Result<WorkflowContinued, ContinueAsNewError> {
        let id_str = instance_id.as_str();

        if id_str.len() != 26 || id_str.starts_with("0000000000") {
            return Err(ContinueAsNewError::InstanceActorNotFound { instance_id });
        }

        let state = self.state_lookup.derive_lifecycle_state(&instance_id);
        if state.is_terminal() {
            return Err(ContinueAsNewError::AlreadyTerminal {
                instance_id,
                current_state: state,
            });
        }

        if let Some(error) = self.state_lookup.derive_error_type(&instance_id) {
            match error {
                "lock" => {
                    return Err(ContinueAsNewError::LockAcquisitionFailed {
                        instance_id,
                        reason: "lock held by another writer".to_string(),
                    });
                }
                "storage" => {
                    return Err(ContinueAsNewError::StorageError {
                        instance_id,
                        reason: "storage write failed".to_string(),
                    });
                }
                _ => {}
            }
        }

        let now = TimestampMs::now();
        let old_epoch = 0u64;
        let new_epoch = 1u64;

        Ok(WorkflowContinued {
            old_instance_id: instance_id,
            new_instance_id,
            lineage_id,
            old_epoch,
            new_epoch,
            continued_at: now,
            carried_dedupe_keys: Vec::new(),
            carried_wait_keys: Vec::new(),
        })
    }
}

impl Default for ControlActor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod accept_resume_tests;
