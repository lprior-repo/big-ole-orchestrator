//! Actor framework for vo-engine.
//!
//! Provides the actor model implementation using the Ractor library.
//! Actors are the fundamental units of computation in the engine.

use bytes::Bytes;
pub use vo_common::NamespaceId;
use vo_types::InstanceId;

pub mod heartbeat;

pub mod master {
    pub struct MasterOrchestrator;
    pub struct OrchestratorConfig;
}

pub mod actor_messages;
pub mod async_message_router;
pub mod fairness;
pub mod instance;
pub mod instance_registry;
pub mod lifecycle;
pub mod master;
pub mod message_router;
pub mod orchestrator_msg;
pub mod port;
pub mod probe;
pub mod reanimator;
pub mod routing;
pub mod semaphore;
pub mod signal_buffer;
pub mod signal_messages;
pub mod signals;
pub mod spawn_supervisor;
pub mod instance_actor_message;
pub mod control_actor_message;
pub mod control_actor;

#[cfg(test)]
pub mod signal_buffer_tests;

#[cfg(test)]
pub mod instance_registry_tests;
pub mod timer_lifecycle;
pub mod timer_supervisor;
pub mod timer_supervisor_tests;
pub mod timers;

pub use master::{MasterOrchestrator, OrchestratorConfig};

pub use orchestrator_msg::{
    CompensateError, InstancePhaseView, InstanceSnapshot, NamespaceId, OrchestratorMsg,
    ReservedPermitBudget, SignalError, StartError, TerminateError, WorkflowParadigm,
};
pub use fairness::WorkloadClass;

pub use signal_messages::mock_signal_storage;
pub use signal_messages::mock_signal_storage::{MockSignalStorage, MockSignalWorkQueue};
pub use signal_messages::{
    AcceptResumeError, AcceptResumeOutcome, BinaryHash, CancelError, CancelRequested,
    ContinueAsNewError, InstanceResumed, LifecycleState, NodeName, ResumeError, RolloverState,
    SecretId, SignalAccepted, SignalName, SignalPayload, SignalStorage, SignalStorageError,
    SignalWorkQueue, SignalWorkQueueError, StateLookup, TestStateLookup, TimestampMs, WaitKey,
    WorkflowCancelled, WorkflowContinued,
};

/// Messages sent to the orchestrator actor.
#[derive(Debug)]
pub enum OrchestratorMsg {
    /// Start a new workflow instance
    StartWorkflow {
        namespace: NamespaceId,
        instance_id: InstanceId,
        workflow_type: String,
        paradigm: WorkflowParadigm,
        input: Bytes,
        reply: ractor::port::RpcReplyPort<Result<(), crate::StartError>>,
    },
    /// Get status of a workflow instance
    GetStatus {
        instance_id: InstanceId,
        reply: ractor::port::RpcReplyPort<Option<crate::InstanceSnapshot>>,
    },
    /// Terminate a workflow instance
    Terminate {
        instance_id: InstanceId,
        reason: String,
        reply: ractor::port::RpcReplyPort<Result<(), TerminateError>>,
    },
    /// List all active workflow instances
    ListActive {
        reply: ractor::port::RpcReplyPort<Vec<crate::InstanceSnapshot>>,
    },
    /// Trigger compensation for a workflow instance
    Compensate {
        instance_id: InstanceId,
        reply: ractor::port::RpcReplyPort<Result<(), CompensateError>>,
    },
    /// Send a signal to a workflow instance
    Signal {
        instance_id: InstanceId,
        signal_name: String,
        payload: Bytes,
        reply: ractor::port::RpcReplyPort<Result<(), SignalError>>,
    },
}

/// Error type for compensation operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompensateError {
    #[error("instance not found: {0}")]
    NotFound(String),
    #[error("compensation failed: {0}")]
    Failed(String),
}

/// Error type for signal operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SignalError {
    #[error("instance not found: {0}")]
    NotFound(String),
    #[error("signal failed: {0}")]
    Failed(String),
}

/// Instance snapshot for status queries.
#[derive(Debug, Clone)]
pub struct InstanceSnapshot {
    pub instance_id: InstanceId,
    pub namespace: NamespaceId,
    pub workflow_type: String,
    pub paradigm: WorkflowParadigm,
    pub phase: InstancePhaseView,
    pub events_applied: u64,
}

#[cfg(test)]
mod signal_error_tests {
    use super::*;

    #[test]
    fn signal_error_variants_can_be_constructed() {
        let err = SignalError::NotFound("inst-1".to_string());
        assert!(matches!(err, SignalError::NotFound(msg) if msg == "inst-1"));

        let err = SignalError::Failed("timeout".to_string());
        assert!(matches!(err, SignalError::Failed(msg) if msg == "timeout"));
    }

    #[test]
    fn orchestrator_msg_signal_variant_exists() {
        fn _check(_msg: OrchestratorMsg) {
            if let OrchestratorMsg::Signal {
                namespace: _,
                instance_id: _,
                signal_name: _,
                payload: _,
                reply: _,
            } = _msg
            {}
        }
    }
}

#[cfg(test)]
mod terminate_error_tests {
    use super::*;

    #[test]
    fn terminate_error_variants_can_be_constructed() {
        let err_not_found = TerminateError::NotFound("wf-123".to_string());
        assert!(matches!(err_not_found, TerminateError::NotFound(msg) if msg == "wf-123"));

        let err_failed = TerminateError::Failed("crashed".to_string());
        assert!(matches!(err_failed, TerminateError::Failed(msg) if msg == "crashed"));
    }
}

// Actor message types
pub mod actor_messages;
pub mod signal_messages;

pub use signal_messages::mock_signal_storage;
pub use signal_messages::mock_signal_storage::{MockSignalStorage, MockSignalWorkQueue};
pub use signal_messages::{
    AcceptResumeError, AcceptResumeOutcome, BinaryHash, CancelError, CancelRequested,
    ContinueAsNewError, InstanceResumed, LifecycleState, NodeName, ResumeError, RolloverState,
    SecretId, SignalAccepted, SignalPayload, SignalStorage, SignalStorageError, SignalWorkQueue,
    SignalWorkQueueError, StateLookup, TestStateLookup, TimestampMs, WaitKey, WorkflowCancelled,
    WorkflowContinued,
};

// =============================================================================
// Workload Classes and Reserved Permit Budget (ADR-033)
// =============================================================================

pub use fairness::WorkloadClass;

/// Errors from actor start operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StartError {
    #[error("Budget exhausted for {class:?}: requested {requested}, available {available}")]
    BudgetExhaustion {
        class: WorkloadClass,
        requested: u32,
        available: u32,
    },
    #[error("Invalid config: {0}")]
    InvalidConfig(String),
    #[error("At capacity: {running}/{max} instances running")]
    AtCapacity { running: u32, max: u32 },
    #[error("Instance {0} already exists")]
    AlreadyExists(String),
    #[error("Spawn failed: {0}")]
    SpawnFailed(String),
}

/// Reserved permit budget tracking per workload class.
/// Ensures each class maintains its reserved capacity per ADR-033.
#[derive(Debug, Clone)]
pub struct ReservedPermitBudget {
    max_per_class: u32,
    class_counts: std::collections::HashMap<WorkloadClass, u32>,
}

impl ReservedPermitBudget {
    /// Creates a new budget with the specified maximum per class.
    ///
    /// # Panics
    /// Panics if `max_per_class` is zero.
    #[must_use]
    pub fn new(max_per_class: u32) -> Self {
        assert!(max_per_class > 0, "max_per_class must be > 0");
        Self {
            max_per_class,
            class_counts: std::collections::HashMap::new(),
        }
    }

    /// Attempts to acquire a permit for the given class.
    ///
    /// # Errors
    /// Returns `StartError::BudgetExhaustion` if no permits available.
    pub fn try_acquire(&mut self, class: WorkloadClass) -> Result<(), StartError> {
        let current = self.class_counts.get(&class).copied().unwrap_or(0);
        if current >= self.max_per_class {
            return Err(StartError::BudgetExhaustion {
                class,
                requested: 1,
                available: self.max_per_class - current,
            });
        }
        *self.class_counts.entry(class).or_insert(0) += 1;
        Ok(())
    }

    /// Releases a permit for the given class.
    /// If count is already zero, this is a no-op.
    pub fn release(&mut self, class: WorkloadClass) {
        let count = self.class_counts.get(&class).copied().unwrap_or(0);
        if count == 0 {
            return;
        }
        self.class_counts.insert(class, count - 1);
    }

    /// Resets all class counts to zero.
    pub fn reset(&mut self) {
        self.class_counts.clear();
    }

    /// Returns the number of available permits for the given class.
    #[must_use]
    pub fn available(&self, class: WorkloadClass) -> u32 {
        let used = self.class_counts.get(&class).copied().unwrap_or(0);
        self.max_per_class.saturating_sub(used)
    }

    /// Returns true if the given class has no available permits.
    #[must_use]
    pub fn is_exhausted(&self, class: WorkloadClass) -> bool {
        self.available(class) == 0
    }
}

#[cfg(test)]
mod reserved_permit_budget_tests {
    use super::*;

    mod workload_class_tests {
        use super::*;

        #[test]
        fn workload_class_variants_exist() {
            assert!(matches!(WorkloadClass::Recovery, WorkloadClass::Recovery));
            assert!(matches!(
                WorkloadClass::NewInstance,
                WorkloadClass::NewInstance
            ));
            assert!(matches!(WorkloadClass::Internal, WorkloadClass::Internal));
        }

        #[test]
        fn workload_class_debug_format() {
            assert_eq!(format!("{:?}", WorkloadClass::Recovery), "Recovery");
            assert_eq!(format!("{:?}", WorkloadClass::NewInstance), "NewInstance");
            assert_eq!(format!("{:?}", WorkloadClass::Internal), "Internal");
        }

        #[test]
        fn workload_class_eq() {
            assert_eq!(WorkloadClass::Recovery, WorkloadClass::Recovery);
            assert_eq!(WorkloadClass::NewInstance, WorkloadClass::NewInstance);
            assert_eq!(WorkloadClass::Internal, WorkloadClass::Internal);
            assert_ne!(WorkloadClass::Recovery, WorkloadClass::NewInstance);
        }

        #[test]
        fn workload_class_clone() {
            let a = WorkloadClass::Recovery;
            let b = a;
            assert_eq!(a, b);
        }

        #[test]
        fn workload_class_copy() {
            let a = WorkloadClass::Recovery;
            let b = a;
            assert_eq!(a, b);
        }
    }

    mod start_error_tests {
        use super::*;

        #[test]
        fn budget_exhaustion_contains_fields() {
            let err = StartError::BudgetExhaustion {
                class: WorkloadClass::Recovery,
                requested: 1,
                available: 0,
            };
            assert!(matches!(err, StartError::BudgetExhaustion { .. }));
        }

        #[test]
        fn budget_exhaustion_display() {
            let err = StartError::BudgetExhaustion {
                class: WorkloadClass::Recovery,
                requested: 1,
                available: 0,
            };
            let display = format!("{}", err);
            assert!(display.contains("Recovery"));
            assert!(display.contains("requested"));
            assert!(display.contains("available"));
        }

        #[test]
        fn invalid_config_display() {
            let err = StartError::InvalidConfig("test error".to_string());
            let display = format!("{}", err);
            assert!(display.contains("Invalid config"));
            assert!(display.contains("test error"));
        }

        #[test]
        fn budget_exhaustion_partial_eq() {
            let err1 = StartError::BudgetExhaustion {
                class: WorkloadClass::Recovery,
                requested: 1,
                available: 0,
            };
            let err2 = StartError::BudgetExhaustion {
                class: WorkloadClass::Recovery,
                requested: 1,
                available: 0,
            };
            assert_eq!(err1, err2);
        }

        #[test]
        fn budget_exhaustion_different_classes_not_equal() {
            let err1 = StartError::BudgetExhaustion {
                class: WorkloadClass::Recovery,
                requested: 1,
                available: 0,
            };
            let err2 = StartError::BudgetExhaustion {
                class: WorkloadClass::NewInstance,
                requested: 1,
                available: 0,
            };
            assert_ne!(err1, err2);
        }
    }

    mod reserved_permit_budget_tests {
        use super::*;

        #[test]
        fn budget_creation() {
            let budget = ReservedPermitBudget::new(5);
            assert_eq!(budget.available(WorkloadClass::Recovery), 5);
            assert_eq!(budget.available(WorkloadClass::NewInstance), 5);
            assert_eq!(budget.available(WorkloadClass::Internal), 5);
        }

        #[test]
        fn budget_acquire_decrements_available() {
            let mut budget = ReservedPermitBudget::new(5);
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            assert_eq!(budget.available(WorkloadClass::Recovery), 4);
        }

        #[test]
        fn budget_acquire_multiple() {
            let mut budget = ReservedPermitBudget::new(5);
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            assert_eq!(budget.available(WorkloadClass::Recovery), 3);
        }

        #[test]
        fn budget_acquire_returns_err_when_exhausted() {
            let mut budget = ReservedPermitBudget::new(2);
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            let result = budget.try_acquire(WorkloadClass::Recovery);
            assert!(matches!(
                result,
                Err(StartError::BudgetExhaustion {
                    class: WorkloadClass::Recovery,
                    requested: 1,
                    available: 0,
                })
            ));
        }

        #[test]
        fn budget_release_increments_available() {
            let mut budget = ReservedPermitBudget::new(5);
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.release(WorkloadClass::Recovery);
            assert_eq!(budget.available(WorkloadClass::Recovery), 4);
        }

        #[test]
        fn budget_release_on_zero_is_noop() {
            let mut budget = ReservedPermitBudget::new(5);
            budget.release(WorkloadClass::Recovery);
            assert_eq!(budget.available(WorkloadClass::Recovery), 5);
        }

        #[test]
        fn budget_reset_clears_counts() {
            let mut budget = ReservedPermitBudget::new(5);
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::NewInstance).unwrap();
            budget.reset();
            assert_eq!(budget.available(WorkloadClass::Recovery), 5);
            assert_eq!(budget.available(WorkloadClass::NewInstance), 5);
        }

        #[test]
        fn budget_is_exhausted_false_when_available() {
            let budget = ReservedPermitBudget::new(5);
            assert!(!budget.is_exhausted(WorkloadClass::Recovery));
        }

        #[test]
        fn budget_is_exhausted_true_when_empty() {
            let mut budget = ReservedPermitBudget::new(2);
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            assert!(budget.is_exhausted(WorkloadClass::Recovery));
        }

        #[test]
        fn budget_classes_are_independent() {
            let mut budget = ReservedPermitBudget::new(3);
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            assert!(budget.try_acquire(WorkloadClass::Internal).is_ok());
            assert_eq!(budget.available(WorkloadClass::Internal), 2);
        }

        #[test]
        fn budget_exhaustion_error_contains_class_and_available() {
            let mut budget = ReservedPermitBudget::new(1);
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            let result = budget.try_acquire(WorkloadClass::Recovery);
            match result {
                Err(StartError::BudgetExhaustion {
                    class,
                    requested: _,
                    available,
                }) => {
                    assert_eq!(class, WorkloadClass::Recovery);
                    assert_eq!(available, 0);
                }
                _ => panic!("Expected BudgetExhaustion error"),
            }
        }
    }
}

#[cfg(test)]
mod control_actor_tests {
    use super::*;

    #[tokio::test]
    async fn test_cancel_succeeds_for_non_terminal_instance() {
        let actor = ControlActor::new();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00R000").unwrap();

        let result = actor.handle_cancel(instance_id.clone());

        assert!(result.is_ok(), "Cancel should succeed for non-terminal instance");
        let (cancel_requested, workflow_cancelled) = result.unwrap();
        assert_eq!(cancel_requested.instance_id, instance_id);
        assert_eq!(workflow_cancelled.instance_id, instance_id);
    }

    #[tokio::test]
    async fn test_cancel_fails_for_terminal_instance() {
        let actor = ControlActor::new();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00CXXX").unwrap();

        let result = actor.handle_cancel(instance_id.clone());

        assert!(result.is_err(), "Cancel should fail for terminal instance");
        let err = result.unwrap_err();
        assert!(
            matches!(err, CancelError::AlreadyTerminal { .. }),
            "Expected AlreadyTerminal error, got {:?}",
            err
        );
    }

    #[tokio::test]
    async fn test_cancel_fails_for_nonexistent_instance() {
        let actor = ControlActor::new();
        let instance_id = InstanceId::parse("0000000000XXXXXXXXXXXXXXXX").unwrap();

        let result = actor.handle_cancel(instance_id.clone());

        assert!(result.is_err(), "Cancel should fail for non-existent instance");
        let err = result.unwrap_err();
        assert!(
            matches!(err, CancelError::InstanceActorNotFound { .. }),
            "Expected InstanceActorNotFound error, got {:?}",
            err
        );
    }

    #[tokio::test]
    async fn test_resume_succeeds_for_failed_instance() {
        let actor = ControlActor::new();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00F000").unwrap();

        let result = actor.handle_resume(instance_id.clone());

        assert!(result.is_ok(), "Resume should succeed for Failed instance");
        let resumed = result.unwrap();
        assert_eq!(resumed.instance_id, instance_id);
    }

    #[tokio::test]
    async fn test_resume_fails_for_non_failed_instance() {
        let actor = ControlActor::new();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00C000").unwrap();

        let result = actor.handle_resume(instance_id.clone());

        assert!(result.is_err(), "Resume should fail for non-Failed instance");
        let err = result.unwrap_err();
        assert!(
            matches!(err, ResumeError::InvalidLifecycleState { .. }),
            "Expected InvalidLifecycleState error, got {:?}",
            err
        );
    }

    #[tokio::test]
    async fn test_resume_fails_for_nonexistent_instance() {
        let actor = ControlActor::new();
        let instance_id = InstanceId::parse("0000000000XXXXXXXXXXXXXXXX").unwrap();

        let result = actor.handle_resume(instance_id.clone());

        assert!(result.is_err(), "Resume should fail for non-existent instance");
        let err = result.unwrap_err();
        assert!(
            matches!(err, ResumeError::InstanceActorNotFound { .. }),
            "Expected InstanceActorNotFound error, got {:?}",
            err
        );
    }
}

#[cfg(test)]
mod accept_resume_tests {
    use super::*;

    #[test]
    fn waitkey_parse_succeeds_for_valid_input() {
        let key = WaitKey::parse("approval-v2").unwrap();
        assert_eq!(key.as_str(), "approval-v2");
    }

    #[test]
    fn waitkey_parse_rejects_empty_string() {
        let result = WaitKey::parse("");
        assert_eq!(result, Err("WaitKey cannot be empty".to_string()));
    }

    #[test]
    fn waitkey_parse_rejects_over_256_chars() {
        let long_key = "a".repeat(257);
        let result = WaitKey::parse(&long_key);
        assert_eq!(
            result,
            Err(format!(
                "WaitKey exceeds 256 characters: {}",
                long_key.len()
            ))
        );
    }

    #[test]
    fn waitkey_new_unchecked_bypasses_validation() {
        let key = WaitKey::new_unchecked("");
        assert_eq!(key.as_str(), "");
    }

    #[test]
    fn signal_payload_from_bytes_succeeds_for_valid_payload() {
        let payload = SignalPayload::from_bytes(vec![1, 2, 3]).unwrap();
        assert_eq!(payload.as_bytes(), &[1, 2, 3]);
    }

    #[test]
    fn signal_payload_from_bytes_rejects_over_64kib() {
        let big = vec![0u8; 65537];
        let result = SignalPayload::from_bytes(big);
        assert_eq!(
            result,
            Err("SignalPayload exceeds 64 KiB: 65537 bytes".to_string())
        );
    }

    #[test]
    fn signal_payload_empty_creates_zero_length_payload() {
        let payload = SignalPayload::empty();
        assert!(payload.is_empty());
        assert_eq!(payload.len(), 0);
    }

    #[test]
    fn signal_payload_len_and_is_empty_are_correct() {
        let payload = SignalPayload::from_bytes(vec![42]).unwrap();
        assert!(!payload.is_empty());
        assert_eq!(payload.len(), 1);
    }

    #[test]
    fn waiting_for_signal_is_not_terminal() {
        assert!(!LifecycleState::WaitingForSignal.is_terminal());
    }

    #[test]
    fn lifecycle_state_all_variants_is_terminal_correctness() {
        assert!(!LifecycleState::Running.is_terminal());
        assert!(!LifecycleState::Failed.is_terminal());
        assert!(LifecycleState::Completed.is_terminal());
        assert!(LifecycleState::Cancelled.is_terminal());
        assert!(!LifecycleState::WaitingForSignal.is_terminal());
    }

    #[test]
    fn accept_resume_error_precondition_variants_are_correct() {
        let iid = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000000").unwrap();
        let err = AcceptResumeError::InstanceActorNotFound { instance_id: iid.clone() };
        assert!(matches!(err, AcceptResumeError::InstanceActorNotFound { .. }));
    }

    #[test]
    fn accept_resume_error_invalid_lifecycle_state_format() {
        let iid = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000000").unwrap();
        let err = AcceptResumeError::InvalidLifecycleState {
            instance_id: iid,
            actual: LifecycleState::Running,
            expected: LifecycleState::WaitingForSignal,
        };
        let display = format!("{}", err);
        assert!(display.contains("Running") && display.contains("WaitingForSignal"));
    }

    #[tokio::test]
    async fn accept_and_resume_succeeds_for_waiting_instance() {
        let actor = ControlActor::new();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
        let wait_key = WaitKey::parse("approval-v2").unwrap();
        let payload = SignalPayload::empty();

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "sig-1".to_string(),
            payload,
        );

        assert!(result.is_ok(), "accept_and_resume should succeed for WaitingForSignal instance");
        let outcome = result.unwrap();
        assert_eq!(outcome.accepted.instance_id, instance_id);
        assert_eq!(outcome.resumed.instance_id, instance_id);
    }

    #[tokio::test]
    async fn accept_and_resume_fails_for_non_waiting_instance() {
        let actor = ControlActor::new();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00C000").unwrap();
        let wait_key = WaitKey::parse("approval-v2").unwrap();
        let payload = SignalPayload::empty();

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "sig-1".to_string(),
            payload,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, AcceptResumeError::InvalidLifecycleState { .. }),
            "Expected InvalidLifecycleState error, got {:?}",
            err
        );
    }

    #[tokio::test]
    async fn accept_and_resume_fails_for_nonexistent_instance() {
        let actor = ControlActor::new();
        let instance_id = InstanceId::parse("0000000000XXXXXXXXXXXXXXXX").unwrap();
        let wait_key = WaitKey::parse("approval-v2").unwrap();
        let payload = SignalPayload::empty();

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "sig-1".to_string(),
            payload,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, AcceptResumeError::InstanceActorNotFound { .. }),
            "Expected InstanceActorNotFound error, got {:?}",
            err
        );
    }

    #[tokio::test]
    async fn accept_and_resume_fails_for_waitkey_mismatch() {
        let actor = ControlActor::new();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
        let wait_key = WaitKey::parse("approval-v2").unwrap();
        let payload = SignalPayload::empty();

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "mismatch-sig-1".to_string(),
            payload,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, AcceptResumeError::WaitKeyMismatch { .. }),
            "Expected WaitKeyMismatch error, got {:?}",
            err
        );
    }

    #[tokio::test]
    async fn accept_and_resume_fails_for_payload_too_large() {
        let actor = ControlActor::new();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
        let wait_key = WaitKey::parse("approval-v2").unwrap();
        let big_payload = vec![0u8; 65537];
        let payload = SignalPayload::new_unchecked(big_payload);

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "sig-1".to_string(),
            payload,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, AcceptResumeError::PayloadTooLarge { .. }),
            "Expected PayloadTooLarge error, got {:?}",
            err
        );
    }

    #[tokio::test]
    async fn test_transition_fails_gracefully_if_workflow_is_in_a_terminal_state() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00C000").unwrap();
        let actor = ControlActor::new();
        let wait_key = WaitKey::parse("approval-v2").unwrap();
        let payload = SignalPayload::empty();

        let result =
            actor.accept_and_resume(instance_id.clone(), wait_key, "sig-1".to_string(), payload);

        assert!(
            result.is_err(),
            "accept_and_resume should fail when workflow is in terminal state"
        );
        let err = result.unwrap_err();
        assert!(
            matches!(err, AcceptResumeError::InvalidLifecycleState { .. }),
            "Expected InvalidLifecycleState error, got {:?}",
            err
        );
    }

    #[tokio::test]
    async fn test_transition_fails_gracefully_if_workflow_is_in_a_terminal_state_duplicate_for_sch()
    {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00X000").unwrap();
        let actor = ControlActor::new();
        let wait_key = WaitKey::parse("approval-v2").unwrap();
        let payload = SignalPayload::empty();

        let result =
            actor.accept_and_resume(instance_id.clone(), wait_key, "sig-2".to_string(), payload);

        assert!(
            result.is_err(),
            "accept_and_resume should fail when workflow is in terminal state"
        );
        let err = result.unwrap_err();
        assert!(
            matches!(err, AcceptResumeError::InvalidLifecycleState { .. }),
            "Expected InvalidLifecycleState error for Cancelled state, got {:?}",
            err
        );
    }
}

pub use actor_messages::{ControlActorMessage, InstanceActorMessage};
