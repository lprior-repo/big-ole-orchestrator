//! ATTACK VECTOR 4: INV-007 — Eviction edge cases.

use super::helpers::*;
use std::time::{Duration, Instant};

use vo_core::circuit_breaker::{record_failure, CircuitBreakerConfig, CircuitBreakerState};

/// Attack: Record 4 failures at various times, all expire, then record 5 new
/// ones. The window should be clean and quarantine on 5 new ones.
#[test]
fn attack_inv007_full_eviction_then_fresh_threshold() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("attack-wf-07");

    (0..4).for_each(|i| {
        record_failure(&wf, &hash_from_idx(i), &config, &state, t0).unwrap();
    });

    let t_fresh = t0 + Duration::from_secs(660);
    (100..104).for_each(|i| {
        let result = record_failure(&wf, &hash_from_idx(i), &config, &state, t_fresh);
        assert_eq!(result, Ok(None));
    });
    let result = record_failure(&wf, &hash_from_idx(104), &config, &state, t_fresh);
    assert!(
        matches!(result, Ok(Some(_))),
        "5 fresh failures after full eviction should quarantine"
    );
}

/// Attack: Record failures with 255 unique hashes in a very large window.
/// Tests u8 threshold with many more unique hashes than threshold.
#[test]
fn attack_inv007_many_unique_hashes_exceeding_u8_max() {
    let config = CircuitBreakerConfig::new(
        Duration::from_secs(60),
        Duration::from_secs(86400), // 24 hour window
        5,
    )
    .unwrap();

    let state = CircuitBreakerState::new();
    let t0 = Instant::now();
    let wf = make_wf("attack-wf-07b");

    (0..4).for_each(|i| {
        record_failure(&wf, &hash_from_idx(i), &config, &state, t0).unwrap();
    });

    let result = record_failure(&wf, &hash_from_idx(4), &config, &state, t0);
    assert!(matches!(result, Ok(Some(_))));

    (5..300).for_each(|i| {
        let result = record_failure(
            &wf,
            &hash_from_idx(i),
            &config,
            &state,
            t0 + Duration::from_secs(i as u64),
        );
        drop(result.unwrap());
    });
}
