//! Declarative State Machine Compiler
//!
//! Provides a declarative `TransitionTable` with builder API, guard conditions,
//! side effects, and DOT visualization output.

mod definition;
mod transitions;
mod validation;

pub use definition::{
    CompilerTransitionError, TransitionBuilder, TransitionRule, TransitionTable,
    TransitionTableBuilder,
};
pub use transitions::create_lifecycle_table;
pub use validation::{
    allows_recovery, is_valid_transition, Guard, GuardFn, GuardResult, SideEffect, SideEffectFn,
    SideEffectResult,
};
