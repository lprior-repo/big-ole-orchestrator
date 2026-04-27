//! Lifecycle state machine with exhaustive transition rules.
//!
//! Defines the flat state enum, its superstate mapping (ADR-039),
//! operational status classification, and the transition event vocabulary.

use std::hash::Hash;

use serde::{Deserialize, Serialize};

/// Lifecycle state of a bead in the workflow
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    /// Initial state: bead is queued, not yet assigned
    Pending,

    /// Decision phase: bead is evaluating which step to execute
    RunningDecision,

    /// Step is scheduled but not yet executing
    StepScheduled,

    /// Step is actively executing
    StepExecuting,

    /// Waiting for external timer/callback
    WaitingForTimer,

    /// Waiting for external signal matching wait_key (ADR-039, ADR-042)
    WaitingForSignal,

    /// Publication barrier: waiting for blob to be verified durable (ADR-040)
    PendingPublication,

    /// Terminal state: bead completed successfully
    Completed,

    /// Terminal state: bead failed
    Failed,

    /// Terminal state: bead was cancelled
    Cancelled,
}

impl LifecycleState {
    /// Get the operational status for a given state
    #[must_use]
    pub fn get_operational_status(&self) -> OperationalStatus {
        match self {
            LifecycleState::Pending
            | LifecycleState::RunningDecision
            | LifecycleState::StepScheduled
            | LifecycleState::StepExecuting => OperationalStatus::Healthy,
            LifecycleState::WaitingForTimer | LifecycleState::WaitingForSignal => {
                OperationalStatus::Healthy
            }
            LifecycleState::PendingPublication => {
                OperationalStatus::Blocked(BlockedReason::DependenciesPending)
            }
            LifecycleState::Completed | LifecycleState::Cancelled => {
                OperationalStatus::Blocked(BlockedReason::ManualHold)
            }
            LifecycleState::Failed => OperationalStatus::Recovering,
        }
    }

    /// Check if a state is terminal
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            LifecycleState::Completed | LifecycleState::Failed | LifecycleState::Cancelled
        )
    }

    /// Map this flat state to its hierarchical superstate (ADR-039).
    ///
    /// Superstate assignments per ADR-039:
    /// - `Active`: states where the workflow is actively processing
    /// - `Suspended`: states where execution is paused (waiting/holding)
    /// - `Recovering`: states where the instance is being recovered from failure
    /// - `Compensating`: states where forward effects are being compensated
    /// - `Terminal`: states where the workflow has permanently ended
    #[must_use]
    pub fn superstate(&self) -> crate::lifecycle_superstate::LifecycleSuperstate {
        match self {
            LifecycleState::Pending
            | LifecycleState::RunningDecision
            | LifecycleState::StepScheduled
            | LifecycleState::StepExecuting => {
                crate::lifecycle_superstate::LifecycleSuperstate::Active
            }
            LifecycleState::WaitingForTimer
            | LifecycleState::WaitingForSignal
            | LifecycleState::PendingPublication => {
                crate::lifecycle_superstate::LifecycleSuperstate::Suspended
            }
            LifecycleState::Failed => crate::lifecycle_superstate::LifecycleSuperstate::Recovering,
            LifecycleState::Completed | LifecycleState::Cancelled => {
                crate::lifecycle_superstate::LifecycleSuperstate::Terminal
            }
        }
    }

    /// Get all valid transitions from a state
    #[must_use]
    pub fn get_valid_transitions(&self) -> Vec<TransitionEvent> {
        match self {
            LifecycleState::Pending => {
                vec![TransitionEvent::AssignToNode, TransitionEvent::Cancel]
            }
            LifecycleState::RunningDecision => {
                vec![
                    TransitionEvent::StepScheduled,
                    TransitionEvent::Cancel,
                    TransitionEvent::Fail,
                ]
            }
            LifecycleState::StepScheduled => {
                vec![
                    TransitionEvent::ExecuteStep,
                    TransitionEvent::Cancel,
                    TransitionEvent::Fail,
                ]
            }
            LifecycleState::StepExecuting => vec![
                TransitionEvent::WaitForTimer,
                TransitionEvent::WaitForSignal,
                TransitionEvent::YieldWithBlob,
                TransitionEvent::CompleteStep,
                TransitionEvent::Cancel,
                TransitionEvent::Fail,
            ],
            LifecycleState::WaitingForTimer => vec![
                TransitionEvent::TimerFired,
                TransitionEvent::TimerExpired,
                TransitionEvent::Cancel,
                TransitionEvent::Fail,
            ],
            LifecycleState::WaitingForSignal => vec![
                TransitionEvent::SignalReceived,
                TransitionEvent::Cancel,
                TransitionEvent::Fail,
            ],
            LifecycleState::PendingPublication => vec![
                TransitionEvent::ConfirmPublication,
                TransitionEvent::PublicationFailed,
                TransitionEvent::Cancel,
            ],
            LifecycleState::Completed | LifecycleState::Cancelled => vec![],
            LifecycleState::Failed => vec![TransitionEvent::InstanceResumed],
        }
    }
}

/// Operational status of a bead instance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationalStatus {
    /// Normal operation
    Healthy,

    /// Blocked with specific reason
    Blocked(BlockedReason),

    /// Recovering from failure
    Recovering,
}

/// Reason why a bead is blocked
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockedReason {
    /// Waiting for dependencies
    DependenciesPending,
    /// Resource contention
    ResourceContention,
    /// Manual hold
    ManualHold,
}

/// Transition event that triggers state changes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionEvent {
    // From Pending
    AssignToNode,
    Cancel,

    // From RunningDecision
    StepScheduled,
    Fail,

    // From StepScheduled
    ExecuteStep,

    // From StepExecuting
    WaitForTimer,
    WaitForSignal,
    CompleteStep,
    YieldWithBlob,

    // From WaitingForTimer
    TimerFired,
    TimerExpired,

    // From WaitingForSignal (ADR-039, ADR-042)
    SignalReceived,

    // From PendingPublication
    ConfirmPublication,
    PublicationFailed,

    // From Completed (terminal but allows EmitOutputRef for post-publication emission)
    EmitOutputRef,

    // From Failed (only InstanceResumed valid)
    InstanceResumed,
    // From Cancelled (terminal - no transitions)
}

impl TransitionEvent {
    /// Get all valid `TransitionEvent` variants for iteration
    #[must_use]
    pub fn all_variants() -> &'static [TransitionEvent] {
        &[
            TransitionEvent::AssignToNode,
            TransitionEvent::Cancel,
            TransitionEvent::StepScheduled,
            TransitionEvent::Fail,
            TransitionEvent::ExecuteStep,
            TransitionEvent::WaitForTimer,
            TransitionEvent::WaitForSignal,
            TransitionEvent::CompleteStep,
            TransitionEvent::YieldWithBlob,
            TransitionEvent::TimerFired,
            TransitionEvent::TimerExpired,
            TransitionEvent::SignalReceived,
            TransitionEvent::ConfirmPublication,
            TransitionEvent::PublicationFailed,
            TransitionEvent::EmitOutputRef,
            TransitionEvent::InstanceResumed,
        ]
    }
}
