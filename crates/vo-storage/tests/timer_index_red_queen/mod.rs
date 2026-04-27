#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::redundant_pattern_matching)]
//! Red Queen adversarial tests for the timer_index partition.
//!
//! These tests attempt to break the implementation through:
//! - Key encoding attacks (invalid lengths, boundary values)
//! - Value validation attacks (zero duration)
//! - Dual-clock invariant violations
//! - Timer set validation edge cases
//! - Scan due timers boundary conditions
//! - Delete operations under various conditions
//!
//! bead_id: ve-c45
//! bead_title: RED QUEEN: nitro test 2
//! module: timer_index (12 attack vectors)

mod helpers;
mod key_value_validation;
mod scan_tests;
mod timer_set_tests;
mod crash_recovery_tests;
