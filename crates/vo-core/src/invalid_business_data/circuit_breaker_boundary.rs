mod circuit_breaker_boundary {
    use super::*;
    use crate::circuit_breaker::{
        self, CircuitBreakerConfig, CircuitBreakerState, ConfigValidationError,
        RegistrationOutcome, RegistrationStatus, TokenBucketConfig, TokenBucketRateLimiter,
    };
    use std::time::{Duration, Instant};

    #[test]
    fn evaluate_registration_force_flag_bypasses_quarantine() {
        let config = CircuitBreakerConfig::default_config().unwrap();
        let state = CircuitBreakerState::new();
        let wf = vo_types::WorkflowName::parse("force-test").unwrap();
        state.set_status(wf.clone(), RegistrationStatus::Quarantined);

        let request = circuit_breaker::RegistrationRequest {
            workflow_name: wf,
            binary_hash: vo_types::BinaryHash::parse("abc12399").unwrap(),
            force: true,
        };

        let result =
            circuit_breaker::evaluate_registration(&request, &config, &state, Instant::now())
                .unwrap();
        assert_eq!(result, RegistrationOutcome::Allowed);
    }

    #[test]
    fn evaluate_registration_quarantined_workflow_rejected() {
        let config = CircuitBreakerConfig::default_config().unwrap();
        let state = CircuitBreakerState::new();
        let wf = vo_types::WorkflowName::parse("quarantined-wf").unwrap();
        state.set_status(wf.clone(), RegistrationStatus::Quarantined);

        let request = circuit_breaker::RegistrationRequest {
            workflow_name: wf.clone(),
            binary_hash: vo_types::BinaryHash::parse("abc12399").unwrap(),
            force: false,
        };

        let result =
            circuit_breaker::evaluate_registration(&request, &config, &state, Instant::now())
                .unwrap();
        assert!(matches!(
            result,
            RegistrationOutcome::WorkflowQuarantined { .. }
        ));
    }

    #[test]
    fn evaluate_registration_deactivated_workflow_rejected() {
        let config = CircuitBreakerConfig::default_config().unwrap();
        let state = CircuitBreakerState::new();
        let wf = vo_types::WorkflowName::parse("deactivated-wf").unwrap();
        state.set_status(wf.clone(), RegistrationStatus::Deactivated);

        let request = circuit_breaker::RegistrationRequest {
            workflow_name: wf.clone(),
            binary_hash: vo_types::BinaryHash::parse("abc12399").unwrap(),
            force: false,
        };

        let result =
            circuit_breaker::evaluate_registration(&request, &config, &state, Instant::now())
                .unwrap();
        assert!(matches!(
            result,
            RegistrationOutcome::WorkflowDeactivated { .. }
        ));
    }

    #[test]
    fn evaluate_registration_active_workflow_rate_limited() {
        let config = CircuitBreakerConfig::default_config().unwrap();
        let state = CircuitBreakerState::new();
        let wf = vo_types::WorkflowName::parse("rate-limited-wf").unwrap();
        let now = Instant::now();
        state.set_rate_limit(wf.clone(), now);

        let request = circuit_breaker::RegistrationRequest {
            workflow_name: wf,
            binary_hash: vo_types::BinaryHash::parse("abc12399").unwrap(),
            force: false,
        };

        let result =
            circuit_breaker::evaluate_registration(&request, &config, &state, now).unwrap();
        assert!(matches!(result, RegistrationOutcome::RateLimited { .. }));
    }

    #[test]
    fn evaluate_registration_active_workflow_allowed_after_window() {
        let config =
            CircuitBreakerConfig::new(Duration::from_millis(1), Duration::from_secs(600), 5)
                .unwrap();
        let state = CircuitBreakerState::new();
        let wf = vo_types::WorkflowName::parse("allowed-wf").unwrap();
        let past = Instant::now() - Duration::from_secs(10);
        state.set_rate_limit(wf.clone(), past);

        let request = circuit_breaker::RegistrationRequest {
            workflow_name: wf,
            binary_hash: vo_types::BinaryHash::parse("abc12399").unwrap(),
            force: false,
        };

        let result =
            circuit_breaker::evaluate_registration(&request, &config, &state, Instant::now())
                .unwrap();
        assert_eq!(result, RegistrationOutcome::Allowed);
    }

    #[test]
    fn unquarantine_fails_for_unknown_workflow() {
        let state = CircuitBreakerState::new();
        let wf = vo_types::WorkflowName::parse("unknown-wf").unwrap();
        let result = circuit_breaker::unquarantine(&wf, "admin", &state);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            circuit_breaker::CircuitBreakerError::WorkflowNotFound { .. }
        ));
    }

    #[test]
    fn unquarantine_fails_for_active_workflow() {
        let state = CircuitBreakerState::new();
        let wf = vo_types::WorkflowName::parse("active-wf").unwrap();
        state.set_status(wf.clone(), RegistrationStatus::Active);

        let result = circuit_breaker::unquarantine(&wf, "admin", &state);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            circuit_breaker::CircuitBreakerError::NotQuarantined { .. }
        ));
    }

    #[test]
    fn unquarantine_succeeds_for_quarantined_workflow() {
        let state = CircuitBreakerState::new();
        let wf = vo_types::WorkflowName::parse("quar-wf").unwrap();
        state.set_status(wf.clone(), RegistrationStatus::Quarantined);
        let now = Instant::now();
        state.set_rate_limit(wf.clone(), now);

        let result = circuit_breaker::unquarantine(&wf, "admin", &state);
        assert!(result.is_ok());
        let unq = result.unwrap();
        assert_eq!(unq.failures_cleared, 0);
        assert_eq!(state.get_rate_limit(&wf), None);
        assert_eq!(state.get_status(&wf), RegistrationStatus::Active);
    }

    #[test]
    fn token_bucket_config_zero_burst_first_request_allowed_vacant_path() {
        let config = TokenBucketConfig::new(0, 10.0, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        let (allowed, _) = limiter.check_and_consume("key", now);
        assert!(
            allowed,
            "zero-burst bucket allows first request via vacant path (unconditional insert)"
        );

        let (allowed2, retry_after) = limiter.check_and_consume("key", now);
        assert!(
            !allowed2,
            "second request should be denied: tokens went negative after first consume"
        );
        assert!(retry_after > 0);
    }

    #[test]
    fn token_bucket_config_zero_cost_all_requests_free() {
        let config = TokenBucketConfig::new(10, 10.0, 0);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        for i in 0..100 {
            let (allowed, _) = limiter.check_and_consume("key", now);
            assert!(allowed, "zero-cost request {i} should always succeed");
        }
    }

    #[test]
    fn record_failure_triggers_quarantine_at_threshold() {
        let config =
            CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 3)
                .unwrap();
        let state = CircuitBreakerState::new();
        let wf = vo_types::WorkflowName::parse("fail-wf").unwrap();
        let now = Instant::now();

        for i in 0..2 {
            let hash = vo_types::BinaryHash::parse(&format!("aabbcc{i:02x}")).unwrap();
            let result =
                circuit_breaker::record_failure(&wf, &hash, &config, &state, now).unwrap();
            assert!(result.is_none(), "should not quarantine at count {}", i + 1);
        }

        let hash = vo_types::BinaryHash::parse("aabbccdd").unwrap();
        let result = circuit_breaker::record_failure(&wf, &hash, &config, &state, now).unwrap();
        assert!(result.is_some());
        assert_eq!(state.get_status(&wf), RegistrationStatus::Quarantined);
    }

    #[test]
    fn record_failure_ignores_already_quarantined() {
        let config =
            CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 1)
                .unwrap();
        let state = CircuitBreakerState::new();
        let wf = vo_types::WorkflowName::parse("already-q").unwrap();
        let now = Instant::now();

        let hash = vo_types::BinaryHash::parse("aabbcc00").unwrap();
        circuit_breaker::record_failure(&wf, &hash, &config, &state, now).unwrap();
        assert_eq!(state.get_status(&wf), RegistrationStatus::Quarantined);

        let hash2 = vo_types::BinaryHash::parse("aabbcc01").unwrap();
        let result =
            circuit_breaker::record_failure(&wf, &hash2, &config, &state, now).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn config_validation_rejects_zero_rate_limit_window() {
        let result = CircuitBreakerConfig::new(Duration::ZERO, Duration::from_secs(600), 5);
        assert!(matches!(
            result,
            Err(ConfigValidationError::ZeroRateLimitWindow)
        ));
    }

    #[test]
    fn config_validation_rejects_zero_failure_window() {
        let result = CircuitBreakerConfig::new(Duration::from_secs(60), Duration::ZERO, 5);
        assert!(matches!(
            result,
            Err(ConfigValidationError::ZeroFailureWindow)
        ));
    }

    #[test]
    fn config_validation_rejects_zero_failure_threshold() {
        let result =
            CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 0);
        assert!(matches!(
            result,
            Err(ConfigValidationError::ZeroFailureThreshold)
        ));
    }
}