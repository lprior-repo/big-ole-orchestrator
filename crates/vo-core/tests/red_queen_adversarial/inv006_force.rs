//! ATTACK VECTOR 3: INV-006 — Force flag bypass exhaustive testing.

use super::helpers::*;
use std::time::{Duration, Instant};

use vo_core::circuit_breaker::{
    evaluate_registration, record_failure, FailureWindow, CircuitBreakerState, RegistrationOutcome,
    RegistrationRequest,
};

/// Attack: Force flag on a workflow that is simultaneously quarantined,
/// rate-limited, and has a full failure window.
#[test]
fn attack_inv006_force_bypasses_all_three_guards_simultaneously() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("attack-wf-06");

    state
        .statuses
        .insert(wf.clone(), vo_core::circuit_breaker::RegistrationStatus::Quarantined);
    state.rate_limiter.insert(wf.clone(), t0);

    let mut window = FailureWindow::new();
    (0..5).for_each(|i| {
        vo_core::circuit_breaker::failure_window::record_failure_in_window(
            &mut window,
            hash_from_idx(i),
            t0,
            Duration::from_secs(600),
        );
    });
    state.failure_tracker.insert(wf.clone(), window);

    let request = RegistrationRequest {
        workflow_name: wf.clone(),
        binary_hash: make_hash("abcdef01"),
        force: true,
    };
    let now = t0 + Duration::from_secs(10);
    let result = evaluate_registration(&request, &config, &state, now);
    assert_eq!(result, Ok(RegistrationOutcome::Allowed));

    let rl = state.rate_limiter.get(&wf).map(|r| *r);
    assert_eq!(rl, Some(now), "Force should update rate limiter to now");
}
