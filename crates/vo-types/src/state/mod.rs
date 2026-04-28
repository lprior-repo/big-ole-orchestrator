//! Domain state types for the vo-engine.
//!
//! Lifecycle state machine with exhaustive transition rules.
#![allow(dead_code)]

pub mod compiler;
mod lifecycle;
mod semantic_types;
mod transition;

pub use compiler::{
    create_lifecycle_table, CompilerTransitionError, Guard, GuardFn, GuardResult, SideEffect,
    SideEffectFn, SideEffectResult, TransitionBuilder, TransitionRule, TransitionTable,
    TransitionTableBuilder,
};

pub use lifecycle::{BlockedReason, LifecycleState, OperationalStatus, TransitionEvent};
pub use semantic_types::{AttemptNumber, InstanceState, NodeName, TimerId};
pub use transition::{
    apply, get_operational_status, get_valid_transitions, is_terminal, LeaseRecord, TransitionError,
};

#[cfg(test)]
mod tests_apply_errors;
#[cfg(test)]
mod tests_apply_happy;
#[cfg(test)]
mod tests_derives;
#[cfg(test)]
mod tests_helpers;
#[cfg(test)]
mod tests_lease;

#[cfg(feature = "proptest")]
mod tests_proptest;

#[cfg(test)]
mod tests_bdd_lifecycle;
