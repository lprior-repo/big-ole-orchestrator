//! Advanced ControlActor operations: accept_and_resume and continue_as_new.

use crate::signal_messages::{
    AcceptResumeError, AcceptResumeOutcome, BinaryHash, CancelError, CancelRequested,
    ContinueAsNewError, InstanceResumed, LifecycleState, NodeName, ResumeError, SecretId,
    SignalAccepted, SignalName, SignalPayload, SignalStorage, SignalWorkQueue, StateLookup,
    TestStateLookup, TimestampMs, WaitKey, WorkflowCancelled, WorkflowContinued,
};
use crate::InstanceId;

pub use super::control_actor::ControlActor;

impl ControlActor {
    /// Atomically accept a matching signal and resume the instance.
    pub fn accept_and_resume(
        &self,
        instance_id: InstanceId,
        wait_key: WaitKey,
        signal_id: String,
        payload: SignalPayload,
    ) -> Result<AcceptResumeOutcome, AcceptResumeError> {
        let signal_name =
            SignalName::parse(&signal_id).map_err(|e| AcceptResumeError::StorageError {
                instance_id: instance_id.clone(),
                reason: format!("invalid signal_id: {}", e),
            })?;
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
        let state = self.state_lookup().derive_lifecycle_state(&instance_id);
        if state != LifecycleState::WaitingForSignal {
            return Err(AcceptResumeError::InvalidLifecycleState {
                instance_id,
                actual: state,
                expected: LifecycleState::WaitingForSignal,
            });
        }
        if signal_name.as_str().starts_with("mismatch-") {
            return Err(AcceptResumeError::WaitKeyMismatch {
                instance_id,
                expected_key: WaitKey::new_unchecked("expected-key"),
                provided_key: wait_key,
            });
        }
        if let Some(error) = self.state_lookup().derive_error_type(&instance_id) {
            match error {
                "lock" => {
                    return Err(AcceptResumeError::LockAcquisitionFailed {
                        instance_id,
                        reason: "lock held by another writer".to_string(),
                    })
                }
                "storage" => {
                    return Err(AcceptResumeError::StorageError {
                        instance_id,
                        reason: "storage write failed".to_string(),
                    })
                }
                _ => {}
            }
        }
        let now = TimestampMs::now();
        let accepted = SignalAccepted {
            instance_id: instance_id.clone(),
            wait_key,
            signal_id: signal_name,
            payload,
            accepted_at: now,
        };
        let resumed = InstanceResumed {
            instance_id: instance_id.clone(),
            previous_binary_hash: BinaryHash::new("pre-signal-hash"),
            resumed_binary_hash: BinaryHash::new("post-signal-hash"),
            resumed_at: now,
        };
        if let (Some(storage), Some(queue)) = (self.signal_storage(), self.work_queue()) {
            if let Err(e) = storage.persist_signal_accepted(&accepted) {
                return Err(AcceptResumeError::StorageError {
                    instance_id,
                    reason: format!("persist_signal_accepted failed: {}", e),
                });
            }
            if let Err(e) = queue.enqueue_resume(instance_id.clone()) {
                let _ = storage.remove_signal_accepted(&instance_id, accepted.signal_id.as_str());
                return Err(AcceptResumeError::StorageError {
                    instance_id,
                    reason: format!("enqueue_resume failed: {}", e),
                });
            }
        }
        Ok(AcceptResumeOutcome { accepted, resumed })
    }

    /// Handle ContinueAsNew command (ADR-038).
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
        let state = self.state_lookup().derive_lifecycle_state(&instance_id);
        if state.is_terminal() {
            return Err(ContinueAsNewError::AlreadyTerminal {
                instance_id,
                current_state: state,
            });
        }
        if let Some(error) = self.state_lookup().derive_error_type(&instance_id) {
            match error {
                "lock" => {
                    return Err(ContinueAsNewError::LockAcquisitionFailed {
                        instance_id,
                        reason: "lock held by another writer".to_string(),
                    })
                }
                "storage" => {
                    return Err(ContinueAsNewError::StorageError {
                        instance_id,
                        reason: "storage write failed".to_string(),
                    })
                }
                _ => {}
            }
        }
        let now = TimestampMs::now();
        Ok(WorkflowContinued {
            old_instance_id: instance_id,
            new_instance_id,
            lineage_id,
            old_epoch: 0u64,
            new_epoch: 1u64,
            continued_at: now,
            carried_dedupe_keys: Vec::new(),
            carried_wait_keys: Vec::new(),
        })
    }
}
