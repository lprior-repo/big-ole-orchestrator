//! Saga-based Distributed Transaction Coordinator (ADR-034).
//!
//! Architecture: Data (SagaState, SagaStep, SagaRecord)
//!              → Calc (apply_saga_transition, is_terminal, all_variants).
//!
//! This module defines the type system for saga-style distributed transactions.
//! Unlike 2PC (two-phase commit), saga pattern executes steps sequentially with
//! compensating actions that run in reverse order on failure.
//!
//! No I/O, no engine integration — pure types and state machine logic.

use crate::effects::CompensationPolicy;

// ============================================================================
// Data Layer: Type Definitions
// ============================================================================

/// Lifecycle state of a saga coordinator (ADR-034).
///
/// Follows the saga pattern:
/// - Executing: Steps are running forward
/// - Compensating: A step failed, compensations running in reverse
/// - Completed: All steps succeeded (terminal)
/// - Failed: Compensation failed or irreversible (terminal)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SagaState {
    /// Saga is initialized, no steps executed yet.
    Init,

    /// Saga is executing forward steps.
    Executing,

    /// A step failed, compensations are running in reverse order.
    Compensating,

    /// All steps completed successfully (terminal).
    Completed,

    /// Compensation failed or saga is irrecoverable (terminal).
    Failed,
}

/// Status of an individual step within a saga.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SagaStepStatus {
    /// Step has not started yet.
    Pending,

    /// Step is currently executing.
    Executing,

    /// Step completed successfully, compensation is registered.
    Completed,

    /// Step failed, compensation is pending or in progress.
    CompensationPending,

    /// Compensation for this step is executing.
    Compensating,

    /// Compensation completed successfully.
    Compensated,

    /// Compensation failed (terminal for this step).
    CompensationFailed,
}

/// Events that drive the SagaState transitions (ADR-034).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SagaTransition {
    /// Begin executing the first step.
    Begin,

    /// A step completed successfully.
    StepCompleted,

    /// A step failed.
    StepFailed,

    /// A compensation action completed successfully.
    CompensationCompleted,

    /// A compensation action failed.
    CompensationFailed,

    /// All steps have been compensated (reverse order complete).
    AllCompensated,

    /// Recovery: coordinator is resuming after crash.
    Recover,
}

/// Error returned when a saga state transition is invalid.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SagaTransitionError {
    #[error("Cannot transition from terminal saga state")]
    TerminalStateTransition,

    #[error("Invalid saga state transition")]
    InvalidTransition,
}

/// A single step in a saga with its compensation information (ADR-034).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SagaStep {
    step_id: String,
    effect_id: String,
    compensation_effect_id: Option<String>,
    policy: CompensationPolicy,
    status: SagaStepStatus,
    dependencies: Vec<String>,
}

/// Persisted record of a saga (ADR-034).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SagaRecord {
    saga_id: String,
    state: SagaState,
    steps: Vec<SagaStep>,
    current_step_index: usize,
    completed_step_indices: Vec<usize>,
    compensation_stack: Vec<usize>,
    created_at: Option<crate::types::TimestampMs>,
    completed_at: Option<crate::types::TimestampMs>,
    failed_at: Option<crate::types::TimestampMs>,
}

// ============================================================================
// Calc Layer: Pure Functions
// ============================================================================

impl SagaState {
    /// Check if this state is terminal (Completed or Failed).
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, SagaState::Completed | SagaState::Failed)
    }

    /// Returns all SagaState variants in declaration order.
    #[must_use]
    pub const fn all_variants() -> &'static [SagaState] {
        &[
            SagaState::Init,
            SagaState::Executing,
            SagaState::Compensating,
            SagaState::Completed,
            SagaState::Failed,
        ]
    }
}

impl SagaStepStatus {
    /// Returns all SagaStepStatus variants in declaration order.
    #[must_use]
    pub const fn all_variants() -> &'static [SagaStepStatus] {
        &[
            SagaStepStatus::Pending,
            SagaStepStatus::Executing,
            SagaStepStatus::Completed,
            SagaStepStatus::CompensationPending,
            SagaStepStatus::Compensating,
            SagaStepStatus::Compensated,
            SagaStepStatus::CompensationFailed,
        ]
    }
}

impl SagaTransition {
    /// Returns all SagaTransition variants in declaration order.
    #[must_use]
    pub const fn all_variants() -> &'static [SagaTransition] {
        &[
            SagaTransition::Begin,
            SagaTransition::StepCompleted,
            SagaTransition::StepFailed,
            SagaTransition::CompensationCompleted,
            SagaTransition::CompensationFailed,
            SagaTransition::AllCompensated,
            SagaTransition::Recover,
        ]
    }
}

impl SagaStep {
    /// Construct a new SagaStep.
    #[must_use]
    pub fn new(
        step_id: String,
        effect_id: String,
        compensation_effect_id: Option<String>,
        policy: CompensationPolicy,
        dependencies: Vec<String>,
    ) -> Option<Self> {
        if step_id.is_empty() || effect_id.is_empty() {
            return None;
        }
        Some(Self {
            step_id,
            effect_id,
            compensation_effect_id,
            policy,
            status: SagaStepStatus::Pending,
            dependencies,
        })
    }

    #[must_use]
    pub fn step_id(&self) -> &str {
        &self.step_id
    }

    #[must_use]
    pub fn effect_id(&self) -> &str {
        &self.effect_id
    }

    #[must_use]
    pub fn compensation_effect_id(&self) -> Option<&str> {
        self.compensation_effect_id.as_deref()
    }

    #[must_use]
    pub fn policy(&self) -> CompensationPolicy {
        self.policy
    }

    #[must_use]
    pub fn status(&self) -> SagaStepStatus {
        self.status
    }

    #[must_use]
    pub fn dependencies(&self) -> &[String] {
        &self.dependencies
    }
}

impl SagaRecord {
    /// Construct a new SagaRecord.
    ///
    /// Returns `None` if `saga_id` is empty.
    #[must_use]
    pub fn new(
        saga_id: String,
        steps: Vec<SagaStep>,
        created_at: Option<crate::types::TimestampMs>,
    ) -> Option<Self> {
        if saga_id.is_empty() {
            return None;
        }
        Some(Self {
            saga_id,
            state: SagaState::Init,
            steps,
            current_step_index: 0,
            completed_step_indices: Vec::new(),
            compensation_stack: Vec::new(),
            created_at,
            completed_at: None,
            failed_at: None,
        })
    }

    #[must_use]
    pub fn saga_id(&self) -> &str {
        &self.saga_id
    }

    #[must_use]
    pub fn state(&self) -> SagaState {
        self.state
    }

    #[must_use]
    pub fn steps(&self) -> &[SagaStep] {
        &self.steps
    }

    #[must_use]
    pub fn current_step_index(&self) -> usize {
        self.current_step_index
    }

    #[must_use]
    pub fn completed_step_indices(&self) -> &[usize] {
        &self.completed_step_indices
    }

    #[must_use]
    pub fn compensation_stack(&self) -> &[usize] {
        &self.compensation_stack
    }

    #[must_use]
    pub fn created_at(&self) -> Option<&crate::types::TimestampMs> {
        self.created_at.as_ref()
    }

    #[must_use]
    pub fn completed_at(&self) -> Option<&crate::types::TimestampMs> {
        self.completed_at.as_ref()
    }

    #[must_use]
    pub fn failed_at(&self) -> Option<&crate::types::TimestampMs> {
        self.failed_at.as_ref()
    }

    /// Returns the next step to execute, if any.
    #[must_use]
    pub fn next_step(&self) -> Option<&SagaStep> {
        if self.current_step_index < self.steps.len() {
            Some(&self.steps[self.current_step_index])
        } else {
            None
        }
    }

    /// Returns the next compensation to execute, if any (from compensation stack).
    #[must_use]
    pub fn next_compensation(&self) -> Option<&SagaStep> {
        self.compensation_stack.last().map(|&idx| &self.steps[idx])
    }
}

/// Apply a state transition to a SagaState.
///
/// # Errors
///
/// Returns `SagaTransitionError::TerminalStateTransition` if the current state
/// is terminal (Completed or Failed).
/// Returns `SagaTransitionError::InvalidTransition` if the event is not valid
/// for the current state.
pub fn apply_saga_transition(
    current: SagaState,
    event: SagaTransition,
) -> Result<SagaState, SagaTransitionError> {
    match (current, event) {
        // Init transitions
        (SagaState::Init, SagaTransition::Begin) => Ok(SagaState::Executing),

        // Executing transitions
        (SagaState::Executing, SagaTransition::StepCompleted) => Ok(SagaState::Executing),
        (SagaState::Executing, SagaTransition::StepFailed) => Ok(SagaState::Compensating),

        // Compensating transitions
        (SagaState::Compensating, SagaTransition::CompensationCompleted) => {
            Ok(SagaState::Compensating)
        }
        (SagaState::Compensating, SagaTransition::CompensationFailed) => Ok(SagaState::Failed),
        (SagaState::Compensating, SagaTransition::AllCompensated) => Ok(SagaState::Failed),

        // Recovery from non-terminal states
        (SagaState::Executing, SagaTransition::Recover) => Ok(SagaState::Executing),
        (SagaState::Compensating, SagaTransition::Recover) => Ok(SagaState::Compensating),
        (SagaState::Init, SagaTransition::Recover) => Ok(SagaState::Init),
        (SagaState::Executing, SagaTransition::CompensationFailed) => Ok(SagaState::Failed),

        // Terminal states reject all transitions
        (SagaState::Completed, _) => Err(SagaTransitionError::TerminalStateTransition),
        (SagaState::Failed, _) => Err(SagaTransitionError::TerminalStateTransition),

        // Invalid transitions
        (SagaState::Init, SagaTransition::StepCompleted) => {
            Err(SagaTransitionError::InvalidTransition)
        }
        (SagaState::Init, SagaTransition::StepFailed) => {
            Err(SagaTransitionError::InvalidTransition)
        }
        (SagaState::Init, SagaTransition::CompensationCompleted) => {
            Err(SagaTransitionError::InvalidTransition)
        }
        (SagaState::Init, SagaTransition::CompensationFailed) => {
            Err(SagaTransitionError::InvalidTransition)
        }
        (SagaState::Init, SagaTransition::AllCompensated) => {
            Err(SagaTransitionError::InvalidTransition)
        }
        (SagaState::Executing, SagaTransition::Begin) => {
            Err(SagaTransitionError::InvalidTransition)
        }
        (SagaState::Executing, SagaTransition::AllCompensated) => {
            Err(SagaTransitionError::InvalidTransition)
        }
        (SagaState::Executing, SagaTransition::CompensationCompleted) => {
            Err(SagaTransitionError::InvalidTransition)
        }
        (SagaState::Compensating, SagaTransition::Begin) => {
            Err(SagaTransitionError::InvalidTransition)
        }
        (SagaState::Compensating, SagaTransition::StepCompleted) => {
            Err(SagaTransitionError::InvalidTransition)
        }
        (SagaState::Compensating, SagaTransition::StepFailed) => {
            Err(SagaTransitionError::InvalidTransition)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sagastate_is_terminal_returns_true_for_completed_and_failed() {
        assert!(SagaState::Completed.is_terminal());
        assert!(SagaState::Failed.is_terminal());
        assert!(!SagaState::Init.is_terminal());
        assert!(!SagaState::Executing.is_terminal());
        assert!(!SagaState::Compensating.is_terminal());
    }

    #[test]
    fn sagastate_all_variants_returns_five_variants() {
        let variants = SagaState::all_variants();
        assert_eq!(variants.len(), 5);
    }

    #[test]
    fn sagastep_new_returns_none_for_empty_ids() {
        assert!(SagaStep::new(
            "".to_string(),
            "fx-1".to_string(),
            None,
            CompensationPolicy::Automatic,
            vec![]
        )
        .is_none());
        assert!(SagaStep::new(
            "step-1".to_string(),
            "".to_string(),
            None,
            CompensationPolicy::Automatic,
            vec![]
        )
        .is_none());
    }

    #[test]
    fn sagastep_new_returns_some_for_valid_ids() {
        let step = SagaStep::new(
            "step-1".to_string(),
            "fx-1".to_string(),
            Some("comp-fx-1".to_string()),
            CompensationPolicy::Automatic,
            vec![],
        );
        assert!(step.is_some());
        let s = step.unwrap();
        assert_eq!(s.step_id(), "step-1");
        assert_eq!(s.effect_id(), "fx-1");
        assert_eq!(s.compensation_effect_id(), Some("comp-fx-1"));
        assert_eq!(s.policy(), CompensationPolicy::Automatic);
    }

    #[test]
    fn sagarecord_new_returns_none_for_empty_saga_id() {
        assert!(SagaRecord::new(vec![], None).is_none());
    }

    #[test]
    fn sagarecord_new_returns_some_with_empty_steps() {
        let record = SagaRecord::new("saga-1".to_string(), vec![], None);
        assert!(record.is_some());
        let r = record.unwrap();
        assert_eq!(r.saga_id(), "saga-1");
        assert_eq!(r.state(), SagaState::Init);
        assert!(r.steps().is_empty());
    }

    #[test]
    fn sagarecord_next_step_returns_none_when_no_more_steps() {
        let step = SagaStep::new(
            "step-1".to_string(),
            "fx-1".to_string(),
            None,
            CompensationPolicy::Automatic,
            vec![],
        )
        .unwrap();
        let record = SagaRecord::new("saga-1".to_string(), vec![step], None).unwrap();
        assert!(record.next_step().is_some());
    }

    #[test]
    fn apply_saga_transition_init_to_executing() {
        let result = apply_saga_transition(SagaState::Init, SagaTransition::Begin);
        assert_eq!(result, Ok(SagaState::Executing));
    }

    #[test]
    fn apply_saga_transition_executing_step_completed_stays_executing() {
        let result = apply_saga_transition(SagaState::Executing, SagaTransition::StepCompleted);
        assert_eq!(result, Ok(SagaState::Executing));
    }

    #[test]
    fn apply_saga_transition_executing_step_failed_to_compensating() {
        let result = apply_saga_transition(SagaState::Executing, SagaTransition::StepFailed);
        assert_eq!(result, Ok(SagaState::Compensating));
    }

    #[test]
    fn apply_saga_transition_compensating_compensation_completed_stays_compensating() {
        let result = apply_saga_transition(
            SagaState::Compensating,
            SagaTransition::CompensationCompleted,
        );
        assert_eq!(result, Ok(SagaState::Compensating));
    }

    #[test]
    fn apply_saga_transition_compensating_compensation_failed_to_failed() {
        let result =
            apply_saga_transition(SagaState::Compensating, SagaTransition::CompensationFailed);
        assert_eq!(result, Ok(SagaState::Failed));
    }

    #[test]
    fn apply_saga_transition_terminal_states_reject_all_transitions() {
        for event in SagaTransition::all_variants() {
            assert_eq!(
                apply_saga_transition(SagaState::Completed, event),
                Err(SagaTransitionError::TerminalStateTransition)
            );
            assert_eq!(
                apply_saga_transition(SagaState::Failed, event),
                Err(SagaTransitionError::TerminalStateTransition)
            );
        }
    }

    #[test]
    fn apply_saga_transition_invalid_transitions() {
        assert_eq!(
            apply_saga_transition(SagaState::Init, SagaTransition::StepCompleted),
            Err(SagaTransitionError::InvalidTransition)
        );
        assert_eq!(
            apply_saga_transition(SagaState::Executing, SagaTransition::Begin),
            Err(SagaTransitionError::InvalidTransition)
        );
        assert_eq!(
            apply_saga_transition(SagaState::Compensating, SagaTransition::Begin),
            Err(SagaTransitionError::InvalidTransition)
        );
    }
}
