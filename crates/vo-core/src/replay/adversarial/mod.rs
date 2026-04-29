//! Adversarial tests for the replay engine (Red Queen).
//!
//! These tests probe edge cases, boundary conditions, and invalid inputs
//! to verify the replay engine handles adversarial conditions correctly.

use super::engine::ReplayEngine;
use super::test_helpers::*;
use super::types::ReplayError;
use vo_types::events::EventEnvelope;
use vo_types::state::LifecycleState;

#[cfg(test)]
mod adversarial_transitions;
#[cfg(test)]
mod corrupted_payload_injection;
#[cfg(test)]
mod exponential_blowup_scenarios;
#[cfg(test)]
mod max_history_depth_boundary;
#[cfg(test)]
mod mismatched_instance_id_injection;
#[cfg(test)]
mod memory_pressure_with_large_payloads;
#[cfg(test)]
mod random_position_corruption_injection;
#[cfg(test)]
mod aggressive_exponential_blowup;
#[cfg(test)]
mod memory_pressure_aggressive;
#[cfg(test)]
mod concurrency_adversarial;