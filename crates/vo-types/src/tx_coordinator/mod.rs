//! Transaction coordinator runtime types for distributed two-phase commit.
//!
//! This module defines the type system for coordinating distributed transactions
//! across multiple resources (connectors). No I/O, no engine integration —
//! pure types and state machine logic.

mod transition;
mod types;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod red_queen_tests;

<<<<<<< HEAD
#[cfg(test)]
mod integration_tests;

=======
>>>>>>> origin/polecat/synth-mnw6kj8v
#[cfg(feature = "proptest")]
mod proptests;

#[cfg(kani)]
mod verification;

// Re-export all public API items
pub use types::apply_coordinator_transition;
pub use types::{
    CoordinatorDecision, CoordinatorTransition, CoordinatorTransitionError, ParticipantRecord,
    ParticipantStatus, ParticipantVote, TransactionRecord, TransactionState,
};
