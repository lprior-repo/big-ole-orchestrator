//! Declarative State Machine Compiler
//!
//! Provides a declarative TransitionTable with builder API, guard conditions,
//! side effects, and DOT visualization output.
//!
//! # Example
//!
//! ```rust
//! use vo-types::state::lifecycle::{LifecycleState, TransitionEvent};
//! use vo-types::state::compiler::{TransitionTable, Guard, SideEffect};
//!
//! let table = TransitionTable::builder()
//!     .add_transition(LifecycleState::Pending, TransitionEvent::AssignToNode)
//!         .to(LifecycleState::RunningDecision)
//!         .with_guard(Guard::Always)
//!         .with_side_effect(SideEffect::None)
//!         .build()
//!     .build();
//! ```

mod guard;
mod lifecycle_table;
mod side_effect;
mod transition_rule;
mod transition_table;

pub use guard::{Guard, GuardFn, GuardResult};
pub use lifecycle_table::create_lifecycle_table;
pub use side_effect::{SideEffect, SideEffectFn, SideEffectResult};
pub use transition_rule::TransitionRule;
pub use transition_table::{
    CompilerTransitionError, TransitionBuilder, TransitionTable, TransitionTableBuilder,
};

#[cfg(test)]
mod tests;
