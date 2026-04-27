//! Red Queen adversarial tests for `effect_journal`.
//!
//! These tests attempt to find bugs by violating contracts and testing edge cases.
//!
//! Organization:
//! - `effect_id` — `EffectId` construction and shape tests
//! - `codec` — key and record codec tests
//! - `journal` — journal lifecycle and idempotency tests
//! - `crash_recovery` — crash after prepare recovery tests
//! - `cross_instance_isolation` — cross-instance isolation tests
//! - `idempotency_stress` — idempotency stress tests
//! - `data_corruption` — data corruption rejection tests
//! - `concurrent_access` — concurrent access tests
//! - `boundary_conditions` — unicode, long intent_id, special JSON tests
//! - `state_machine` — state machine exhaustive transition tests
//! - `partition_integrity` — partition constant integrity tests
//! - `compact` — compact functionality tests

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

mod boundary_conditions;
mod compact;
mod concurrent_access;
mod crash_recovery;
mod cross_instance_isolation;
mod data_corruption;
mod effect_id;
mod idempotency_stress;
mod journal;
mod partition_integrity;
mod state_machine;