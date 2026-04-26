//! BDD tests for circuit breaker quarantine behavior (ADR-026).
//!
//! Given-When-Then scenarios validating that workflows are quarantined
//! when the failure threshold is reached across distinct binary hashes.

use std::time::{Duration, Instant};

use vo_core::circuit_breaker::{
    record_failure, CircuitBreakerConfig, CircuitBreakerState, RegistrationStatus,
};
use vo_types::{BinaryHash, WorkflowName};

fn make_hash(s: &str) -> BinaryHash {
    BinaryHash::parse(s).expect("test hash should be valid")
}

fn make_wf(s: &str) -> WorkflowName {
    WorkflowName::parse(s).expect("test workflow name should be valid")
}

#[test]
fn given_failure_threshold_across_hashes_when_recorded_then_workflow_is_quarantined() {
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
    assert_eq!(
        state.get_status(&wf),
        RegistrationStatus::Active,
        "workflow should still be active after 2 failures"
    );

    let r3 = record_failure(&wf, &h3, &config, &state, now + Duration::from_secs(240));
    assert!(r3.is_ok(), "third failure should succeed");
    assert!(r3.unwrap().is_none(), "should not quarantine at 3 failures");
    assert_eq!(
        state.get_status(&wf),
        RegistrationStatus::Active,
        "workflow should still be active after 3 failures"
    );

    let r4 = record_failure(&wf, &h4, &config, &state, now + Duration::from_secs(360));
    assert!(r4.is_ok(), "fourth failure should succeed");
    assert!(r4.unwrap().is_none(), "should not quarantine at 4 failures");
    assert_eq!(
        state.get_status(&wf),
        RegistrationStatus::Active,
        "workflow should still be active after 4 failures"
    );

    let r5 = record_failure(&wf, &h5, &config, &state, now + Duration::from_secs(480));
    assert!(r5.is_ok(), "fifth failure should succeed");
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