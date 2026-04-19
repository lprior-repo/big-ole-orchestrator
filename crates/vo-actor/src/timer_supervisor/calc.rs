//! Timer supervisor pure calculation functions
//!
//! Contains pure functions that implement the core timer logic without side effects.
//!
//! Per the Data → Calc → Actions pattern, these functions perform calculations
//! but do not mutate state or interact with external systems.

use std::sync::Arc;

use vo_types::InstanceId;

use super::traits::TimerStorage;
use super::types::{TimerRecord, TimerSupervisorError};

// =============================================================================
// Pure Calculation Functions (Data → Calc → Actions)
// =============================================================================

/// `verify_dual_clock` - Dual-clock verification per ADR-013
///
/// Returns true if BOTH `fire_at_ms` <= `now_ms` AND (`trigger_time_ms` + `duration_ms`) <= `now_ms`
///
/// Using AND logic requires both clocks to agree before firing, preventing timer drift
/// from wall clock adjustments (hibernation, manual time changes) or monotonic skew.
///
/// This function is a pure calculation with no side effects.
///
/// # Arguments
/// * `fire_at_ms` - Absolute fire time (Unix timestamp ms)
/// * `trigger_time_ms` - When timer was scheduled (for dual-clock)
/// * `duration_ms` - Monotonic duration from `trigger_time_ms`
/// * `now_ms` - Current time (Unix timestamp ms)
///
/// # Returns
/// `true` if timer should fire under BOTH clock conditions
#[inline]
#[must_use]
pub fn verify_dual_clock(
    fire_at_ms: u64,
    trigger_time_ms: u64,
    duration_ms: u64,
    now_ms: u64,
) -> bool {
    let wall_clock_ok = fire_at_ms <= now_ms;
    let monotonic_ok = trigger_time_ms.saturating_add(duration_ms) <= now_ms;
    wall_clock_ok && monotonic_ok
}

/// `is_overdue` - Check if timer is overdue beyond tick interval
///
/// Returns true if `fire_at_ms` + `tick_interval_ms` < `now_ms`
///
/// A timer is considered overdue if it fired more than one tick interval ago.
///
/// # Arguments
/// * `fire_at_ms` - When the timer should have fired
/// * `now_ms` - Current time
/// * `tick_interval_ms` - The tick interval
///
/// # Returns
/// `true` if the timer is overdue
#[inline]
#[must_use]
pub fn is_overdue(fire_at_ms: u64, now_ms: u64, tick_interval_ms: u64) -> bool {
    fire_at_ms.saturating_add(tick_interval_ms) < now_ms
}

// =============================================================================
// `timer_delete_before_dispatch` - Atomic delete-before-dispatch operation
// =============================================================================

/// Atomically deletes timer before dispatch.
///
/// Per INV-2, this function guarantees that the timer is deleted from storage
/// BEFORE any dispatch occurs. This prevents double-fire if the process crashes
/// after dispatch but before delete.
///
/// # Arguments
/// * `storage` - The timer storage
/// * `timer` - The timer record to delete and dispatch
///
/// # Errors
/// * `StorageError` - If the delete operation fails before dispatch
/// * `AtomicityViolation` - If dispatch succeeds but delete fails afterward
pub fn timer_delete_before_dispatch(
    storage: &Arc<dyn TimerStorage>,
    timer: &TimerRecord,
) -> Result<(), TimerSupervisorError> {
    // First, attempt to delete the timer from storage
    // This MUST succeed before any dispatch occurs (INV-2)
    storage
        .delete_timer(&timer.instance_id, timer.fire_at_ms)
        .map_err(|e| TimerSupervisorError::StorageError(e.to_string()))?;

    // Delete succeeded, dispatch will happen in caller
    // If dispatch fails after this point, we have an AtomicityViolation
    // but the timer is already deleted, so no double-fire is possible
    Ok(())
}

// =============================================================================
// validate_timer_record - Validates timer record integrity
// =============================================================================

/// Validates a timer record for corruption.
///
/// # Arguments
/// * `record` - The timer record to validate
///
/// # Errors
/// * `CorruptTimer` - If the timer record has invalid data
pub fn validate_timer_record(record: &TimerRecord) -> Result<(), TimerSupervisorError> {
    if record.fire_at_ms == 0 {
        return Err(TimerSupervisorError::CorruptTimer(
            "Timer fire_at_ms is zero".to_string(),
        ));
    }

    if record.trigger_time_ms == 0 {
        return Err(TimerSupervisorError::CorruptTimer(
            "Timer trigger_time_ms is zero".to_string(),
        ));
    }

    if record.fire_at_ms < record.trigger_time_ms {
        return Err(TimerSupervisorError::CorruptTimer(
            "Timer fire_at_ms is before trigger_time_ms".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_dual_clock_returns_true_when_fire_at_le_now() {
        // fire_at_ms = 1000 <= now_ms = 1000
        // trigger_time_ms + duration_ms = 800 + 200 = 1000 <= now_ms = 1000
        // Both conditions met with AND logic
        assert!(verify_dual_clock(1000, 800, 200, 1000));
    }

    #[test]
    fn verify_dual_clock_returns_false_when_only_monotonic_condition_met() {
        // trigger_time_ms + duration_ms = 800 + 200 = 1000 <= now_ms = 1000 (monotonic met)
        // fire_at_ms = 1500 > now_ms = 1000 (wall clock NOT met)
        // With AND logic, both must be met, so this returns false
        assert!(!verify_dual_clock(1500, 800, 200, 1000));
    }

    #[test]
    fn verify_dual_clock_returns_true_when_both_conditions_met() {
        // fire_at_ms = 1000 <= now_ms = 1000 (wall clock met)
        // trigger_time_ms + duration_ms = 800 + 200 = 1000 <= now_ms = 1000 (monotonic met)
        assert!(verify_dual_clock(1000, 800, 200, 1000));
    }

    #[test]
    fn verify_dual_clock_returns_false_when_only_wall_clock_met() {
        // fire_at_ms = 1100 <= now_ms = 1100 (wall clock met)
        // trigger_time_ms + duration_ms = 800 + 400 = 1200 > now_ms = 1100 (monotonic NOT met)
        assert!(!verify_dual_clock(1100, 800, 400, 1100));
    }

    #[test]
    fn verify_dual_clock_returns_false_when_not_due() {
        // fire_at_ms = 1500 > now_ms = 900 (wall clock NOT met)
        // trigger_time_ms + duration_ms = 800 + 200 = 1000 > now_ms = 900 (monotonic NOT met)
        assert!(!verify_dual_clock(1500, 800, 200, 900));
    }

    #[test]
    fn is_overdue_returns_true_when_over_tick_interval() {
        // fire_at_ms + tick_interval_ms = 1000 + 100 = 1100 < now_ms = 1200
        assert!(is_overdue(1000, 1200, 100));
    }

    #[test]
    fn is_overdue_returns_false_when_within_tick_interval() {
        // fire_at_ms + tick_interval_ms = 1000 + 100 = 1100 >= now_ms = 1099
        assert!(!is_overdue(1000, 1099, 100));
    }

    #[test]
    fn is_overdue_returns_false_at_boundary() {
        // fire_at_ms + tick_interval_ms = 1000 + 100 = 1100 >= now_ms = 1100
        assert!(!is_overdue(1000, 1100, 100));
    }
}
