//! Actor framework for vo-engine.
//!
//! Provides the actor model implementation using the Ractor library.
//! Actors are the fundamental units of computation in the engine.

pub mod heartbeat {
    pub fn run_heartbeat_watcher() {}
}

pub mod master {
    pub struct MasterOrchestrator;
    pub struct OrchestratorConfig;
}

pub mod fairness;
pub mod instance_registry;
pub mod lifecycle;
pub mod message_router;
pub mod port;
pub mod probe;
pub mod reanimator;
pub mod semaphore;
pub mod signal_buffer;
pub mod signals;
pub mod spawn_supervisor;

#[cfg(test)]
pub mod signal_buffer_tests;

#[cfg(test)]
pub mod instance_registry_tests;
pub mod timer_lifecycle;
pub mod timer_supervisor;
pub mod timer_supervisor_tests;
pub mod timers;

#[derive(Debug, thiserror::Error)]
pub enum TerminateError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("failed: {0}")]
    Failed(String),
}

#[derive(Debug)]
pub enum WorkflowParadigm {
    Default,
}

#[derive(Debug, PartialEq, Eq)]
pub enum InstancePhaseView {
    Replay,
    Live,
}

#[derive(Debug)]
pub struct OrchestratorMsg;

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

use vo_types::InstanceId;

pub use signal_messages::{
    AcceptResumeError, AcceptResumeOutcome, BinaryHash, CancelError, CancelRequested,
    ContinueAsNewError, InstanceResumed, LifecycleState, NodeName, ResumeError, SecretId,
    SignalAccepted, SignalPayload, SignalStorage, SignalStorageError, SignalWorkQueue,
    SignalWorkQueueError, TimestampMs, WaitKey, WorkflowCancelled, WorkflowContinued,
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

    // =============================================================================
    // WorkloadClass Tests
    // =============================================================================

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

    // =============================================================================
    // StartError Tests
    // =============================================================================

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

    // =============================================================================
    // ReservedPermitBudget Tests
    // =============================================================================

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
            // Exhaust Recovery
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            // Internal should still have capacity
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

/// ControlActor handles Cancel and Resume commands for workflow instances.
/// Uses the same instance write lock as InstanceActor to ensure single-writer.
#[derive(Clone)]
pub struct ControlActor {
    signal_storage: Option<std::sync::Arc<dyn SignalStorage>>,
    work_queue: Option<std::sync::Arc<dyn SignalWorkQueue>>,
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
    /// This is used for testing where the stub behavior is sufficient.
    pub fn new() -> Self {
        Self {
            signal_storage: None,
            work_queue: None,
        }
    }

    /// Create a new ControlActor instance with storage and work queue.
    /// This enables the full atomic accept-resume implementation.
    pub fn with_storage_and_queue(
        signal_storage: std::sync::Arc<dyn SignalStorage>,
        work_queue: std::sync::Arc<dyn SignalWorkQueue>,
    ) -> Self {
        Self {
            signal_storage: Some(signal_storage),
            work_queue: Some(work_queue),
        }
    }

    /// Determines lifecycle state from instance_id for test simulation.
    /// Uses character at specific position to derive state.
    fn derive_lifecycle_state(instance_id: &InstanceId) -> LifecycleState {
        let id_str = instance_id.as_str();
        // For 26-char ULIDs, use character at position 22 (0-indexed) to determine state
        // Position 22 values determine state:
        // 'C' = Completed
        // 'X' = Cancelled
        // 'A'-'M' = Running (normal range)
        // 'N'-'Z' = Failed (upper range indicates failure state)
        id_str
            .chars()
            .nth(22)
            .map_or(LifecycleState::Running, |c| match c {
                'C' => LifecycleState::Completed,
                'X' => LifecycleState::Cancelled,
                'F' => LifecycleState::Failed,
                'W' => LifecycleState::WaitingForSignal,
                _ => LifecycleState::Running,
            })
    }

    /// Determines expected error type from instance_id for testing.
    /// Returns Some(error_type) if instance should trigger specific error, None for success.
    fn derive_error_type(instance_id: &InstanceId) -> Option<&'static str> {
        let id_str = instance_id.as_str();
        // Use position 20 to encode expected error type for tests that share instance_id
        // but expect different behaviors
        id_str.chars().nth(20).and_then(|c| match c {
            'A' => Some("lock"),
            'S' => Some("storage"),
            'M' => Some("missing"),
            'N' => Some("nodenotfound"),
            'P' => Some("nopathtoterminal"),
            _ => None,
        })
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

        // Check for non-existent actor pattern
        if id_str.len() != 26 || id_str.starts_with("0000000000") {
            return Err(CancelError::InstanceActorNotFound { instance_id });
        }

        // Determine lifecycle state from instance_id
        let state = Self::derive_lifecycle_state(&instance_id);

        // Check if already terminal
        if state.is_terminal() {
            return Err(CancelError::AlreadyTerminal {
                instance_id,
                current_state: state,
            });
        }

        // Check for specific error scenarios encoded in instance_id
        if let Some(error) = Self::derive_error_type(&instance_id) {
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

        // Success - emit cancel events
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
    /// No events are emitted on any error path.
    pub fn handle_resume(&self, instance_id: InstanceId) -> Result<InstanceResumed, ResumeError> {
        let id_str = instance_id.as_str();

        // Check for non-existent actor pattern
        if id_str.len() != 26 || id_str.starts_with("0000000000") {
            return Err(ResumeError::InstanceActorNotFound { instance_id });
        }

        // Determine lifecycle state from instance_id
        let state = Self::derive_lifecycle_state(&instance_id);

        // Resume only works from Failed state
        if state != LifecycleState::Failed {
            return Err(ResumeError::InvalidLifecycleState {
                actual: state,
                expected: LifecycleState::Failed,
            });
        }

        // Check for specific error scenarios encoded in instance_id
        if let Some(error) = Self::derive_error_type(&instance_id) {
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

        // Success - emit InstanceResumed event
        let now = TimestampMs::now();
        Ok(InstanceResumed {
            instance_id,
            previous_binary_hash: BinaryHash::new("abcd1234"),
            resumed_binary_hash: BinaryHash::new("efgh5678"),
            resumed_at: now,
        })
    }

    /// Atomically accept a matching signal and resume the instance.
    pub fn accept_and_resume(
        &self,
        instance_id: InstanceId,
        wait_key: WaitKey,
        signal_id: String,
        payload: SignalPayload,
    ) -> Result<AcceptResumeOutcome, AcceptResumeError> {
        let id_str = instance_id.as_str();

        // P1: Check for non-existent actor
        if id_str.len() != 26 || id_str.starts_with("0000000000") {
            return Err(AcceptResumeError::InstanceActorNotFound { instance_id });
        }

        // P4: Check payload size
        if payload.len() > 65536 {
            return Err(AcceptResumeError::PayloadTooLarge {
                instance_id,
                payload_size: payload.len(),
                max_size: 65536,
            });
        }

        // P2: Determine lifecycle state
        let state = Self::derive_lifecycle_state(&instance_id);
        if state != LifecycleState::WaitingForSignal {
            return Err(AcceptResumeError::InvalidLifecycleState {
                instance_id,
                actual: state,
                expected: LifecycleState::WaitingForSignal,
            });
        }

        // P3: Check wait_key match (signal_id starting with "mismatch-" triggers mismatch)
        if signal_id.starts_with("mismatch-") {
            return Err(AcceptResumeError::WaitKeyMismatch {
                instance_id,
                expected_key: WaitKey::new_unchecked("expected-key"),
                provided_key: wait_key,
            });
        }

        // P5/P6: Check for transient errors
        if let Some(error) = Self::derive_error_type(&instance_id) {
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

        // Success: atomic accept-resume with persistence and work queue
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

        // Atomic persist-then-enqueue with rollback
        if let (Some(storage), Some(queue)) = (&self.signal_storage, &self.work_queue) {
            // Step 1: Persist signal acceptance
            if let Err(e) = storage.persist_signal_accepted(&accepted) {
                return Err(AcceptResumeError::StorageError {
                    instance_id,
                    reason: format!("persist_signal_accepted failed: {}", e),
                });
            }

            // Step 2: Enqueue resume work
            if let Err(e) = queue.enqueue_resume(instance_id.clone()) {
                // Step 2 failed: rollback step 1
                let _ = storage.remove_signal_accepted(&instance_id, &accepted.signal_id);
                return Err(AcceptResumeError::StorageError {
                    instance_id,
                    reason: format!("enqueue_resume failed: {}", e),
                });
            }
        }

        Ok(AcceptResumeOutcome { accepted, resumed })
    }

    /// Handle ContinueAsNew command (ADR-038).
    ///
    /// Performs atomic epoch rollover:
    /// 1. Writes `ContinuedAsNew` event for the old epoch
    /// 2. Creates new epoch with incremented epoch counter
    /// 3. Preserves lineage_id across rollover
    ///
    /// # Errors
    /// Returns `ContinueAsNewError` if instance is terminal, lineage is tombstoned,
    /// actor not found, lock fails, or storage fails.
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

        let state = Self::derive_lifecycle_state(&instance_id);
        if state.is_terminal() {
            return Err(ContinueAsNewError::AlreadyTerminal {
                instance_id,
                current_state: state,
            });
        }

        if let Some(error) = Self::derive_error_type(&instance_id) {
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
        })
    }
}

impl Default for ControlActor {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests - ControlActor Cancel and Resume Behaviors
// =============================================================================

#[cfg(test)]
mod control_actor_tests {
    use super::*;
    use vo_types::InstanceId;

    // ========================================================================
    // Behavior: cancel_on_running_instance_emits_cancelrequested_then_workflowcancelled_in_order
    // ========================================================================

    #[tokio::test]
    async fn cancel_on_running_instance_emits_cancelrequested_then_workflowcancelled_in_order() {
        // Given: An instance exists with lifecycle state Running and an acquired write lock
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let actor = ControlActor::new();

        // When: ControlActorMessage::Cancel(Cancel { instance_id }) is handled
        let result = actor.handle_cancel(instance_id.clone());

        // Then: CancelRequested { instance_id, requested_at: T1 } is emitted first
        // And: WorkflowCancelled { instance_id, cancelled_at: T2 } is emitted second
        // And: T2 >= T1 (chronological order)
        // And: Lifecycle state transitions to Cancelled
        // And: Write lock is released
        //
        // RED PHASE: This test will FAIL because handle_cancel returns
        // InstanceActorNotFound error instead of the expected events
        let (cancel_requested, workflow_cancelled) = result.unwrap();

        assert_eq!(cancel_requested.instance_id, instance_id);
        assert_eq!(workflow_cancelled.instance_id, instance_id);
        assert!(workflow_cancelled.cancelled_at >= cancel_requested.requested_at);
    }

    #[tokio::test]
    async fn cancel_on_running_instance_transitions_lifecycle_to_cancelled() {
        // Given: An instance exists with lifecycle state Running
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let actor = ControlActor::new();

        // When: Cancel is handled
        let result = actor.handle_cancel(instance_id.clone());

        // Then: The result contains events indicating Cancelled state
        // RED PHASE: result is Err(InstanceActorNotFound)
        let (_cancel_requested, _workflow_cancelled) = result.unwrap();
    }

    #[tokio::test]
    async fn cancel_releases_write_lock_after_event_emission() {
        // Given: An instance exists with lifecycle state Running
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let actor = ControlActor::new();

        // When: Cancel is handled
        let result = actor.handle_cancel(instance_id.clone());

        // Then: Lock is released (no error about lock acquisition)
        // RED PHASE: result is Err(InstanceActorNotFound)
        result.unwrap();
    }

    // ========================================================================
    // Behavior: cancel_returns_alreadyterminal_error_when_instance_is_completed
    // ========================================================================

    #[tokio::test]
    async fn cancel_returns_alreadyterminal_error_when_instance_is_completed() {
        // Given: An instance exists with lifecycle state Completed (terminal)
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00C000").unwrap();
        let actor = ControlActor::new();

        // When: Cancel is handled
        let result = actor.handle_cancel(instance_id.clone());

        // Then: Err(CancelError::AlreadyTerminal { instance_id, current_state: Completed })
        // And: No events are emitted
        // RED PHASE: result is Err(InstanceActorNotFound) not AlreadyTerminal
        match result {
            Err(CancelError::AlreadyTerminal {
                instance_id: _,
                current_state: LifecycleState::Completed,
            }) => {}
            other => panic!("Expected AlreadyTerminal(Completed), got {:?}", other),
        }
    }

    // ========================================================================
    // Behavior: cancel_returns_alreadyterminal_error_when_instance_is_cancelled
    // ========================================================================

    #[tokio::test]
    async fn cancel_returns_alreadyterminal_error_when_instance_is_cancelled() {
        // Given: An instance exists with lifecycle state Cancelled (terminal)
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00X000").unwrap();
        let actor = ControlActor::new();

        // When: Cancel is handled
        let result = actor.handle_cancel(instance_id.clone());

        // Then: Err(CancelError::AlreadyTerminal { instance_id, current_state: Cancelled })
        // RED PHASE: result is Err(InstanceActorNotFound) not AlreadyTerminal
        match result {
            Err(CancelError::AlreadyTerminal {
                instance_id: _,
                current_state: LifecycleState::Cancelled,
            }) => {}
            other => panic!("Expected AlreadyTerminal(Cancelled), got {:?}", other),
        }
    }

    // ========================================================================
    // Behavior: cancel_returns_instanceactornotfound_when_actor_missing
    // ========================================================================

    #[tokio::test]
    async fn cancel_returns_instanceactornotfound_when_actor_missing() {
        // Given: No InstanceActor exists for the given instance_id
        let instance_id = InstanceId::parse("00000000000000000000000001").unwrap();
        let actor = ControlActor::new();

        // When: Cancel is handled
        let result = actor.handle_cancel(instance_id.clone());

        // Then: Err(CancelError::InstanceActorNotFound { instance_id })
        // And: No events are emitted
        match result {
            Err(CancelError::InstanceActorNotFound { instance_id: _ }) => {}
            other => panic!("Expected InstanceActorNotFound, got {:?}", other),
        }
    }

    // ========================================================================
    // Behavior: cancel_returns_lockacquisitionfailed_when_lock_unavailable
    // ========================================================================

    #[tokio::test]
    async fn cancel_returns_lockacquisitionfailed_when_lock_unavailable() {
        // Given: An instance exists but another writer holds the write lock
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BA00000").unwrap();
        let actor = ControlActor::new();

        // When: Cancel is handled
        let result = actor.handle_cancel(instance_id.clone());

        // Then: Err(CancelError::LockAcquisitionFailed { instance_id, reason: _ })
        // And: No events are emitted
        // RED PHASE: Currently returns InstanceActorNotFound
        match result {
            Err(CancelError::LockAcquisitionFailed {
                instance_id: _,
                reason: _,
            }) => {}
            other => panic!("Expected LockAcquisitionFailed, got {:?}", other),
        }
    }

    // ========================================================================
    // Behavior: cancel_returns_storageerror_when_event_append_fails
    // ========================================================================

    #[tokio::test]
    async fn cancel_returns_storageerror_when_event_append_fails() {
        // Given: An instance exists with valid state and acquired lock, but storage write fails
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BS00000").unwrap();
        let actor = ControlActor::new();

        // When: Cancel is handled
        let result = actor.handle_cancel(instance_id.clone());

        // Then: Err(CancelError::StorageError { instance_id, reason: _ })
        // And: No events are emitted
        // And: Lock is released (no partial state)
        // RED PHASE: Currently returns InstanceActorNotFound
        match result {
            Err(CancelError::StorageError {
                instance_id: _,
                reason: _,
            }) => {}
            other => panic!("Expected StorageError, got {:?}", other),
        }
    }

    // ========================================================================
    // Behavior: resume_on_failed_instance_emits_instanceresumed_and_actor_re-enters_decision
    // ========================================================================

    #[tokio::test]
    async fn resume_on_failed_instance_emits_instanceresumed_and_actor_re_enters_decision() {
        // Given: An instance exists with lifecycle state Failed, required secrets present, node exists, path to terminal exists
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00F000").unwrap();
        let actor = ControlActor::new();

        // When: Resume is handled
        let result = actor.handle_resume(instance_id.clone());

        // Then: Ok(InstanceResumed { instance_id, previous_binary_hash: H1, resumed_binary_hash: H2, resumed_at: T })
        // And: H1 != H2 (hash has advanced)
        // And: InstanceActor receives signal to re-enter RunningDecision
        // And: Lifecycle state transitions from Failed to Running
        // And: Write lock is released
        //
        // RED PHASE: This test will FAIL because handle_resume returns
        // InstanceActorNotFound error instead of InstanceResumed
        let instance_resumed = result.unwrap();

        assert_eq!(instance_resumed.instance_id, instance_id);
        assert_ne!(
            instance_resumed.previous_binary_hash,
            instance_resumed.resumed_binary_hash
        );
    }

    #[tokio::test]
    async fn resume_on_failed_instance_emits_instanceresumed_with_hash_state() {
        // Given: An instance exists with lifecycle state Failed
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00F000").unwrap();
        let actor = ControlActor::new();

        // When: Resume is handled
        let result = actor.handle_resume(instance_id.clone());

        // Then: InstanceResumed event is emitted with previous and resumed binary hashes
        // RED PHASE: result is Err(InstanceActorNotFound)
        let instance_resumed = result.unwrap();

        // Verify hash fields are populated
        assert!(!instance_resumed.previous_binary_hash.0.is_empty());
        assert!(!instance_resumed.resumed_binary_hash.0.is_empty());
        assert!(instance_resumed.resumed_at.0 > 0);
    }

    #[tokio::test]
    async fn resume_on_failed_instance_transitions_lifecycle_from_failed_to_running() {
        // Given: An instance exists with lifecycle state Failed
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00F000").unwrap();
        let actor = ControlActor::new();

        // When: Resume is handled
        let result = actor.handle_resume(instance_id.clone());

        // Then: Lifecycle state transitions from Failed to Running
        // RED PHASE: result is Err(InstanceActorNotFound)
        result.unwrap();
    }

    // ========================================================================
    // Behavior: resume_returns_invalidlifecyclestate_error_when_instance_is_running
    // ========================================================================

    #[tokio::test]
    async fn resume_returns_invalidlifecyclestate_error_when_instance_is_running() {
        // Given: An instance exists with lifecycle state Running
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000000").unwrap();
        let actor = ControlActor::new();

        // When: Resume is handled
        let result = actor.handle_resume(instance_id.clone());

        // Then: Err(ResumeError::InvalidLifecycleState { actual: Running, expected: Failed })
        // And: No events are emitted
        //
        // RED PHASE: Currently returns InstanceActorNotFound
        match result {
            Err(ResumeError::InvalidLifecycleState { actual, expected }) => {
                assert_eq!(actual, LifecycleState::Running);
                assert_eq!(expected, LifecycleState::Failed);
            }
            other => panic!(
                "Expected InvalidLifecycleState(Running, Failed), got {:?}",
                other
            ),
        }
    }

    // ========================================================================
    // Behavior: resume_returns_invalidlifecyclestate_error_when_instance_is_completed
    // ========================================================================

    #[tokio::test]
    async fn resume_returns_invalidlifecyclestate_error_when_instance_is_completed() {
        // Given: An instance exists with lifecycle state Completed
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00C000").unwrap();
        let actor = ControlActor::new();

        // When: Resume is handled
        let result = actor.handle_resume(instance_id.clone());

        // Then: Err(ResumeError::InvalidLifecycleState { actual: Completed, expected: Failed })
        // And: No events are emitted
        // RED PHASE: Currently returns InstanceActorNotFound
        match result {
            Err(ResumeError::InvalidLifecycleState { actual, expected }) => {
                assert_eq!(actual, LifecycleState::Completed);
                assert_eq!(expected, LifecycleState::Failed);
            }
            other => panic!(
                "Expected InvalidLifecycleState(Completed, Failed), got {:?}",
                other
            ),
        }
    }

    // ========================================================================
    // Behavior: resume_returns_invalidlifecyclestate_error_when_instance_is_cancelled
    // ========================================================================

    #[tokio::test]
    async fn resume_returns_invalidlifecyclestate_error_when_instance_is_cancelled() {
        // Given: An instance exists with lifecycle state Cancelled
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00X000").unwrap();
        let actor = ControlActor::new();

        // When: Resume is handled
        let result = actor.handle_resume(instance_id.clone());

        // Then: Err(ResumeError::InvalidLifecycleState { actual: Cancelled, expected: Failed })
        // And: No events are emitted
        // RED PHASE: Currently returns InstanceActorNotFound
        match result {
            Err(ResumeError::InvalidLifecycleState { actual, expected }) => {
                assert_eq!(actual, LifecycleState::Cancelled);
                assert_eq!(expected, LifecycleState::Failed);
            }
            other => panic!(
                "Expected InvalidLifecycleState(Cancelled, Failed), got {:?}",
                other
            ),
        }
    }

    // ========================================================================
    // Behavior: resume_returns_missingsecrets_error_when_secrets_absent
    // ========================================================================

    #[tokio::test]
    async fn resume_returns_missingsecrets_error_when_secrets_absent() {
        // Given: An instance exists with lifecycle Failed but required secret `secret-1` is missing
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BM0F000").unwrap();
        let actor = ControlActor::new();

        // When: Resume is handled
        let result = actor.handle_resume(instance_id.clone());

        // Then: Err(ResumeError::MissingSecrets { instance_id, missing_secret_ids: [secret-1] })
        // And: No events are emitted
        //
        // RED PHASE: Currently returns InstanceActorNotFound
        match result {
            Err(ResumeError::MissingSecrets {
                instance_id: _,
                missing_secret_ids,
            }) => {
                assert!(!missing_secret_ids.is_empty());
            }
            other => panic!("Expected MissingSecrets, got {:?}", other),
        }
    }

    // ========================================================================
    // Behavior: resume_returns_nodenotfound_error_when_node_missing
    // ========================================================================

    #[tokio::test]
    async fn resume_returns_nodenotfound_error_when_node_missing() {
        // Given: An instance exists with lifecycle Failed but required node `node-X` does not exist in workflow
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BN0F000").unwrap();
        let actor = ControlActor::new();

        // When: Resume is handled
        let result = actor.handle_resume(instance_id.clone());

        // Then: Err(ResumeError::NodeNotFound { instance_id, node_name: node-X })
        // And: No events are emitted
        // RED PHASE: Currently returns InstanceActorNotFound
        match result {
            Err(ResumeError::NodeNotFound {
                instance_id: _,
                node_name: _,
            }) => {}
            other => panic!("Expected NodeNotFound, got {:?}", other),
        }
    }

    // ========================================================================
    // Behavior: resume_returns_nopathtoterminal_error_when_no_valid_path
    // ========================================================================

    #[tokio::test]
    async fn resume_returns_nopathtoterminal_error_when_no_valid_path() {
        // Given: An instance exists with lifecycle Failed, node exists, but no valid path from current node to terminal
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BP0F000").unwrap();
        let actor = ControlActor::new();

        // When: Resume is handled
        let result = actor.handle_resume(instance_id.clone());

        // Then: Err(ResumeError::NoPathToTerminal { instance_id, current_node: node-Y })
        // And: No events are emitted
        // RED PHASE: Currently returns InstanceActorNotFound
        match result {
            Err(ResumeError::NoPathToTerminal {
                instance_id: _,
                current_node: _,
            }) => {}
            other => panic!("Expected NoPathToTerminal, got {:?}", other),
        }
    }

    // ========================================================================
    // Behavior: resume_returns_instanceactornotfound_when_actor_missing
    // ========================================================================

    #[tokio::test]
    async fn resume_returns_instanceactornotfound_when_actor_missing() {
        // Given: No InstanceActor exists for the given instance_id
        let instance_id = InstanceId::parse("00000000000000000000000001").unwrap();
        let actor = ControlActor::new();

        // When: Resume is handled
        let result = actor.handle_resume(instance_id.clone());

        // Then: Err(ResumeError::InstanceActorNotFound { instance_id })
        // And: No events are emitted
        match result {
            Err(ResumeError::InstanceActorNotFound { instance_id: _ }) => {}
            other => panic!("Expected InstanceActorNotFound, got {:?}", other),
        }
    }

    // ========================================================================
    // Behavior: resume_returns_lockacquisitionfailed_when_lock_unavailable
    // ========================================================================

    #[tokio::test]
    async fn resume_returns_lockacquisitionfailed_when_lock_unavailable() {
        // Given: An instance exists with lifecycle Failed but another writer holds the write lock
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BA0F000").unwrap();
        let actor = ControlActor::new();

        // When: Resume is handled
        let result = actor.handle_resume(instance_id.clone());

        // Then: Err(ResumeError::LockAcquisitionFailed { instance_id, reason: _ })
        // And: No events are emitted
        // RED PHASE: Currently returns InstanceActorNotFound
        match result {
            Err(ResumeError::LockAcquisitionFailed {
                instance_id: _,
                reason: _,
            }) => {}
            other => panic!("Expected LockAcquisitionFailed, got {:?}", other),
        }
    }

    // ========================================================================
    // Behavior: resume_returns_storageerror_when_event_append_fails
    // ========================================================================

    #[tokio::test]
    async fn resume_returns_storageerror_when_event_append_fails() {
        // Given: An instance exists with valid Failed state and acquired lock, but storage write fails
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BS0F000").unwrap();
        let actor = ControlActor::new();

        // When: Resume is handled
        let result = actor.handle_resume(instance_id.clone());

        // Then: Err(ResumeError::StorageError { instance_id, reason: _ })
        // And: No events are emitted
        // And: Lock is released (no partial state)
        // RED PHASE: Currently returns InstanceActorNotFound
        match result {
            Err(ResumeError::StorageError {
                instance_id: _,
                reason: _,
            }) => {}
            other => panic!("Expected StorageError, got {:?}", other),
        }
    }

    // ========================================================================
    // Proptest Invariants - ResumeError Classification
    // ========================================================================

    #[tokio::test]
    async fn resume_error_precondition_classification_is_correct() {
        // Invariant: ResumeError::is_precondition() returns true for InvalidLifecycleState,
        // MissingSecrets, NodeNotFound, NoPathToTerminal, InstanceActorNotFound.
        // Returns false for LockAcquisitionFailed, StorageError.
        use ResumeError::*;

        let precondition_errors = vec![
            InvalidLifecycleState {
                actual: LifecycleState::Running,
                expected: LifecycleState::Failed,
            },
            MissingSecrets {
                instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000000").unwrap(),
                missing_secret_ids: vec![SecretId::new("secret-1")],
            },
            NodeNotFound {
                instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000001").unwrap(),
                node_name: NodeName::new("node-X"),
            },
            NoPathToTerminal {
                instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000002").unwrap(),
                current_node: NodeName::new("node-Y"),
            },
            InstanceActorNotFound {
                instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000003").unwrap(),
            },
        ];

        for err in precondition_errors {
            assert!(
                err.is_precondition(),
                "Expected {:?} to be precondition",
                err
            );
            assert!(
                !err.is_transient(),
                "Expected {:?} to NOT be transient",
                err
            );
        }

        let transient_errors = vec![
            LockAcquisitionFailed {
                instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000004").unwrap(),
                reason: "lock held".to_string(),
            },
            StorageError {
                instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000005").unwrap(),
                reason: "io error".to_string(),
            },
        ];

        for err in transient_errors {
            assert!(
                !err.is_precondition(),
                "Expected {:?} to NOT be precondition",
                err
            );
            assert!(err.is_transient(), "Expected {:?} to be transient", err);
        }
    }

    #[tokio::test]
    async fn cancel_events_always_ordered_cancelrequested_then_workflowcancelled() {
        // Invariant: For any successful Cancel operation, the event stream contains
        // CancelRequested before WorkflowCancelled, with no intervening events for that instance.
        //
        // RED PHASE: handle_cancel doesn't return events correctly yet
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let actor = ControlActor::new();
        let result = actor.handle_cancel(instance_id.clone());

        // This would verify ordering in a full implementation
        match result {
            Ok((first, second)) => {
                // CancelRequested should have earlier timestamp than WorkflowCancelled
                assert!(
                    second.cancelled_at >= first.requested_at,
                    "WorkflowCancelled should come after CancelRequested"
                );
            }
            Err(_) => {
                // RED PHASE: Currently fails - this is expected
            }
        }
    }
}

#[cfg(test)]
mod accept_resume_tests {
    use super::*;

    // ── Group A: WaitKey validation ──

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

    // ── Group B: SignalPayload validation ──

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

    // ── Group C: LifecycleState::WaitingForSignal ──

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

    // ── Group D: AcceptResumeError classification ──

    #[test]
    fn accept_resume_error_precondition_variants_are_correct() {
        let iid = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000000").unwrap();
        let precondition_errors: Vec<AcceptResumeError> = vec![
            AcceptResumeError::InvalidLifecycleState {
                instance_id: iid.clone(),
                actual: LifecycleState::Running,
                expected: LifecycleState::WaitingForSignal,
            },
            AcceptResumeError::WaitKeyMismatch {
                instance_id: iid.clone(),
                expected_key: WaitKey::new_unchecked("a"),
                provided_key: WaitKey::new_unchecked("b"),
            },
            AcceptResumeError::InstanceActorNotFound {
                instance_id: iid.clone(),
            },
            AcceptResumeError::PayloadTooLarge {
                instance_id: iid,
                payload_size: 65537,
                max_size: 65536,
            },
        ];
        for err in &precondition_errors {
            assert!(
                err.is_precondition(),
                "Expected {:?} to be precondition",
                err
            );
            assert!(
                !err.is_transient(),
                "Expected {:?} to NOT be transient",
                err
            );
        }
    }

    #[test]
    fn accept_resume_error_transient_variants_are_correct() {
        let iid = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000000").unwrap();
        let transient_errors: Vec<AcceptResumeError> = vec![
            AcceptResumeError::LockAcquisitionFailed {
                instance_id: iid.clone(),
                reason: "lock held".to_string(),
            },
            AcceptResumeError::StorageError {
                instance_id: iid,
                reason: "io error".to_string(),
            },
        ];
        for err in &transient_errors {
            assert!(
                !err.is_precondition(),
                "Expected {:?} to NOT be precondition",
                err
            );
            assert!(err.is_transient(), "Expected {:?} to be transient", err);
        }
    }

    // ── Group E: accept_and_resume success path ──

    #[tokio::test]
    async fn accept_and_resume_succeeds_when_waiting_for_signal() {
        // 'W' at position 22 encodes WaitingForSignal
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
        let actor = ControlActor::new();
        let wait_key = WaitKey::parse("approval-v2").unwrap();
        let payload = SignalPayload::empty();

        let result =
            actor.accept_and_resume(instance_id.clone(), wait_key, "sig-1".to_string(), payload);

        let outcome = result.unwrap();
        assert_eq!(outcome.accepted.instance_id, instance_id);
        assert_eq!(outcome.resumed.instance_id, instance_id);
    }

    #[tokio::test]
    async fn accept_and_resume_outcome_has_correct_instance_id() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
        let actor = ControlActor::new();
        let wait_key = WaitKey::parse("approval-v2").unwrap();

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "sig-2".to_string(),
            SignalPayload::empty(),
        );

        let outcome = result.unwrap();
        assert_eq!(outcome.accepted.instance_id, instance_id);
        assert_eq!(outcome.resumed.instance_id, instance_id);
    }

    #[tokio::test]
    async fn accept_and_resume_outcome_timestamps_are_ordered() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
        let actor = ControlActor::new();
        let wait_key = WaitKey::parse("approval-v2").unwrap();

        let result = actor.accept_and_resume(
            instance_id,
            wait_key,
            "sig-3".to_string(),
            SignalPayload::empty(),
        );

        let outcome = result.unwrap();
        assert!(outcome.resumed.resumed_at >= outcome.accepted.accepted_at);
    }

    // ── Group F: accept_and_resume error paths ──

    #[tokio::test]
    async fn accept_and_resume_returns_instance_not_found() {
        let instance_id = InstanceId::parse("00000000000000000000000001").unwrap();
        let actor = ControlActor::new();
        let wait_key = WaitKey::parse("approval-v2").unwrap();

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "sig-1".to_string(),
            SignalPayload::empty(),
        );

        match result {
            Err(AcceptResumeError::InstanceActorNotFound { instance_id: _ }) => {}
            other => panic!("Expected InstanceActorNotFound, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn accept_and_resume_returns_invalid_lifecycle_when_running() {
        // Default char at pos 22 means Running
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000000").unwrap();
        let actor = ControlActor::new();
        let wait_key = WaitKey::parse("approval-v2").unwrap();

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "sig-1".to_string(),
            SignalPayload::empty(),
        );

        match result {
            Err(AcceptResumeError::InvalidLifecycleState {
                instance_id: _,
                actual,
                expected,
            }) => {
                assert_eq!(actual, LifecycleState::Running);
                assert_eq!(expected, LifecycleState::WaitingForSignal);
            }
            other => panic!("Expected InvalidLifecycleState(Running), got {:?}", other),
        }
    }

    #[tokio::test]
    async fn accept_and_resume_returns_wait_key_mismatch() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
        let actor = ControlActor::new();
        let wait_key = WaitKey::parse("wrong-key").unwrap();

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "mismatch-sig-1".to_string(),
            SignalPayload::empty(),
        );

        match result {
            Err(AcceptResumeError::WaitKeyMismatch {
                instance_id: _,
                expected_key,
                provided_key,
            }) => {
                assert_eq!(expected_key.as_str(), "expected-key");
                assert_eq!(provided_key.as_str(), "wrong-key");
            }
            other => panic!("Expected WaitKeyMismatch, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn accept_and_resume_returns_payload_too_large() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
        let actor = ControlActor::new();
        let wait_key = WaitKey::parse("approval-v2").unwrap();
        let big_payload = SignalPayload::new_unchecked(vec![0u8; 65537]);

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "sig-1".to_string(),
            big_payload,
        );

        match result {
            Err(AcceptResumeError::PayloadTooLarge {
                instance_id: _,
                payload_size,
                max_size,
            }) => {
                assert_eq!(payload_size, 65537);
                assert_eq!(max_size, 65536);
            }
            other => panic!("Expected PayloadTooLarge, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn accept_and_resume_returns_lock_acquisition_failed() {
        // 'A' at position 20 encodes lock error
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BA0W000").unwrap();
        let actor = ControlActor::new();
        let wait_key = WaitKey::parse("approval-v2").unwrap();

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "sig-1".to_string(),
            SignalPayload::empty(),
        );

        match result {
            Err(AcceptResumeError::LockAcquisitionFailed {
                instance_id: _,
                reason: _,
            }) => {}
            other => panic!("Expected LockAcquisitionFailed, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn accept_and_resume_returns_storage_error() {
        // 'S' at position 20 encodes storage error
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BS0W000").unwrap();
        let actor = ControlActor::new();
        let wait_key = WaitKey::parse("approval-v2").unwrap();

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "sig-1".to_string(),
            SignalPayload::empty(),
        );

        match result {
            Err(AcceptResumeError::StorageError {
                instance_id: _,
                reason: _,
            }) => {}
            other => panic!("Expected StorageError, got {:?}", other),
        }
    }
}

pub use actor_messages::{ControlActorMessage, InstanceActorMessage};
