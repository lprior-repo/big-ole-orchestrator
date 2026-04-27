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

mod av01_timer_key;
mod av02_timer_value;
mod av03_timer_record;
mod av04_timer_set;
mod av05_scan_due_timers;
mod av06_timer_delete;
mod av07_multiple_timers;
mod av08_scan_all_timers;
mod av09_crash_recovery;
mod av10_cancellation;
mod helpers;

#[cfg(test)]
mod tests {
    mod av01_timer_key;
    mod av02_timer_value;
    mod av03_timer_record;
    mod av04_timer_set;
    mod av05_scan_due_timers;
    mod av06_timer_delete;
    mod av07_multiple_timers;
    mod av08_scan_all_timers;
    mod av09_crash_recovery;
    mod av10_cancellation;
}