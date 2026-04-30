//! Hierarchical lifecycle model for vo-actor (ADR-039).
//!
//! Provides lifecycle states, parent-child relationships, and graceful shutdown
//! propagation for the actor hierarchy.
//!
//! # Lifecycle States
//!
//! - `Pending`: Actor created but not yet started
//! - `Running`: Actor is actively processing
//! - `Stopping`: Actor is initiating graceful shutdown
//! - `Stopped`: Actor has completed shutdown
//! - `Failed`: Actor encountered an unrecoverable error
//!
//! # Hierarchy
//!
//! Actors exist in a parent-child relationship where:
//! - Parent actors supervise child actors
//! - Shutdown propagates hierarchically from parent to children
//! - Children must stop before parent can complete shutdown

pub mod child_registry;
pub mod failures;
pub mod ordered_drop;
pub mod shutdown;
pub mod state;
pub mod transition;

// Re-export top-level symbols for backward compatibility
pub use child_registry::{ChildInfo, ParentChildRegistry};
pub use failures::{compute_failure_outcome, FailureOutcome};
pub use ordered_drop::{DropAction, MaybeDoneAction, OrderedDropRegistry, ShutdownOrder};
pub use shutdown::{ShutdownPropagator, ShutdownResult};
pub use state::{ActorLifecycleState, LifecycleTransition};
pub use transition::{compute_next_state, is_valid_transition, LifecycleError};
