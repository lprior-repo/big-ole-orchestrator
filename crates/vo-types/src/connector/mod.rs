//! Connector runtime types for managed effects (ADR-041).
//!
//! Architecture: Data (ConnectorState, ConnectorResult, ReconcileAction)
//!             → Calc (apply_connector_transition, is_terminal, all_variants).
//!
//! This module defines the type system for the managed connector lifecycle.
//! No I/O, no engine integration — pure types and state machine logic.

mod transition;
mod types;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod transition_tests;
#[cfg(test)]
mod type_derive_tests;

#[cfg(feature = "proptest")]
mod proptests;

#[cfg(kani)]
mod verification;

// Re-export all public API items
pub use transition::apply_connector_transition;
pub use types::{
    ConnectorResult, ConnectorState, ConnectorTransition, ConnectorTransitionError, ReconcileAction,
};
