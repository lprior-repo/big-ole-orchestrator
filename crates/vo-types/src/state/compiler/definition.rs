//! State machine type definitions.
//!
//! Core types: error types, guard conditions, side effects, and the TransitionRule struct.

use crate::state::lifecycle::{LifecycleState, TransitionEvent};

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CompilerTransitionError {
    #[error("Cannot transition from terminal state")]
    TerminalStateTransition,
    #[error("Invalid transition for current state")]
    InvalidTransition,
    #[error("Guard condition rejected transition")]
    GuardRejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GuardResult {
    Accepted,
    Rejected,
}

pub type GuardFn = Box<dyn Fn(LifecycleState, TransitionEvent) -> GuardResult + Send + Sync>;

#[derive(Default)]
pub enum Guard {
    #[default]
    Always,
    Never,
    If(fn(LifecycleState, TransitionEvent) -> bool),
    Fn {
        f: GuardFn,
    },
}

impl Clone for Guard {
    fn clone(&self) -> Self {
        match self {
            Guard::Always => Guard::Always,
            Guard::Never => Guard::Never,
            Guard::If(predicate) => Guard::If(*predicate),
            Guard::Fn { .. } => Guard::Fn {
                f: Box::new(|_, _| GuardResult::Rejected),
            },
        }
    }
}

impl Guard {
    pub fn check(&self, state: LifecycleState, event: TransitionEvent) -> GuardResult {
        match self {
            Guard::Always => GuardResult::Accepted,
            Guard::Never => GuardResult::Rejected,
            Guard::If(predicate) => {
                if predicate(state, event) {
                    GuardResult::Accepted
                } else {
                    GuardResult::Rejected
                }
            }
            Guard::Fn { f } => f(state, event),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SideEffectResult {
    Executed,
    Skipped,
}

pub type SideEffectFn =
    Box<dyn Fn(LifecycleState, TransitionEvent, LifecycleState) -> SideEffectResult + Send + Sync>;

#[derive(Default)]
pub enum SideEffect {
    #[default]
    None,
    Log {
        message: String,
    },
    Fn {
        f: SideEffectFn,
    },
}

impl Clone for SideEffect {
    fn clone(&self) -> Self {
        match self {
            SideEffect::None => SideEffect::None,
            SideEffect::Log { message } => SideEffect::Log {
                message: message.clone(),
            },
            SideEffect::Fn { .. } => SideEffect::Fn {
                f: Box::new(|_, _, _| SideEffectResult::Skipped),
            },
        }
    }
}

impl SideEffect {
    pub fn execute(
        &self,
        from: LifecycleState,
        event: TransitionEvent,
        to: LifecycleState,
    ) -> SideEffectResult {
        match self {
            SideEffect::None => SideEffectResult::Skipped,
            SideEffect::Log { message } => {
                eprintln!("Transition side effect: {from:?} -> {to:?} via {event:?}: {message}");
                SideEffectResult::Executed
            }
            SideEffect::Fn { f } => f(from, event, to),
        }
    }
}

pub struct TransitionRule {
    pub(crate) from: LifecycleState,
    pub(crate) event: TransitionEvent,
    pub(crate) to: LifecycleState,
    pub(crate) guard: Guard,
    pub(crate) side_effect: SideEffect,
    pub(crate) description: Option<String>,
}

impl Clone for TransitionRule {
    fn clone(&self) -> Self {
        Self {
            from: self.from,
            event: self.event,
            to: self.to,
            guard: self.guard.clone(),
            side_effect: self.side_effect.clone(),
            description: self.description.clone(),
        }
    }
}

impl std::fmt::Debug for TransitionRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransitionRule")
            .field("from", &self.from)
            .field("event", &self.event)
            .field("to", &self.to)
            .field("guard", &"Guard")
            .field("side_effect", &"SideEffect")
            .field("description", &self.description)
            .finish()
    }
}

impl TransitionRule {
    pub fn new(from: LifecycleState, event: TransitionEvent, to: LifecycleState) -> Self {
        Self {
            from,
            event,
            to,
            guard: Guard::Always,
            side_effect: SideEffect::None,
            description: None,
        }
    }
}
