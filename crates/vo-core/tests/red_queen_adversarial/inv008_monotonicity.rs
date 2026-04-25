//! ATTACK VECTOR 5: INV-008 — Quarantine monotonicity attacks.

use super::helpers::*;
use std::time::{Duration, Instant};

use vo_core::circuit_breaker::{
    evaluate_registration, CircuitBreakerState, RegistrationOutcome, RegistrationStatus,
};

/// Attack: After quarantine, try to register without force, then with force,
/// then without force again. Non-force attempts must always be blocked.
#[test]
fn attack_inv008_quarantine_monotonicity_after_force_registration() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("attack-wf-08");

    state
        .statuses
        .insert(wf.clone(), RegistrationStatus::Quarantined);

    let request = make_request("attack-wf-08", "abcdef01", false);
    let result = evaluate_registration(&request, &config, &state, t0);
    assert!(matches!(
        result,
        Ok(RegistrationOutcome::WorkflowQuarantined { .. })
    ));

    let force_request = make_request("attack-wf-08", "abcdef02", true);
    let t1 = t0 + Duration::from_secs(1);
    let result = evaluate_registration(&force_request, &config, &state, t1);
    assert_eq!(result, Ok(RegistrationOutcome::Allowed));

    let t2 = t1 + Duration::from_secs(120);
    let request2 = make_request("attack-wf-08", "abcdef03", false);
    let result = evaluate_registration(&request2, &config, &state, t2);
    assert!(
        matches!(result, Ok(RegistrationOutcome::WorkflowQuarantined { .. })),
        "Quarantine must persist after force bypass. Got: {result:?}"
    );
}

/// Attack: Quarantine survives time passage (1 year of time advancement).
/// INV-008: No amount of time can auto-clear quarantine.
#[test]
fn attack_inv008_quarantine_survives_one_year_of_time() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("attack-wf-08b");

    state
        .statuses
        .insert(wf.clone(), RegistrationStatus::Quarantined);

    let one_year = Duration::from_secs(365 * 24 * 3600);
    let t_future = t0 + one_year;

    let request = make_request("attack-wf-08b", "abcdef01", false);
    let result = evaluate_registration(&request, &config, &state, t_future);
    assert!(
        matches!(result, Ok(RegistrationOutcome::WorkflowQuarantined { .. })),
        "Quarantine must survive indefinitely. Got: {result:?}"
    );
}
