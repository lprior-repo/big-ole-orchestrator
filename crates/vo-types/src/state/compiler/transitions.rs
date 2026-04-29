//! Transition table compilation.
//!
//! Provides `create_lifecycle_table()` which builds the complete lifecycle
//! state machine transition table with all defined transitions.

use super::definition::*;
use super::validation::Guard;
use crate::state::lifecycle::{LifecycleState, TransitionEvent};

pub fn create_lifecycle_table() -> TransitionTable {
    TransitionTable::builder()
        .add_transition(LifecycleState::Pending, TransitionEvent::AssignToNode)
        .to(LifecycleState::RunningDecision)
        .with_description("Assign pending bead to node")
        .build()
        .add_transition(LifecycleState::Pending, TransitionEvent::Cancel)
        .to(LifecycleState::Cancelled)
        .with_description("Cancel pending bead")
        .build()
        .add_transition(
            LifecycleState::RunningDecision,
            TransitionEvent::StepScheduled,
        )
        .to(LifecycleState::StepScheduled)
        .with_description("Schedule step for execution")
        .build()
        .add_transition(
            LifecycleState::RunningDecision,
            TransitionEvent::Cancel,
        )
        .to(LifecycleState::Cancelled)
        .with_description("Cancel during decision")
        .build()
        .add_transition(
            LifecycleState::RunningDecision,
            TransitionEvent::Fail,
        )
        .to(LifecycleState::Failed)
        .with_description("Fail during decision")
        .build()
        .add_transition(
            LifecycleState::StepScheduled,
            TransitionEvent::ExecuteStep,
        )
        .to(LifecycleState::StepExecuting)
        .with_description("Begin step execution")
        .build()
        .add_transition(LifecycleState::StepScheduled, TransitionEvent::Cancel)
        .to(LifecycleState::Cancelled)
        .with_description("Cancel scheduled step")
        .build()
        .add_transition(LifecycleState::StepScheduled, TransitionEvent::Fail)
        .to(LifecycleState::Failed)
        .with_description("Fail scheduled step")
        .build()
        .add_transition(
            LifecycleState::StepExecuting,
            TransitionEvent::WaitForTimer,
        )
        .to(LifecycleState::WaitingForTimer)
        .with_description("Wait for timer")
        .build()
        .add_transition(
            LifecycleState::StepExecuting,
            TransitionEvent::CompleteStep,
        )
        .to(LifecycleState::Completed)
        .with_description("Complete step successfully")
        .build()
        .add_transition(LifecycleState::StepExecuting, TransitionEvent::Cancel)
        .to(LifecycleState::Cancelled)
        .with_description("Cancel executing step")
        .build()
        .add_transition(LifecycleState::StepExecuting, TransitionEvent::Fail)
        .to(LifecycleState::Failed)
        .with_description("Fail executing step")
        .build()
        .add_transition(
            LifecycleState::StepExecuting,
            TransitionEvent::PrepareEffect,
        )
        .to(LifecycleState::PreparingEffect)
        .with_description("Begin preparing managed effect")
        .build()
        .add_transition(
            LifecycleState::PreparingEffect,
            TransitionEvent::EffectPrepared,
        )
        .to(LifecycleState::StepExecuting)
        .with_description("Effect prepared, resume execution")
        .build()
        .add_transition(LifecycleState::PreparingEffect, TransitionEvent::Cancel)
        .to(LifecycleState::Cancelled)
        .with_description("Cancel while preparing effect")
        .build()
        .add_transition(LifecycleState::PreparingEffect, TransitionEvent::Fail)
        .to(LifecycleState::Failed)
        .with_description("Fail while preparing effect")
        .build()
        .add_transition(
            LifecycleState::WaitingForTimer,
            TransitionEvent::TimerFired,
        )
        .to(LifecycleState::StepExecuting)
        .with_description("Timer fired, resume execution")
        .build()
        .add_transition(
            LifecycleState::WaitingForTimer,
            TransitionEvent::TimerExpired,
        )
        .to(LifecycleState::Failed)
        .with_description("Timer expired")
        .build()
        .add_transition(
            LifecycleState::WaitingForTimer,
            TransitionEvent::Cancel,
        )
        .to(LifecycleState::Cancelled)
        .with_description("Cancel while waiting for timer")
        .build()
        .add_transition(
            LifecycleState::WaitingForTimer,
            TransitionEvent::Fail,
        )
        .to(LifecycleState::Failed)
        .with_description("Fail while waiting for timer")
        .build()
        .add_transition(
            LifecycleState::Failed,
            TransitionEvent::InstanceResumed,
        )
        .to(LifecycleState::RunningDecision)
        .with_description("Resume failed instance")
        .with_guard(Guard::Always)
        .build()
        .build()
}
