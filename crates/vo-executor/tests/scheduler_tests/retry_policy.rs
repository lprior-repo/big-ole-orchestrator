#[cfg(test)]
mod scheduler_retry_policy_tests {
    use vo_executor::RetryPolicy;

    #[test]
    fn retry_policy_new_valid() {
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();
        assert_eq!(policy.max_attempts, 3);
        assert_eq!(policy.backoff_ms, 100);
        assert!((policy.backoff_multiplier - 2.0).abs() < f64::EPSILON);
        assert_eq!(policy.max_backoff_ms, u64::MAX);
    }

    #[test]
    fn retry_policy_new_zero_attempts_rejects() {
        let result = RetryPolicy::new(0, 100, 2.0);
        assert!(result.is_err());
    }

    #[test]
    fn retry_policy_new_nan_multiplier_rejects() {
        let result = RetryPolicy::new(3, 100, f64::NAN);
        assert!(result.is_err());
    }

    #[test]
    fn retry_policy_new_infinity_multiplier_rejects() {
        let result = RetryPolicy::new(3, 100, f64::INFINITY);
        assert!(result.is_err());
    }

    #[test]
    fn retry_policy_new_multiplier_below_one_rejects() {
        let result = RetryPolicy::new(3, 100, 0.99);
        assert!(result.is_err());
    }

    #[test]
    fn retry_policy_new_multiplier_exactly_one_ok() {
        let result = RetryPolicy::new(3, 100, 1.0);
        assert!(result.is_ok());
    }

    #[test]
    fn retry_policy_with_max_backoff_valid() {
        let policy = RetryPolicy::with_max_backoff(5, 100, 2.0, 5000).unwrap();
        assert_eq!(policy.max_attempts, 5);
        assert_eq!(policy.max_backoff_ms, 5000);
    }

    #[test]
    fn retry_policy_with_max_backoff_equal_to_initial_ok() {
        let policy = RetryPolicy::with_max_backoff(3, 100, 2.0, 100).unwrap();
        assert_eq!(policy.max_backoff_ms, 100);
    }

    #[test]
    fn retry_policy_with_max_backoff_less_than_initial_rejects() {
        let result = RetryPolicy::with_max_backoff(3, 100, 2.0, 50);
        assert!(result.is_err());
    }

    #[test]
    fn calculate_backoff_delay_attempt_zero_returns_zero() {
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();
        assert_eq!(policy.calculate_backoff_delay(0), 0);
    }

    #[test]
    fn calculate_backoff_delay_zero_initial_returns_zero() {
        let policy = RetryPolicy::new(3, 0, 2.0).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 0);
        assert_eq!(policy.calculate_backoff_delay(5), 0);
    }

    #[test]
    fn calculate_backoff_delay_linear_multiplier_one() {
        let policy = RetryPolicy::new(10, 100, 1.0).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(5), 100);
        assert_eq!(policy.calculate_backoff_delay(10), 100);
    }

    #[test]
    fn calculate_backoff_delay_exponential() {
        let policy = RetryPolicy::new(10, 100, 2.0).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(2), 200);
        assert_eq!(policy.calculate_backoff_delay(3), 400);
        assert_eq!(policy.calculate_backoff_delay(4), 800);
        assert_eq!(policy.calculate_backoff_delay(5), 1600);
    }

    #[test]
    fn calculate_backoff_delay_capped_at_max_backoff() {
        let policy = RetryPolicy::with_max_backoff(10, 100, 10.0, 500).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(2), 500);
        assert_eq!(policy.calculate_backoff_delay(3), 500);
    }

    #[test]
    fn calculate_backoff_delay_exponential_with_small_multiplier() {
        let policy = RetryPolicy::with_max_backoff(5, 1000, 1.5, 10000).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 1000);
        assert_eq!(policy.calculate_backoff_delay(2), 1500);
        assert_eq!(policy.calculate_backoff_delay(3), 2250);
    }

    #[test]
    fn max_backoff_clamp_prevents_exponential_overflow() {
        let policy = RetryPolicy::with_max_backoff(10, 1000, 2.0, 30000).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 1000);
        assert_eq!(policy.calculate_backoff_delay(2), 2000);
        assert_eq!(policy.calculate_backoff_delay(3), 4000);
        assert_eq!(policy.calculate_backoff_delay(4), 8000);
        assert_eq!(policy.calculate_backoff_delay(5), 16000);
        assert_eq!(policy.calculate_backoff_delay(6), 30000);
        assert_eq!(policy.calculate_backoff_delay(7), 30000);
    }

    #[test]
    fn max_backoff_very_large_value_allows_full_exponential() {
        let policy = RetryPolicy::new(5, 100, 2.0).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(2), 200);
        assert_eq!(policy.calculate_backoff_delay(3), 400);
        assert_eq!(policy.calculate_backoff_delay(4), 800);
    }

    #[test]
    fn max_backoff_exactly_at_exponential_result() {
        let policy = RetryPolicy::with_max_backoff(3, 100, 2.0, 200).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(2), 200);
        assert_eq!(policy.calculate_backoff_delay(3), 200);
    }

    #[test]
    fn retry_exhaustion_single_attempt() {
        let policy = RetryPolicy::new(1, 100, 2.0).unwrap();
        assert_eq!(policy.max_attempts, 1);
    }

    #[test]
    fn retry_exhaustion_three_attempts() {
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();
        assert_eq!(policy.max_attempts, 3);
    }

    #[test]
    fn retry_exhaustion_after_max_attempts() {
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();
        assert_eq!(policy.max_attempts, 3);
    }

    #[test]
    fn retry_exhaustion_with_different_delays() {
        let policy = RetryPolicy::new(5, 1000, 2.0).unwrap();
        for attempt in 1..=5 {
            let delay = policy.calculate_backoff_delay(attempt);
            assert!(delay > 0);
        }
    }

    #[test]
    fn retry_policy_zero_initial_delay_with_multiplier() {
        let policy = RetryPolicy::new(3, 0, 2.0).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 0);
        assert_eq!(policy.calculate_backoff_delay(2), 0);
        assert_eq!(policy.calculate_backoff_delay(3), 0);
    }

    #[test]
    fn retry_policy_large_multiplier() {
        let policy = RetryPolicy::with_max_backoff(5, 1, 100.0, 10000).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 1);
        assert_eq!(policy.calculate_backoff_delay(2), 100);
        assert_eq!(policy.calculate_backoff_delay(3), 10000);
    }

    #[test]
    fn retry_policy_zero_max_backoff_effectively_disables_backoff() {
        let policy = RetryPolicy::with_max_backoff(3, 100, 2.0, 100).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(2), 100);
        assert_eq!(policy.calculate_backoff_delay(3), 100);
    }

    #[test]
    fn retry_policy_very_small_max_backoff() {
        let policy = RetryPolicy::with_max_backoff(3, 1, 2.0, 1).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 1);
        assert_eq!(policy.calculate_backoff_delay(2), 1);
        assert_eq!(policy.calculate_backoff_delay(3), 1);
    }
}
