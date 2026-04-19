//! BDD tests for ADR-030: Managed Effects and Sink Contracts.
//!
//! Scenarios:
//! 1. Given effect record E, When transition applied, Then effect state machine progresses correctly.
//! 2. Given effect failure, When retry policy applied, Then retry occurs within policy limits.
//! 3. Given compensating effect, When compensation runs, Then inverse operation recorded.

#![allow(clippy::unwrap_used)]

use vo_types::{
    apply_compensation_transition, apply_effect_transition, CompensationPolicy, CompensationRecord,
    CompensationStatus, CompensationTransitionEvent, EffectIntent, EffectKind, EffectRecord,
    EffectTransitionEvent, RetryPolicy,
};

// ============================================================================
// Scenario 1: Effect state machine progression
// ============================================================================

#[test]
fn given_prepared_effect_when_commit_transition_applied_then_state_is_committed() {
    let record = EffectRecord::new(
        "fx-commit-001".to_string(),
        EffectKind::HttpCall,
        serde_json::json!({"url": "https://api.example.com/charge"}),
        EffectIntent::Prepared,
        None,
    )
    .expect("valid effect record");

    assert_eq!(record.status(), EffectIntent::Prepared);
    assert!(!record.status().is_terminal());

    let next = apply_effect_transition(EffectIntent::Prepared, EffectTransitionEvent::Commit)
        .expect("commit transition should succeed");

    assert_eq!(next, EffectIntent::Committed);
    assert!(next.is_terminal());
}

#[test]
fn given_prepared_effect_when_rollback_transition_applied_then_state_is_rolled_back() {
    let record = EffectRecord::new(
        "fx-rollback-001".to_string(),
        EffectKind::SqlQuery,
        serde_json::json!({"query": "INSERT INTO orders VALUES (1)"}),
        EffectIntent::Prepared,
        None,
    )
    .expect("valid effect record");

    assert_eq!(record.status(), EffectIntent::Prepared);

    let next = apply_effect_transition(EffectIntent::Prepared, EffectTransitionEvent::Rollback)
        .expect("rollback transition should succeed");

    assert_eq!(next, EffectIntent::RolledBack);
    assert!(next.is_terminal());
}

#[test]
fn given_committed_effect_when_any_transition_applied_then_terminal_error_returned() {
    let next = apply_effect_transition(EffectIntent::Committed, EffectTransitionEvent::Commit);
    assert!(next.is_err());

    let next = apply_effect_transition(EffectIntent::Committed, EffectTransitionEvent::Rollback);
    assert!(next.is_err());
}

#[test]
fn given_rolled_back_effect_when_any_transition_applied_then_terminal_error_returned() {
    let next = apply_effect_transition(EffectIntent::RolledBack, EffectTransitionEvent::Commit);
    assert!(next.is_err());

    let next = apply_effect_transition(EffectIntent::RolledBack, EffectTransitionEvent::Rollback);
    assert!(next.is_err());
}

#[test]
fn given_effect_record_when_constructed_then_all_fields_preserved() {
    let ts = vo_types::TimestampMs::try_from(1_700_000_000_000u64).unwrap();
    let record = EffectRecord::new(
        "fx-preserve-001".to_string(),
        EffectKind::BlobWrite,
        serde_json::json!({"bucket": "data-bucket", "key": "obj-42"}),
        EffectIntent::Prepared,
        Some(ts),
    )
    .expect("valid effect record");

    assert_eq!(record.intent_id(), "fx-preserve-001");
    assert_eq!(record.kind(), EffectKind::BlobWrite);
    assert_eq!(
        record.params_json(),
        &serde_json::json!({"bucket": "data-bucket", "key": "obj-42"})
    );
    assert_eq!(record.status(), EffectIntent::Prepared);
    assert_eq!(record.committed_at(), Some(&ts));
}

#[test]
fn given_effect_record_when_empty_intent_id_then_construction_rejected() {
    let result = EffectRecord::new(
        "".to_string(),
        EffectKind::HttpCall,
        serde_json::json!({}),
        EffectIntent::Prepared,
        None,
    );
    assert_eq!(result, None);
}

#[test]
fn given_effect_kind_when_all_variants_inspected_then_exactly_three_kinds() {
    let variants = EffectKind::all_variants();
    assert_eq!(variants.len(), 3);
    assert!(variants.contains(&EffectKind::HttpCall));
    assert!(variants.contains(&EffectKind::SqlQuery));
    assert!(variants.contains(&EffectKind::BlobWrite));
}

#[test]
fn given_effect_intent_when_full_lifecycle_then_prepared_to_committed_is_one_directional() {
    assert!(!EffectIntent::Prepared.is_terminal());
    assert!(EffectIntent::Committed.is_terminal());
    assert!(EffectIntent::RolledBack.is_terminal());

    let commit = apply_effect_transition(EffectIntent::Prepared, EffectTransitionEvent::Commit);
    assert_eq!(commit, Ok(EffectIntent::Committed));

    let rollback = apply_effect_transition(EffectIntent::Prepared, EffectTransitionEvent::Rollback);
    assert_eq!(rollback, Ok(EffectIntent::RolledBack));
}

// ============================================================================
// Scenario 2: Effect failure + retry policy
// ============================================================================

#[test]
fn given_valid_retry_policy_when_constructed_then_fields_preserved() {
    let policy = RetryPolicy::new(3, 100, 2.0).expect("valid retry policy");

    assert_eq!(policy.max_attempts, 3);
    assert_eq!(policy.backoff_ms, 100);
    assert_eq!(policy.backoff_multiplier, 2.0);
}

#[test]
fn given_retry_policy_when_backoff_calculated_then_respects_exponential_formula() {
    let policy = RetryPolicy::new(5, 100, 2.0).expect("valid retry policy");

    assert_eq!(policy.calculate_backoff_delay(0), 0);
    assert_eq!(policy.calculate_backoff_delay(1), 100);
    assert_eq!(policy.calculate_backoff_delay(2), 200);
    assert_eq!(policy.calculate_backoff_delay(3), 400);
    assert_eq!(policy.calculate_backoff_delay(4), 800);
}

#[test]
fn given_retry_policy_with_max_backoff_when_delay_exceeds_cap_then_capped() {
    let policy =
        RetryPolicy::with_max_backoff(5, 100, 2.0, 300).expect("valid retry policy with cap");

    assert_eq!(policy.calculate_backoff_delay(1), 100);
    assert_eq!(policy.calculate_backoff_delay(2), 200);
    assert_eq!(policy.calculate_backoff_delay(3), 300);
    assert_eq!(policy.calculate_backoff_delay(4), 300);
}

#[test]
fn given_zero_max_attempts_when_retry_policy_constructed_then_error_returned() {
    let result = RetryPolicy::new(0, 100, 2.0);
    assert!(result.is_err());
}

#[test]
fn given_invalid_multiplier_when_retry_policy_constructed_then_error_returned() {
    let nan = RetryPolicy::new(3, 100, f64::NAN);
    assert!(nan.is_err());

    let inf = RetryPolicy::new(3, 100, f64::INFINITY);
    assert!(inf.is_err());

    let below_one = RetryPolicy::new(3, 100, 0.5);
    assert!(below_one.is_err());
}

#[test]
fn given_effect_failure_when_retry_within_policy_then_retransition_possible() {
    let record = EffectRecord::new(
        "fx-retry-001".to_string(),
        EffectKind::HttpCall,
        serde_json::json!({"url": "https://flaky.example.com"}),
        EffectIntent::Prepared,
        None,
    )
    .expect("valid effect record");

    assert_eq!(record.status(), EffectIntent::Prepared);

    let policy = RetryPolicy::new(3, 100, 2.0).expect("valid policy");

    for attempt in 1..=policy.max_attempts {
        let _delay = policy.calculate_backoff_delay(attempt as u32);

        let result = apply_effect_transition(EffectIntent::Prepared, EffectTransitionEvent::Commit);
        assert_eq!(result, Ok(EffectIntent::Committed));
        break;
    }
}

#[test]
fn given_effect_failure_when_retry_exhausted_then_effect_stays_in_prepared_for_rollback() {
    let policy = RetryPolicy::new(3, 100, 2.0).expect("valid policy");

    let mut delays: Vec<u64> = Vec::new();
    for attempt in 1..=policy.max_attempts {
        delays.push(policy.calculate_backoff_delay(attempt as u32));
    }

    assert_eq!(delays.len(), 3);

    let result = apply_effect_transition(EffectIntent::Prepared, EffectTransitionEvent::Rollback);
    assert_eq!(result, Ok(EffectIntent::RolledBack));
    assert!(result.unwrap().is_terminal());
}

#[test]
fn given_retry_policy_when_max_backoff_too_small_then_error_returned() {
    let result = RetryPolicy::with_max_backoff(3, 500, 2.0, 100);
    assert!(result.is_err());
}

#[test]
fn given_retry_policy_when_backoff_multiplier_is_one_then_constant_delay() {
    let policy = RetryPolicy::new(5, 100, 1.0).expect("valid policy");

    for attempt in 1..=5 {
        assert_eq!(policy.calculate_backoff_delay(attempt as u32), 100);
    }
}

// ============================================================================
// Scenario 3: Compensating effect — inverse operation recorded
// ============================================================================

#[test]
fn given_compensating_effect_when_compensation_starts_then_status_progresses_to_in_progress() {
    let record = CompensationRecord::new(
        "fx-comp-001".to_string(),
        CompensationPolicy::Automatic,
        CompensationStatus::Pending,
        Some("comp-fx-comp-001".to_string()),
        None,
        None,
    )
    .expect("valid compensation record");

    assert_eq!(record.status(), CompensationStatus::Pending);
    assert!(!record.status().is_terminal());
    assert_eq!(record.policy(), CompensationPolicy::Automatic);
    assert_eq!(record.compensation_effect_id(), Some("comp-fx-comp-001"));

    let next = apply_compensation_transition(
        CompensationStatus::Pending,
        CompensationTransitionEvent::Start,
    )
    .expect("start transition should succeed");

    assert_eq!(next, CompensationStatus::InProgress);
    assert!(!next.is_terminal());
}

#[test]
fn given_in_progress_compensation_when_succeeds_then_status_is_succeeded() {
    let next = apply_compensation_transition(
        CompensationStatus::InProgress,
        CompensationTransitionEvent::Succeed,
    )
    .expect("succeed transition should work");

    assert_eq!(next, CompensationStatus::Succeeded);
    assert!(next.is_terminal());
}

#[test]
fn given_in_progress_compensation_when_fails_then_status_is_failed() {
    let next = apply_compensation_transition(
        CompensationStatus::InProgress,
        CompensationTransitionEvent::Fail,
    )
    .expect("fail transition should work");

    assert_eq!(next, CompensationStatus::Failed);
    assert!(next.is_terminal());
}

#[test]
fn given_pending_compensation_when_directly_fails_then_status_is_failed() {
    let next = apply_compensation_transition(
        CompensationStatus::Pending,
        CompensationTransitionEvent::Fail,
    )
    .expect("fail from pending should work");

    assert_eq!(next, CompensationStatus::Failed);
    assert!(next.is_terminal());
}

#[test]
fn given_compensation_record_when_inverse_operation_recorded_then_fields_captured() {
    let started = vo_types::TimestampMs::try_from(1000u64).unwrap();
    let completed = vo_types::TimestampMs::try_from(2500u64).unwrap();
    let record = CompensationRecord::new(
        "fx-inv-001".to_string(),
        CompensationPolicy::Automatic,
        CompensationStatus::Succeeded,
        Some("comp-fx-inv-001".to_string()),
        Some(started),
        Some(completed),
    )
    .expect("valid record");

    assert_eq!(record.effect_id(), "fx-inv-001");
    assert_eq!(record.policy(), CompensationPolicy::Automatic);
    assert_eq!(record.status(), CompensationStatus::Succeeded);
    assert_eq!(record.compensation_effect_id(), Some("comp-fx-inv-001"));
    assert_eq!(record.started_at(), Some(&started));
    assert_eq!(record.completed_at(), Some(&completed));
}

#[test]
fn given_compensation_policy_none_when_checked_then_no_compensation_needed() {
    let record = CompensationRecord::new(
        "fx-none-001".to_string(),
        CompensationPolicy::None,
        CompensationStatus::NotNeeded,
        None,
        None,
        None,
    )
    .expect("valid record");

    assert_eq!(record.policy(), CompensationPolicy::None);
    assert_eq!(record.status(), CompensationStatus::NotNeeded);
    assert!(record.status().is_terminal());
    assert_eq!(record.compensation_effect_id(), None);
}

#[test]
fn given_manual_compensation_when_pending_then_human_intervention_required() {
    let record = CompensationRecord::new(
        "fx-manual-001".to_string(),
        CompensationPolicy::Manual,
        CompensationStatus::Pending,
        None,
        None,
        None,
    )
    .expect("valid record");

    assert_eq!(record.policy(), CompensationPolicy::Manual);
    assert_eq!(record.status(), CompensationStatus::Pending);
    assert!(!record.status().is_terminal());
}

#[test]
fn given_compensation_when_full_lifecycle_then_pending_to_in_progress_to_succeeded() {
    let result1 = apply_compensation_transition(
        CompensationStatus::Pending,
        CompensationTransitionEvent::Start,
    );
    assert_eq!(result1, Ok(CompensationStatus::InProgress));

    let result2 = apply_compensation_transition(
        CompensationStatus::InProgress,
        CompensationTransitionEvent::Succeed,
    );
    assert_eq!(result2, Ok(CompensationStatus::Succeeded));
    assert!(result2.unwrap().is_terminal());
}

#[test]
fn given_terminal_compensation_when_any_transition_attempted_then_error_returned() {
    let terminal_states = [
        CompensationStatus::NotNeeded,
        CompensationStatus::Succeeded,
        CompensationStatus::Failed,
    ];
    let events = CompensationTransitionEvent::all_variants();

    for state in &terminal_states {
        for event in events {
            let result = apply_compensation_transition(*state, *event);
            assert!(
                result.is_err(),
                "terminal state {:?} should reject event {:?}",
                state,
                event
            );
        }
    }
}

#[test]
fn given_compensation_record_when_empty_effect_id_then_construction_rejected() {
    let result = CompensationRecord::new(
        "".to_string(),
        CompensationPolicy::Automatic,
        CompensationStatus::Pending,
        None,
        None,
        None,
    );
    assert_eq!(result, None);
}

#[test]
fn given_compensation_record_when_serialized_then_round_trip_preserves_fields() {
    let ts = vo_types::TimestampMs::try_from(9999u64).unwrap();
    let record = CompensationRecord::new(
        "fx-serde-001".to_string(),
        CompensationPolicy::Automatic,
        CompensationStatus::InProgress,
        Some("comp-serde-001".to_string()),
        Some(ts),
        None,
    )
    .expect("valid record");

    let json = serde_json::to_string(&record).expect("serialize");
    let recovered: CompensationRecord = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(recovered, record);
}
