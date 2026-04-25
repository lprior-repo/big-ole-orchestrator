//! ATTACK VECTOR 6: INV-009 — Rate-limited request must NOT count as failure.

use super::helpers::*;
use std::time::{Duration, Instant};

use vo_core::circuit_breaker::{
    evaluate_registration, CircuitBreakerState, RegistrationOutcome, RegistrationRequest,
    RegistrationStatus,
};

/// Attack: Try to trigger quarantine by rapidly submitting requests during
/// rate limit window. The rate limiter should block them before they count.
#[test]
fn attack_inv009_rapid_requests_during_rate_limit_never_count() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("attack-wf-09");

    let req = make_request("attack-wf-09", "abcdef01", false);
    let result = evaluate_registration(&req, &config, &state, t0);
    assert_eq!(result, Ok(RegistrationOutcome::Allowed));

    (1..100).for_each(|i| {
        let t = t0 + Duration::from_millis(i * 100);
        let req = RegistrationRequest {
            workflow_name: wf.clone(),
            binary_hash: hash_from_idx(i as usize),
            force: false,
        };
        let result = evaluate_registration(&req, &config, &state, t);
        assert!(
            matches!(result, Ok(RegistrationOutcome::RateLimited { .. })),
            "Request at +{}ms should be rate-limited, got {result:?}",
            i * 100
        );
    });

    let has_failures = state.failure_tracker.get(&wf).map(|t| t.len()).unwrap_or(0);
    assert_eq!(
        has_failures, 0,
        "Rate-limited requests must never create failure entries"
    );

    let status = state
        .statuses
        .get(&wf)
        .map(|s| *s)
        .unwrap_or(RegistrationStatus::Active);
    assert_eq!(status, RegistrationStatus::Active);
}
