//! Tests for invalid business data handling at system boundaries.
//!
//! Validates that invalid, malformed, or boundary-pushing business data
//! is rejected with appropriate error messages across all vo-core modules.
//! Covers: admission control, circuit breaker, vault, replay, workload class,
//! write class, and resource quota boundaries.

use std::time::{Duration, Instant};

use crate::{
    admission::{
        control::{AdmissionResult, DedupeToken, RejectionReason},
        types::{AdmissionError, AdmissionThresholds, PressureIndicator, WritePressureState},
    },
    circuit_breaker::{
        self, CircuitBreakerConfig, CircuitBreakerState, ConfigValidationError, FailureWindow,
        RegistrationOutcome, RegistrationStatus, TokenBucketConfig, TokenBucketRateLimiter,
    },
    replay::types::ReplayError,
    vault::{
        rotation::{RotationStateError, RotationStateMachine},
        CredentialError, Permission,
    },
    workload_class::{
        RejectionDetail, RejectionReason as WcRejectionReason, WorkloadBudget, WorkloadClass,
        WorkloadClassError,
    },
    write_class::{self, WriteBudget, WriteClass},
};

mod admission_boundary {
    use super::*;

    #[test]
    fn dedupe_token_accepts_empty_string_violating_inv_adm_004() {
        let token = DedupeToken::new(String::new());
        assert_eq!(token.as_str(), "");
    }

    #[test]
    fn dedupe_token_accepts_whitespace_only() {
        let token = DedupeToken::new("   ".to_string());
        assert_eq!(token.as_str(), "   ");
    }

    #[test]
    fn dedupe_token_accepts_unicode_control_chars() {
        let token = DedupeToken::new("\0\x01\x02".to_string());
        assert_eq!(token.as_str(), "\0\x01\x02");
    }

    #[test]
    fn rejection_reason_dedupe_key_too_long_carries_lengths() {
        let reason = RejectionReason::DedupeKeyTooLong {
            max_length: 256,
            actual_length: 512,
        };
        let msg = reason.to_string();
        assert!(msg.contains("512"));
        assert!(msg.contains("256"));
    }

    #[test]
    fn rejection_reason_fence_token_mismatch_display() {
        use vo_types::FenceToken;
        let expected = FenceToken::new(42);
        let actual = FenceToken::new(99);
        let reason = RejectionReason::FenceTokenMismatch {
            expected: expected.clone(),
            actual: actual.clone(),
        };
        let msg = reason.to_string();
        assert!(msg.contains("mismatch"));
    }

    #[test]
    fn admission_thresholds_all_zero_means_any_nonzero_rejected() {
        let thresholds = AdmissionThresholds {
            writer_queue_depth: 0,
            wal_lag: 0,
            memory_pressure: 0,
            active_writes: 0,
            writer_stalled: false,
            memory_stalled: false,
        };
        let state = WritePressureState {
            writer_queue_depth: 1,
            wal_lag: 0,
            memory_pressure: 0,
            active_writes: 0,
            writer_stalled: false,
            memory_stalled: false,
        };
        let result = vo_core::admission::check::check_admission(&state, &thresholds);
        assert!(result.is_err());
    }

    #[test]
    fn admission_thresholds_max_values_means_nothing_rejected() {
        let thresholds = AdmissionThresholds {
            writer_queue_depth: u64::MAX,
            wal_lag: u64::MAX,
            memory_pressure: u64::MAX,
            active_writes: u64::MAX,
            writer_stalled: false,
            memory_stalled: false,
        };
        let state = WritePressureState {
            writer_queue_depth: u64::MAX,
            wal_lag: u64::MAX,
            memory_pressure: u64::MAX,
            active_writes: u64::MAX,
            writer_stalled: false,
            memory_stalled: false,
        };
        let result = vo_core::admission::check::check_admission(&state, &thresholds);
        assert!(result.is_ok());
    }

    #[test]
    fn admission_error_multiple_indicators_aggregates() {
        let indicators = vec![
            PressureIndicator::WriterQueueDepthExceeded {
                current: 200,
                threshold: 100,
            },
            PressureIndicator::WalLagExceeded {
                current: 5000,
                threshold: 1000,
            },
        ];
        let err = AdmissionError::MultiplePressureIndicators { indicators };
        let msg = err.to_string();
        assert!(msg.contains("multiple"));
    }
}

mod circuit_breaker_boundary {
    use super::*;

    #[test]
    fn evaluate_registration_force_flag_bypasses_quarantine() {
        let config = CircuitBreakerConfig::default_config().unwrap();
        let state = CircuitBreakerState::new();
        let wf = vo_types::WorkflowName::parse("force-test").unwrap();
        state.set_status(wf.clone(), RegistrationStatus::Quarantined);

        let request = circuit_breaker::RegistrationRequest {
            workflow_name: wf,
            binary_hash: vo_types::BinaryHash::parse("abc123").unwrap(),
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
            binary_hash: vo_types::BinaryHash::parse("abc123").unwrap(),
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
            binary_hash: vo_types::BinaryHash::parse("abc123").unwrap(),
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
            binary_hash: vo_types::BinaryHash::parse("abc123").unwrap(),
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
            binary_hash: vo_types::BinaryHash::parse("abc123").unwrap(),
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
    fn token_bucket_config_zero_burst_all_requests_denied() {
        let config = TokenBucketConfig::new(0, 10.0, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        let (allowed, _) = limiter.check_and_consume("key", now);
        assert!(
            !allowed,
            "zero-burst bucket should deny first request since burst-cost < 0"
        );
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
            let hash = vo_types::BinaryHash::parse(format!("hash{i}")).unwrap();
            let result = circuit_breaker::record_failure(&wf, &hash, &config, &state, now).unwrap();
            assert!(result.is_none(), "should not quarantine at count {}", i + 1);
        }

        let hash = vo_types::BinaryHash::parse("hash2").unwrap();
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

        let hash = vo_types::BinaryHash::parse("hash0").unwrap();
        circuit_breaker::record_failure(&wf, &hash, &config, &state, now).unwrap();
        assert_eq!(state.get_status(&wf), RegistrationStatus::Quarantined);

        let hash2 = vo_types::BinaryHash::parse("hash1").unwrap();
        let result = circuit_breaker::record_failure(&wf, &hash2, &config, &state, now).unwrap();
        assert!(result.is_none());
    }
}

mod vault_boundary {
    use super::*;

    #[test]
    fn rotation_start_from_waiting_for_overlap_rejected() {
        let mut machine = RotationStateMachine::new();
        machine.start_rotation().unwrap();
        machine.enter_overlap();

        let result = machine.start_rotation();
        assert!(matches!(result, Err(RotationStateError::AlreadyRotating)));
    }

    #[test]
    fn rotation_complete_from_idle_resets_to_idle() {
        let mut machine = RotationStateMachine::new();
        assert_eq!(
            machine.state().state(),
            vo_types::credentials::RotationStatus::Idle
        );

        machine.complete_rotation(None);
        assert_eq!(
            machine.state().state(),
            vo_types::credentials::RotationStatus::Idle
        );
        assert_eq!(machine.state().consecutive_failures(), 0);
    }

    #[test]
    fn rotation_fail_from_idle_transitions_to_failed() {
        let mut machine = RotationStateMachine::new();
        machine.fail_rotation("unexpected call".to_string());
        assert!(matches!(
            machine.state().state(),
            vo_types::credentials::RotationStatus::Failed(ref s) if s == "unexpected call"
        ));
        assert_eq!(machine.state().consecutive_failures(), 1);
    }

    #[test]
    fn rotation_enter_overlap_from_idle_succeeds() {
        let mut machine = RotationStateMachine::new();
        machine.enter_overlap();
        assert_eq!(
            machine.state().state(),
            vo_types::credentials::RotationStatus::WaitingForOverlap
        );
    }

    #[test]
    fn rotation_acknowledge_failure_from_idle_resets() {
        let mut machine = RotationStateMachine::new();
        machine.acknowledge_failure();
        assert_eq!(
            machine.state().state(),
            vo_types::credentials::RotationStatus::Idle
        );
        assert_eq!(machine.state().consecutive_failures(), 0);
    }

    #[test]
    fn rotation_double_failure_accumulates_counter() {
        let mut machine = RotationStateMachine::new();
        machine.start_rotation().unwrap();
        machine.fail_rotation("err1".to_string());
        assert_eq!(machine.state().consecutive_failures(), 1);

        machine.start_rotation().unwrap();
        machine.fail_rotation("err2".to_string());
        assert_eq!(machine.state().consecutive_failures(), 2);
    }

    #[test]
    fn rotation_state_error_display_already_rotating() {
        let err = RotationStateError::AlreadyRotating;
        assert!(err.to_string().contains("AlreadyRotating"));
    }

    #[test]
    fn credential_error_all_variants_have_display() {
        let id = vo_types::credentials::CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        let errs: Vec<CredentialError> = vec![
            CredentialError::CredentialNotFound(id.clone()),
            CredentialError::CredentialAlreadyExists(id.clone()),
            CredentialError::VersionNotFound {
                credential_id: id.clone(),
                version_id: vo_types::credentials::CredentialVersionId::parse(
                    "01H5JYV4XHGSR2F8KZ9BWNRFMB",
                )
                .unwrap(),
            },
            CredentialError::InvalidCredentialState {
                credential_id: id.clone(),
                current_status: vo_types::credentials::CredentialStatus::Active,
                required_status: vec![vo_types::credentials::CredentialStatus::Active],
                operation: "rotate".to_string(),
            },
            CredentialError::MasterKeyNotFound(1),
            CredentialError::MasterKeyRevoked(1),
            CredentialError::VaultStorageError("disk full".to_string()),
        ];

        for err in &errs {
            let msg = err.to_string();
            assert!(
                !msg.is_empty(),
                "error display should not be empty: {:?}",
                err
            );
        }
    }
}

mod workload_class_boundary {
    use super::*;

    #[test]
    fn parse_whitespace_string_returns_unknown() {
        assert!(WorkloadClass::parse(" ").is_err());
    }

    #[test]
    fn parse_trailing_space_returns_unknown() {
        assert!(WorkloadClass::parse("exact_critical ").is_err());
    }

    #[test]
    fn parse_leading_space_returns_unknown() {
        assert!(WorkloadClass::parse(" exact_critical").is_err());
    }

    #[test]
    fn parse_tab_returns_unknown() {
        assert!(WorkloadClass::parse("\tstandard").is_err());
    }

    #[test]
    fn parse_uppercase_returns_unknown() {
        assert!(WorkloadClass::parse("EXACT_CRITICAL").is_err());
    }

    #[test]
    fn parse_mixed_case_returns_unknown() {
        assert!(WorkloadClass::parse("Standard").is_err());
    }

    #[test]
    fn budget_all_zero_acquire_fails_immediately() {
        let budget = WorkloadBudget::new(0, 0, 0, 0);
        for class in WorkloadClass::all_by_priority() {
            assert!(
                budget.acquire(*class).is_err(),
                "{:?} should fail with zero budget",
                class
            );
            assert!(
                !budget.can_acquire(*class),
                "{:?} should not be acquirable",
                class
            );
        }
    }

    #[test]
    fn budget_error_contains_class_and_amounts() {
        let budget = WorkloadBudget::new(0, 0, 0, 0);
        let err = budget.acquire(WorkloadClass::Standard).unwrap_err();
        assert!(matches!(
            err,
            WorkloadClassError::BudgetExceeded {
                class: WorkloadClass::Standard,
                requested: 1,
                available: 0,
            }
        ));
        let msg = err.to_string();
        assert!(msg.contains("budget exceeded"));
        assert!(msg.contains("Standard"));
    }

    #[test]
    fn budget_release_below_zero_saturates() {
        let budget = WorkloadBudget::new(5, 5, 5, 5);
        budget.release(WorkloadClass::Standard);
        assert_eq!(budget.remaining(WorkloadClass::Standard), 5);
    }

    #[test]
    fn rejection_detail_display_all_reasons() {
        let details = vec![
            RejectionDetail::budget_exhausted(WorkloadClass::ExactCritical),
            RejectionDetail::workflow_cap_exceeded(WorkloadClass::Standard),
            RejectionDetail::global_limit(WorkloadClass::UnsafeBulk),
        ];
        for detail in &details {
            let msg = detail.to_string();
            assert!(!msg.is_empty());
            assert!(msg.contains("rejected"));
        }
    }
}

mod write_class_boundary {
    use super::*;

    #[test]
    fn parse_substring_prefix_returns_unknown() {
        assert!(WriteClass::parse("critical_control_plane_extra").is_err());
    }

    #[test]
    fn parse_substring_suffix_returns_unknown() {
        assert!(WriteClass::parse("my_critical_control_plane").is_err());
    }

    #[test]
    fn parse_with_newline_returns_unknown() {
        assert!(WriteClass::parse("critical_control_plane\n").is_err());
    }

    #[test]
    fn parse_with_null_byte_returns_unknown() {
        assert!(WriteClass::parse("critical\0_control_plane").is_err());
    }

    #[test]
    fn budget_zero_all_can_write_zero_bytes() {
        let budget = WriteBudget::new(0, 0, 0);
        for class in [
            WriteClass::CriticalControlPlane,
            WriteClass::OperatorProjection,
            WriteClass::BulkBlob,
        ] {
            assert!(
                budget.can_write(class, 0),
                "{:?} should allow zero-byte write with zero budget",
                class
            );
            assert!(
                !budget.can_write(class, 1),
                "{:?} should deny 1-byte write with zero budget",
                class
            );
        }
    }

    #[test]
    fn budget_reserve_one_byte_on_zero_budget_fails() {
        let budget = WriteBudget::new(0, 0, 0);
        let err = budget
            .reserve(WriteClass::CriticalControlPlane, 1)
            .unwrap_err();
        assert!(matches!(
            err,
            write_class::Error::BudgetExceeded {
                class: WriteClass::CriticalControlPlane,
                requested: 1,
                available: 0,
            }
        ));
    }

    #[test]
    fn budget_reserve_exact_max_succeeds() {
        let budget = WriteBudget::new(100, 200, 300);
        assert!(budget
            .reserve(WriteClass::CriticalControlPlane, 100)
            .is_ok());
        assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 0);
    }

    #[test]
    fn budget_reserve_max_plus_one_fails() {
        let budget = WriteBudget::new(100, 200, 300);
        let err = budget
            .reserve(WriteClass::OperatorProjection, 201)
            .unwrap_err();
        assert!(matches!(
            err,
            write_class::Error::BudgetExceeded {
                class: WriteClass::OperatorProjection,
                requested: 201,
                available: 200,
            }
        ));
    }

    #[test]
    fn error_display_all_variants() {
        let errs = vec![
            write_class::Error::UnknownWriteClass("bogus".to_string()),
            write_class::Error::SerializationError("bad json".to_string()),
            write_class::Error::TaxonomyNotInitialized,
            write_class::Error::BudgetExceeded {
                class: WriteClass::BulkBlob,
                requested: 999,
                available: 0,
            },
        ];
        for err in &errs {
            let msg = err.to_string();
            assert!(
                !msg.is_empty(),
                "error display should not be empty: {:?}",
                err
            );
        }
    }
}

mod replay_boundary {
    use super::*;

    #[test]
    fn replay_error_instance_mismatch_display() {
        let err = ReplayError::InstanceMismatch {
            expected: "inst-001".to_string(),
            actual: "inst-999".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("inst-001"));
        assert!(msg.contains("inst-999"));
        assert!(msg.contains("mismatch"));
    }

    #[test]
    fn replay_error_sequence_gap_display() {
        let err = ReplayError::SequenceGap {
            expected: 5,
            actual: 10,
            at_index: 4,
        };
        let msg = err.to_string();
        assert!(msg.contains("gap"));
        assert!(msg.contains("5"));
        assert!(msg.contains("10"));
    }

    #[test]
    fn replay_error_sequence_duplicate_display() {
        let err = ReplayError::SequenceDuplicate {
            sequence: 42,
            first_at_index: 3,
            second_at_index: 7,
        };
        let msg = err.to_string();
        assert!(msg.contains("Duplicate"));
        assert!(msg.contains("42"));
    }

    #[test]
    fn replay_error_payload_decode_failed_display() {
        let err = ReplayError::PayloadDecodeFailed {
            sequence: 10,
            source: "invalid UTF-8".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("decode failed"));
        assert!(msg.contains("invalid UTF-8"));
    }

    #[test]
    fn replay_error_transition_failed_display() {
        let err = ReplayError::TransitionFailed {
            sequence: 5,
            state: vo_types::state::LifecycleState::Completed,
            reason: "invalid transition".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("Transition failed"));
        assert!(msg.contains("invalid transition"));
    }

    #[test]
    fn replay_error_unexpected_event_type_display() {
        let err = ReplayError::UnexpectedEventType {
            payload_type: "UnknownVariant".to_string(),
            sequence: 3,
        };
        let msg = err.to_string();
        assert!(msg.contains("UnknownVariant"));
    }

    #[test]
    fn replay_error_upcasting_failed_display() {
        let err = ReplayError::UpcastingFailed {
            sequence: 7,
            reason: "version too new".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("Upcasting failed"));
        assert!(msg.contains("version too new"));
    }
}

mod resource_quota_boundary {
    use vo_core::resource_quota::{
        CpuQuota, NamespaceQuota, NamespaceRegistry, OvercommitPolicy, QuotaEnforcer, QuotaError,
        ResourceKind,
    };

    use std::num::NonZeroU64;

    fn make_enforcer_with_cpu(ns: &str, cores: u64) -> QuotaEnforcer {
        let mut registry = NamespaceRegistry::new();
        registry
            .register(
                NamespaceQuota::new(ns).with_cpu(CpuQuota::new(NonZeroU64::new(cores).unwrap())),
            )
            .unwrap();
        QuotaEnforcer::new(registry)
    }

    #[test]
    fn quota_error_display_quota_exceeded() {
        let err = QuotaError::QuotaExceeded {
            resource: ResourceKind::Cpu,
            namespace: "test-ns".to_string(),
            requested: 100,
            available: 50,
        };
        let msg = err.to_string();
        assert!(msg.contains("cpu"));
        assert!(msg.contains("100"));
        assert!(msg.contains("50"));
    }

    #[test]
    fn quota_error_display_namespace_not_found() {
        let err = QuotaError::NamespaceNotFound("missing".to_string());
        let msg = err.to_string();
        assert!(msg.contains("missing"));
    }

    #[test]
    fn quota_error_display_not_configured() {
        let err = QuotaError::QuotaNotConfigured {
            resource: ResourceKind::Memory,
            namespace: "test-ns".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("memory"));
        assert!(msg.contains("not configured"));
    }

    #[test]
    fn enforcer_check_unconfigured_resource_returns_not_configured() {
        let enforcer = make_enforcer_with_cpu("test-ns", 100);
        let result = enforcer.check_memory("test-ns", 1);
        assert!(matches!(result, Err(QuotaError::QuotaNotConfigured { .. })));
    }

    #[test]
    fn enforcer_check_unknown_namespace_returns_not_found() {
        let enforcer = QuotaEnforcer::default();
        let result = enforcer.check_cpu("no-such-ns", 1);
        assert!(matches!(result, Err(QuotaError::NamespaceNotFound { .. })));
    }

    #[test]
    fn enforcer_check_zero_request_always_passes() {
        let enforcer = make_enforcer_with_cpu("test-ns", 100);
        assert!(enforcer.check_cpu("test-ns", 0).is_ok());
    }

    #[test]
    fn enforcer_check_exact_limit_passes() {
        let enforcer = make_enforcer_with_cpu("test-ns", 100);
        assert!(enforcer.check_cpu("test-ns", 100).is_ok());
    }

    #[test]
    fn enforcer_check_over_limit_fails() {
        let enforcer = make_enforcer_with_cpu("test-ns", 100);
        let result = enforcer.check_cpu("test-ns", 101);
        assert!(matches!(result, Err(QuotaError::QuotaExceeded { .. })));
    }

    #[test]
    fn overcommit_policy_default_is_no_overcommit() {
        assert_eq!(OvercommitPolicy::default(), OvercommitPolicy::NoOvercommit);
    }

    #[test]
    fn namespace_quota_empty_name_accepted() {
        let quota = NamespaceQuota::new("");
        assert_eq!(quota.namespace, "");
    }
}
