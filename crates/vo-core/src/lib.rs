//! Core engine implementation for vo-engine.
//!
//! This crate contains the main workflow engine, execution engine, scheduler,
//! persistence layer, and state machine implementation.
//!
//! # Key Modules
//!
//! - [`exact_once_verification`] - Crash recovery verification and assertion helpers
//!   - [`exact_once_verification::assertions`] - Invariant checking for recovery
//!   - [`exact_once_verification::crash_points`] - Injectable crash scenarios for testing
//!   - [`exact_once_verification::harness`] - Verification harness for lineage routing
//! - [`workflow_version`] - Workflow schema versioning and migration
//! - [`replay`] - Event sourcing and state replay logic
//! - [`snapshot_compat`] - Snapshot compatibility checks
//! - [`vault`] - Secure credential and secret management
//! - [`circuit_breaker`] - Fault tolerance pattern implementation
//! - [`resource_quota`] - Resource usage tracking and limits
//!
//! # Architecture
//!
//! The engine follows an event-sourcing architecture where workflow state is
//! determined by replaying the event sequence. The exact-once verification
//! subsystem ensures crash safety by validating that effects are applied
//! exactly once even across system failures.

pub mod admission;
pub mod circuit_breaker;
pub mod config_hot_reload;
pub mod connector;
mod db_writer_message;
pub mod debounce;
pub mod exact_once_verification;
pub mod lease_calc;

pub use exact_once_verification::assertions::{
    assert_fence_token_ordering, assert_invariant_no_orphans, assert_no_duplicate_effects,
    RecoveryAssertion, RecoveryAssertionError, RecoveryContext,
};
pub use exact_once_verification::crash_points::{CrashPoint, CrashPosition, CrashScenario};
pub use exact_once_verification::macros::CrashError;
pub use exact_once_verification::harness::{LineageRolloverEvent, LineageRoutingState, VerificationHarness};
pub mod quadtree;
pub mod replay;
pub mod resource_quota;
pub mod segment_tree;
pub mod snapshot_compat;
pub mod upcaster;
pub mod vault;
pub mod workflow_version;
pub mod workspace_swap;
pub mod workload_class;
pub mod write_class;

#[cfg(kani)]
pub mod write_class_verification;

#[cfg(test)]
mod invalid_business_data_tests;
