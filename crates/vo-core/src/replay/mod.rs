//! Deterministic event-sourced replay engine (ADR-027).
//!
//! Replays event sequences through the pure `apply()` state machine
//! to reconstruct `LifecycleState` from event history.

mod engine;
mod types;

#[cfg(test)]
mod error_tests;
#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod kani_proptests;
#[cfg(test)]
mod red_queen_adversarial_tests;
#[cfg(test)]
mod test_helpers;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod upcaster_tests;

pub use engine::ReplayEngine;
pub use types::{ReplayError, ReplayResult};
