//! ATTACK VECTOR 7: Unquarantine state corruption attacks.

use super::helpers::*;
use std::time::{Duration, Instant};

use vo_core::circuit_breaker::{
    record_failure, unquarantine, CircuitBreakerError, CircuitBreakerState,
    RegistrationOutcome, RegistrationRequest, RegistrationStatus,
};

/// Attack: Unquarantine, then immediately try to record 5 failures again.
/// The workflow should be re-quarantinable after unquarantine.
#[test]
fn attack_unquarantine_then_reaquarantine() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("attack-wf-unq");

    (0..5).for_each(|i| {
        record_failure(&wf, &hash_from_idx(i), &config, &state, t0).unwrap();
    });
    assert_eq!(
        state.statuses.get(&wf).map(|s| *s),
        Some(RegistrationStatus::Quarantined)
    );

    let unq_result = unquarantine(&wf, "operator", &state);
    assert!(matches!(unq_result, Ok(_)));
    assert_eq!(
        state.statuses.get(&wf).map(|s| *s),
        Some(RegistrationStatus::Active)
    );

    let t1 = t0 + Duration::from_secs(1);
    (10..14).for_each(|i| {
        let result = record_failure(&wf, &hash_from_idx(i), &config, &state, t1);
        assert_eq!(result, Ok(None));
    });
    let result = record_failure(&wf, &hash_from_idx(14), &config, &state, t1);
    assert!(
        matches!(result, Ok(Some(_))),
        "Should re-quarantine after unquarantine + 5 new failures"
    );
    assert_eq!(
        state.statuses.get(&wf).map(|s| *s),
        Some(RegistrationStatus::Quarantined)
    );
}

/// Attack: Double unquarantine — second call should fail with NotQuarantined.
#[test]
fn attack_double_unquarantine_fails() {
    let state = CircuitBreakerState::new();
    let wf = make_wf("attack-wf-dblunq");

    state
        .statuses
        .insert(wf.clone(), RegistrationStatus::Quarantined);

    let result1 = unquarantine(&wf, "op1", &state);
    assert!(matches!(result1, Ok(_)));

    let result2 = unquarantine(&wf, "op2", &state);
    assert_eq!(
        result2,
        Err(CircuitBreakerError::NotQuarantined {
            workflow_name: "attack-wf-dblunq".to_string(),
            current_status: RegistrationStatus::Active,
        })
    );
}
