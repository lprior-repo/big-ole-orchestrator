//! ATTACK VECTOR 2: INV-004 — Duplicate hash flood attack.

use super::helpers::*;
use std::time::{Duration, Instant};

use vo_core::circuit_breaker::{record_failure, CircuitBreakerState, RegistrationStatus};

/// Attack: Record the same hash 1000 times to try to inflate the count.
#[test]
fn attack_inv004_duplicate_hash_flood_never_inflates_count() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("attack-wf-04");

    (0..1000).for_each(|i| {
        let t = t0 + Duration::from_millis(i);
        let result = record_failure(&wf, &make_hash("deadbeef"), &config, &state, t);
        assert_eq!(
            result,
            Ok(None),
            "Duplicate hash must never trigger quarantine"
        );
    });

    let tracker = state.failure_tracker.get(&wf).unwrap();
    assert_eq!(tracker.len(), 1, "Should have exactly 1 unique hash");

    let status = state
        .statuses
        .get(&wf)
        .map(|s| *s)
        .unwrap_or(RegistrationStatus::Active);
    assert_eq!(status, RegistrationStatus::Active);
}

/// Attack: Alternate between 4 unique hashes repeatedly, then add a 5th.
/// The 4 duplicates should not inflate the count.
#[test]
fn attack_inv004_alternating_duplicates_then_threshold() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("attack-wf-04b");

    let hashes = ["aaaa0001", "aaaa0002", "aaaa0003", "aaaa0004"];

    (0u64..3).for_each(|cycle| {
        (hashes.iter().enumerate()).for_each(|(i, h)| {
            let t = t0 + Duration::from_secs(cycle * 10 + i as u64);
            let result = record_failure(&wf, &make_hash(h), &config, &state, t);
            assert_eq!(result, Ok(None), "cycle={cycle}, i={i}, hash={h}");
        });
    });

    {
        let tracker = state.failure_tracker.get(&wf).unwrap();
        assert_eq!(tracker.len(), 4);
    }

    let t_final = t0 + Duration::from_secs(60);
    let result = record_failure(&wf, &make_hash("aaaa0005"), &config, &state, t_final);
    assert!(matches!(result, Ok(Some(_))));
}
