//! Timer supervisor pure calculation functions
//!
//! Contains pure functions that implement the core timer logic without side effects.
//!
//! Per the Data → Calc → Actions pattern, these functions perform calculations
//! but do not mutate state or interact with external systems.

use vo_types::TimestampMs;

// =============================================================================
// Pure Calculation Functions (Data → Calc → Actions)
// =============================================================================

/// `verify_dual_clock` - Simplified dual-clock verification
///
/// Returns true if `fire_at_ms` <= `now_ms`
///
/// Note: The unified TimerRecord doesn't include trigger_time_ms and duration_ms.
/// The dual-clock verification has been simplified to single-clock (wall clock only)
/// as per the unified TimerStorage design.
///
/// This function is a pure calculation with no side effects.
///
/// # Arguments
/// * `fire_at_ms` - Absolute fire time (TimestampMs)
/// * `now_ms` - Current time (TimestampMs)
///
/// # Returns
/// `true` if timer should fire based on wall clock
#[inline]
#[must_use]
pub fn verify_dual_clock(fire_at_ms: TimestampMs, now_ms: TimestampMs) -> bool {
    fire_at_ms <= now_ms
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
pub fn is_overdue(fire_at_ms: TimestampMs, now_ms: TimestampMs, tick_interval_ms: u64) -> bool {
    let fire_at_u64 = fire_at_ms.as_u64();
    let now_u64 = now_ms.as_u64();
    fire_at_u64.saturating_add(tick_interval_ms) < now_u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_dual_clock_returns_true_when_fire_at_le_now() {
        let fire_at = TimestampMs::new_unchecked(1000);
        let now = TimestampMs::new_unchecked(1000);
        assert!(verify_dual_clock(fire_at, now));
    }

    #[test]
    fn verify_dual_clock_returns_false_when_fire_at_gt_now() {
        let fire_at = TimestampMs::new_unchecked(1500);
        let now = TimestampMs::new_unchecked(1000);
        assert!(!verify_dual_clock(fire_at, now));
    }

    #[test]
    fn is_overdue_returns_true_when_over_tick_interval() {
        let fire_at = TimestampMs::new_unchecked(1000);
        let now = TimestampMs::new_unchecked(1200);
        assert!(is_overdue(fire_at, now, 100));
    }

    #[test]
    fn is_overdue_returns_false_when_within_tick_interval() {
        let fire_at = TimestampMs::new_unchecked(1000);
        let now = TimestampMs::new_unchecked(1099);
        assert!(!is_overdue(fire_at, now, 100));
    }

    #[test]
    fn is_overdue_returns_false_at_boundary() {
        let fire_at = TimestampMs::new_unchecked(1000);
        let now = TimestampMs::new_unchecked(1100);
        assert!(!is_overdue(fire_at, now, 100));
    }
}
