use std::time::{Duration, Instant};

use super::*;

mod token_bucket_config_tests {
    use super::*;

    #[test]
    fn token_bucket_config_new_creates_valid_config() {
        let config = TokenBucketConfig::new(100, 10.0, 1);
        assert_eq!(config.burst, 100);
        assert_eq!(config.sustained_rate, 10.0);
        assert_eq!(config.cost_per_request, 1);
    }

    #[test]
    fn token_bucket_config_default_has_correct_values() {
        let config = TokenBucketConfig::default();
        assert_eq!(config.burst, 100);
        assert_eq!(config.sustained_rate, 10.0);
        assert_eq!(config.cost_per_request, 1);
    }

    #[test]
    fn token_bucket_config_tokens_per_second_returns_sustained_rate() {
        let config = TokenBucketConfig::new(100, 10.0, 1);
        assert_eq!(config.tokens_per_second(), 10.0);

        let config2 = TokenBucketConfig::new(50, 25.5, 2);
        assert_eq!(config2.tokens_per_second(), 25.5);
    }
}

mod token_bucket_tests {
    use super::*;

    #[test]
    fn token_bucket_new_key_starts_with_full_burst() {
        let config = TokenBucketConfig::new(100, 10.0, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        let (allowed, retry) = limiter.check_and_consume("key1", now);

        assert!(allowed);
        assert_eq!(retry, 0);
    }

    #[test]
    fn token_bucket_burst_capacity_respected() {
        let config = TokenBucketConfig::new(3, 10.0, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        assert!(limiter.check_and_consume("key1", now).0);
        assert!(limiter.check_and_consume("key1", now).0);
        assert!(limiter.check_and_consume("key1", now).0);

        let (allowed, retry) = limiter.check_and_consume("key1", now);
        assert!(!allowed);
        assert!(retry > 0);
    }

    #[test]
    fn token_bucket_sustained_rate_replenishes() {
        let config = TokenBucketConfig::new(10, 10.0, 5);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        limiter.check_and_consume("key1", now);

        let later = now + Duration::from_secs(1);
        let tokens = limiter.available_tokens("key1", later);
        assert!(tokens >= 9.0);
    }

    #[test]
    fn token_bucket_per_key_tracking_independent() {
        let config = TokenBucketConfig::new(5, 10.0, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        for _ in 0..5 {
            limiter.check_and_consume("key1", now);
        }

        let (allowed, _) = limiter.check_and_consume("key2", now);
        assert!(allowed);
    }

    #[test]
    fn token_bucket_sliding_window_smooth_accumulation() {
        let config = TokenBucketConfig::new(10, 100.0, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        limiter.check_and_consume("key1", now);

        let later = now + Duration::from_millis(100);
        let tokens = limiter.available_tokens("key1", later);
        assert!(tokens >= 9.0);
    }

    #[test]
    fn token_bucket_cost_per_request_respected() {
        let config = TokenBucketConfig::new(10, 10.0, 5);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        assert!(limiter.check_and_consume("key1", now).0);
        assert!(limiter.check_and_consume("key1", now).0);

        let (allowed, _) = limiter.check_and_consume("key1", now);
        assert!(!allowed);
    }

    #[test]
    fn token_bucket_reset_clears_bucket() {
        let config = TokenBucketConfig::new(10, 10.0, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        for _ in 0..10 {
            limiter.check_and_consume("key1", now);
        }

        limiter.reset("key1");

        let (allowed, _) = limiter.check_and_consume("key1", now);
        assert!(allowed);
    }

    #[test]
    fn token_bucket_key_count_tracking() {
        let config = TokenBucketConfig::new(10, 10.0, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        assert_eq!(limiter.key_count(), 0);

        limiter.check_and_consume("key1", now);
        assert_eq!(limiter.key_count(), 1);

        limiter.check_and_consume("key2", now);
        assert_eq!(limiter.key_count(), 2);

        limiter.reset("key1");
        assert_eq!(limiter.key_count(), 1);
    }

    #[test]
    fn token_bucket_zero_sustained_rate_no_replenishment() {
        let config = TokenBucketConfig::new(5, 0.0, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        for _ in 0..5 {
            limiter.check_and_consume("key1", now);
        }

        let later = now + Duration::from_secs(100);
        let tokens = limiter.available_tokens("key1", later);
        assert_eq!(tokens, 0.0);
    }

    #[test]
    fn token_bucket_available_tokens_correct() {
        let config = TokenBucketConfig::new(10, 10.0, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        let tokens = limiter.available_tokens("key1", now);
        assert!((tokens - 10.0).abs() < 0.001);

        limiter.check_and_consume("key1", now);
        let tokens = limiter.available_tokens("key1", now);
        assert!((tokens - 9.0).abs() < 0.001);
    }

    #[test]
    fn token_bucket_wait_time_calculation() {
        let config = TokenBucketConfig::new(10, 10.0, 10);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        let (allowed, wait) = limiter.check_and_consume("key1", now);
        assert!(allowed);
        assert_eq!(wait, 0);

        let (_, wait) = limiter.check_and_consume("key1", now);
        assert!(wait >= 1);
    }

    #[test]
    fn token_bucket_wait_time_u64_max_when_zero_rate() {
        let config = TokenBucketConfig::new(5, 0.0, 5);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        limiter.check_and_consume("key1", now);

        let wait = limiter.wait_time("key1", now);
        assert_eq!(wait, u64::MAX);
    }

    #[test]
    fn token_bucket_consume_and_peek_consistent() {
        let config = TokenBucketConfig::new(10, 10.0, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        limiter.check_and_consume("key1", now);
        let peeked = limiter.peek_tokens("key1", now);
        let available = limiter.available_tokens("key1", now);
        assert_eq!(peeked, available);
    }

    #[test]
    fn token_bucket_available_tokens_after_reset() {
        let config = TokenBucketConfig::new(10, 0.0, 5);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        limiter.check_and_consume("key1", now);
        let tokens_before_reset = limiter.available_tokens("key1", now);
        assert!((tokens_before_reset - 5.0).abs() < 0.001);

        limiter.reset("key1");

        let tokens_after_reset = limiter.available_tokens("key1", now);
        assert!((tokens_after_reset - 10.0).abs() < 0.001);
    }

    #[test]
    fn token_bucket_new_key_created_with_burst_minus_cost() {
        let config = TokenBucketConfig::new(10, 0.0, 3);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        limiter.check_and_consume("key1", now);
        let tokens = limiter.available_tokens("key1", now);
        assert!((tokens - 7.0).abs() < 0.001);
    }

    #[test]
    fn token_bucket_burst_never_exceeded() {
        let config = TokenBucketConfig::new(5, 100.0, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        limiter.check_and_consume("key1", now);

        let later = now + Duration::from_secs(10);
        let tokens = limiter.available_tokens("key1", later);
        assert!(
            tokens <= 5.0,
            "tokens {} should be capped at burst 5.0",
            tokens
        );
    }

    #[test]
    fn token_bucket_fair_queuing_peek() {
        let config = TokenBucketConfig::new(5, 10.0, 5);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        limiter.check_and_consume("key1", now);

        let tokens1 = limiter.peek_tokens("key1", now);
        let tokens2 = limiter.peek_tokens("key1", now);
        let tokens3 = limiter.peek_tokens("key1", now);

        assert_eq!(tokens1, tokens2);
        assert_eq!(tokens2, tokens3);

        let (allowed, _) = limiter.check_and_consume("key1", now);
        assert!(!allowed);
    }
}

mod token_bucket_proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn token_bucket_tokens_never_exceed_burst(
            burst in 1u64..=1000u64,
            sustained_rate in 0f64..=1000f64,
            elapsed_secs in 0u64..=10u64,
        ) {
            let config = TokenBucketConfig::new(burst, sustained_rate, 1);
            let limiter = TokenBucketRateLimiter::new(config);
            let now = Instant::now();
            limiter.check_and_consume("key", now);
            let later = now + Duration::from_secs(elapsed_secs);
            let tokens = limiter.available_tokens("key", later);
            prop_assert!(tokens <= burst as f64 + 0.001);
        }

        #[test]
        fn token_bucket_cost_exact(
            burst in 1u64..=100u64,
            cost in 1u64..=10u64,
            count in 0u64..20u64,
        ) {
            let valid_cost = cost.min(burst);
            let total_cost = count.saturating_mul(valid_cost);
            prop_assume!(total_cost <= burst);

            let config = TokenBucketConfig::new(burst, 0.0, valid_cost);
            let limiter = TokenBucketRateLimiter::new(config);
            let now = Instant::now();
            for _ in 0..count {
                limiter.check_and_consume("key", now);
            }
            let expected = (burst as i64 - count as i64 * valid_cost as i64).max(0) as f64;
            let actual = limiter.available_tokens("key", now);
            prop_assert!((actual - expected).abs() < 0.001);
        }

        #[test]
        fn wait_time_zero_when_tokens_available(
            burst in 1u64..=100u64,
            sustained_rate in 1f64..=100f64,
            cost in 1u64..=10u64,
        ) {
            let config = TokenBucketConfig::new(burst, sustained_rate, cost);
            let limiter = TokenBucketRateLimiter::new(config);
            let now = Instant::now();
            let available = limiter.available_tokens("key", now);
            let wait = limiter.wait_time("key", now);
            if available >= cost as f64 {
                prop_assert_eq!(wait, 0);
            }
        }

        #[test]
        fn wait_time_ceiling_calculation(
            burst in 1u64..=100u64,
            rate in 1f64..=100f64,
            cost in 1u64..=10u64,
            elapsed_ms in 0u64..=10000u64,
        ) {
            let config = TokenBucketConfig::new(burst, rate, cost);
            let limiter = TokenBucketRateLimiter::new(config);
            let now = Instant::now();
            limiter.check_and_consume("key", now);
            let later = now + Duration::from_millis(elapsed_ms);
            let wait = limiter.wait_time("key", later);
            let tokens = limiter.available_tokens("key", later);
            if tokens < cost as f64 {
                let needed = cost as f64 - tokens;
                let expected = (needed / rate).ceil() as u64;
                prop_assert_eq!(wait, expected);
            }
        }

        #[test]
        fn key_count_accurate(
            keys in prop::collection::vec("[a-z]{1,10}", 1..20usize),
        ) {
            let config = TokenBucketConfig::new(10, 10.0, 1);
            let limiter = TokenBucketRateLimiter::new(config);
            let now = Instant::now();
            let mut unique_keys: std::collections::HashSet<&str> = std::collections::HashSet::new();
            for key in &keys {
                limiter.check_and_consume(key, now);
                unique_keys.insert(key.as_str());
            }
            prop_assert_eq!(limiter.key_count(), unique_keys.len());
        }

        #[test]
        fn reset_decreases_key_count(
            keys in prop::collection::vec("[a-z]{1,10}", 1..10usize),
            reset_idx in 0u64..10u64,
        ) {
            let config = TokenBucketConfig::new(10, 10.0, 1);
            let limiter = TokenBucketRateLimiter::new(config);
            let now = Instant::now();
            for key in &keys {
                limiter.check_and_consume(key, now);
            }
            let initial_count = limiter.key_count();
            if (reset_idx as usize) < keys.len() {
                limiter.reset(&keys[reset_idx as usize]);
                prop_assert_eq!(limiter.key_count(), initial_count - 1);
            }
        }

        #[test]
        fn replenishment_deterministic(
            burst in 1u64..=100u64,
            rate in 0f64..=100f64,
            elapsed_ms in 0u64..=10000u64,
        ) {
            let config = TokenBucketConfig::new(burst, rate, 1);
            let limiter = TokenBucketRateLimiter::new(config);
            let now = Instant::now();
            limiter.check_and_consume("key1", now);
            let later = now + Duration::from_millis(elapsed_ms);
            let t1 = limiter.available_tokens("key1", later);
            limiter.reset("key1");
            limiter.check_and_consume("key2", now);
            let t2 = limiter.available_tokens("key2", later);
            prop_assert_eq!(t1, t2);
        }
    }
}
