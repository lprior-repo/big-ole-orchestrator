//! Time calculation helpers for lease expiration.
//!
//! Pure functions for determining lease expiry and calculating
//! expiration timestamps from TTL values.

// ============================================================================
// Calc layer — time helpers
// ============================================================================

#[must_use]
pub const fn is_expired(expires_at_ms: u64, now_ms: u64) -> bool {
    now_ms >= expires_at_ms
}

pub fn calc_expires(now_ms: u64, ttl_ms: u64) -> Result<u64, super::LeaseError> {
    if ttl_ms == 0 {
        return Err(super::LeaseError::ZeroTtl);
    }
    now_ms
        .checked_add(ttl_ms)
        .ok_or(super::LeaseError::FenceTokenExhausted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calc::LeaseError;

    #[test]
    fn is_expired_returns_false_when_time_before_expiry() {
        assert!(!is_expired(6000, 5000));
    }

    #[test]
    fn is_expired_returns_true_when_time_at_expiry() {
        assert!(is_expired(6000, 6000));
    }

    #[test]
    fn is_expired_returns_true_when_time_after_expiry() {
        assert!(is_expired(6000, 7000));
    }

    #[test]
    fn calc_expires_returns_correct_timestamp() {
        let result = calc_expires(1000, 5000).unwrap();
        assert_eq!(result, 6000);
    }

    #[test]
    fn calc_expires_returns_zero_ttl_error() {
        let result = calc_expires(1000, 0);
        assert_eq!(result, Err(LeaseError::ZeroTtl));
    }

    #[test]
    fn calc_expires_returns_overflow_error() {
        let result = calc_expires(u64::MAX, 1);
        assert_eq!(result, Err(LeaseError::FenceTokenExhausted));
    }

    #[test]
    fn calc_expires_with_large_ttl_no_overflow() {
        let result = calc_expires(1000, u64::MAX - 1000).unwrap();
        assert_eq!(result, u64::MAX);
    }
}
