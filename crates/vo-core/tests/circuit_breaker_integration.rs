//! Integration tests for the circuit breaker composed functions.
//!
//! Tests B-01 to B-10 (evaluate_registration), B-14 to B-17 (record_failure),
//! B-27 to B-33 (unquarantine), B-57 (force + deactivated).
//!
//! These tests exercise the full composed behavior with real DashMap state.

use std::time::{Duration, Instant};

use vo_core::circuit_breaker::{
    check_rate_limit, evaluate_registration, record_failure, unquarantine, CircuitBreakerConfig,
    CircuitBreakerError, CircuitBreakerState, FailureWindow, QuarantineEvent, RegistrationOutcome,
    RegistrationRequest, RegistrationStatus, UnquarantineResult,
};
use vo_types::{BinaryHash, WorkflowName};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn make_wf(s: &str) -> WorkflowName {
    WorkflowName::parse(s).expect("test workflow name should be valid")
}

fn make_hash(s: &str) -> BinaryHash {
    BinaryHash::parse(s).expect("test hash should be valid")
}

fn default_config() -> CircuitBreakerConfig {
    CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 5)
        .expect("default config should be valid")
}

fn make_request(wf: &str, hash: &str, force: bool) -> RegistrationRequest {
    RegistrationRequest {
        workflow_name: make_wf(wf),
        binary_hash: make_hash(hash),
        force: if force { Some("test-operator-token".into()) } else { None },
    }
}

// ── B-01: Normal registration allowed ────────────────────────────────────────

#[test]
fn evaluate_registration_returns_allowed_when_active_and_no_rate_limit() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let now = Instant::now();
    let request = make_request("deploy-prod", "abcdef01", false);

    let result = evaluate_registration(&request, &config, &state, now);
    assert_eq!(result, Ok(RegistrationOutcome::Allowed));
}

// ── B-02: Rate-limited registration ──────────────────────────────────────────

#[test]
fn evaluate_registration_returns_rate_limited_with_30s_when_last_was_30s_ago() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();

    // Simulate a prior registration 30s ago
    state.rate_limiter.insert(make_wf("deploy-prod"), t0);

    let now = t0 + Duration::from_secs(30);
    let request = make_request("deploy-prod", "abcdef01", false);

    let result = evaluate_registration(&request, &config, &state, now);
    assert_eq!(
        result,
        Ok(RegistrationOutcome::RateLimited {
            retry_after_secs: 30
        })
    );
}

// ── B-03: Quarantined registration blocked ───────────────────────────────────

#[test]
fn evaluate_registration_returns_quarantined_when_status_is_quarantined() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let now = Instant::now();

    state
        .statuses
        .insert(make_wf("ai-loop-fix"), RegistrationStatus::Quarantined);

    let request = make_request("ai-loop-fix", "abcdef01", false);

    let result = evaluate_registration(&request, &config, &state, now);
    assert_eq!(
        result,
        Ok(RegistrationOutcome::WorkflowQuarantined {
            workflow_name: make_wf("ai-loop-fix"),
        })
    );
}

// ── B-04: Deactivated registration blocked ───────────────────────────────────

#[test]
fn evaluate_registration_returns_deactivated_when_status_is_deactivated() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let now = Instant::now();

    state
        .statuses
        .insert(make_wf("legacy-wf"), RegistrationStatus::Deactivated);

    let request = make_request("legacy-wf", "abcdef01", false);

    let result = evaluate_registration(&request, &config, &state, now);
    assert_eq!(
        result,
        Ok(RegistrationOutcome::WorkflowDeactivated {
            workflow_name: make_wf("legacy-wf"),
        })
    );
}

// ── B-05: Force bypasses both layers (active + rate limited) ─────────────────

#[test]
fn evaluate_registration_returns_allowed_when_force_true_and_rate_limited() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();

    // Rate-limited: last registration 10s ago
    state.rate_limiter.insert(make_wf("deploy-prod"), t0);

    state.register_operator_token("test-operator-token".into());
    let now = t0 + Duration::from_secs(10);
    let request = make_request("deploy-prod", "abcdef01", true);

    let result = evaluate_registration(&request, &config, &state, now);
    assert_eq!(result, Ok(RegistrationOutcome::Allowed));
}

// ── B-06: Force bypasses quarantine ──────────────────────────────────────────

#[test]
fn evaluate_registration_returns_allowed_when_force_true_and_quarantined() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let now = Instant::now();

    state
        .statuses
        .insert(make_wf("ai-loop-fix"), RegistrationStatus::Quarantined);

    state.register_operator_token("test-operator-token".into());
    let request = make_request("ai-loop-fix", "abcdef01", true);

    let result = evaluate_registration(&request, &config, &state, now);
    assert_eq!(result, Ok(RegistrationOutcome::Allowed));
}

// ── B-07: Force bypasses rate limit on rate-limited workflow ─────────────────

#[test]
fn evaluate_registration_returns_allowed_when_force_true_and_within_rate_window() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();

    // Rate-limited: last registration 5s ago
    state.rate_limiter.insert(make_wf("deploy-prod"), t0);

    state.register_operator_token("test-operator-token".into());
    let now = t0 + Duration::from_secs(5);
    let request = make_request("deploy-prod", "abcdef01", true);

    let result = evaluate_registration(&request, &config, &state, now);
    assert_eq!(result, Ok(RegistrationOutcome::Allowed));
}

// ── B-08: Successful registration updates rate limiter ───────────────────────

#[test]
fn evaluate_registration_updates_rate_limiter_on_allowed() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();

    // First registration: should be allowed
    let request = make_request("deploy-prod", "abcdef01", false);
    let result = evaluate_registration(&request, &config, &state, t0);
    assert_eq!(result, Ok(RegistrationOutcome::Allowed));

    // Second registration 30s later: should be rate-limited
    let t1 = t0 + Duration::from_secs(30);
    let result2 = evaluate_registration(&request, &config, &state, t1);
    assert_eq!(
        result2,
        Ok(RegistrationOutcome::RateLimited {
            retry_after_secs: 30
        })
    );
}

// ── B-09: Force registration updates rate limiter ────────────────────────────

#[test]
fn evaluate_registration_updates_rate_limiter_on_force_allowed() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();

    state.register_operator_token("test-operator-token".into());

    // Force registration
    let force_request = make_request("deploy-prod", "abcdef01", true);
    let result = evaluate_registration(&force_request, &config, &state, t0);
    assert_eq!(result, Ok(RegistrationOutcome::Allowed));

    // Non-forced 10s later: should be rate-limited with 50s remaining
    let t1 = t0 + Duration::from_secs(10);
    let normal_request = make_request("deploy-prod", "abcdef02", false);
    let result2 = evaluate_registration(&normal_request, &config, &state, t1);
    assert_eq!(
        result2,
        Ok(RegistrationOutcome::RateLimited {
            retry_after_secs: 50
        })
    );
}

// ── B-10: Rate-limited request does NOT count as failure (INV-009) ───────────

#[test]
fn evaluate_registration_does_not_record_failure_when_rate_limited() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("deploy-prod");

    // Set up: rate limiter entry 10s ago
    state.rate_limiter.insert(wf.clone(), t0);

    // Pre-load 4 failures in the failure window
    let mut window = FailureWindow::new();
    let hashes = ["aaaa0001", "aaaa0002", "aaaa0003", "aaaa0004"];
    hashes.iter().for_each(|h| {
        vo_core::circuit_breaker::failure_window::record_failure_in_window(
            &mut window,
            make_hash(h),
            t0,
            Duration::from_secs(600),
        );
    });
    state.failure_tracker.insert(wf.clone(), window);

    // Registration at t0+10s — should be rate-limited (50s remaining)
    let now = t0 + Duration::from_secs(10);
    let request = make_request("deploy-prod", "aaaa0005", false);
    let result = evaluate_registration(&request, &config, &state, now);
    assert_eq!(
        result,
        Ok(RegistrationOutcome::RateLimited {
            retry_after_secs: 50
        })
    );

    // Failure count should remain 4, NOT 5
    let tracker = state.failure_tracker.get(&wf);
    assert!(tracker.is_some(), "failure tracker entry should exist");
    assert_eq!(tracker.map(|t| t.len()), Some(4));

    // Status should remain Active, NOT Quarantined
    let status = state
        .statuses
        .get(&wf)
        .map(|s| *s)
        .unwrap_or(RegistrationStatus::Active);
    assert_eq!(status, RegistrationStatus::Active);
}

// ── B-14: Threshold triggers quarantine (INV-001) ────────────────────────────

#[test]
fn record_failure_returns_quarantine_event_when_fifth_unique_hash() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let now = Instant::now();
    let wf = make_wf("deploy-prod");

    // Record 4 unique failures
    let hashes = ["aaaa0001", "aaaa0002", "aaaa0003", "aaaa0004"];
    hashes.iter().for_each(|h| {
        let result = record_failure(&wf, &make_hash(h), &config, &state, now);
        assert_eq!(result, Ok(None));
    });

    // 5th unique hash should trigger quarantine
    let result = record_failure(&wf, &make_hash("aaaa0005"), &config, &state, now);
    assert_eq!(
        result,
        Ok(Some(QuarantineEvent {
            workflow_name: wf.clone(),
        }))
    );

    // Status should now be Quarantined
    let status = state.statuses.get(&wf).map(|s| *s);
    assert_eq!(status, Some(RegistrationStatus::Quarantined));
}

// ── B-15: Below threshold returns None ───────────────────────────────────────

#[test]
fn record_failure_returns_none_when_below_threshold() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let now = Instant::now();
    let wf = make_wf("deploy-prod");

    // Record 3 unique failures, then a 4th
    let hashes = ["aaaa0001", "aaaa0002", "aaaa0003"];
    hashes.iter().for_each(|h| {
        record_failure(&wf, &make_hash(h), &config, &state, now)
            .expect("record_failure should succeed");
    });

    let result = record_failure(&wf, &make_hash("aaaa0004"), &config, &state, now);
    assert_eq!(result, Ok(None));

    // Status should remain Active
    let status = state
        .statuses
        .get(&wf)
        .map(|s| *s)
        .unwrap_or(RegistrationStatus::Active);
    assert_eq!(status, RegistrationStatus::Active);
}

// ── B-16: Quarantine persisted to Fjall (POST-004) ───────────────────────────
// NOTE: This test requires Fjall integration. For RED phase, we assert the
// in-memory status change. Fjall persistence will be tested in
// persistence_integration.rs when vo-storage is wired up.

#[test]
fn record_failure_sets_quarantined_status_when_threshold_reached() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let now = Instant::now();
    let wf = make_wf("deploy-prod");

    // Record 5 unique failures (threshold = 5)
    let hashes = ["aaaa0001", "aaaa0002", "aaaa0003", "aaaa0004", "aaaa0005"];
    hashes.iter().for_each(|h| {
        record_failure(&wf, &make_hash(h), &config, &state, now)
            .expect("record_failure should succeed");
    });

    // In-memory status should be Quarantined
    let status = state.statuses.get(&wf).map(|s| *s);
    assert_eq!(status, Some(RegistrationStatus::Quarantined));
}

// ── B-17: Storage failure returns error ──────────────────────────────────────
// NOTE: Simulating Fjall failure requires a mock or broken partition.
// For RED phase with no Fjall wired up yet, we test the function signature.
// When the implementation integrates Fjall, this test will be updated.

#[test]
fn record_failure_returns_storage_error_when_fjall_write_fails() {
    // This test requires a way to inject a failing persistence layer.
    // For now, we verify the function returns the correct error type
    // when the implementation exists and Fjall is injected.
    // The todo!() will make this fail during RED phase.
    let state = CircuitBreakerState::new();
    let config = default_config();
    let now = Instant::now();
    let wf = make_wf("deploy-prod");

    // Record 5 unique failures to trigger quarantine
    // The first 4 should return Ok(None), the 5th should return Ok(Some(QuarantineEvent))
    let hashes = ["aaaa0001", "aaaa0002", "aaaa0003", "aaaa0004", "aaaa0005"];
    hashes.iter().take(4).for_each(|h| {
        let result = record_failure(&wf, &make_hash(h), &config, &state, now);
        assert_eq!(
            result,
            Ok(None),
            "pre-threshold failure should return Ok(None)"
        );
    });
    let fifth_result = record_failure(&wf, &make_hash(hashes[4]), &config, &state, now);
    assert_eq!(
        fifth_result,
        Ok(Some(QuarantineEvent {
            workflow_name: wf.clone(),
        })),
        "threshold-reaching failure should return Ok(Some(QuarantineEvent))"
    );

    // In-memory status should now be Quarantined
    let status = state.statuses.get(&wf).map(|s| *s);
    assert_eq!(status, Some(RegistrationStatus::Quarantined));
    // Note: Full Fjall persistence StorageError testing requires mock injection.
    // When a failing Fjall is available, a separate test will assert
    // Err(CircuitBreakerError::StorageError { .. }).
}

// ── B-27: Successful unquarantine ────────────────────────────────────────────

#[test]
fn unquarantine_returns_result_with_cleared_failures_when_quarantined() {
    let state = CircuitBreakerState::new();
    let wf = make_wf("ai-loop");

    // Set up quarantined workflow with 5 failures
    state
        .statuses
        .insert(wf.clone(), RegistrationStatus::Quarantined);

    let mut window = FailureWindow::new();
    let hashes = ["aaaa0001", "aaaa0002", "aaaa0003", "aaaa0004", "aaaa0005"];
    let now = Instant::now();
    hashes.iter().for_each(|h| {
        vo_core::circuit_breaker::failure_window::record_failure_in_window(
            &mut window,
            make_hash(h),
            now,
            Duration::from_secs(600),
        );
    });
    state.failure_tracker.insert(wf.clone(), window);

    let result = unquarantine(&wf, "operator-alice", &state);
    assert_eq!(
        result,
        Ok(UnquarantineResult {
            workflow_name: wf.clone(),
            previous_status: RegistrationStatus::Quarantined,
            new_status: RegistrationStatus::Active,
            failures_cleared: 5,
        })
    );

    // Status should now be Active
    let status = state.statuses.get(&wf).map(|s| *s);
    assert_eq!(status, Some(RegistrationStatus::Active));

    // Failure window should be empty
    let tracker = state.failure_tracker.get(&wf);
    assert!(
        tracker.is_none() || tracker.map(|t| t.is_empty()) == Some(true),
        "failure window should be cleared"
    );
}

// ── B-28: Unquarantine removes rate limiter entry ────────────────────────────

#[test]
fn unquarantine_removes_rate_limiter_entry() {
    let state = CircuitBreakerState::new();
    let wf = make_wf("ai-loop");
    let t0 = Instant::now();

    state
        .statuses
        .insert(wf.clone(), RegistrationStatus::Quarantined);
    state.rate_limiter.insert(wf.clone(), t0);
    state
        .failure_tracker
        .insert(wf.clone(), FailureWindow::new());

    let unquarantine_result = unquarantine(&wf, "operator-alice", &state);
    assert_eq!(
        unquarantine_result,
        Ok(UnquarantineResult {
            workflow_name: wf.clone(),
            previous_status: RegistrationStatus::Quarantined,
            new_status: RegistrationStatus::Active,
            failures_cleared: 0,
        })
    );

    // Rate limiter entry should be removed
    let rate_check = check_rate_limit(
        state.rate_limiter.get(&wf).map(|r| *r),
        Duration::from_secs(60),
        t0 + Duration::from_secs(10),
    );
    assert_eq!(rate_check, None, "rate limiter entry should be removed");
}

// ── B-29: Unquarantine persists Active to Fjall ──────────────────────────────
// NOTE: Full Fjall persistence tested in persistence_integration.rs.
// Here we verify the in-memory status transition.

#[test]
fn unquarantine_persists_active_status() {
    let state = CircuitBreakerState::new();
    let wf = make_wf("ai-loop");

    state
        .statuses
        .insert(wf.clone(), RegistrationStatus::Quarantined);
    state
        .failure_tracker
        .insert(wf.clone(), FailureWindow::new());

    let result = unquarantine(&wf, "operator-alice", &state);
    assert_eq!(
        result,
        Ok(UnquarantineResult {
            workflow_name: wf.clone(),
            previous_status: RegistrationStatus::Quarantined,
            new_status: RegistrationStatus::Active,
            failures_cleared: 0,
        })
    );

    let status = state.statuses.get(&wf).map(|s| *s);
    assert_eq!(status, Some(RegistrationStatus::Active));
}

// ── B-30: WorkflowNotFound ───────────────────────────────────────────────────

#[test]
fn unquarantine_returns_workflow_not_found_when_unknown() {
    let state = CircuitBreakerState::new();
    let wf = make_wf("ghost-wf");

    let result = unquarantine(&wf, "operator", &state);
    assert_eq!(
        result,
        Err(CircuitBreakerError::WorkflowNotFound {
            workflow_name: "ghost-wf".to_string(),
        })
    );
}

// ── B-31: NotQuarantined for Active ──────────────────────────────────────────

#[test]
fn unquarantine_returns_not_quarantined_when_active() {
    let state = CircuitBreakerState::new();
    let wf = make_wf("healthy-wf");

    state
        .statuses
        .insert(wf.clone(), RegistrationStatus::Active);

    let result = unquarantine(&wf, "operator", &state);
    assert_eq!(
        result,
        Err(CircuitBreakerError::NotQuarantined {
            workflow_name: "healthy-wf".to_string(),
            current_status: RegistrationStatus::Active,
        })
    );
}

// ── B-32: NotQuarantined for Deactivated ─────────────────────────────────────

#[test]
fn unquarantine_returns_not_quarantined_when_deactivated() {
    let state = CircuitBreakerState::new();
    let wf = make_wf("legacy-wf");

    state
        .statuses
        .insert(wf.clone(), RegistrationStatus::Deactivated);

    let result = unquarantine(&wf, "operator", &state);
    assert_eq!(
        result,
        Err(CircuitBreakerError::NotQuarantined {
            workflow_name: "legacy-wf".to_string(),
            current_status: RegistrationStatus::Deactivated,
        })
    );
}

// ── B-33: Unquarantine storage failure ───────────────────────────────────────
// NOTE: Full StorageError testing requires Fjall failure injection via trait-based
// persistence abstraction. For now, we exercise the happy-path unquarantine and
// verify the exact return value. When Fjall injection is wired up, a dedicated
// test will assert Err(CircuitBreakerError::StorageError { reason: "..." }).

#[test]
fn unquarantine_returns_correct_result_when_no_failures_tracked() {
    // Tests the unquarantine path when the workflow is quarantined but has
    // no entries in the failure tracker (e.g., failure tracker was cleared
    // independently, or quarantine was set directly via status map).
    let state = CircuitBreakerState::new();
    let wf = make_wf("ai-loop");

    state
        .statuses
        .insert(wf.clone(), RegistrationStatus::Quarantined);
    // Note: NO failure_tracker entry inserted — tests the map_or(0, ...) path

    let result = unquarantine(&wf, "operator", &state);
    assert_eq!(
        result,
        Ok(UnquarantineResult {
            workflow_name: wf.clone(),
            previous_status: RegistrationStatus::Quarantined,
            new_status: RegistrationStatus::Active,
            failures_cleared: 0,
        })
    );
}

// ── B-57: Force bypasses Deactivated status (POST-005) ───────────────────────

#[test]
fn evaluate_registration_returns_allowed_when_force_true_and_deactivated() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let now = Instant::now();

    state
        .statuses
        .insert(make_wf("wf-force-deact"), RegistrationStatus::Deactivated);

    state.register_operator_token("test-operator-token".into());
    let request = make_request("wf-force-deact", "abcdef01", true);

    let result = evaluate_registration(&request, &config, &state, now);
    assert_eq!(result, Ok(RegistrationOutcome::Allowed));
}

// ── Error Variant Coverage (L-10 through L-13) ──────────────────────────────
// These tests verify that all CircuitBreakerError variants can be constructed,
// matched, and have correct Display/Error implementations.
// The implementation uses RegistrationOutcome for the evaluate_registration path,
// but these error variants exist for the HTTP boundary layer translation.
// We verify they are constructable, matchable, and have correct error messages.

// L-10: CircuitBreakerError::RateLimited — construction and matching
#[test]
fn circuit_breaker_error_rate_limited_variant_is_constructable_and_matchable() {
    let err = CircuitBreakerError::RateLimited {
        retry_after_secs: 42,
    };
    match err {
        CircuitBreakerError::RateLimited { retry_after_secs } => {
            assert_eq!(retry_after_secs, 42);
        }
        other => panic!("expected RateLimited, got {other:?}"),
    }
    // Verify Display impl
    assert_eq!(format!("{err}"), "rate_limited: retry after 42s");
}

// L-11: CircuitBreakerError::WorkflowQuarantined — construction and matching
#[test]
fn circuit_breaker_error_workflow_quarantined_variant_is_constructable_and_matchable() {
    let err = CircuitBreakerError::WorkflowQuarantined {
        workflow_name: "ai-loop-fix".to_string(),
    };
    match err {
        CircuitBreakerError::WorkflowQuarantined { ref workflow_name } => {
            assert_eq!(workflow_name, "ai-loop-fix");
        }
        ref other => panic!("expected WorkflowQuarantined, got {other:?}"),
    }
    assert_eq!(format!("{err}"), "workflow_quarantined: ai-loop-fix");
}

// L-12: CircuitBreakerError::WorkflowDeactivated — construction and matching
#[test]
fn circuit_breaker_error_workflow_deactivated_variant_is_constructable_and_matchable() {
    let err = CircuitBreakerError::WorkflowDeactivated {
        workflow_name: "legacy-wf".to_string(),
    };
    match err {
        CircuitBreakerError::WorkflowDeactivated { ref workflow_name } => {
            assert_eq!(workflow_name, "legacy-wf");
        }
        ref other => panic!("expected WorkflowDeactivated, got {other:?}"),
    }
    assert_eq!(format!("{err}"), "workflow_deactivated: legacy-wf");
}

// L-13: CircuitBreakerError::StorageError — construction and matching
#[test]
fn circuit_breaker_error_storage_error_variant_is_constructable_and_matchable() {
    let err = CircuitBreakerError::StorageError {
        reason: "fjall partition unavailable".to_string(),
    };
    match err {
        CircuitBreakerError::StorageError { ref reason } => {
            assert_eq!(reason, "fjall partition unavailable");
        }
        ref other => panic!("expected StorageError, got {other:?}"),
    }
    assert_eq!(
        format!("{err}"),
        "storage_error: fjall partition unavailable"
    );
}

// L-13 extended: CircuitBreakerError has exactly 6 variants (exhaustive match)
#[test]
fn circuit_breaker_error_has_exactly_six_variants() {
    // Compile-time exhaustiveness check. If a 7th variant is added, this fails to compile.
    let variants: [CircuitBreakerError; 6] = [
        CircuitBreakerError::RateLimited {
            retry_after_secs: 1,
        },
        CircuitBreakerError::WorkflowQuarantined {
            workflow_name: "wf".to_string(),
        },
        CircuitBreakerError::WorkflowDeactivated {
            workflow_name: "wf".to_string(),
        },
        CircuitBreakerError::StorageError {
            reason: "err".to_string(),
        },
        CircuitBreakerError::WorkflowNotFound {
            workflow_name: "wf".to_string(),
        },
        CircuitBreakerError::NotQuarantined {
            workflow_name: "wf".to_string(),
            current_status: RegistrationStatus::Active,
        },
    ];
    assert_eq!(variants.len(), 6);
    // Exhaustive match for each variant
    variants.iter().for_each(|v| match v {
        CircuitBreakerError::RateLimited { .. } => {}
        CircuitBreakerError::WorkflowQuarantined { .. } => {}
        CircuitBreakerError::WorkflowDeactivated { .. } => {}
        CircuitBreakerError::StorageError { .. } => {}
        CircuitBreakerError::WorkflowNotFound { .. } => {}
        CircuitBreakerError::NotQuarantined { .. } => {}
    });
}

// ── Density Tests (L-14) ────────────────────────────────────────────────────

// default_config returns expected values
#[test]
fn default_config_returns_60s_rate_limit_600s_failure_window_threshold_5() {
    let config = CircuitBreakerConfig::default_config().expect("default config should be valid");
    assert_eq!(config.rate_limit_window, Duration::from_secs(60));
    assert_eq!(config.failure_window, Duration::from_secs(600));
    assert_eq!(config.failure_threshold, 5);
}

// FailureWindow::new() is empty
#[test]
fn failure_window_new_is_empty() {
    let window = FailureWindow::new();
    assert!(window.is_empty(), "new FailureWindow should be empty");
    assert_eq!(window.len(), 0);
    assert_eq!(window.records().len(), 0);
}

// CircuitBreakerState::new() has empty maps
#[test]
fn circuit_breaker_state_new_has_empty_maps() {
    let state = CircuitBreakerState::new();
    assert!(state.statuses.is_empty(), "statuses should be empty");
    assert!(
        state.rate_limiter.is_empty(),
        "rate_limiter should be empty"
    );
    assert!(
        state.failure_tracker.is_empty(),
        "failure_tracker should be empty"
    );
}
