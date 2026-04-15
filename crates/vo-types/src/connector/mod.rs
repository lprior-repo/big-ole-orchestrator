//! Connector runtime types for managed effects (ADR-041).
//!
//! Architecture: Data (ConnectorState, ConnectorResult, ReconcileAction)
//!             → Calc (apply_connector_transition, is_terminal, all_variants).
//!             → Runtime (Connector trait, reconcile_ambiguous).
//!
//! This module defines the type system for the managed connector lifecycle.
//! No I/O, no engine integration — pure types and state machine logic.
//!
//! # Ambiguity Handling (ADR-041 §3)
//!
//! When a connector operation returns `Ambiguous` (timeout with unknown server state),
//! the system routes through reconciliation rather than blindly retrying. This prevents
//! duplicate commits when the original operation actually succeeded.

mod runtime;
mod transition;
mod types;

#[cfg(test)]
mod runtime_tests;
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
pub use runtime::{
    reconcile_ambiguous, execute_with_reconciliation, Connector, ConnectorError,
    ReconciliationResult,
};
pub use transition::apply_connector_transition;
pub use types::{
    ConnectorResult, ConnectorState, ConnectorTransition, ConnectorTransitionError, ReconcileAction,
};
