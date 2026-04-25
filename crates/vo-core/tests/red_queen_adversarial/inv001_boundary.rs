//! ATTACK VECTOR 1: INV-001 — Boundary at exactly 5 failures in 10-min window.

use super::helpers::*;
use std::time::{Duration, Instant};

use vo_core::circuit_breaker::{record_failure, CircuitBreakerState, RegistrationStatus};

/// Attack: 4 failures spread across the window, then 1 more at the exact edge.
/// The 5th failure should trigger quarantine even if the first failure is
/// exactly at the window boundary.
#[test]
fn attack_inv001_boundary_five_failures_at_window_edge() {
    let state = CircuitBreakerState::new();
    let config = default_config(); // 600s window, threshold 5
    let t0 = Instant::now();
    let wf = make_wf("attack-wf-01");

    let result = record_failure(&wf, &make_hash("aaaa0001"), &config, &state, t0);
    assert_eq!(result, Ok(None));

    let result = record_failure(
        &wf,
        &make_hash("aaaa0002"),
        &config,
        &state,
        t0 + Duration::from_secs(200),
    );
    assert_eq!(result, Ok(None));

    let result = record_failure(
        &wf,
        &make_hash("aaaa0003"),
        &config,
        &state,
        t0 + Duration::from_secs(400),
    );
    assert_eq!(result, Ok(None));

    let result = record_failure(
        &wf,
        &make_hash("aaaa0004"),
        &config,
        &state,
        t0 + Duration::from_secs(500),
    );
    assert_eq!(result, Ok(None));

    // Record h5 at t0+600s — exactly at the window boundary from h1
    // h1 was at t0, window is 600s, so h1 at t0+600 is exactly at the edge.
    // The eviction uses `elapsed <= window_duration`, so h1 at exactly 600s
    // should be RETAINED (not evicted). All 5 should be present.
    let t_edge = t0 + Duration::from_secs(600);
    let result = record_failure(&wf, &make_hash("aaaa0005"), &config, &state, t_edge);

    assert!(
        matches!(result, Ok(Some(_))),
        "Expected quarantine at exact boundary, got {result:?}"
    );
    let status = state.statuses.get(&wf).map(|s| *s);
    assert_eq!(status, Some(RegistrationStatus::Quarantined));
}

/// Attack: 4 failures, then 5th at boundary+1s (first failure just expired).
/// Only 4 unique hashes should remain — no quarantine.
#[test]
fn attack_inv001_boundary_plus_one_second_evicts_first() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("attack-wf-01b");

    record_failure(&wf, &make_hash("aaaa0001"), &config, &state, t0).unwrap();
    let t_mid = t0 + Duration::from_secs(300);
    record_failure(&wf, &make_hash("aaaa0002"), &config, &state, t_mid).unwrap();
    record_failure(&wf, &make_hash("aaaa0003"), &config, &state, t_mid).unwrap();
    record_failure(&wf, &make_hash("aaaa0004"), &config, &state, t_mid).unwrap();

    // Record h5 at t0+601s — h1 (at t0) is now expired (601 > 600)
    let t_expired = t0 + Duration::from_secs(601);
    let result = record_failure(&wf, &make_hash("aaaa0005"), &config, &state, t_expired);

    assert_eq!(
        result,
        Ok(None),
        "Should NOT quarantine with only 4 in window"
    );
    let status = state
        .statuses
        .get(&wf)
        .map(|s| *s)
        .unwrap_or(RegistrationStatus::Active);
    assert_eq!(status, RegistrationStatus::Active);
}
