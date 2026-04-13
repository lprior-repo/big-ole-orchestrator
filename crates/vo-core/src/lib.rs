//! Core engine implementation for vo-engine.
//!
//! Contains the main workflow engine, execution engine, scheduler,
//! persistence layer, and state machine implementation.

pub mod admission;
pub mod circuit_breaker;
pub mod config_hot_reload;
mod db_writer_message;
pub mod debounce;
pub mod exact_once_verification;

pub use exact_once_verification::assertions::{
    assert_fence_token_ordering, assert_invariant_no_orphans, assert_no_duplicate_effects,
    RecoveryAssertion, RecoveryAssertionError, RecoveryContext,
};
pub use exact_once_verification::crash_points::{CrashPoint, CrashPosition, CrashScenario};
pub use exact_once_verification::macros::CrashError;
pub use exact_once_verification::harness::{LineageRolloverEvent, LineageRoutingState, VerificationHarness};
pub use exact_once_verification::macros::{
    crash_injection, crash_injection_result, crash_injection_stress, crash_injection_wait,
    crash_invariant_assert, crash_point_matrix_tests,
};
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
