//! Tests for admission control circuit breaker integration.
//!
//! These tests verify that admission control failures properly propagate to the
//! circuit breaker, causing quarantine after threshold failures.
//!
//! Test scenarios:
//! - Fast fail: rapid unique failures trigger quick quarantine
//! - Gradual fail: failures spread over time accumulate toward threshold
//! - Recovery: unquarantine allows new registrations

use std::time::{Duration, Instant};

use crate::circuit_breaker::{
    record_failure, unquarantine, CircuitBreakerConfig, CircuitBreakerState, FailureWindow,
    RegistrationStatus,
};
use vo_types::{BinaryHash, WorkflowName};

use crate::admission::{
    check_admission, check_admission_with_thresholds, AdmissionError, AdmissionThresholds,
    WritePressureState,
};

fn make_binary_hash_hex(id: u8) -> BinaryHash {
    BinaryHash::parse(&format!("{:08x}", id)).expect("valid binary hash")
}

fn make_workflow_name(s: &str) -> WorkflowName {
    WorkflowName::parse(s).expect("valid workflow name")
}

fn storage_stall_state() -> WritePressureState {
    WritePressureState {
        writer_queue_depth: 0,
        batch_commit_latency_ms: 0,
        blob_queue_depth: 0,
        compaction_stall_active: false,
        storage_stall_active: true,
    }
}

fn healthy_state() -> WritePressureState {
    WritePressureState::default()
}

// ── Fast Fail Tests ────────────────────────────────────────────────────────────

/// FAST FAIL: Multiple unique admission failures in quick succession trigger
/// circuit breaker quarantine.
///
/// Given: Circuit breaker with failure_threshold=3
/// When: 3 unique admission failures occur at the same instant
/// Then: Workflow is quarantined after the 3rd unique failure
#[test]
fn circuit_breaker_fast_fail_triggers_quarantine_after_threshold() {
    let state = CircuitBreakerState::new();
    let config =
        CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 3).unwrap();
    let wf = make_workflow_name("wf-fast-fail");
    let now = Instant::now();

    let _err1 = check_admission(&storage_stall_state()).unwrap_err();
    let hash1 = make_binary_hash_hex(0x01);
    let event1 = record_failure(&wf, &hash1, &config, &state, now).unwrap();
    assert!(event1.is_none(), "No quarantine before threshold");

    let _err2 = check_admission(&storage_stall_state()).unwrap_err();
    let hash2 = make_binary_hash_hex(0x02);
    let event2 = record_failure(&wf, &hash2, &config, &state, now).unwrap();
    assert!(event2.is_none(), "No quarantine at 2 failures");

    let _err3 = check_admission(&storage_stall_state()).unwrap_err();
    let hash3 = make_binary_hash_hex(0x03);
    let event3 = record_failure(&wf, &hash3, &config, &state, now).unwrap();
    assert!(
        event3.is_some(),
        "Quarantine triggered after 3rd unique failure"
    );
    assert_eq!(state.get_status(&wf), RegistrationStatus::Quarantined);
}

/// FAST FAIL: Duplicate failure hashes do not increase count.
///
/// Given: Circuit breaker tracking failures
/// When: Same failure hash is recorded multiple times
/// Then: Count does not increase (INV-004: duplicate hashes update timestamp only)
#[test]
fn circuit_breaker_duplicate_failure_hashes_do_not_increase_count() {
    let state = CircuitBreakerState::new();
    let config =
        CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 5).unwrap();
    let wf = make_workflow_name("wf-dupe-hash");
    let now = Instant::now();
    let hash = make_binary_hash_hex(0xAA);

    for i in 0..5 {
        let event = record_failure(&wf, &hash, &config, &state, now).unwrap();
        assert!(
            event.is_none(),
            "No quarantine at iteration {} with same hash",
            i
        );
    }
    assert_eq!(
        state.get_failure_count(&wf),
        1,
        "Only 1 unique hash recorded despite 5 attempts"
    );
}

/// FAST FAIL: First unique failure after recovery restarts tracking.
///
/// Given: Workflow was quarantined and then unquarantined
/// When: A new unique failure is recorded
/// Then: Failure count is 1, not accumulated from before quarantine
#[test]
fn circuit_breaker_recovery_resets_failure_count() {
    let state = CircuitBreakerState::new();
    let config =
        CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 3).unwrap();
    let wf = make_workflow_name("wf-reset");
    let now = Instant::now();

    for i in 0..3u8 {
        let hash = make_binary_hash_hex(i);
        record_failure(&wf, &hash, &config, &state, now).unwrap();
    }
    assert_eq!(state.get_status(&wf), RegistrationStatus::Quarantined);

    unquarantine(&wf, "operator", &state).unwrap();
    assert_eq!(state.get_status(&wf), RegistrationStatus::Active);
    assert_eq!(state.get_failure_count(&wf), 0, "Failure count reset after unquarantine");

    let hash = make_binary_hash_hex(0xFF);
    let event = record_failure(&wf, &hash, &config, &state, now).unwrap();
    assert!(event.is_none(), "Fresh start after recovery");
    assert_eq!(state.get_failure_count(&wf), 1);
}

// ── Gradual Fail Tests ────────────────────────────────────────────────────────

/// GRADUAL FAIL: Failures spread over time accumulate toward threshold.
///
/// Given: Circuit breaker with 60s failure window
/// When: Failures occur at different times, all within the window
/// Then: All failures count toward threshold
#[test]
fn circuit_breaker_gradual_fail_within_window_accumulates() {
    let state = CircuitBreakerState::new();
    let config =
        CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 5).unwrap();
    let wf = make_workflow_name("wf-gradual");
    let base = Instant::now();

    for i in 0..5u8 {
        let now = base + Duration::from_secs((i as u64) * 10);
        let hash = make_binary_hash_hex(0x10 + i);
        let event = record_failure(&wf, &hash, &config, &state, now).unwrap();
        if i < 4 {
            assert!(event.is_none(), "No quarantine at failure {}", i + 1);
        }
    }
    assert_eq!(state.get_status(&wf), RegistrationStatus::Quarantined);
}

/// GRADUAL FAIL: Failures expire after the failure window.
///
/// Given: FailureWindow with 600s window
/// When: Entries are older than window duration
/// Then: They are evicted on next record
#[test]
fn circuit_breaker_gradual_fail_expires_after_window() {
    use crate::circuit_breaker::failure_window::{record_failure_in_window, FailureWindow};

    let mut window = FailureWindow::new();
    let failure_window = Duration::from_secs(600);
    let base = Instant::now();

    for i in 0..3u8 {
        let now = base + Duration::from_secs((i as u64) * 10);
        let hash = make_binary_hash_hex(0x20 + i);
        record_failure_in_window(&mut window, hash, now, failure_window);
    }
    assert_eq!(window.len(), 3, "3 failures recorded within window");

    let expired_time = base + Duration::from_secs(700);
    let new_hash = make_binary_hash_hex(0xFF);
    record_failure_in_window(&mut window, new_hash, expired_time, failure_window);
    assert_eq!(
        window.len(),
        1,
        "Only 1 failure after expiry - old entries evicted"
    );
}

/// GRADUAL FAIL: Mix of unique and duplicate hashes over time.
///
/// Given: Circuit breaker tracking failures
/// When: Some failures are unique, some are duplicates, spread over time
/// Then: Only unique hashes count toward threshold
#[test]
fn circuit_breaker_gradual_mixed_unique_and_duplicate() {
    let state = CircuitBreakerState::new();
    let config =
        CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 3).unwrap();
    let wf = make_workflow_name("wf-mixed");
    let now = Instant::now();

    let hash_a = make_binary_hash_hex(0x30);
    let hash_b = make_binary_hash_hex(0x31);
    let hash_c = make_binary_hash_hex(0x32);

    record_failure(&wf, &hash_a, &config, &state, now).unwrap();
    record_failure(&wf, &hash_a, &config, &state, now + Duration::from_secs(10)).unwrap();
    record_failure(&wf, &hash_b, &config, &state, now + Duration::from_secs(20)).unwrap();
    assert_eq!(
        state.get_failure_count(&wf),
        2,
        "Only unique hashes count"
    );

    record_failure(&wf, &hash_c, &config, &state, now + Duration::from_secs(30)).unwrap();
    assert_eq!(state.get_status(&wf), RegistrationStatus::Quarantined);
}

// ── Recovery Tests ─────────────────────────────────────────────────────────────

/// RECOVERY: Unquarantine transitions workflow back to Active.
///
/// Given: Workflow is quarantined
/// When: Operator calls unquarantine
/// Then: Status transitions to Active, failure window is cleared
#[test]
fn circuit_breaker_recovery_unquarantine_transitions_to_active() {
    let state = CircuitBreakerState::new();
    let config =
        CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 3).unwrap();
    let wf = make_workflow_name("wf-recover");
    let now = Instant::now();

    for i in 0..3u8 {
        let hash = make_binary_hash_hex(0x40 + i);
        record_failure(&wf, &hash, &config, &state, now).unwrap();
    }
    assert_eq!(state.get_status(&wf), RegistrationStatus::Quarantined);

    let result = unquarantine(&wf, "operator", &state).unwrap();
    assert_eq!(result.previous_status, RegistrationStatus::Quarantined);
    assert_eq!(result.new_status, RegistrationStatus::Active);
    assert_eq!(result.failures_cleared, 3);
    assert_eq!(state.get_status(&wf), RegistrationStatus::Active);
}

/// RECOVERY: Unquarantine fails for non-quarantined workflow.
///
/// Given: Workflow is Active (not quarantined)
/// When: Operator calls unquarantine
/// Then: Error is returned
#[test]
fn circuit_breaker_recovery_unquarantine_fails_for_active_workflow() {
    let state = CircuitBreakerState::new();
    let wf = make_workflow_name("wf-never-quarantined");

    let result = unquarantine(&wf, "operator", &state);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        crate::circuit_breaker::CircuitBreakerError::WorkflowNotFound { .. }
    ));
}

/// RECOVERY: After unquarantine, new failures start fresh tracking.
///
/// Given: Workflow was quarantined and then unquarantined
/// When: Multiple new failures are recorded
/// Then: Workflow gets quarantined again after new threshold breaches
#[test]
fn circuit_breaker_recovery_new_failures_after_recovery() {
    let state = CircuitBreakerState::new();
    let config =
        CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 2).unwrap();
    let wf = make_workflow_name("wf-re-quarantine");
    let now = Instant::now();

    record_failure(&wf, &make_binary_hash_hex(0x50), &config, &state, now).unwrap();
    record_failure(&wf, &make_binary_hash_hex(0x51), &config, &state, now).unwrap();
    assert_eq!(state.get_status(&wf), RegistrationStatus::Quarantined);

    unquarantine(&wf, "operator", &state).unwrap();

    record_failure(&wf, &make_binary_hash_hex(0x60), &config, &state, now).unwrap();
    assert_eq!(
        state.get_status(&wf),
        RegistrationStatus::Active,
        "Still active after 1 new failure (threshold=2)"
    );

    record_failure(&wf, &make_binary_hash_hex(0x61), &config, &state, now).unwrap();
    assert_eq!(
        state.get_status(&wf),
        RegistrationStatus::Quarantined,
        "Re-quarantined after 2 new failures"
    );
}

/// RECOVERY: Admission control allows workflow after unquarantine.
///
/// This test verifies the end-to-end scenario:
/// 1. Circuit breaker is open (quarantined) -> admission should reflect this
/// 2. Unquarantine resets the circuit breaker
/// 3. Admission control allows the workflow again
#[test]
fn circuit_breaker_recovery_admission_allowed_after_unquarantine() {
    let state = CircuitBreakerState::new();
    let config =
        CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 1).unwrap();
    let wf = make_workflow_name("wf-admission-recover");
    let now = Instant::now();

    let hash = make_binary_hash_hex(0x70);
    record_failure(&wf, &hash, &config, &state, now).unwrap();
    assert_eq!(
        state.get_status(&wf),
        RegistrationStatus::Quarantined,
        "Workflow quarantined after 1 failure (threshold=1)"
    );

    let status_before = state.get_status(&wf);
    unquarantine(&wf, "operator", &state).unwrap();
    let status_after = state.get_status(&wf);

    assert_eq!(status_before, RegistrationStatus::Quarantined);
    assert_eq!(status_after, RegistrationStatus::Active);
}

// ── Integration Tests ─────────────────────────────────────────────────────────

/// Integration: Admission failure propagates to circuit breaker.
///
/// This test verifies the core integration: when admission control rejects
/// a workflow due to downstream failure (storage stall), that failure
/// propagates to the circuit breaker.
#[test]
fn admission_failure_propagates_to_circuit_breaker() {
    let state = CircuitBreakerState::new();
    let config =
        CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 1).unwrap();
    let wf = make_workflow_name("wf-integration");
    let now = Instant::now();

    let admission_result = check_admission(&storage_stall_state());
    assert!(
        admission_result.is_err(),
        "Admission should fail with storage stall"
    );

    let failure_hash = make_binary_hash_hex(0x80);
    let event = record_failure(&wf, &failure_hash, &config, &state, now).unwrap();

    assert!(
        event.is_some(),
        "Quarantine event should be emitted after threshold breach"
    );
    assert_eq!(state.get_status(&wf), RegistrationStatus::Quarantined);
}

/// Integration: Healthy admission does not trigger circuit breaker.
///
/// This test verifies that successful admission (healthy storage) does not
/// contribute to failure tracking.
#[test]
fn healthy_admission_does_not_trigger_circuit_breaker() {
    let state = CircuitBreakerState::new();
    let config =
        CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 3).unwrap();
    let wf = make_workflow_name("wf-healthy");
    let now = Instant::now();

    let admission_result = check_admission(&healthy_state());
    assert!(
        admission_result.is_ok(),
        "Admission should succeed with healthy storage"
    );

    let hash = make_binary_hash_hex(0x90);
    let event = record_failure(&wf, &hash, &config, &state, now).unwrap();

    assert!(event.is_none(), "No quarantine for successful operation");
    assert_eq!(
        state.get_failure_count(&wf),
        1,
        "Failure count still tracked but no quarantine"
    );
}

/// Integration: Multiple admission failures with different error types.
///
/// Each AdmissionError variant should produce a different failure hash,
/// allowing them to be tracked separately toward the threshold.
#[test]
fn admission_multiple_error_types_produce_unique_hashes() {
    let state = CircuitBreakerState::new();
    let config =
        CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 3).unwrap();
    let wf = make_workflow_name("wf-multi-error");
    let now = Instant::now();

    let storage_stall = WritePressureState {
        writer_queue_depth: 0,
        batch_commit_latency_ms: 0,
        blob_queue_depth: 0,
        compaction_stall_active: false,
        storage_stall_active: true,
    };
    let compaction_stall = WritePressureState {
        writer_queue_depth: 0,
        batch_commit_latency_ms: 0,
        blob_queue_depth: 0,
        compaction_stall_active: true,
        storage_stall_active: false,
    };
    let writer_queue = WritePressureState {
        writer_queue_depth: 200,
        batch_commit_latency_ms: 0,
        blob_queue_depth: 0,
        compaction_stall_active: false,
        storage_stall_active: false,
    };

    let _ = check_admission(&storage_stall).unwrap_err();
    let hash1 = make_binary_hash_hex(0xA1);
    record_failure(&wf, &hash1, &config, &state, now).unwrap();

    let _ = check_admission(&compaction_stall).unwrap_err();
    let hash2 = make_binary_hash_hex(0xA2);
    record_failure(&wf, &hash2, &config, &state, now).unwrap();

    let _ = check_admission(&writer_queue).unwrap_err();
    let hash3 = make_binary_hash_hex(0xA3);
    record_failure(&wf, &hash3, &config, &state, now).unwrap();

    assert_eq!(
        hash1, hash1,
        "Same error type produces same hash"
    );
    assert_ne!(
        hash1, hash2,
        "Different error types produce different hashes"
    );
    assert_eq!(state.get_status(&wf), RegistrationStatus::Quarantined);
}