//! Deterministic event-sourced replay engine (ADR-027).
//!
//! Replays event sequences through the pure `apply()` state machine
//! to reconstruct `LifecycleState` from event history.

mod engine;
pub mod event_sourcing_engine;
#[cfg(test)]
pub mod event_sourcing_engine_tests;
pub mod projection;
mod types;

#[cfg(test)]
mod adr035_event_versioning_tests;
#[cfg(test)]
mod crash_injection_tests;
#[cfg(test)]
mod deterministic_replay_tests;
#[cfg(test)]
mod error_propagation_tests;
#[cfg(test)]
mod error_tests;
#[cfg(test)]
mod event_ordering_tests;
#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod kani_proptests;
#[cfg(test)]
mod red_queen_adversarial_tests;
#[cfg(test)]
mod stale_event_rejection_tests;
#[cfg(test)]
pub mod test_helpers;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod upcaster_tests;

pub use engine::ReplayEngine;
pub use types::{ReplayError, ReplayErrorKind, ReplayResult};
