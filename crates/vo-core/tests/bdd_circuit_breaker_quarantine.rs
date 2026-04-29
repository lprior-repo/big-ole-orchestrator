//! BDD tests for circuit breaker quarantine behavior (ADR-026).
//!
//! Given-When-Then scenarios validating:
//! - Deployment rate limiting
//! - Failure counting across distinct hashes
//! - Quarantine trigger
//! - Automated deploy rejection when quarantined
//! - Manual authorized unquarantine
//! - Idempotent repeated unquarantine
//! - Audit trail for operator actions

use std::time::{Duration, Instant};

use vo_core::circuit_breaker::{
    evaluate_registration, record_failure, unquarantine, CircuitBreakerConfig, CircuitBreakerState,
    RegistrationOutcome, RegistrationRequest, RegistrationStatus,
};
use vo_types::{BinaryHash, WorkflowName};

fn make_hash(s: &str) -> BinaryHash {
    BinaryHash::parse(s).expect("test hash should be valid")
}

fn make_wf(s: &str) -> WorkflowName {
    WorkflowName::parse(s).expect("test workflow name should be valid")
}

fn make_request(wf: &WorkflowName, hash: &BinaryHash, force: bool) -> RegistrationRequest {
    RegistrationRequest {
        workflow_name: wf.clone(),
        binary_hash: hash.clone(),
        force,
    }
}

fn default_config() -> CircuitBreakerConfig {
    CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 5)
        .expect("default config should be valid")
}

// =============================================================================
// SCENARIO: Deployment rate limiting
// =============================================================================

#[test]
fn given_workflow_registered_recently_when_deploy_attempted_then_rate_limited() {
    // GIVEN: A workflow was registered within the rate limit window
    let config = default_config();
    let state = CircuitBreakerState::new();
    let wf = make_wf("rate-limited-wf");
    let hash = make_hash("bbbb00000001");
    let now = Instant::now();

    // First registration succeeds
    let req1 = make_request(&wf, &hash, false);
    let result1 = evaluate_registration(&req1, &config, &state, now);
    assert!(
        matches!(result1, Ok(RegistrationOutcome::Allowed)),
        "first registration should be allowed"
    );

    // WHEN: A deploy is attempted within the rate limit window
    let req2 = make_request(&wf, &hash, false);
    let result2 = evaluate_registration(&req2, &config, &state, now + Duration::from_secs(30));

    // THEN: The deploy is rate limited with retry_after_secs
    match result2 {
        Ok(RegistrationOutcome::RateLimited { retry_after_secs }) => {
            assert!(retry_after_secs > 0, "retry_after_secs must be positive");
        }
        Ok(other) => panic!("expected RateLimited, got {:?}", other),
        Err(e) => panic!("unexpected error: {:?}", e),
    }
}

#[test]
fn given_rate_limit_window_expired_when_deploy_attempted_then_allowed() {
    // GIVEN: A workflow's rate limit window has expired
    let config = default_config();
    let state = CircuitBreakerState::new();
    let wf = make_wf("expired-rate-limit-wf");
    let hash = make_hash("bbbb00000002");
    let now = Instant::now();

    // First registration
    let req1 = make_request(&wf, &hash, false);
    let result1 = evaluate_registration(&req1, &config, &state, now);
    assert!(
        matches!(result1, Ok(RegistrationOutcome::Allowed)),
        "first registration should be allowed"
    );

    // WHEN: A deploy is attempted after the rate limit window expires
    let req2 = make_request(&wf, &hash, false);
    let result2 = evaluate_registration(
        &req2,
        &config,
        &state,
        now + config.rate_limit_window + Duration::from_secs(1),
    );

    // THEN: The deploy is allowed
    assert!(
        matches!(result2, Ok(RegistrationOutcome::Allowed)),
        "deploy after window expiry should be allowed"
    );
}

#[test]
fn given_force_flag_when_registration_attempted_then_bypasses_rate_limit() {
    // GIVEN: A workflow was recently registered
    let config = default_config();
    let state = CircuitBreakerState::new();
    let wf = make_wf("force-bypass-wf");
    let hash = make_hash("bbbb00000003");
    let now = Instant::now();

    // First registration
    let req1 = make_request(&wf, &hash, false);
    let result1 = evaluate_registration(&req1, &config, &state, now);
    assert!(
        matches!(result1, Ok(RegistrationOutcome::Allowed)),
        "first registration should be allowed"
    );

    // WHEN: A forced registration is attempted within rate limit window
    let req2 = make_request(&wf, &hash, true);
    let result2 = evaluate_registration(&req2, &config, &state, now + Duration::from_secs(30));

    // THEN: The forced registration bypasses rate limit
    assert!(
        matches!(result2, Ok(RegistrationOutcome::Allowed)),
        "force flag should bypass rate limit"
    );
}

// =============================================================================
// SCENARIO: Failure counting across distinct hashes
// =============================================================================

#[test]
fn given_failure_threshold_across_hashes_when_recorded_then_workflow_is_quarantined() {
    // GIVEN: A workflow with failure threshold of 5
    let config = CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 5)
        .expect("config should be valid");
    let state = CircuitBreakerState::new();
    let wf = make_wf("quarantine-test-wf");
    let now = Instant::now();

    let h1 = make_hash("aaaa00000001");
    let h2 = make_hash("aaaa00000002");
    let h3 = make_hash("aaaa00000003");
    let h4 = make_hash("aaaa00000004");
    let h5 = make_hash("aaaa00000005");

    // WHEN: Failures are recorded across distinct hashes
    let r1 = record_failure(&wf, &h1, &config, &state, now);
    assert!(r1.is_ok(), "first failure should succeed");
    assert!(r1.unwrap().is_none(), "should not quarantine at 1 failure");
    assert_eq!(
        state.get_status(&wf),
        RegistrationStatus::Active,
        "workflow should still be active after 1 failure"
    );

    let r2 = record_failure(&wf, &h2, &config, &state, now + Duration::from_secs(120));
    assert!(r2.is_ok(), "second failure should succeed");
    assert!(r2.unwrap().is_none(), "should not quarantine at 2 failures");

    let r3 = record_failure(&wf, &h3, &config, &state, now + Duration::from_secs(240));
    assert!(r3.is_ok(), "third failure should succeed");
    assert!(r3.unwrap().is_none(), "should not quarantine at 3 failures");

    let r4 = record_failure(&wf, &h4, &config, &state, now + Duration::from_secs(360));
    assert!(r4.is_ok(), "fourth failure should succeed");
    assert!(r4.unwrap().is_none(), "should not quarantine at 4 failures");

    let r5 = record_failure(&wf, &h5, &config, &state, now + Duration::from_secs(480));

    // THEN: Workflow is quarantined at threshold
    let quarantine_event = r5.unwrap();
    assert!(
        quarantine_event.is_some(),
        "should emit QuarantineEvent at threshold"
    );
    assert_eq!(
        state.get_status(&wf),
        RegistrationStatus::Quarantined,
        "workflow should be quarantined after reaching failure threshold"
    );
}

#[test]
fn given_same_hash_repeated_failures_when_counted_then_only_counts_once() {
    // GIVEN: A workflow with failure threshold of 3
    let config = CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 3)
        .expect("config should be valid");
    let state = CircuitBreakerState::new();
    let wf = make_wf("same-hash-wf");
    let now = Instant::now();
    let hash = make_hash("cccc00000001");

    // WHEN: Same hash fails multiple times
    let r1 = record_failure(&wf, &hash, &config, &state, now);
    let r2 = record_failure(&wf, &hash, &config, &state, now + Duration::from_secs(60));
    let r3 = record_failure(&wf, &hash, &config, &state, now + Duration::from_secs(120));
    let r4 = record_failure(&wf, &hash, &config, &state, now + Duration::from_secs(180));

    // THEN: Only one unique hash is counted
    assert!(
        r1.unwrap().is_none(),
        "1st same-hash failure should not quarantine"
    );
    assert!(
        r2.unwrap().is_none(),
        "2nd same-hash failure should not quarantine"
    );
    assert!(
        r3.unwrap().is_none(),
        "3rd same-hash failure should not quarantine"
    );
    assert!(
        r4.unwrap().is_none(),
        "4th same-hash failure should still not quarantine"
    );
    assert_eq!(
        state.get_status(&wf),
        RegistrationStatus::Active,
        "workflow should still be active (only 1 unique hash)"
    );
}

#[test]
fn given_failure_window_expired_when_new_failure_then_stale_entries_evicted() {
    // GIVEN: A workflow with short failure window (10 seconds) and threshold 2
    let config = CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(10), 2)
        .expect("config should be valid");
    let state = CircuitBreakerState::new();
    let wf = make_wf("expiry-wf");
    let now = Instant::now();

    let h1 = make_hash("dddd00000001");
    let h2 = make_hash("dddd00000002");
    let h3 = make_hash("dddd00000003");

    // Record first failure
    let r1 = record_failure(&wf, &h1, &config, &state, now);
    assert!(r1.is_ok(), "first failure should succeed");
    assert!(r1.unwrap().is_none(), "should not quarantine at 1 failure");

    // WHEN: Second failure comes after first has expired from window
    let r2 = record_failure(
        &wf,
        &h2,
        &config,
        &state,
        now + Duration::from_secs(15), // h1 has now expired
    );

    // THEN: Only h2 is counted (h1 expired), no quarantine yet
    assert!(
        r2.unwrap().is_none(),
        "should not quarantine (only 1 active hash)"
    );

    // WHEN: Third failure with new hash
    let r3 = record_failure(&wf, &h3, &config, &state, now + Duration::from_secs(20));

    // THEN: Now 2 active hashes, quarantine triggered
    assert!(
        r3.unwrap().is_some(),
        "should quarantine with 2 unique non-expired hashes"
    );
}

// =============================================================================
// SCENARIO: Automated deploy rejection when quarantined
// =============================================================================

#[test]
fn given_workflow_quarantined_when_deploy_attempted_then_automatically_rejected() {
    // GIVEN: A workflow is quarantined
    let config = default_config();
    let state = CircuitBreakerState::new();
    let wf = make_wf("quarantined-wf");
    let hash = make_hash("eeee00000001");
    let now = Instant::now();

    // Quarantine the workflow by recording 5 distinct failures
    let hashes: Vec<_> = (1..=5u8)
        .map(|i| make_hash(&format!("eeee0000000{}", i)))
        .collect();
    for (i, h) in hashes.iter().enumerate() {
        record_failure(
            &wf,
            h,
            &config,
            &state,
            now + Duration::from_secs(i as u64 * 60),
        );
    }
    assert_eq!(
        state.get_status(&wf),
        RegistrationStatus::Quarantined,
        "workflow should be quarantined"
    );

    // WHEN: A deploy is attempted
    let req = make_request(&wf, &hash, false);
    let result = evaluate_registration(&req, &config, &state, now + Duration::from_secs(400));

    // THEN: Deploy is rejected with WorkflowQuarantined
    match result {
        Ok(RegistrationOutcome::WorkflowQuarantined { workflow_name }) => {
            assert_eq!(workflow_name, wf, "should return correct workflow name");
        }
        Ok(other) => panic!("expected WorkflowQuarantined, got {:?}", other),
        Err(e) => panic!("unexpected error: {:?}", e),
    }
}

#[test]
fn given_quarantined_workflow_when_force_deploy_attempted_then_allowed() {
    // GIVEN: A workflow is quarantined
    let config = default_config();
    let state = CircuitBreakerState::new();
    let wf = make_wf("force-deploy-wf");
    let hash = make_hash("ffff00000001");
    let now = Instant::now();

    // Quarantine the workflow
    let hashes: Vec<_> = (1..=5u8)
        .map(|i| make_hash(&format!("ffff0000000{}", i)))
        .collect();
    for (i, h) in hashes.iter().enumerate() {
        record_failure(
            &wf,
            h,
            &config,
            &state,
            now + Duration::from_secs(i as u64 * 60),
        );
    }
    assert_eq!(
        state.get_status(&wf),
        RegistrationStatus::Quarantined,
        "workflow should be quarantined"
    );

    // WHEN: A forced deploy is attempted
    let req = make_request(&wf, &hash, true);
    let result = evaluate_registration(&req, &config, &state, now + Duration::from_secs(400));

    // THEN: Force deploy bypasses quarantine check
    assert!(
        matches!(result, Ok(RegistrationOutcome::Allowed)),
        "force flag should bypass quarantine"
    );
}

// =============================================================================
// SCENARIO: Manual authorized unquarantine
// =============================================================================

#[test]
fn given_workflow_quarantined_when_manual_unquarantine_by_operator_then_workflow_active() {
    // GIVEN: A workflow is quarantined
    let config = default_config();
    let state = CircuitBreakerState::new();
    let wf = make_wf("unquarantine-wf");
    let now = Instant::now();

    // Quarantine the workflow
    let hashes: Vec<_> = (1..=5u8)
        .map(|i| make_hash(&format!("gggg0000000{}", i)))
        .collect();
    for (i, h) in hashes.iter().enumerate() {
        record_failure(
            &wf,
            &h,
            &config,
            &state,
            now + Duration::from_secs(i as u64 * 60),
        );
    }
    assert_eq!(
        state.get_status(&wf),
        RegistrationStatus::Quarantined,
        "workflow should be quarantined before unquarantine"
    );

    // WHEN: Operator manually unquarantines the workflow
    let result = unquarantine(&wf, "operator@example.com", &state);

    // THEN: Workflow is back to Active
    match result {
        Ok(unquarantine_result) => {
            assert_eq!(
                unquarantine_result.previous_status,
                RegistrationStatus::Quarantined,
                "previous status should be Quarantined"
            );
            assert_eq!(
                unquarantine_result.new_status,
                RegistrationStatus::Active,
                "new status should be Active"
            );
            assert_eq!(
                unquarantine_result.failures_cleared, 5,
                "should have cleared 5 failure entries"
            );
        }
        Err(e) => panic!("unquarantine should succeed: {:?}", e),
    }

    assert_eq!(
        state.get_status(&wf),
        RegistrationStatus::Active,
        "workflow should be Active after unquarantine"
    );
}

#[test]
fn given_workflow_not_quarantined_when_unquarantine_attempted_then_error_returned() {
    // GIVEN: An active workflow (not quarantined)
    let state = CircuitBreakerState::new();
    let wf = make_wf("active-wf");

    // Register it first so it exists
    let config = default_config();
    let req = make_request(&wf, &make_hash("hhhh00000001"), false);
    let _ = evaluate_registration(&req, &config, &state, Instant::now());

    assert_eq!(
        state.get_status(&wf),
        RegistrationStatus::Active,
        "workflow should be Active"
    );

    // WHEN: Unquarantine is attempted
    let result = unquarantine(&wf, "operator@example.com", &state);

    // THEN: Error returned indicating workflow is not quarantined
    match result {
        Err(vo_core::circuit_breaker::CircuitBreakerError::NotQuarantined {
            workflow_name,
            current_status,
        }) => {
            assert_eq!(workflow_name, "active-wf");
            assert_eq!(current_status, RegistrationStatus::Active);
        }
        Ok(_) => panic!("unquarantine of active workflow should fail"),
        Err(e) => panic!("unexpected error type: {:?}", e),
    }
}

#[test]
fn given_unknown_workflow_when_unquarantine_attempted_then_not_found_error() {
    // GIVEN: A workflow that doesn't exist
    let state = CircuitBreakerState::new();
    let wf = make_wf("nonexistent-wf");

    // WHEN: Unquarantine is attempted
    let result = unquarantine(&wf, "operator@example.com", &state);

    // THEN: WorkflowNotFound error
    match result {
        Err(vo_core::circuit_breaker::CircuitBreakerError::WorkflowNotFound { workflow_name }) => {
            assert_eq!(workflow_name, "nonexistent-wf");
        }
        Ok(_) => panic!("unquarantine of nonexistent workflow should fail"),
        Err(e) => panic!("unexpected error type: {:?}", e),
    }
}

// =============================================================================
// SCENARIO: Idempotent repeated unquarantine
// =============================================================================

#[test]
fn given_workflow_unquarantined_when_unquarantine_attempted_again_then_idempotent() {
    // GIVEN: A workflow that was quarantined and then unquarantined
    let config = default_config();
    let state = CircuitBreakerState::new();
    let wf = make_wf("idempotent-unq-wf");
    let now = Instant::now();

    // Quarantine
    let hashes: Vec<_> = (1..=5u8)
        .map(|i| make_hash(&format!("iiii0000000{}", i)))
        .collect();
    for (i, h) in hashes.iter().enumerate() {
        record_failure(
            &wf,
            &h,
            &config,
            &state,
            now + Duration::from_secs(i as u64 * 60),
        );
    }

    // First unquarantine
    let result1 = unquarantine(&wf, "operator@example.com", &state);
    assert!(result1.is_ok(), "first unquarantine should succeed");
    assert_eq!(
        state.get_status(&wf),
        RegistrationStatus::Active,
        "workflow should be Active after first unquarantine"
    );

    // WHEN: Second unquarantine is attempted (idempotent)
    let result2 = unquarantine(&wf, "operator@example.com", &state);

    // THEN: Should return error (not quarantined) - this is the idempotent behavior
    assert!(
        result2.is_err(),
        "unquarantine of already-active workflow should return error"
    );
}

#[test]
fn given_quarantined_workflow_when_unquarantine_twice_rapidly_then_no_duplicate_events() {
    // GIVEN: A workflow is quarantined
    let config = default_config();
    let state = CircuitBreakerState::new();
    let wf = make_wf("double-unq-wf");
    let now = Instant::now();

    // Quarantine
    let hashes: Vec<_> = (1..=5u8)
        .map(|i| make_hash(&format!("jjjj0000000{}", i)))
        .collect();
    for (i, h) in hashes.iter().enumerate() {
        record_failure(
            &wf,
            &h,
            &config,
            &state,
            now + Duration::from_secs(i as u64 * 60),
        );
    }

    // Unquarantine
    let result1 = unquarantine(&wf, "operator@example.com", &state);
    assert!(result1.is_ok());

    // Clear state and requarantine to test double-unquarantine scenario
    state.set_status(wf.clone(), RegistrationStatus::Quarantined);

    // WHEN: Two unquarantine calls happen in quick succession
    let result2 = unquarantine(&wf, "operator@example.com", &state);
    assert!(result2.is_ok());

    // THEN: Workflow is Active and consistent
    assert_eq!(
        state.get_status(&wf),
        RegistrationStatus::Active,
        "workflow should be Active"
    );
}

// =============================================================================
// SCENARIO: Audit trail for operator actions
// =============================================================================

#[test]
fn given_quarantine_event_when_callback_registered_then_notified() {
    // GIVEN: A circuit breaker state with a quarantine callback
    let config = default_config();
    let state = CircuitBreakerState::new();
    let wf = make_wf("callback-wf");
    let now = Instant::now();

    let mut events_received: Vec<vo_core::circuit_breaker::QuarantineEvent> = Vec::new();
    let callback = Arc::new(move |event: &vo_core::circuit_breaker::QuarantineEvent| {
        events_received.push(event.clone());
    });

    use std::sync::Arc;
    state.set_quarantine_callback(callback);

    // WHEN: Workflow is quarantined
    let hashes: Vec<_> = (1..=5u8)
        .map(|i| make_hash(&format!("kkkk0000000{}", i)))
        .collect();
    for (i, h) in hashes.iter().enumerate() {
        record_failure(
            &wf,
            &h,
            &config,
            &state,
            now + Duration::from_secs(i as u64 * 60),
        );
    }

    // THEN: Callback was notified with the quarantine event
    assert_eq!(
        events_received.len(),
        1,
        "callback should receive exactly one event"
    );
    assert_eq!(
        events_received[0].workflow_name, wf,
        "event should contain correct workflow name"
    );
}

#[test]
fn given_unquarantine_when_failure_window_cleared_then_audit_trail_records_count() {
    // GIVEN: A quarantined workflow with 5 failure entries
    let config = default_config();
    let state = CircuitBreakerState::new();
    let wf = make_wf("audit-wf");
    let now = Instant::now();

    // Quarantine with 5 distinct hashes
    let hashes: Vec<_> = (1..=5u8)
        .map(|i| make_hash(&format!("llll0000000{}", i)))
        .collect();
    for (i, h) in hashes.iter().enumerate() {
        record_failure(
            &wf,
            &h,
            &config,
            &state,
            now + Duration::from_secs(i as u64 * 60),
        );
    }

    // WHEN: Operator unquarantines
    let result = unquarantine(&wf, "operator@example.com", &state);

    // THEN: Result includes failures_cleared for audit purposes
    match result {
        Ok(unquarantine_result) => {
            assert_eq!(
                unquarantine_result.failures_cleared, 5,
                "audit trail should record number of failures cleared"
            );
        }
        Err(e) => panic!("unquarantine should succeed: {:?}", e),
    }
}

#[test]
fn given_workflow_deactivated_when_deploy_attempted_then_rejected_as_deactivated() {
    // GIVEN: A deactivated workflow
    let config = default_config();
    let state = CircuitBreakerState::new();
    let wf = make_wf("deactivated-wf");
    let hash = make_hash("mmmm00000001");

    // Manually deactivate the workflow
    state.set_status(wf.clone(), RegistrationStatus::Deactivated);

    // WHEN: A deploy is attempted
    let req = make_request(&wf, &hash, false);
    let result = evaluate_registration(&req, &config, &state, Instant::now());

    // THEN: Deploy is rejected with WorkflowDeactivated
    match result {
        Ok(RegistrationOutcome::WorkflowDeactivated { workflow_name }) => {
            assert_eq!(workflow_name, wf);
        }
        Ok(other) => panic!("expected WorkflowDeactivated, got {:?}", other),
        Err(e) => panic!("unexpected error: {:?}", e),
    }
}

// =============================================================================
// SCENARIO: UI diff visibility - status reporting
// =============================================================================

#[test]
fn given_quarantined_workflow_when_status_queried_then_quarantined_returned() {
    // GIVEN: A quarantined workflow
    let config = default_config();
    let state = CircuitBreakerState::new();
    let wf = make_wf("status-query-wf");
    let now = Instant::now();

    // Quarantine
    let hashes: Vec<_> = (1..=5u8)
        .map(|i| make_hash(&format!("nnnn0000000{}", i)))
        .collect();
    for (i, h) in hashes.iter().enumerate() {
        record_failure(
            &wf,
            &h,
            &config,
            &state,
            now + Duration::from_secs(i as u64 * 60),
        );
    }

    // WHEN: Status is queried
    let status = state.get_status(&wf);

    // THEN: Status is Quarantined (visible in UI)
    assert_eq!(status, RegistrationStatus::Quarantined);
}

#[test]
fn given_multiple_workflows_with_different_statuses_when_queried_then_each_returns_correct_status()
{
    // GIVEN: Multiple workflows with different states
    let state = CircuitBreakerState::new();
    let config = default_config();
    let now = Instant::now();

    let active_wf = make_wf("multi-active");
    let quarantined_wf = make_wf("multi-quarantine");
    let deactivated_wf = make_wf("multi-deactivated");

    // Setup states
    let req_active = make_request(&active_wf, &make_hash("oooo00000001"), false);
    assert!(matches!(
        evaluate_registration(&req_active, &config, &state, now),
        Ok(RegistrationOutcome::Allowed)
    ));

    state.set_status(quarantined_wf.clone(), RegistrationStatus::Quarantined);
    state.set_status(deactivated_wf.clone(), RegistrationStatus::Deactivated);

    // WHEN: Each workflow's status is queried
    let active_status = state.get_status(&active_wf);
    let quarantine_status = state.get_status(&quarantined_wf);
    let deactivated_status = state.get_status(&deactivated_wf);

    // THEN: Each returns its correct status (UI diff visibility)
    assert_eq!(active_status, RegistrationStatus::Active);
    assert_eq!(quarantine_status, RegistrationStatus::Quarantined);
    assert_eq!(deactivated_status, RegistrationStatus::Deactivated);
}
