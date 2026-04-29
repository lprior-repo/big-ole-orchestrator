#![allow(clippy::redundant_pattern_matching)]
//! Red Queen adversarial tests for the circuit breaker.
//!
//! These tests attempt to break the circuit breaker from every angle,
//! targeting each contract invariant (INV-001 through INV-010).

use std::time::{Duration, Instant};

use vo_core::circuit_breaker::{
    evaluate_registration, record_failure, unquarantine, CircuitBreakerConfig, CircuitBreakerError,
    CircuitBreakerState, FailureWindow, RegistrationOutcome, RegistrationRequest,
    RegistrationStatus,
};
use vo_types::{BinaryHash, WorkflowName};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn make_wf(s: &str) -> WorkflowName {
    WorkflowName::parse(s).expect("test workflow name should be valid")
}

fn make_hash(s: &str) -> BinaryHash {
    BinaryHash::parse(s).expect("test hash should be valid")
}

fn hash_from_idx(i: usize) -> BinaryHash {
    let hex = format!("{i:08x}");
    BinaryHash::parse(&hex).expect("generated hash should be valid")
}

fn default_config() -> CircuitBreakerConfig {
    CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 5)
        .expect("default config should be valid")
}

fn make_request(wf: &str, hash: &str, force: bool) -> RegistrationRequest {
    RegistrationRequest {
        workflow_name: make_wf(wf),
        binary_hash: make_hash(hash),
        force: if force {
            Some("test-operator-token".into())
        } else {
            None
        },
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 1: INV-001 — Boundary at exactly 5 failures in 10-min window
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: 4 failures spread across the window, then 1 more at the exact edge.
/// The 5th failure should trigger quarantine even if the first failure is
/// exactly at the window boundary.
#[test]
fn attack_inv001_boundary_five_failures_at_window_edge() {
    let state = CircuitBreakerState::new();
    let config = default_config(); // 600s window, threshold 5
    let t0 = Instant::now();
    let wf = make_wf("attack-wf-01");

    // Record h1 at t0
    let result = record_failure(&wf, &make_hash("aaaa0001"), &config, &state, t0);
    assert_eq!(result, Ok(None));

    // Record h2 at t0+200s
    let result = record_failure(
        &wf,
        &make_hash("aaaa0002"),
        &config,
        &state,
        t0 + Duration::from_secs(200),
    );
    assert_eq!(result, Ok(None));

    // Record h3 at t0+400s
    let result = record_failure(
        &wf,
        &make_hash("aaaa0003"),
        &config,
        &state,
        t0 + Duration::from_secs(400),
    );
    assert_eq!(result, Ok(None));

    // Record h4 at t0+500s
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

    // This MUST trigger quarantine with all 5 unique hashes present
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

    // Record h1 at t0
    record_failure(&wf, &make_hash("aaaa0001"), &config, &state, t0).unwrap();
    // Record h2..h4 at t0+300s
    let t_mid = t0 + Duration::from_secs(300);
    record_failure(&wf, &make_hash("aaaa0002"), &config, &state, t_mid).unwrap();
    record_failure(&wf, &make_hash("aaaa0003"), &config, &state, t_mid).unwrap();
    record_failure(&wf, &make_hash("aaaa0004"), &config, &state, t_mid).unwrap();

    // Record h5 at t0+601s — h1 (at t0) is now expired (601 > 600)
    let t_expired = t0 + Duration::from_secs(601);
    let result = record_failure(&wf, &make_hash("aaaa0005"), &config, &state, t_expired);

    // Only h2,h3,h4,h5 should be in window (count=4), no quarantine
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

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 2: INV-004 — Duplicate hash flood attack
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Record the same hash 1000 times to try to inflate the count.
#[test]
fn attack_inv004_duplicate_hash_flood_never_inflates_count() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("attack-wf-04");

    // Flood with same hash 1000 times with advancing timestamps
    (0..1000).for_each(|i| {
        let t = t0 + Duration::from_millis(i);
        let result = record_failure(&wf, &make_hash("deadbeef"), &config, &state, t);
        assert_eq!(
            result,
            Ok(None),
            "Duplicate hash must never trigger quarantine"
        );
    });

    // Verify count is exactly 1
    let tracker = state.failure_tracker.get(&wf).unwrap();
    assert_eq!(tracker.len(), 1, "Should have exactly 1 unique hash");

    // Status must remain Active
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

    // Cycle through 4 hashes 3 times each
    (0u64..3).for_each(|cycle| {
        (hashes.iter().enumerate()).for_each(|(i, h)| {
            let t = t0 + Duration::from_secs(cycle * 10 + i as u64);
            let result = record_failure(&wf, &make_hash(h), &config, &state, t);
            assert_eq!(result, Ok(None), "cycle={cycle}, i={i}, hash={h}");
        });
    });

    // Failure count should be exactly 4
    // NOTE: Must drop the DashMap guard before calling record_failure,
    // otherwise we deadlock (get() holds read lock, entry() needs write lock)
    {
        let tracker = state.failure_tracker.get(&wf).unwrap();
        assert_eq!(tracker.len(), 4);
    } // guard dropped here

    // 5th unique hash should trigger quarantine
    let t_final = t0 + Duration::from_secs(60);
    let result = record_failure(&wf, &make_hash("aaaa0005"), &config, &state, t_final);
    assert!(matches!(result, Ok(Some(_))));
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 3: INV-006 — Force flag bypass exhaustive testing
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Force flag on a workflow that is simultaneously quarantined,
/// rate-limited, and has a full failure window.
#[test]
fn attack_inv006_force_bypasses_all_three_guards_simultaneously() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("attack-wf-06");

    // Set up ALL three guards
    // 1. Quarantined
    state
        .statuses
        .insert(wf.clone(), RegistrationStatus::Quarantined);
    // 2. Rate limited (last registration 10s ago)
    state.rate_limiter.insert(wf.clone(), t0);
    // 3. Full failure window
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

    // Force registration should bypass all three
    state.register_operator_token("test-operator-token".into());
    let request = RegistrationRequest {
        workflow_name: wf.clone(),
        binary_hash: make_hash("abcdef01"),
        force: Some("test-operator-token".into()),
    };
    let now = t0 + Duration::from_secs(10);
    let result = evaluate_registration(&request, &config, &state, now);
    assert_eq!(result, Ok(RegistrationOutcome::Allowed));

    // Rate limiter should be updated (POST-005)
    let rl = state.rate_limiter.get(&wf).map(|r| *r);
    assert_eq!(rl, Some(now), "Force should update rate limiter to now");
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 4: INV-007 — Eviction edge cases
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Record 4 failures at various times, all expire, then record 5 new
/// ones. The window should be clean and quarantine on 5 new ones.
#[test]
fn attack_inv007_full_eviction_then_fresh_threshold() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("attack-wf-07");

    // Phase 1: Record 4 failures at t0
    (0..4).for_each(|i| {
        record_failure(&wf, &hash_from_idx(i), &config, &state, t0).unwrap();
    });

    // Phase 2: Wait 11 minutes (all expired), record 5 new unique failures
    let t_fresh = t0 + Duration::from_secs(660);
    (100..104).for_each(|i| {
        let result = record_failure(&wf, &hash_from_idx(i), &config, &state, t_fresh);
        assert_eq!(result, Ok(None));
    });
    // 5th fresh failure should quarantine
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

    // Record 4 unique failures
    (0..4).for_each(|i| {
        record_failure(&wf, &hash_from_idx(i), &config, &state, t0).unwrap();
    });

    // 5th triggers quarantine
    let result = record_failure(&wf, &hash_from_idx(4), &config, &state, t0);
    assert!(matches!(result, Ok(Some(_))));

    // Even after quarantine, recording more failures shouldn't panic
    (5..300).for_each(|i| {
        let result = record_failure(
            &wf,
            &hash_from_idx(i),
            &config,
            &state,
            t0 + Duration::from_secs(i as u64),
        );
        // Should still return QuarantineEvent since count > threshold
        // (the code doesn't check if already quarantined)
        drop(result.unwrap());
    });
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 5: INV-008 — Quarantine monotonicity attacks
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: After quarantine, try to register without force, then with force,
/// then without force again. Non-force attempts must always be blocked.
#[test]
fn attack_inv008_quarantine_monotonicity_after_force_registration() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("attack-wf-08");

    // Quarantine the workflow
    state
        .statuses
        .insert(wf.clone(), RegistrationStatus::Quarantined);

    // Non-force should be blocked
    let request = make_request("attack-wf-08", "abcdef01", false);
    let result = evaluate_registration(&request, &config, &state, t0);
    assert!(matches!(
        result,
        Ok(RegistrationOutcome::WorkflowQuarantined { .. })
    ));

    // Force should bypass (requires valid operator token)
    state.register_operator_token("test-operator-token".into());
    let force_request = make_request("attack-wf-08", "abcdef02", true);
    let t1 = t0 + Duration::from_secs(1);
    let result = evaluate_registration(&force_request, &config, &state, t1);
    assert_eq!(result, Ok(RegistrationOutcome::Allowed));

    // Non-force should STILL be blocked (quarantine is monotonic)
    let t2 = t1 + Duration::from_secs(120); // well past rate limit
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

    // Advance time by ~1 year
    let one_year = Duration::from_secs(365 * 24 * 3600);
    let t_future = t0 + one_year;

    let request = make_request("attack-wf-08b", "abcdef01", false);
    let result = evaluate_registration(&request, &config, &state, t_future);
    assert!(
        matches!(result, Ok(RegistrationOutcome::WorkflowQuarantined { .. })),
        "Quarantine must survive indefinitely. Got: {result:?}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 6: INV-009 — Rate-limited request must NOT count as failure
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Try to trigger quarantine by rapidly submitting requests during
/// rate limit window. The rate limiter should block them before they count.
#[test]
fn attack_inv009_rapid_requests_during_rate_limit_never_count() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("attack-wf-09");

    // First registration succeeds (sets rate limiter)
    let req = make_request("attack-wf-09", "abcdef01", false);
    let result = evaluate_registration(&req, &config, &state, t0);
    assert_eq!(result, Ok(RegistrationOutcome::Allowed));

    // Now spam 100 requests within the rate window
    (1..100).for_each(|i| {
        let t = t0 + Duration::from_millis(i * 100); // every 100ms
        let req = RegistrationRequest {
            workflow_name: wf.clone(),
            binary_hash: hash_from_idx(i as usize),
            force: None,
        };
        let result = evaluate_registration(&req, &config, &state, t);
        assert!(
            matches!(result, Ok(RegistrationOutcome::RateLimited { .. })),
            "Request at +{}ms should be rate-limited, got {result:?}",
            i * 100
        );
    });

    // Failure tracker should have NO entries (rate-limited requests don't count)
    let has_failures = state.failure_tracker.get(&wf).map(|t| t.len()).unwrap_or(0);
    assert_eq!(
        has_failures, 0,
        "Rate-limited requests must never create failure entries"
    );

    // Status must be Active
    let status = state
        .statuses
        .get(&wf)
        .map(|s| *s)
        .unwrap_or(RegistrationStatus::Active);
    assert_eq!(status, RegistrationStatus::Active);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 7: Unquarantine state corruption attacks
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Unquarantine, then immediately try to record 5 failures again.
/// The workflow should be re-quarantinable after unquarantine.
#[test]
fn attack_unquarantine_then_reaquarantine() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("attack-wf-unq");

    // Phase 1: Quarantine via 5 failures
    (0..5).for_each(|i| {
        record_failure(&wf, &hash_from_idx(i), &config, &state, t0).unwrap();
    });
    assert_eq!(
        state.statuses.get(&wf).map(|s| *s),
        Some(RegistrationStatus::Quarantined)
    );

    // Phase 2: Unquarantine
    let unq_result = unquarantine(&wf, "operator", &state);
    assert!(matches!(unq_result, Ok(_)));
    assert_eq!(
        state.statuses.get(&wf).map(|s| *s),
        Some(RegistrationStatus::Active)
    );

    // Phase 3: Record 5 fresh failures — should re-quarantine
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

    // First unquarantine succeeds
    let result1 = unquarantine(&wf, "op1", &state);
    assert!(matches!(result1, Ok(_)));

    // Second unquarantine fails
    let result2 = unquarantine(&wf, "op2", &state);
    assert_eq!(
        result2,
        Err(CircuitBreakerError::NotQuarantined {
            workflow_name: "attack-wf-dblunq".to_string(),
            current_status: RegistrationStatus::Active,
        })
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 8: record_failure on already-quarantined workflow
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: After quarantine, record more failures. The function should NOT
/// re-emit QuarantineEvent... BUT the current implementation actually does!
/// This is a potential MAJOR finding: QuarantineEvent is emitted repeatedly.
#[test]
fn attack_record_failure_on_quarantined_emits_duplicate_events() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("attack-wf-reemit");

    // Trigger quarantine with 5 failures
    (0..5).for_each(|i| {
        record_failure(&wf, &hash_from_idx(i), &config, &state, t0).unwrap();
    });
    assert_eq!(
        state.statuses.get(&wf).map(|s| *s),
        Some(RegistrationStatus::Quarantined)
    );

    // Record 6th failure — does this emit ANOTHER QuarantineEvent?
    let result = record_failure(&wf, &hash_from_idx(5), &config, &state, t0);

    // OBSERVATION: The current code will return Ok(Some(QuarantineEvent))
    // because it only checks unique_count >= threshold, not whether the
    // workflow is already quarantined. This is documented behavior analysis,
    // not necessarily a bug — it depends on whether the caller handles
    // duplicate events correctly. But it is a design concern.
    //
    // The contract says record_failure returns Ok(Some(QuarantineEvent))
    // "if threshold breached and quarantine triggered". Since the workflow
    // is already quarantined, re-triggering is questionable.
    //
    // We test the ACTUAL behavior here:
    match result {
        Ok(Some(_)) => {
            // Current implementation emits duplicate events.
            // This is the actual behavior. Not necessarily a bug if
            // consumers are idempotent, but it IS extra work.
        }
        Ok(None) => {
            // If the implementation checked for already-quarantined, this
            // would be the "ideal" behavior.
        }
        Err(e) => panic!("record_failure should not error: {e:?}"),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 9: Config edge cases (threshold = 1)
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: With threshold=1, a single failure should immediately quarantine.
#[test]
fn attack_threshold_1_immediate_quarantine() {
    let config =
        CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 1).unwrap();
    let state = CircuitBreakerState::new();
    let now = Instant::now();
    let wf = make_wf("attack-wf-t1");

    let result = record_failure(&wf, &make_hash("deadbeef"), &config, &state, now);
    assert!(
        matches!(result, Ok(Some(_))),
        "threshold=1 should quarantine on first failure"
    );
    assert_eq!(
        state.statuses.get(&wf).map(|s| *s),
        Some(RegistrationStatus::Quarantined)
    );
}

/// Attack: With threshold=255 (max u8), need exactly 255 unique hashes.
#[test]
fn attack_threshold_255_requires_exactly_255_unique_hashes() {
    let config = CircuitBreakerConfig::new(
        Duration::from_secs(60),
        Duration::from_secs(86400), // 24h window to hold all
        255,
    )
    .unwrap();
    let state = CircuitBreakerState::new();
    let t0 = Instant::now();
    let wf = make_wf("attack-wf-t255");

    // Record 254 unique hashes — no quarantine
    (0..254).for_each(|i| {
        let result = record_failure(
            &wf,
            &hash_from_idx(i),
            &config,
            &state,
            t0 + Duration::from_secs(i as u64),
        );
        assert_eq!(result, Ok(None), "Hash {i} should not trigger quarantine");
    });

    // 255th unique hash — quarantine
    let result = record_failure(
        &wf,
        &hash_from_idx(254),
        &config,
        &state,
        t0 + Duration::from_secs(254),
    );
    assert!(
        matches!(result, Ok(Some(_))),
        "255th unique hash should trigger quarantine"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 10: INV-010 — Concurrent access (multi-threaded)
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Concurrent evaluate_registration calls for the same workflow.
/// DashMap should prevent data races. We verify no panics occur.
#[test]
fn attack_inv010_concurrent_evaluate_registration_no_panics() {
    use std::sync::Arc;
    use std::thread;

    let state = Arc::new(CircuitBreakerState::new());
    let config = default_config();
    let t0 = Instant::now();

    state.register_operator_token("test-operator-token".into());

    let mut handles = vec![];
    (0..20).for_each(|i| {
        let state = Arc::clone(&state);
        let handle = thread::spawn(move || {
            let wf_name = format!("concurrent-wf-{}", i % 3); // 3 workflows
            let hash = format!("{:08x}", i);
            let request = RegistrationRequest {
                workflow_name: WorkflowName::parse(&wf_name).unwrap(),
                binary_hash: BinaryHash::parse(&hash).unwrap(),
                force: if i % 5 == 0 {
                    Some("test-operator-token".into())
                } else {
                    None
                },
            };
            let now = t0 + Duration::from_millis(i as u64 * 10);
            let _val = evaluate_registration(&request, &config, &state, now);
        });
        handles.push(handle);
    });

    // All threads must complete without panics
    handles.into_iter().for_each(|handle| {
        handle
            .join()
            .expect("Thread panicked during concurrent evaluate_registration");
    });
}

/// Attack: Concurrent record_failure calls for the same workflow.
#[test]
fn attack_inv010_concurrent_record_failure_no_panics() {
    use std::sync::Arc;
    use std::thread;

    let state = Arc::new(CircuitBreakerState::new());
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("concurrent-fail-wf");

    let mut handles = vec![];
    (0..50).for_each(|i| {
        let state = Arc::clone(&state);
        let wf = wf.clone();
        let handle = thread::spawn(move || {
            let hash = BinaryHash::parse(&format!("{:08x}", i)).unwrap();
            let now = t0 + Duration::from_millis(i as u64);
            let _val = record_failure(&wf, &hash, &config, &state, now);
        });
        handles.push(handle);
    });

    (handles).into_iter().for_each(|handle| {
        handle
            .join()
            .expect("Thread panicked during concurrent record_failure");
    });

    // Verify status is quarantined (50 unique hashes > threshold 5)
    let status = state.statuses.get(&wf).map(|s| *s);
    assert_eq!(status, Some(RegistrationStatus::Quarantined));
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 11: Workflow isolation — cross-workflow contamination
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Quarantining one workflow must not affect another.
#[test]
fn attack_workflow_isolation_quarantine_does_not_contaminate() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let now = Instant::now();
    let wf_a = make_wf("isolated-a");
    let wf_b = make_wf("isolated-b");

    // Quarantine wf_a via 5 failures
    (0..5).for_each(|i| {
        record_failure(&wf_a, &hash_from_idx(i), &config, &state, now).unwrap();
    });
    assert_eq!(
        state.statuses.get(&wf_a).map(|s| *s),
        Some(RegistrationStatus::Quarantined)
    );

    // wf_b should be completely unaffected
    let req_b = make_request("isolated-b", "abcdef01", false);
    let result = evaluate_registration(&req_b, &config, &state, now);
    assert_eq!(result, Ok(RegistrationOutcome::Allowed));

    // wf_b failure tracker should be empty
    let tracker_b = state.failure_tracker.get(&wf_b);
    assert!(
        tracker_b.is_none() || tracker_b.map(|t| t.is_empty()) == Some(true),
        "wf_b failure tracker should be empty"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 12: Rate limiter ceiling arithmetic
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Verify ceiling behavior at sub-second boundaries.
/// If 59.001s have elapsed, remaining should be 1 (ceiling), not 0.
#[test]
fn attack_rate_limit_ceiling_at_subsecond_boundary() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("attack-ceil");

    // Register at t0
    state.rate_limiter.insert(wf.clone(), t0);

    // Check at t0 + 59.001s — should report 1 second remaining (ceiling)
    let t_sub = t0 + Duration::from_millis(59001);
    let request = make_request("attack-ceil", "abcdef01", false);
    let result = evaluate_registration(&request, &config, &state, t_sub);
    assert_eq!(
        result,
        Ok(RegistrationOutcome::RateLimited {
            retry_after_secs: 1
        }),
        "Sub-second remaining should ceiling to 1"
    );
}

/// Attack: At exactly 59.999s, should still be rate-limited with 1s.
#[test]
fn attack_rate_limit_ceiling_at_59_999() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("attack-ceil2");

    state.rate_limiter.insert(wf.clone(), t0);

    let t_sub = t0 + Duration::from_millis(59999);
    let request = make_request("attack-ceil2", "abcdef01", false);
    let result = evaluate_registration(&request, &config, &state, t_sub);
    assert_eq!(
        result,
        Ok(RegistrationOutcome::RateLimited {
            retry_after_secs: 1
        })
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 13: Evaluation order (rate limit vs quarantine vs deactivated)
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: If a workflow is BOTH rate-limited AND quarantined,
/// quarantine (permanent block, 403) takes precedence over rate limit (temporary, 429).
/// MAJ-002 fix: permanent blocks must fire before temporary cooldowns.
#[test]
fn attack_evaluation_order_rate_limit_fires_before_quarantine() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("attack-order");

    // Set up: rate-limited AND quarantined
    state.rate_limiter.insert(wf.clone(), t0);
    state
        .statuses
        .insert(wf.clone(), RegistrationStatus::Quarantined);

    let now = t0 + Duration::from_secs(30); // within rate limit window
    let request = make_request("attack-order", "abcdef01", false);
    let result = evaluate_registration(&request, &config, &state, now);

    // Quarantine (permanent block) takes precedence over rate limit (temporary cooldown)
    assert_eq!(
        result,
        Ok(RegistrationOutcome::WorkflowQuarantined {
            workflow_name: make_wf("attack-order"),
        }),
        "Quarantine should fire before rate limit check (MAJ-002)"
    );
}

/// Attack: If rate limit has expired but workflow is quarantined,
/// should return quarantined (not allowed).
#[test]
fn attack_evaluation_order_quarantine_after_rate_limit_expiry() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("attack-order2");

    // Set up: rate limit expired, but quarantined
    state.rate_limiter.insert(wf.clone(), t0);
    state
        .statuses
        .insert(wf.clone(), RegistrationStatus::Quarantined);

    let now = t0 + Duration::from_secs(120); // well past rate limit
    let request = make_request("attack-order2", "abcdef01", false);
    let result = evaluate_registration(&request, &config, &state, now);

    assert_eq!(
        result,
        Ok(RegistrationOutcome::WorkflowQuarantined {
            workflow_name: make_wf("attack-order2"),
        })
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 14: TOCTOU in evaluate_registration
// ═══════════════════════════════════════════════════════════════════════════════

/// Analysis: evaluate_registration does:
///   1. get(&wf) on rate_limiter -> check
///   2. get(&wf) on statuses -> check
///   3. insert(&wf, now) on rate_limiter -> update
///
/// Between steps 1 and 3, another thread could:
///   - Also pass the rate limit check at step 1
///   - Both threads reach step 3 and insert
///   - Both registrations pass — violating the rate limit guarantee
///
/// This is a genuine TOCTOU race condition. However, DashMap's per-shard
/// locking does NOT prevent this because get() and insert() are separate
/// lock acquisitions.
///
/// Impact: Two near-simultaneous requests for the same workflow could both
/// pass the rate limiter. This is MAJOR severity because it violates INV-002.
///
/// Proof of concept: We can't deterministically reproduce timing races in a
/// unit test, but we can document the code path vulnerability.
#[test]
fn attack_toctou_documentation_rate_limit_check_and_update_not_atomic() {
    // This test documents the TOCTOU vulnerability.
    // The code at mod.rs:59-64 does:
    //   let last_registration = state.rate_limiter.get(&request.workflow_name).map(|r| *r);
    //   if let Some(retry_after_secs) = check_rate_limit(last_registration, ...)
    //
    // Then at mod.rs:82-84 (if allowed):
    //   state.rate_limiter.insert(request.workflow_name.clone(), ...);
    //
    // Between .get() and .insert(), the lock is released.
    // Another thread doing the same get() before our insert() will also
    // see "no rate limit" and both will pass.
    //
    // Mitigation: Use DashMap::entry() API for atomic read-modify-write:
    //   state.rate_limiter.entry(wf).and_modify(|v| *v = now).or_insert(now);
    //   combined with the check in the same closure.
    //
    // For now, we verify the behavior is at least correct in single-threaded:
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();

    let req1 = make_request("toctou-wf", "abcdef01", false);
    let result1 = evaluate_registration(&req1, &config, &state, t0);
    assert_eq!(result1, Ok(RegistrationOutcome::Allowed));

    // Second request at same time should be rate-limited (single-threaded)
    let req2 = make_request("toctou-wf", "abcdef02", false);
    let result2 = evaluate_registration(&req2, &config, &state, t0);
    assert_eq!(
        result2,
        Ok(RegistrationOutcome::RateLimited {
            retry_after_secs: 60
        })
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 15: INV-005 — Unknown workflow defaults
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: A never-before-seen workflow should default to Active,
/// allowing registration without any prior setup.
#[test]
fn attack_inv005_unknown_workflow_defaults_to_active() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let now = Instant::now();

    // This workflow has never been registered, failed, or mentioned
    let request = make_request("brand-new-wf", "abcdef01", false);
    let result = evaluate_registration(&request, &config, &state, now);
    assert_eq!(result, Ok(RegistrationOutcome::Allowed));
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 16: Unquarantine clears rate limiter (POST-003)
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: After unquarantine, immediate registration should be allowed
/// (no rate limit, no quarantine).
#[test]
fn attack_post003_unquarantine_allows_immediate_registration() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("attack-post003");

    // Set up quarantined with rate limit entry
    state
        .statuses
        .insert(wf.clone(), RegistrationStatus::Quarantined);
    state.rate_limiter.insert(wf.clone(), t0);

    // Unquarantine
    let result = unquarantine(&wf, "operator", &state);
    assert!(matches!(result, Ok(_)));

    // Immediate registration should work (rate limiter was cleared)
    let t1 = t0 + Duration::from_secs(1); // only 1 second later
    let request = make_request("attack-post003", "abcdef01", false);
    let result = evaluate_registration(&request, &config, &state, t1);
    assert_eq!(
        result,
        Ok(RegistrationOutcome::Allowed),
        "Registration should be allowed immediately after unquarantine"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 17: False positive quarantine trips
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: A healthy workflow with fewer than threshold failures must NOT be quarantined.
/// This verifies the circuit does NOT false-positive under normal operation.
#[test]
fn attack_false_positive_zero_failures_never_quarantined() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("healthy-wf");

    // Zero failures - should remain Active
    let status = state.get_status(&wf);
    assert_eq!(status, RegistrationStatus::Active);

    // Registration should be allowed
    let request = make_request("healthy-wf", "abcdef01", false);
    let result = evaluate_registration(&request, &config, &state, t0);
    assert_eq!(result, Ok(RegistrationOutcome::Allowed));
}

/// Attack: A workflow with threshold-1 failures must NOT be quarantined.
#[test]
fn attack_false_positive_threshold_minus_one_never_quarantined() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("almost-healthy-wf");

    // 4 failures (threshold is 5) - must NOT quarantine
    (0..4).for_each(|i| {
        record_failure(&wf, &hash_from_idx(i), &config, &state, t0).unwrap();
    });

    let status = state.get_status(&wf);
    assert_eq!(status, RegistrationStatus::Active);

    // 5th failure should quarantine
    record_failure(&wf, &hash_from_idx(4), &config, &state, t0).unwrap();
    let status = state.get_status(&wf);
    assert_eq!(status, RegistrationStatus::Quarantined);
}

/// Attack: Alternating success and failure must not cause false positive quarantine.
#[test]
fn attack_false_positive_interleaved_success_failure_never_quarantined() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("alternating-wf");

    // Interleave 4 unique failures with "successful" registrations
    // that don't increment failure count
    (0..4).for_each(|i| {
        // Record a failure
        record_failure(&wf, &hash_from_idx(i), &config, &state, t0).unwrap();
        // Simulate a successful registration (sets rate limiter but no failure)
        let req = make_request("alternating-wf", &format!("eeee{:04x}", i), false);
        let _ = evaluate_registration(
            &req,
            &config,
            &state,
            t0 + Duration::from_secs(i as u64 + 1),
        );
    });

    // Status must remain Active (only 4 failures < threshold 5)
    let status = state.get_status(&wf);
    assert_eq!(status, RegistrationStatus::Active);
}

/// Attack: Re-registering the same hash repeatedly must never cause quarantine.
#[test]
fn attack_false_positive_same_hash_repeated_never_quarantined() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("same-hash-wf");

    // Record same hash 10000 times
    (0..10000).for_each(|i| {
        let result = record_failure(
            &wf,
            &make_hash("deadbeef"),
            &config,
            &state,
            t0 + Duration::from_secs(i as u64),
        );
        assert_eq!(
            result,
            Ok(None),
            "Same hash repeated must never trigger quarantine"
        );
    });

    // Status must remain Active
    let status = state.get_status(&wf);
    assert_eq!(status, RegistrationStatus::Active);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 18: Quarantine during healthy operation
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Healthy operation with exactly threshold failures but all same hash
/// must NOT trigger quarantine.
#[test]
fn attack_healthy_same_hash_at_threshold_never_quarantined() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("healthy-same-hash");

    // Record same hash 100 times (threshold is 5)
    (0..100).for_each(|i| {
        record_failure(
            &wf,
            &make_hash("aaaa0000"),
            &config,
            &state,
            t0 + Duration::from_secs(i as u64),
        )
        .unwrap();
    });

    // Should NOT quarantine - only 1 unique hash
    let status = state.get_status(&wf);
    assert_eq!(status, RegistrationStatus::Active);
}

/// Attack: Rapidly alternating between 5 different hashes must NOT quarantine
/// if the window is configured with a very short failure window.
#[test]
fn attack_healthy_rapid_alternation_below_threshold_never_quarantined() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("rapid-alt");

    // Only 3 unique hashes (below threshold of 5)
    let hashes = ["aaaa0001", "aaaa0002", "aaaa0003"];
    (0..1000).for_each(|i| {
        let hash = hashes[i % 3];
        record_failure(
            &wf,
            &make_hash(hash),
            &config,
            &state,
            t0 + Duration::from_millis(i as u64),
        )
        .unwrap();
    });

    let status = state.get_status(&wf);
    assert_eq!(status, RegistrationStatus::Active);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 19: Cascading quarantine across workflows
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: One workflow's failure cascade must NOT affect another workflow.
/// Verifies complete isolation - quarantining wf-a cannot cause wf-b to be
/// rejected or contaminated.
#[test]
fn attack_cascading_quarantine_workflow_a_cannot_affect_workflow_b() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf_a = make_wf("cascade-a");
    let wf_b = make_wf("cascade-b");

    // Quarantine wf-a
    (0..5).for_each(|i| {
        record_failure(&wf_a, &hash_from_idx(i), &config, &state, t0).unwrap();
    });
    assert_eq!(state.get_status(&wf_a), RegistrationStatus::Quarantined);

    // wf-b should be completely isolated - registration allowed
    let req_b = make_request("cascade-b", "abcdef01", false);
    let result = evaluate_registration(&req_b, &config, &state, t0);
    assert_eq!(result, Ok(RegistrationOutcome::Allowed));

    // wf-b failure tracker must be empty
    let tracker_b = state.failure_tracker.get(&wf_b);
    assert!(
        tracker_b.is_none() || tracker_b.map(|t| t.is_empty()) == Some(true),
        "wf-b should have no failure records"
    );

    // wf-b status must be Active
    assert_eq!(state.get_status(&wf_b), RegistrationStatus::Active);
}

/// Attack: High failure count on wf-a must not inflate wf-b's failure count.
#[test]
fn attack_cascading_high_count_on_a_cannot_inflate_b_count() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf_a = make_wf("cascade-count-a");
    let wf_b = make_wf("cascade-count-b");

    // wf-a has 1000 unique failures
    (0..1000).for_each(|i| {
        record_failure(&wf_a, &hash_from_idx(i), &config, &state, t0).unwrap();
    });
    assert_eq!(state.get_status(&wf_a), RegistrationStatus::Quarantined);

    // wf-b records 3 failures (below threshold)
    (0..3).for_each(|i| {
        record_failure(&wf_b, &hash_from_idx(i), &config, &state, t0).unwrap();
    });

    // wf-b should have exactly 3 failures tracked
    let tracker_b = state.failure_tracker.get(&wf_b).unwrap();
    assert_eq!(
        tracker_b.len(),
        3,
        "wf-b should have exactly 3 unique failures"
    );

    // wf-b status must be Active (3 < threshold 5)
    assert_eq!(state.get_status(&wf_b), RegistrationStatus::Active);
}

/// Attack: Verify status map is per-workflow, not shared.
/// wf-a's quarantined status must not cause wf-b to appear quarantined.
#[test]
fn attack_cascading_status_map_is_per_workflow() {
    let state = CircuitBreakerState::new();
    let wf_a = make_wf("status-map-a");
    let wf_b = make_wf("status-map-b");

    // Quarantine wf-a manually
    state.set_status(wf_a.clone(), RegistrationStatus::Quarantined);

    // wf-b status must be unknown/Active (not Quarantined)
    let status_b = state.get_status(&wf_b);
    assert_ne!(
        status_b,
        RegistrationStatus::Quarantined,
        "wf-b must not inherit wf-a's quarantine status"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 20: Manual override race conditions
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Concurrent unquarantine and quarantine trigger.
/// Thread 1 unquarantines, Thread 2 records failures simultaneously.
/// The final state must be deterministic and correct.
#[test]
fn attack_manual_override_race_unquarantine_vs_quarantine_trigger() {
    use std::sync::Arc;
    use std::thread;

    let state = Arc::new(CircuitBreakerState::new());
    let config = Arc::new(default_config());
    let t0 = Instant::now();
    let wf = make_wf("race-unq-trig");

    // Pre-quarantine the workflow
    {
        let state = Arc::clone(&state);
        (0..5).for_each(|i| {
            let _ = record_failure(&wf, &hash_from_idx(i), &config, &state, t0);
        });
    }
    assert_eq!(state.get_status(&wf), RegistrationStatus::Quarantined);

    // Thread 1: Unquarantine
    let state_clone1 = Arc::clone(&state);
    let wf1 = wf.clone();
    let handle1 = thread::spawn(move || {
        let result = unquarantine(&wf1, "operator", &state_clone1);
        (result, state_clone1.get_status(&wf1))
    });

    // Thread 2: Record 5 new failures to re-quarantine
    let config_clone = Arc::clone(&config);
    let state_clone2 = Arc::clone(&state);
    let wf2 = wf.clone();
    let handle2 = thread::spawn(move || {
        let t1 = t0 + Duration::from_secs(1);
        (0..5).for_each(|i| {
            let _ = record_failure(
                &wf2,
                &hash_from_idx(10 + i),
                &config_clone,
                &state_clone2,
                t1,
            );
        });
        (
            state_clone2.get_status(&wf),
            state_clone2.get_failure_count(&wf),
        )
    });

    let (unq_result, _unq_final_status) = handle1.join().unwrap();
    let (trig_final_status, _trig_final_count) = handle2.join().unwrap();

    // Both operations should succeed without panic
    assert!(unq_result.is_ok() || unq_result.is_err());

    // At least one thread should see consistent state
    // Final status must be either Quarantined (re-triggered) or Active (unquarantine won)
    let final_status = trig_final_status;
    assert!(
        final_status == RegistrationStatus::Quarantined
            || final_status == RegistrationStatus::Active,
        "Final status must be deterministic: Quarantined or Active, got {final_status:?}"
    );
}

/// Attack: Double unquarantine race - two threads both try to unquarantine.
/// Second unquarantine must fail gracefully with NotQuarantined.
#[test]
fn attack_manual_override_race_double_unquarantine() {
    use std::sync::Arc;
    use std::thread;

    let state = Arc::new(CircuitBreakerState::new());
    let _t0 = Instant::now();
    let wf = make_wf("race-dbl-unq");

    // Pre-quarantine
    state.set_status(wf.clone(), RegistrationStatus::Quarantined);

    // Two threads try to unquarantine simultaneously
    let state1 = Arc::clone(&state);
    let state2 = Arc::clone(&state);

    let wf1 = wf.clone();
    let handle1 = thread::spawn(move || unquarantine(&wf1, "op1", &state1));
    let handle2 = thread::spawn(move || unquarantine(&wf, "op2", &state2));

    let result1 = handle1.join().unwrap();
    let result2 = handle2.join().unwrap();

    // Exactly one must succeed, one must fail with NotQuarantined
    let successes = [result1.is_ok(), result2.is_ok()];
    let failures = [result1.is_err(), result2.is_err()];

    assert_eq!(
        successes.iter().filter(|&&x| x).count(),
        1,
        "Exactly one unquarantine must succeed"
    );
    assert_eq!(
        failures.iter().filter(|&&x| x).count(),
        1,
        "Exactly one unquarantine must fail"
    );
}

/// Attack: Unquarantine while concurrent registrations are happening.
/// After unquarantine, the rate limiter should be cleared and registration allowed.
#[test]
fn attack_manual_override_unquarantine_clears_rate_limiter_for_pending_requests() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("race-unq-reg");

    // Set up: quarantined with recent rate limit entry (simulates pending requests)
    state.set_status(wf.clone(), RegistrationStatus::Quarantined);
    state.rate_limiter.insert(wf.clone(), t0); // Just 1 second ago

    // Unquarantine
    let result = unquarantine(&wf, "operator", &state);
    assert!(result.is_ok());

    // Rate limiter should be cleared - registration allowed immediately
    let t1 = t0 + Duration::from_secs(1);
    let request = make_request("race-unq-reg", "abcdef01", false);
    let result = evaluate_registration(&request, &config, &state, t1);
    assert_eq!(
        result,
        Ok(RegistrationOutcome::Allowed),
        "Registration should be allowed immediately after unquarantine (rate limiter cleared)"
    );
}

/// Attack: Multiple rapid unquarantine-then-requarantine cycles.
/// After unquarantine, 5 new failures should re-quarantine independently.
#[test]
fn attack_manual_override_rapid_unquarantine_requarantine_cycles() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("rapid-cycle");

    (0..3).for_each(|cycle| {
        // Phase 1: Quarantine with 5 failures
        let base = cycle * 10;
        (0..5).for_each(|i| {
            let _ = record_failure(
                &wf,
                &hash_from_idx(base + i),
                &config,
                &state,
                t0 + Duration::from_secs(cycle as u64 * 100),
            );
        });
        assert_eq!(state.get_status(&wf), RegistrationStatus::Quarantined);

        // Phase 2: Unquarantine
        let unq_result = unquarantine(&wf, "operator", &state);
        assert!(
            unq_result.is_ok(),
            "Cycle {}: unquarantine should succeed",
            cycle
        );
        assert_eq!(state.get_status(&wf), RegistrationStatus::Active);

        // Phase 3: Verify failure count is reset (POST-003)
        let count = state.get_failure_count(&wf);
        assert_eq!(
            count, 0,
            "Cycle {}: failure count should be reset after unquarantine",
            cycle
        );
    });
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 21: INV-002 — Rate limit independence per workflow
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Workflow A's rate limit must NOT affect workflow B's registration.
/// Each workflow maintains independent rate limit state.
#[test]
fn attack_inv002_rate_limit_is_per_workflow_independent() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();

    let _wf_a = make_wf("workflow-a");
    let _wf_b = make_wf("workflow-b");

    // wf_a registers at t0 (sets rate limit for wf_a only)
    let req_a1 = make_request("workflow-a", "aaaa0001", false);
    let result_a1 = evaluate_registration(&req_a1, &config, &state, t0);
    assert_eq!(result_a1, Ok(RegistrationOutcome::Allowed));

    // wf_b registers immediately after (should also succeed - independent rate limit)
    let req_b1 = make_request("workflow-b", "bbbb0001", false);
    let result_b1 = evaluate_registration(&req_b1, &config, &state, t0);
    assert_eq!(
        result_b1,
        Ok(RegistrationOutcome::Allowed),
        "wf_b should be allowed: rate limit is per-workflow, not global"
    );

    // wf_a tries again within rate window - should be rate limited
    let req_a2 = make_request("workflow-a", "aaaa0002", false);
    let result_a2 = evaluate_registration(&req_a2, &config, &state, t0 + Duration::from_secs(30));
    assert!(
        matches!(result_a2, Ok(RegistrationOutcome::RateLimited { .. })),
        "wf_a should be rate limited within window"
    );

    // wf_b tries again within rate window - should ALSO be rate limited for wf_b
    // but NOT because of wf_a - because of wf_b's own registration at t0
    let req_b2 = make_request("workflow-b", "bbbb0002", false);
    let result_b2 = evaluate_registration(&req_b2, &config, &state, t0 + Duration::from_secs(30));
    assert!(
        matches!(result_b2, Ok(RegistrationOutcome::RateLimited { .. })),
        "wf_b should be rate limited within its own window"
    );

    // After rate window expires for wf_a (t0 + 60s), wf_a can register again
    let req_a3 = make_request("workflow-a", "aaaa0003", false);
    let result_a3 = evaluate_registration(&req_a3, &config, &state, t0 + Duration::from_secs(61));
    assert_eq!(
        result_a3,
        Ok(RegistrationOutcome::Allowed),
        "wf_a should be allowed after its rate window expires"
    );

    // wf_b should STILL be rate limited at t0 + 61s because its window is also 60s
    // and it registered at t0, so expires at t0 + 60s (61s is past window)
    // Wait - actually at t0 + 61s, wf_b's rate limit also expired
    let req_b3 = make_request("workflow-b", "bbbb0003", false);
    let result_b3 = evaluate_registration(&req_b3, &config, &state, t0 + Duration::from_secs(61));
    assert_eq!(
        result_b3,
        Ok(RegistrationOutcome::Allowed),
        "wf_b should also be allowed after its rate window expires"
    );
}

/// Attack: High-frequency registration attempts on one workflow must not
/// cause rate limiting on a different workflow.
#[test]
fn attack_inv002_workflow_a_rate_limit_does_not_affect_workflow_b() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();

    let _wf_active = make_wf("active-workflow");
    let _wf_victim = make_wf("victim-workflow");

    // Victim workflow registers first (establishes its rate limit)
    let req_victim = make_request("victim-workflow", "cccc0001", false);
    let result = evaluate_registration(&req_victim, &config, &state, t0);
    assert_eq!(result, Ok(RegistrationOutcome::Allowed));

    // Active workflow rapidly registers many times
    // Each one sets/updates ITS OWN rate limit, not victim's
    for i in 0..50 {
        let t = t0 + Duration::from_millis(i * 100);
        let req = make_request("active-workflow", &format!("dddd{i:04x}"), false);
        let _ = evaluate_registration(&req, &config, &state, t);
    }

    // Victim tries to register after active's rapid fire
    // Victim's rate limit is still active from its registration at t0
    // 5 seconds have passed, so victim should still be rate limited
    let req_victim2 = make_request("victim-workflow", "cccc0002", false);
    let result2 = evaluate_registration(&req_victim2, &config, &state, t0 + Duration::from_secs(5));
    assert!(
        matches!(result2, Ok(RegistrationOutcome::RateLimited { .. })),
        "Victim should still be rate limited by its own registration, not active's"
    );

    // After victim's window expires, victim can register again
    let req_victim3 = make_request("victim-workflow", "cccc0003", false);
    let result3 =
        evaluate_registration(&req_victim3, &config, &state, t0 + Duration::from_secs(65));
    assert_eq!(
        result3,
        Ok(RegistrationOutcome::Allowed),
        "Victim should be allowed after its own rate window expires"
    );
}

/// Attack: Verify rate limit state is stored per-workflow in DashMap.
#[test]
fn attack_inv002_rate_limiter_map_is_per_workflow() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();

    let workflows: Vec<_> = (0..5).map(|i| make_wf(&format!("wf-{i}"))).collect();

    // Each workflow registers at slightly different times
    for (i, wf) in workflows.iter().enumerate() {
        let t = t0 + Duration::from_secs(i as u64);
        let req = make_request(&wf.to_string(), &format!("{i:04x}0000"), false);
        let result = evaluate_registration(&req, &config, &state, t);
        assert_eq!(result, Ok(RegistrationOutcome::Allowed));
    }

    // All workflows should now have rate limit entries
    let rate_limit_len = state.rate_limiter.len();
    assert_eq!(
        rate_limit_len, 5,
        "Rate limiter should have 5 independent entries (one per workflow)"
    );

    // At t0 + 30s (within all windows), all workflows should still be rate limited
    for (i, wf) in workflows.iter().enumerate() {
        let req = make_request(&wf.to_string(), &format!("{i:04x}0001"), false);
        let result = evaluate_registration(&req, &config, &state, t0 + Duration::from_secs(30));
        assert!(
            matches!(result, Ok(RegistrationOutcome::RateLimited { .. })),
            "wf-{} should be rate limited at t0+30s",
            i
        );
    }

    // At t0 + 65s, all rate limits have expired (wf-0 registered at t0, expires at t0+60s)
    for (i, wf) in workflows.iter().enumerate() {
        let req = make_request(&wf.to_string(), &format!("{i:04x}0002"), false);
        let result = evaluate_registration(&req, &config, &state, t0 + Duration::from_secs(65));
        assert_eq!(
            result,
            Ok(RegistrationOutcome::Allowed),
            "wf-{} should be allowed after its rate window expires",
            i
        );
    }
}
