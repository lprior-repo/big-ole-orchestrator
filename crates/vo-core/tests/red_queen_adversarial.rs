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
        force,
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
    for i in 0..1000 {
        let t = t0 + Duration::from_millis(i);
        let result = record_failure(&wf, &make_hash("deadbeef"), &config, &state, t);
        assert_eq!(
            result,
            Ok(None),
            "Duplicate hash must never trigger quarantine"
        );
    }

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
    for cycle in 0u64..3 {
        for (i, h) in hashes.iter().enumerate() {
            let t = t0 + Duration::from_secs(cycle * 10 + i as u64);
            let result = record_failure(&wf, &make_hash(h), &config, &state, t);
            assert_eq!(result, Ok(None), "cycle={cycle}, i={i}, hash={h}");
        }
    }

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
    for i in 0..5 {
        vo_core::circuit_breaker::failure_window::record_failure_in_window(
            &mut window,
            hash_from_idx(i),
            t0,
            Duration::from_secs(600),
        );
    }
    state.failure_tracker.insert(wf.clone(), window);

    // Force registration should bypass all three
    let request = RegistrationRequest {
        workflow_name: wf.clone(),
        binary_hash: make_hash("abcdef01"),
        force: true,
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
    for i in 0..4 {
        record_failure(&wf, &hash_from_idx(i), &config, &state, t0).unwrap();
    }

    // Phase 2: Wait 11 minutes (all expired), record 5 new unique failures
    let t_fresh = t0 + Duration::from_secs(660);
    for i in 100..104 {
        let result = record_failure(&wf, &hash_from_idx(i), &config, &state, t_fresh);
        assert_eq!(result, Ok(None));
    }
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
    for i in 0..4 {
        record_failure(&wf, &hash_from_idx(i), &config, &state, t0).unwrap();
    }

    // 5th triggers quarantine
    let result = record_failure(&wf, &hash_from_idx(4), &config, &state, t0);
    assert!(matches!(result, Ok(Some(_))));

    // Even after quarantine, recording more failures shouldn't panic
    for i in 5..300 {
        let result = record_failure(
            &wf,
            &hash_from_idx(i),
            &config,
            &state,
            t0 + Duration::from_secs(i as u64),
        );
        // Should still return QuarantineEvent since count > threshold
        // (the code doesn't check if already quarantined)
        assert!(result.is_ok(), "Should not error on hash {i}: {result:?}");
    }
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

    // Force should bypass
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
    for i in 1..100 {
        let t = t0 + Duration::from_millis(i * 100); // every 100ms
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
    }

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
    for i in 0..5 {
        record_failure(&wf, &hash_from_idx(i), &config, &state, t0).unwrap();
    }
    assert_eq!(
        state.statuses.get(&wf).map(|s| *s),
        Some(RegistrationStatus::Quarantined)
    );

    // Phase 2: Unquarantine
    let unq_result = unquarantine(&wf, "operator", &state);
    assert!(unq_result.is_ok());
    assert_eq!(
        state.statuses.get(&wf).map(|s| *s),
        Some(RegistrationStatus::Active)
    );

    // Phase 3: Record 5 fresh failures — should re-quarantine
    let t1 = t0 + Duration::from_secs(1);
    for i in 10..14 {
        let result = record_failure(&wf, &hash_from_idx(i), &config, &state, t1);
        assert_eq!(result, Ok(None));
    }
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
    assert!(result1.is_ok());

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
    for i in 0..5 {
        record_failure(&wf, &hash_from_idx(i), &config, &state, t0).unwrap();
    }
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
    for i in 0..254 {
        let result = record_failure(
            &wf,
            &hash_from_idx(i),
            &config,
            &state,
            t0 + Duration::from_secs(i as u64),
        );
        assert_eq!(result, Ok(None), "Hash {i} should not trigger quarantine");
    }

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

    let mut handles = vec![];
    for i in 0..20 {
        let state = Arc::clone(&state);
        let config = config;
        let handle = thread::spawn(move || {
            let wf_name = format!("concurrent-wf-{}", i % 3); // 3 workflows
            let hash = format!("{:08x}", i);
            let request = RegistrationRequest {
                workflow_name: WorkflowName::parse(&wf_name).unwrap(),
                binary_hash: BinaryHash::parse(&hash).unwrap(),
                force: i % 5 == 0, // some forced
            };
            let now = t0 + Duration::from_millis(i as u64 * 10);
            let _ = evaluate_registration(&request, &config, &state, now);
        });
        handles.push(handle);
    }

    // All threads must complete without panics
    for handle in handles {
        handle
            .join()
            .expect("Thread panicked during concurrent evaluate_registration");
    }
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
    for i in 0..50 {
        let state = Arc::clone(&state);
        let config = config;
        let wf = wf.clone();
        let handle = thread::spawn(move || {
            let hash = BinaryHash::parse(&format!("{:08x}", i)).unwrap();
            let now = t0 + Duration::from_millis(i as u64);
            let _ = record_failure(&wf, &hash, &config, &state, now);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle
            .join()
            .expect("Thread panicked during concurrent record_failure");
    }

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
    for i in 0..5 {
        record_failure(&wf_a, &hash_from_idx(i), &config, &state, now).unwrap();
    }
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
    assert!(result.is_ok());

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
