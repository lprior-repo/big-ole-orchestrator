//! Pure lease state transitions (ADR-039).
//!
//! Architecture: Data (`LeaseState`, `LeaseTransition`, `LeaseError`)
//!            → Calc (`apply`, helper predicates)
//!            → Actions (none — this module is pure).
//!
//! Invariant: A lease cannot be acquired by Node B if held by Node A and
//! unexpired. Only the holding node may renew. Time-based expiration is
//! determined through pure chronological calculation.

pub mod calc;
pub mod tests;
pub mod types;

pub use calc::apply;
pub use types::{LeaseError, LeaseState, LeaseTransition};
