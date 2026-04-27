use std::time::{Duration, Instant};

use crate::{
    admission::{
        control::{DedupeToken, RejectionReason},
        types::{AdmissionError, AdmissionThresholds, PressureIndicator, WritePressureState},
    },
    circuit_breaker::{
        self, CircuitBreakerConfig, CircuitBreakerState, ConfigValidationError,
        RegistrationOutcome, RegistrationStatus, TokenBucketConfig, TokenBucketRateLimiter,
    },
    replay::ReplayError,
    vault::{
        rotation::{RotationStateError, RotationStateMachine},
        CredentialError,
    },
    workload_class::{RejectionDetail, WorkloadBudget, WorkloadClass, WorkloadClassError},
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
        let expected = FenceToken::new(42).unwrap();
        let actual = FenceToken::new(99).unwrap();
        let reason = RejectionReason::FenceTokenMismatch {
            expected,
            actual,
        };
        let msg = reason.to_string();
        assert!(msg.contains("mismatch"));
    }

    #[test]
    fn admission_thresholds_all_zero_means_any_nonzero_rejected() {
        let thresholds = AdmissionThresholds {
            writer_queue_depth_threshold: 0,
            batch_commit_latency_ms_threshold: 0,
            blob_queue_depth_threshold: 0,
        };
        let state = WritePressureState {
            writer_queue_depth: 1,
            batch_commit_latency_ms: 0,
            blob_queue_depth: 0,
            compaction_stall_active: false,
            storage_stall_active: false,
        };
        let result = crate::admission::check::check_admission_with_thresholds(&state, &thresholds);
        assert!(result.is_err());
    }

    #[test]
    fn admission_thresholds_max_values_means_nothing_rejected() {
        let thresholds = AdmissionThresholds {
            writer_queue_depth_threshold: u64::MAX,
            batch_commit_latency_ms_threshold: u64::MAX,
            blob_queue_depth_threshold: u64::MAX,
        };
        let state = WritePressureState {
            writer_queue_depth: u64::MAX,
            batch_commit_latency_ms: u64::MAX,
            blob_queue_depth: u64::MAX,
            compaction_stall_active: false,
            storage_stall_active: false,
        };
        let result = crate::admission::check::check_admission_with_thresholds(&state, &thresholds);
        assert!(result.is_ok());
    }

    #[test]
    fn admission_error_multiple_indicators_aggregates() {
        let indicators = vec![
            PressureIndicator::WriterQueueDepth,
            PressureIndicator::BatchCommitLatency,
        ];
        let err = AdmissionError::MultiplePressureIndicators { indicators };
        let msg = format!("{err:?}");
        assert!(msg.contains("MultiplePressureIndicators"));
    }

    #[test]
    fn admission_compaction_stall_active_rejects() {
        let state = WritePressureState {
            writer_queue_depth: 0,
            batch_commit_latency_ms: 0,
            blob_queue_depth: 0,
            compaction_stall_active: true,
            storage_stall_active: false,
        };
        let result = crate::admission::check::check_admission(&state);
        assert!(result.is_err());
        assert!(matches!(result, Err(AdmissionError::CompactionStallActive)));
    }

    #[test]
    fn admission_storage_stall_active_rejects() {
        let state = WritePressureState {
            writer_queue_depth: 0,
            batch_commit_latency_ms: 0,
            blob_queue_depth: 0,
            compaction_stall_active: false,
            storage_stall_active: true,
        };
        let result = crate::admission::check::check_admission(&state);
        assert!(result.is_err());
        assert!(matches!(result, Err(AdmissionError::StorageStallActive)));
    }

    #[test]
    fn admission_blob_queue_depth_exceeds_threshold() {
        let state = WritePressureState {
            writer_queue_depth: 0,
            batch_commit_latency_ms: 0,
            blob_queue_depth: 100,
            compaction_stall_active: false,
            storage_stall_active: false,
        };
        let result = crate::admission::check::check_admission(&state);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(AdmissionError::BlobQueueDepthExceeded { .. })
        ));
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
            let result = circuit_breaker::record_failure(&wf, &hash, &config, &state, now).unwrap();
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
        let result = circuit_breaker::record_failure(&wf, &hash2, &config, &state, now).unwrap();
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
    fn rotation_state_error_debug_format_contains_variant_name() {
        let err = RotationStateError::AlreadyRotating;
        let msg = format!("{err:?}");
        assert!(msg.contains("AlreadyRotating"));
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
            detail: "invalid UTF-8".to_string(),
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
    use crate::resource_quota::{
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
        let enforcer = QuotaEnforcer::new(NamespaceRegistry::new());
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

    #[test]
    fn enforcer_check_below_limit_passes() {
        let enforcer = make_enforcer_with_cpu("test-ns", 100);
        assert!(enforcer.check_cpu("test-ns", 50).is_ok());
        assert!(enforcer.check_cpu("test-ns", 51).is_ok());
    }

    #[test]
    fn enforcer_check_over_single_limit_fails() {
        let enforcer = make_enforcer_with_cpu("test-ns", 10);
        let result = enforcer.check_cpu("test-ns", 11);
        assert!(matches!(result, Err(QuotaError::QuotaExceeded { .. })));
    }

    #[test]
    fn enforcer_overcommit_allows_exceeding_limit() {
        let mut registry = NamespaceRegistry::new();
        registry
            .register(
                NamespaceQuota::new("test-ns")
                    .with_cpu(CpuQuota::new(NonZeroU64::new(10).unwrap()))
                    .with_overcommit(OvercommitPolicy::AllowOvercommit),
            )
            .unwrap();
        let enforcer = QuotaEnforcer::new(registry);
        assert!(enforcer.check_cpu("test-ns", 100).is_ok());
    }

    #[test]
    fn quota_error_display_all_variants() {
        let errors = vec![
            QuotaError::QuotaExceeded {
                resource: ResourceKind::Cpu,
                namespace: "ns".to_string(),
                requested: 10,
                available: 5,
            },
            QuotaError::NamespaceNotFound("ghost".to_string()),
            QuotaError::QuotaNotConfigured {
                resource: ResourceKind::Memory,
                namespace: "ns".to_string(),
            },
        ];
        for err in &errors {
            let msg = err.to_string();
            assert!(!msg.is_empty(), "error display empty for {:?}", err);
        }
    }
}

mod workflow_version_boundary {
    use crate::workflow_version::{WorkflowVersion, WorkflowVersionError};
    use vo_types::{BinaryHash, TimestampMs, WorkflowName};

    #[test]
    fn short_hash_rejected() {
        let name = WorkflowName::parse("test-wf").unwrap();
        let hash = BinaryHash::parse("aabbccdd").unwrap();
        let ts = TimestampMs::now();
        let result = WorkflowVersion::new(name, hash, ts);
        assert!(matches!(result, Err(WorkflowVersionError::HashTooShort)));
    }

    #[test]
    fn exact_64_char_hash_accepted() {
        let name = WorkflowName::parse("test-wf").unwrap();
        let hash = BinaryHash::parse(&"a".repeat(64)).unwrap();
        let ts = TimestampMs::now();
        let result = WorkflowVersion::new(name, hash, ts);
        assert!(result.is_ok());
        let wv = result.unwrap();
        assert_eq!(wv.schema_version(), 1);
    }

    #[test]
    fn long_hash_accepted() {
        let name = WorkflowName::parse("test-wf").unwrap();
        let hash = BinaryHash::parse(&"b".repeat(128)).unwrap();
        let ts = TimestampMs::now();
        let result = WorkflowVersion::new(name, hash, ts);
        assert!(result.is_ok());
    }

    #[test]
    fn binary_path_includes_hash_and_name() {
        let name = WorkflowName::parse("my-wf").unwrap();
        let hash = BinaryHash::parse(&"c".repeat(64)).unwrap();
        let ts = TimestampMs::now();
        let wv = WorkflowVersion::new(name, hash, ts).unwrap();
        let path = wv.binary_path();
        assert!(path.contains("my-wf"));
        assert!(path.contains(&"c".repeat(64)));
    }
}

mod debounce_boundary {
    use crate::debounce::Error;
    

    #[test]
    fn error_display_all_variants() {
        let errors = vec![
            Error::InvalidDebounceDuration,
            Error::WatcherChannelClosed,
            Error::DebouncerInternal,
            Error::NoRuntime,
        ];
        for err in &errors {
            let msg = err.to_string();
            assert!(!msg.is_empty(), "error display empty for {:?}", err);
        }
    }

    #[test]
    fn error_zero_duration_message_is_descriptive() {
        let msg = Error::InvalidDebounceDuration.to_string();
        assert!(msg.to_lowercase().contains("zero"));
    }
}

mod workspace_swap_boundary {
    use crate::workspace_swap::{AtomicSwap, SwapError, SwapPhase, SwapStatus};
    use std::fs;

    #[test]
    fn swap_status_no_prior_swap_when_no_journal() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::new(&workspace);
        assert_eq!(swap.check_status().unwrap(), SwapStatus::NoPriorSwap);
    }

    #[test]
    fn swap_status_incomplete_after_stage() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();
        assert_eq!(
            swap.check_status().unwrap(),
            SwapStatus::Incomplete(SwapPhase::Staged)
        );
    }

    #[test]
    fn swap_status_complete_after_full_swap() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("file.txt"), "data").unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();
        swap.commit().unwrap();
        assert_eq!(swap.check_status().unwrap(), SwapStatus::NoPriorSwap);
    }

    #[test]
    fn swap_error_display_all_variants() {
        let errors = vec![
            SwapError::NotADirectory("/tmp/x".into()),
            SwapError::WorkspaceNotFound("/tmp/y".into()),
            SwapError::ShadowExists("/tmp/z".into()),
        ];
        for err in &errors {
            let msg = err.to_string();
            assert!(!msg.is_empty(), "error display empty for {:?}", err);
        }
    }

    #[test]
    fn swap_status_equality() {
        assert_eq!(SwapStatus::NoPriorSwap, SwapStatus::NoPriorSwap);
        assert_eq!(SwapStatus::Complete, SwapStatus::Complete);
        assert_eq!(
            SwapStatus::Incomplete(SwapPhase::Staging),
            SwapStatus::Incomplete(SwapPhase::Staging)
        );
        assert_ne!(SwapStatus::NoPriorSwap, SwapStatus::Complete);
    }

    #[test]
    fn swap_stage_fails_for_missing_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nonexistent");
        let swap = AtomicSwap::new(&missing);
        let result = swap.stage();
        assert!(matches!(result, Err(SwapError::WorkspaceNotFound(_))));
    }
}

mod admission_multi_indicator_boundary {
    use crate::admission::types::{
        AdmissionError, AdmissionThresholds, PressureIndicator, WritePressureState,
    };

    #[test]
    fn stall_and_queue_depth_triggers_multiple_indicators() {
        let state = WritePressureState {
            writer_queue_depth: 200,
            batch_commit_latency_ms: 0,
            blob_queue_depth: 0,
            compaction_stall_active: true,
            storage_stall_active: false,
        };
        let thresholds = AdmissionThresholds {
            writer_queue_depth_threshold: 100,
            batch_commit_latency_ms_threshold: 1000,
            blob_queue_depth_threshold: 50,
        };
        let result = crate::admission::check::check_admission_with_thresholds(&state, &thresholds);
        assert!(matches!(
            result,
            Err(AdmissionError::MultiplePressureIndicators { .. })
        ));
        if let Err(AdmissionError::MultiplePressureIndicators { indicators }) = result {
            assert!(indicators.contains(&PressureIndicator::WriterQueueDepth));
            assert!(indicators.contains(&PressureIndicator::CompactionStall));
            assert_eq!(indicators.len(), 2);
        }
    }

    #[test]
    fn all_three_queues_exceeded_triggers_multiple() {
        let state = WritePressureState {
            writer_queue_depth: 200,
            batch_commit_latency_ms: 2000,
            blob_queue_depth: 100,
            compaction_stall_active: false,
            storage_stall_active: true,
        };
        let thresholds = AdmissionThresholds {
            writer_queue_depth_threshold: 100,
            batch_commit_latency_ms_threshold: 1000,
            blob_queue_depth_threshold: 50,
        };
        let result = crate::admission::check::check_admission_with_thresholds(&state, &thresholds);
        assert!(matches!(
            result,
            Err(AdmissionError::MultiplePressureIndicators { .. })
        ));
        if let Err(AdmissionError::MultiplePressureIndicators { indicators }) = result {
            assert_eq!(indicators.len(), 4);
        }
    }

    #[test]
    fn zero_state_zero_thresholds_no_stalls_passes() {
        let state = WritePressureState {
            writer_queue_depth: 0,
            batch_commit_latency_ms: 0,
            blob_queue_depth: 0,
            compaction_stall_active: false,
            storage_stall_active: false,
        };
        let thresholds = AdmissionThresholds {
            writer_queue_depth_threshold: 0,
            batch_commit_latency_ms_threshold: 0,
            blob_queue_depth_threshold: 0,
        };
        let result = crate::admission::check::check_admission_with_thresholds(&state, &thresholds);
        assert!(result.is_ok());
    }
}

mod circuit_breaker_failure_window_boundary {
    use super::*;
    use crate::circuit_breaker::failure_window::{
        record_failure_in_window, unique_failures_in_window, FailureWindow,
    };

    #[test]
    fn failure_window_new_is_empty() {
        let window = FailureWindow::new();
        assert!(window.is_empty());
        assert_eq!(window.len(), 0);
    }

    #[test]
    fn failure_window_record_increases_count() {
        let mut window = FailureWindow::new();
        let now = Instant::now();
        let hash = vo_types::BinaryHash::parse("aabbccdd").unwrap();
        let window_duration = Duration::from_secs(60);
        let count = record_failure_in_window(&mut window, hash, now, window_duration);
        assert_eq!(count, 1);
    }

    #[test]
    fn failure_window_duplicate_hash_does_not_increase_count() {
        let mut window = FailureWindow::new();
        let now = Instant::now();
        let hash = vo_types::BinaryHash::parse("aabbccdd").unwrap();
        let window_duration = Duration::from_secs(60);
        record_failure_in_window(&mut window, hash.clone(), now, window_duration);
        let count = record_failure_in_window(&mut window, hash, now, window_duration);
        assert_eq!(count, 1);
    }

    #[test]
    fn failure_window_records_expire() {
        let mut window = FailureWindow::new();
        let window_duration = Duration::from_millis(1);
        let past = Instant::now() - Duration::from_secs(10);
        let hash = vo_types::BinaryHash::parse("aabbccdd").unwrap();
        record_failure_in_window(&mut window, hash, past, window_duration);
        let now = Instant::now();
        let count = unique_failures_in_window(&mut window, now, window_duration);
        assert_eq!(count, 0);
    }
}
