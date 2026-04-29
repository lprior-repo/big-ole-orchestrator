//! Timer wait-key and sleep state primitives for workflow timers.
//!
//! Defines the fundamental types representing a workflow waiting on a timer,
//! including wake-up keys, timestamps, and the 'Sleeping' state representation.
//!
//! # Architecture: Data -> Calc -> Actions
//!
//! - **Data**: `TimerWaitKey`, `SleepState`, `TimerError`
//! - **Calc**: `compute_fire_at`, `validate_sleep_duration`, `is_timer_expired`
//! - **Actions**: None (pure data and calculation module)

use vo_types::InstanceId;

// =============================================================================
// Error Types
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TimerError {
    #[error("sleep duration must be positive, got {duration_ms}ms")]
    ZeroOrNegativeDuration { duration_ms: i64 },
    #[error("timer wait-key cannot be empty")]
    EmptyWaitKey,
    #[error("timer wait-key exceeds 256 characters: {len}")]
    WaitKeyTooLong { len: usize },
    #[error(
        "fire-at timestamp overflow: duration {duration_ms}ms from base {base_ms}ms exceeds u64"
    )]
    TimestampOverflow { base_ms: u64, duration_ms: u64 },
    #[error("timer already expired at fire_at={fire_at_ms}ms, now={now_ms}ms")]
    AlreadyExpired { fire_at_ms: u64, now_ms: u64 },
}

// =============================================================================
// Data: TimerWaitKey
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TimerWaitKey(String);

impl TimerWaitKey {
    pub fn parse(input: &str) -> Result<Self, TimerError> {
        if input.is_empty() {
            return Err(TimerError::EmptyWaitKey);
        }
        if input.len() > 256 {
            return Err(TimerError::WaitKeyTooLong { len: input.len() });
        }
        Ok(Self(input.to_string()))
    }

    pub fn new_unchecked(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn for_timer(
        instance_id: &InstanceId,
        timer_id: &vo_types::TimerId,
    ) -> Result<Self, TimerError> {
        Self::parse(&format!(
            "timer:{}:{}",
            instance_id.as_str(),
            timer_id.as_str()
        ))
    }
}

// =============================================================================
// Data: SleepState
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SleepState {
    pub instance_id: InstanceId,
    pub wait_key: TimerWaitKey,
    pub fire_at_ms: u64,
    pub scheduled_at_ms: u64,
}

impl SleepState {
    pub fn new(
        instance_id: InstanceId,
        wait_key: TimerWaitKey,
        fire_at_ms: u64,
        scheduled_at_ms: u64,
    ) -> Result<Self, TimerError> {
        if fire_at_ms == 0 {
            return Err(TimerError::ZeroOrNegativeDuration { duration_ms: 0 });
        }
        if scheduled_at_ms == 0 {
            return Err(TimerError::ZeroOrNegativeDuration { duration_ms: 0 });
        }
        if fire_at_ms < scheduled_at_ms {
            return Err(TimerError::ZeroOrNegativeDuration {
                duration_ms: (fire_at_ms as i64) - (scheduled_at_ms as i64),
            });
        }
        Ok(Self {
            instance_id,
            wait_key,
            fire_at_ms,
            scheduled_at_ms,
        })
    }

    pub fn duration_ms(&self) -> u64 {
        self.fire_at_ms.saturating_sub(self.scheduled_at_ms)
    }

    pub fn remaining_ms(&self, now_ms: u64) -> u64 {
        self.fire_at_ms.saturating_sub(now_ms)
    }

    pub fn is_expired(&self, now_ms: u64) -> bool {
        self.fire_at_ms <= now_ms
    }
}

// =============================================================================
// Calculations: Pure Functions
// =============================================================================

pub fn validate_sleep_duration(duration_ms: i64) -> Result<u64, TimerError> {
    if duration_ms <= 0 {
        return Err(TimerError::ZeroOrNegativeDuration { duration_ms });
    }
    Ok(duration_ms as u64)
}

pub fn compute_fire_at(base_ms: u64, duration_ms: u64) -> Result<u64, TimerError> {
    base_ms
        .checked_add(duration_ms)
        .ok_or(TimerError::TimestampOverflow {
            base_ms,
            duration_ms,
        })
}

pub fn is_timer_expired(fire_at_ms: u64, now_ms: u64) -> bool {
    fire_at_ms <= now_ms
}

pub fn create_sleep_state(
    instance_id: InstanceId,
    timer_id: &vo_types::TimerId,
    now_ms: u64,
    duration_ms: i64,
) -> Result<SleepState, TimerError> {
    let valid_duration = validate_sleep_duration(duration_ms)?;
    let fire_at_ms = compute_fire_at(now_ms, valid_duration)?;
    let wait_key = TimerWaitKey::for_timer(&instance_id, timer_id)?;
    SleepState::new(instance_id, wait_key, fire_at_ms, now_ms)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_instance_id() -> InstanceId {
        InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap()
    }

    fn test_timer_id() -> vo_types::TimerId {
        vo_types::TimerId::parse("timer-abc-123").unwrap()
    }

    // -------------------------------------------------------------------------
    // TimerWaitKey
    // -------------------------------------------------------------------------

    #[test]
    fn timer_wait_key_parse_valid() {
        let key = TimerWaitKey::parse("my-timer").unwrap();
        assert_eq!(key.as_str(), "my-timer");
    }

    #[test]
    fn timer_wait_key_parse_rejects_empty() {
        let result = TimerWaitKey::parse("");
        assert!(matches!(result, Err(TimerError::EmptyWaitKey)));
    }

    #[test]
    fn timer_wait_key_parse_rejects_too_long() {
        let long = "x".repeat(257);
        let result = TimerWaitKey::parse(&long);
        assert!(matches!(
            result,
            Err(TimerError::WaitKeyTooLong { len: 257 })
        ));
    }

    #[test]
    fn timer_wait_key_parse_accepts_max_length() {
        let max = "x".repeat(256);
        let key = TimerWaitKey::parse(&max).unwrap();
        assert_eq!(key.as_str().len(), 256);
    }

    #[test]
    fn timer_wait_key_for_timer_constructs_correctly() {
        let key = TimerWaitKey::for_timer(&test_instance_id(), &test_timer_id()).unwrap();
        let s = key.as_str();
        assert!(s.starts_with("timer:"));
        assert!(s.contains("01H5JYV4XHGSR2F8KZ9BWNRFMA"));
        assert!(s.contains("timer-abc-123"));
    }

    #[test]
    fn timer_wait_key_new_unchecked_bypasses_validation() {
        let key = TimerWaitKey::new_unchecked("");
        assert_eq!(key.as_str(), "");
    }

    #[test]
    fn timer_wait_key_ordering() {
        let a = TimerWaitKey::parse("alpha").unwrap();
        let b = TimerWaitKey::parse("beta").unwrap();
        assert!(a < b);
    }

    #[test]
    fn timer_wait_key_equality() {
        let a = TimerWaitKey::parse("same-key").unwrap();
        let b = TimerWaitKey::parse("same-key").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn timer_wait_key_hash_consistency() {
        use std::collections::HashSet;
        let a = TimerWaitKey::parse("key-1").unwrap();
        let b = TimerWaitKey::parse("key-1").unwrap();
        let mut set = HashSet::new();
        set.insert(a.clone());
        assert!(set.contains(&b));
    }

    // -------------------------------------------------------------------------
    // SleepState
    // -------------------------------------------------------------------------

    #[test]
    fn sleep_state_new_valid() {
        let state = SleepState::new(
            test_instance_id(),
            TimerWaitKey::parse("t").unwrap(),
            2000,
            1000,
        )
        .unwrap();
        assert_eq!(state.fire_at_ms, 2000);
        assert_eq!(state.scheduled_at_ms, 1000);
        assert_eq!(state.duration_ms(), 1000);
    }

    #[test]
    fn sleep_state_new_rejects_zero_fire_at() {
        let result = SleepState::new(
            test_instance_id(),
            TimerWaitKey::parse("t").unwrap(),
            0,
            1000,
        );
        assert!(matches!(
            result,
            Err(TimerError::ZeroOrNegativeDuration { .. })
        ));
    }

    #[test]
    fn sleep_state_new_rejects_zero_scheduled_at() {
        let result = SleepState::new(
            test_instance_id(),
            TimerWaitKey::parse("t").unwrap(),
            1000,
            0,
        );
        assert!(matches!(
            result,
            Err(TimerError::ZeroOrNegativeDuration { .. })
        ));
    }

    #[test]
    fn sleep_state_new_rejects_fire_before_scheduled() {
        let result = SleepState::new(
            test_instance_id(),
            TimerWaitKey::parse("t").unwrap(),
            500,
            1000,
        );
        assert!(matches!(
            result,
            Err(TimerError::ZeroOrNegativeDuration { duration_ms: -500 })
        ));
    }

    #[test]
    fn sleep_state_remaining_ms() {
        let state = SleepState::new(
            test_instance_id(),
            TimerWaitKey::parse("t").unwrap(),
            5000,
            1000,
        )
        .unwrap();
        assert_eq!(state.remaining_ms(3000), 2000);
        assert_eq!(state.remaining_ms(5000), 0);
        assert_eq!(state.remaining_ms(6000), 0);
    }

    #[test]
    fn sleep_state_is_expired() {
        let state = SleepState::new(
            test_instance_id(),
            TimerWaitKey::parse("t").unwrap(),
            5000,
            1000,
        )
        .unwrap();
        assert!(!state.is_expired(4000));
        assert!(state.is_expired(5000));
        assert!(state.is_expired(6000));
    }

    #[test]
    fn sleep_state_equality() {
        let iid = test_instance_id();
        let wk = TimerWaitKey::parse("t").unwrap();
        let a = SleepState::new(iid.clone(), wk.clone(), 2000, 1000).unwrap();
        let b = SleepState::new(iid, wk, 2000, 1000).unwrap();
        assert_eq!(a, b);
    }

    // -------------------------------------------------------------------------
    // Pure Calculations
    // -------------------------------------------------------------------------

    #[test]
    fn validate_sleep_duration_accepts_positive() {
        assert_eq!(validate_sleep_duration(100).unwrap(), 100);
        assert_eq!(validate_sleep_duration(1).unwrap(), 1);
    }

    #[test]
    fn validate_sleep_duration_rejects_zero() {
        let result = validate_sleep_duration(0);
        assert!(matches!(
            result,
            Err(TimerError::ZeroOrNegativeDuration { duration_ms: 0 })
        ));
    }

    #[test]
    fn validate_sleep_duration_rejects_negative() {
        let result = validate_sleep_duration(-5);
        assert!(matches!(
            result,
            Err(TimerError::ZeroOrNegativeDuration { duration_ms: -5 })
        ));
    }

    #[test]
    fn compute_fire_at_basic() {
        assert_eq!(compute_fire_at(1000, 500).unwrap(), 1500);
    }

    #[test]
    fn compute_fire_at_overflow() {
        let result = compute_fire_at(u64::MAX, 1);
        assert!(matches!(result, Err(TimerError::TimestampOverflow { .. })));
    }

    #[test]
    fn compute_fire_at_max_safe() {
        assert_eq!(compute_fire_at(u64::MAX - 1, 1).unwrap(), u64::MAX);
    }

    #[test]
    fn is_timer_expired_boundary() {
        assert!(is_timer_expired(1000, 1000));
        assert!(is_timer_expired(999, 1000));
        assert!(!is_timer_expired(1001, 1000));
    }

    #[test]
    fn create_sleep_state_full_pipeline() {
        let state = create_sleep_state(test_instance_id(), &test_timer_id(), 1000, 500).unwrap();
        assert_eq!(state.fire_at_ms, 1500);
        assert_eq!(state.scheduled_at_ms, 1000);
        assert_eq!(state.duration_ms(), 500);
        assert!(state.wait_key.as_str().starts_with("timer:"));
    }

    #[test]
    fn create_sleep_state_rejects_zero_duration() {
        let result = create_sleep_state(test_instance_id(), &test_timer_id(), 1000, 0);
        assert!(matches!(
            result,
            Err(TimerError::ZeroOrNegativeDuration { .. })
        ));
    }

    #[test]
    fn create_sleep_state_rejects_negative_duration() {
        let result = create_sleep_state(test_instance_id(), &test_timer_id(), 1000, -100);
        assert!(matches!(
            result,
            Err(TimerError::ZeroOrNegativeDuration { .. })
        ));
    }

    #[test]
    fn create_sleep_state_rejects_overflow() {
        let result = create_sleep_state(test_instance_id(), &test_timer_id(), u64::MAX, 1);
        assert!(matches!(result, Err(TimerError::TimestampOverflow { .. })));
    }

    // -------------------------------------------------------------------------
    // Acceptance: construct timer wait-keys and calculate valid expirations
    // -------------------------------------------------------------------------

    #[test]
    fn test_construct_timer_wait_keys_and_calculate_valid_expirations() {
        let iid = test_instance_id();
        let tid = test_timer_id();
        let now_ms = 1_000_000u64;

        let wk = TimerWaitKey::for_timer(&iid, &tid).unwrap();
        assert!(!wk.as_str().is_empty());

        let dur = validate_sleep_duration(5000).unwrap();
        let fire_at = compute_fire_at(now_ms, dur).unwrap();
        assert_eq!(fire_at, 1_005_000);

        let state = create_sleep_state(iid, &tid, now_ms, 5000).unwrap();
        assert_eq!(state.fire_at_ms, 1_005_000);
        assert!(!state.is_expired(now_ms));
        assert!(state.is_expired(1_005_000));
    }

    // -------------------------------------------------------------------------
    // Acceptance: reject invalid or completely malformed timestamps
    // -------------------------------------------------------------------------

    #[test]
    fn test_reject_invalid_or_completely_malformed_timestamps() {
        assert!(validate_sleep_duration(0).is_err());
        assert!(validate_sleep_duration(-1).is_err());
        assert!(validate_sleep_duration(-99999).is_err());
        assert!(compute_fire_at(u64::MAX, 1).is_err());
        assert!(TimerWaitKey::parse("").is_err());
        assert!(TimerWaitKey::parse(&"x".repeat(257)).is_err());
        assert!(SleepState::new(
            test_instance_id(),
            TimerWaitKey::parse("t").unwrap(),
            0,
            1000
        )
        .is_err());
        assert!(SleepState::new(
            test_instance_id(),
            TimerWaitKey::parse("t").unwrap(),
            500,
            1000
        )
        .is_err());
    }

    // -------------------------------------------------------------------------
    // Error display
    // -------------------------------------------------------------------------

    #[test]
    fn timer_error_display_messages() {
        assert!(
            format!("{}", TimerError::ZeroOrNegativeDuration { duration_ms: 0 })
                .contains("positive")
        );
        assert!(format!("{}", TimerError::EmptyWaitKey).contains("empty"));
        assert!(format!("{}", TimerError::WaitKeyTooLong { len: 300 }).contains("256"));
        assert!(format!(
            "{}",
            TimerError::TimestampOverflow {
                base_ms: 1,
                duration_ms: 2
            }
        )
        .contains("overflow"));
        assert!(format!(
            "{}",
            TimerError::AlreadyExpired {
                fire_at_ms: 1,
                now_ms: 2
            }
        )
        .contains("expired"));
    }

    // -------------------------------------------------------------------------
    // Proptest invariants
    // -------------------------------------------------------------------------

    #[cfg(feature = "proptest")]
    mod proptest_invariants {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn compute_fire_at_never_wraps(base in 0u64..(u64::MAX - 999_999), dur in 1u64..1_000_000u64) {
                let result = compute_fire_at(base, dur);
                prop_assert!(result.is_ok());
                let fire_at = result.unwrap();
                prop_assert!(fire_at >= base);
                prop_assert!(fire_at <= u64::MAX - 1);
            }

            #[test]
            fn validate_sleep_duration_positive_only(dur in -1_000_000i64..1_000_000i64) {
                let result = validate_sleep_duration(dur);
                if dur > 0 {
                    prop_assert!(result.is_ok());
                    prop_assert_eq!(result.unwrap(), dur as u64);
                } else {
                    prop_assert!(result.is_err());
                }
            }

            #[test]
            fn sleep_state_remaining_never_negative(fire in 1u64..1_000_000_000u64, scheduled in 0u64..999_999_999u64, now in 0u64..2_000_000_000u64) {
                let state = SleepState::new(
                    test_instance_id(),
                    TimerWaitKey::parse("t").unwrap(),
                    fire,
                    scheduled,
                );
                if let Ok(s) = state {
                    let remaining = s.remaining_ms(now);
                    prop_assert!(remaining <= fire);
                }
            }

            #[test]
            fn create_sleep_state_rejects_negative_durations(dur in -1_000_000i64..0i64) {
                let result = create_sleep_state(test_instance_id(), &test_timer_id(), 1_000_000, dur);
                prop_assert!(result.is_err());
            }

            #[test]
            fn timer_wait_key_hash_consistency(key1 in "[a-z]{1,256}", key2 in "[a-z]{1,256}") {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let k1 = TimerWaitKey::parse(&key1);
                let k2 = TimerWaitKey::parse(&key2);
                if k1.is_ok() && k2.is_ok() {
                    let k1 = k1.unwrap();
                    let k2 = k2.unwrap();
                    if k1 == k2 {
                        let mut h1 = DefaultHasher::new();
                        let mut h2 = DefaultHasher::new();
                        k1.hash(&mut h1);
                        k2.hash(&mut h2);
                        prop_assert_eq!(h1.finish(), h2.finish());
                    }
                }
            }
        }
    }
}
