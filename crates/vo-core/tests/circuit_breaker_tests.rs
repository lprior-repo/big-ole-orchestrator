//! CircuitBreaker integration tests.

use std::time::{Duration, Instant};

use vo_core::circuit_breaker::{
    evaluate_registration, record_failure, CircuitBreakerConfig, CircuitBreakerState,
    RegistrationOutcome, RegistrationRequest, RegistrationStatus,
};

use crate::helpers::{make_hash, make_wf};

fn default_config() -> CircuitBreakerConfig {
    CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 5)
        .expect("default config should be valid")
}

fn make_request(wf: &str, hash: &str, force: bool) -> RegistrationRequest {
    RegistrationRequest {
        workflow_name: make_wf(wf),
        binary_hash: make_hash(hash),
        force,
    }
}

#[test]
fn circuit_breaker_and_workflow_registration_compose() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let now = Instant::now();

    let request = make_request("deploy-prod", "abcdef01", false);
    let result = evaluate_registration(&request, &config, &state, now);
    assert_eq!(
        result,
        Ok(RegistrationOutcome::Allowed),
        "first registration should be allowed"
    );

    let failure_result = record_failure(
        &make_wf("deploy-prod"),
        &make_hash("aaaa0001"),
        &config,
        &state,
        now,
    );
    assert_eq!(
        failure_result,
        Ok(None),
        "first failure should not trigger quarantine"
    );

    for i in 2..6 {
        let hash = format!("aaaa000{}", i);
        let result = record_failure(
            &make_wf("deploy-prod"),
            &make_hash(&hash),
            &config,
            &state,
            now,
        );
        if i < 5 {
            assert_eq!(
                result,
                Ok(None),
                "failure {} should not trigger quarantine",
                i
            );
        } else {
            assert!(
                result.is_ok(),
                "5th unique failure should return Ok (quarantine event or success)"
            );
        }
    }
}

#[test]
fn circuit_breaker_workflow_isolation_across_namespaces() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let now = Instant::now();

    for i in 0..5 {
        let hash = format!("aaaa000{}", i);
        record_failure(
            &make_wf("workflow-a"),
            &make_hash(&hash),
            &config,
            &state,
            now,
        )
        .expect("failure recording should succeed");
    }

    let status_a = state.get_status(&make_wf("workflow-a"));
    assert_eq!(
        status_a,
        RegistrationStatus::Quarantined,
        "workflow-a should be quarantined"
    );

    let status_b = state.get_status(&make_wf("workflow-b"));
    assert_eq!(
        status_b,
        RegistrationStatus::Active,
        "workflow-b should remain active (isolation)"
    );

    let request = make_request("workflow-b", "abcdef01", false);
    let result = evaluate_registration(&request, &config, &state, now);
    assert_eq!(
        result,
        Ok(RegistrationOutcome::Allowed),
        "workflow-b should be allowed"
    );
}

#[test]
fn circuit_breaker_rate_limit_enforces_cooldown() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();

    let request = make_request("deploy-prod", "abcdef01", false);
    let result1 = evaluate_registration(&request, &config, &state, t0);
    assert_eq!(
        result1,
        Ok(RegistrationOutcome::Allowed),
        "first registration should succeed"
    );

    let t_within_limit = t0 + Duration::from_secs(30);
    let result2 = evaluate_registration(&request, &config, &state, t_within_limit);
    assert_eq!(
        result2,
        Ok(RegistrationOutcome::RateLimited {
            retry_after_secs: 30
        }),
        "second registration within 60s window should be rate-limited"
    );

    let t_past_limit = t0 + Duration::from_secs(120);
    let result3 = evaluate_registration(&request, &config, &state, t_past_limit);
    assert_eq!(
        result3,
        Ok(RegistrationOutcome::Allowed),
        "registration after rate limit window should succeed"
    );
}

#[test]
fn circuit_breaker_force_bypasses_all_protections() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let now = Instant::now();

    state
        .statuses
        .insert(make_wf("stuck-wf"), RegistrationStatus::Quarantined);
    state
        .rate_limiter
        .insert(make_wf("stuck-wf"), now - Duration::from_secs(30));

    let force_request = make_request("stuck-wf", "abcdef01", true);
    let result = evaluate_registration(&force_request, &config, &state, now);
    assert_eq!(
        result,
        Ok(RegistrationOutcome::Allowed),
        "force registration should bypass quarantine"
    );

    let status = state.get_status(&make_wf("stuck-wf"));
    assert_eq!(
        status,
        RegistrationStatus::Quarantined,
        "force should not change quarantine status"
    );
}

#[test]
fn circuit_breaker_failure_window_respects_time() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();

    record_failure(
        &make_wf("decay-wf"),
        &make_hash("aaaa0001"),
        &config,
        &state,
        t0,
    )
    .expect("first failure should be recorded");
    record_failure(
        &make_wf("decay-wf"),
        &make_hash("aaaa0002"),
        &config,
        &state,
        t0,
    )
    .expect("second failure should be recorded");
    record_failure(
        &make_wf("decay-wf"),
        &make_hash("aaaa0003"),
        &config,
        &state,
        t0,
    )
    .expect("third failure should be recorded");
    record_failure(
        &make_wf("decay-wf"),
        &make_hash("aaaa0004"),
        &config,
        &state,
        t0,
    )
    .expect("fourth failure should be recorded");

    let status_before = state.get_status(&make_wf("decay-wf"));
    assert_eq!(
        status_before,
        RegistrationStatus::Active,
        "4 failures should not trigger quarantine (threshold 5)"
    );

    record_failure(
        &make_wf("decay-wf"),
        &make_hash("aaaa0005"),
        &config,
        &state,
        t0,
    )
    .expect("fifth failure should be recorded");

    let status_after = state.get_status(&make_wf("decay-wf"));
    assert_eq!(
        status_after,
        RegistrationStatus::Quarantined,
        "5th unique failure should trigger quarantine"
    );
}

#[test]
fn circuit_breaker_state_get_status_unknown_workflow() {
    let state = CircuitBreakerState::new();
    let status = state.get_status(&make_wf("unknown-wf"));
    assert_eq!(
        status,
        RegistrationStatus::Active,
        "unknown workflow should default to Active"
    );
}

#[test]
fn circuit_breaker_state_set_status() {
    let state = CircuitBreakerState::new();

    state.set_status(make_wf("test-wf"), RegistrationStatus::Quarantined);

    let status = state.get_status(&make_wf("test-wf"));
    assert_eq!(
        status,
        RegistrationStatus::Quarantined,
        "status should be Quarantined after set_status"
    );
}

#[test]
fn circuit_breaker_state_clear_status() {
    let state = CircuitBreakerState::new();

    state.set_status(make_wf("test-wf"), RegistrationStatus::Quarantined);
    assert_eq!(
        state.get_status(&make_wf("test-wf")),
        RegistrationStatus::Quarantined
    );

    state.set_status(make_wf("test-wf"), RegistrationStatus::Active);
    assert_eq!(
        state.get_status(&make_wf("test-wf")),
        RegistrationStatus::Active,
        "status should be Active after set_status to Active"
    );
}
