#![allow(clippy::needless_for_each)]
//! Property-based integration tests for the circuit breaker.
//!
//! PROP-01: INV-001 — Threshold always triggers quarantine
//! PROP-05: INV-006 — Force always short-circuits
//! PROP-06: INV-008 — Quarantine is monotonic
//! PROP-07: INV-009 — Rate-limited requests never counted

use std::time::{Duration, Instant};

use proptest::prelude::*;

use vo_core::circuit_breaker::{
    evaluate_registration, record_failure, CircuitBreakerConfig, CircuitBreakerState,
    RegistrationOutcome, RegistrationRequest, RegistrationStatus,
};
use vo_types::{BinaryHash, WorkflowName};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn make_wf(s: &str) -> WorkflowName {
    WorkflowName::parse(s).expect("test workflow name should be valid")
}

fn make_hash(s: &str) -> BinaryHash {
    BinaryHash::parse(s).expect("test hash should be valid")
}

fn default_config() -> CircuitBreakerConfig {
    CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 5)
        .expect("default config should be valid")
}

/// Generate a valid 8-char lowercase hex hash from an index.
fn hash_from_idx(i: usize) -> BinaryHash {
    let hex = format!("{i:08x}");
    BinaryHash::parse(&hex).expect("generated hash should be valid")
}

// ── PROP-01: INV-001 — Threshold always triggers quarantine ──────────────────

proptest! {
    #[test]
    fn threshold_unique_failures_always_triggers_quarantine(
        threshold in 1u8..=10,
    ) {
        let config = CircuitBreakerConfig::new(
            Duration::from_secs(60),
            Duration::from_secs(600),
            threshold,
        ).expect("config should be valid");

        let state = CircuitBreakerState::new();
        let now = Instant::now();
        let wf = make_wf("prop-wf-01");

        // Record exactly `threshold` unique hashes
        let mut last_result = None;
        (0..usize::from(threshold)).for_each(|i| {
            let hash = hash_from_idx(i);
            let result = record_failure(&wf, &hash, &config, &state, now);
            last_result = Some(result);
        });

        // The last record_failure call should have returned a QuarantineEvent
        let last = last_result.expect("should have at least one result");
        prop_assert!(
            matches!(last, Ok(Some(_))),
            "Expected Ok(Some(QuarantineEvent)) but got {last:?}"
        );

        // Status should be Quarantined
        let status = state.statuses.get(&wf).map(|s| *s);
        prop_assert_eq!(status, Some(RegistrationStatus::Quarantined));
    }

    // Anti-invariant: threshold-1 unique hashes does NOT trigger quarantine
    #[test]
    fn below_threshold_does_not_trigger_quarantine(
        threshold in 2u8..=10,
    ) {
        let config = CircuitBreakerConfig::new(
            Duration::from_secs(60),
            Duration::from_secs(600),
            threshold,
        ).expect("config should be valid");

        let state = CircuitBreakerState::new();
        let now = Instant::now();
        let wf = make_wf("prop-wf-01a");

        // Record threshold-1 unique hashes
        let all_ok = (0..usize::from(threshold - 1)).into_iter().all(|i| {
            let hash = hash_from_idx(i);
            let result = record_failure(&wf, &hash, &config, &state, now);
            matches!(result, Ok(None))
        });
        prop_assert!(all_ok, "Expected Ok(None) for all threshold-1 unique hashes");

        // Status should remain Active
        let status = state
            .statuses
            .get(&wf)
            .map(|s| *s)
            .unwrap_or(RegistrationStatus::Active);
        prop_assert_eq!(status, RegistrationStatus::Active);
    }
}

// ── PROP-05: INV-006 — Force always short-circuits ───────────────────────────

proptest! {
    #[test]
    fn force_always_returns_allowed(
        status_idx in 0usize..3,
        has_rate_limit in proptest::bool::ANY,
        elapsed_secs in 0u64..=120,
    ) {
        let statuses = [
            RegistrationStatus::Active,
            RegistrationStatus::Deactivated,
            RegistrationStatus::Quarantined,
        ];
        let status = statuses[status_idx];

        let state = CircuitBreakerState::new();
        let config = default_config();
        let t0 = Instant::now();
        let wf = make_wf("prop-wf-05");

        // Set workflow status
        state.statuses.insert(wf.clone(), status);

        // Optionally set rate limiter entry
        if has_rate_limit {
            state.rate_limiter.insert(wf.clone(), t0);
        }

        let now = t0 + Duration::from_secs(elapsed_secs);
        let request = RegistrationRequest {
            workflow_name: wf,
            binary_hash: make_hash("abcdef01"),
            force: true,
        };

        let result = evaluate_registration(&request, &config, &state, now);
        prop_assert_eq!(result, Ok(RegistrationOutcome::Allowed));
    }

    // Anti-invariant: force=false with Quarantined status => NOT Allowed
    #[test]
    fn non_force_quarantined_returns_quarantined(
        elapsed_secs in 0u64..=120,
    ) {
        let state = CircuitBreakerState::new();
        let config = default_config();
        let now = Instant::now() + Duration::from_secs(elapsed_secs);
        let wf = make_wf("prop-wf-05a");

        state
            .statuses
            .insert(wf.clone(), RegistrationStatus::Quarantined);

        let request = RegistrationRequest {
            workflow_name: wf.clone(),
            binary_hash: make_hash("abcdef01"),
            force: false,
        };

        let result = evaluate_registration(&request, &config, &state, now);
        prop_assert_eq!(
            result,
            Ok(RegistrationOutcome::WorkflowQuarantined {
                workflow_name: wf,
            })
        );
    }
}

// ── PROP-06: INV-008 — Quarantine is monotonic ──────────────────────────────

proptest! {
    #[test]
    fn quarantine_is_monotonic_under_random_operations(
        num_ops in 1usize..=20,
        op_types in proptest::collection::vec(0u8..3, 1..=20),
        time_advances in proptest::collection::vec(0u64..=3600, 1..=20),
    ) {
        let state = CircuitBreakerState::new();
        let config = default_config();
        let wf = make_wf("prop-wf-06");
        let t0 = Instant::now();

        // Pre-quarantine the workflow
        state
            .statuses
            .insert(wf.clone(), RegistrationStatus::Quarantined);

        let actual_ops = num_ops.min(op_types.len()).min(time_advances.len());
        let mut current_time = t0;

        (0..actual_ops).into_iter().try_for_each(|i| -> Result<(), proptest::test_runner::TestCaseError> {
            current_time += Duration::from_secs(time_advances[i]);

            match op_types[i] % 3 {
                // record_failure with random hash — result is irrelevant to monotonicity,
                // but we must not discard it. Assert it's not a panic.
                0 => {
                    let hash = hash_from_idx(i);
                    let rf_result = record_failure(&wf, &hash, &config, &state, current_time);
                    prop_assert_eq!(
                        rf_result,
                        Ok(None),
                        "record_failure on already-quarantined workflow should return Ok(None)"
                    );
                }
                // evaluate_registration with force=false
                1 => {
                    let request = RegistrationRequest {
                        workflow_name: wf.clone(),
                        binary_hash: hash_from_idx(i + 100),
                        force: false,
                    };
                    let eval_result = evaluate_registration(&request, &config, &state, current_time);
                    prop_assert_eq!(
                        eval_result,
                        Ok(RegistrationOutcome::WorkflowQuarantined {
                            workflow_name: wf.clone(),
                        }),
                        "evaluate_registration on quarantined workflow should return WorkflowQuarantined"
                    );
                }
                // Just time advancement (no-op)
                _ => {}
            }

            // Status MUST remain Quarantined after every operation
            let status = state.statuses.get(&wf).map(|s| *s);
            prop_assert_eq!(
                status,
                Some(RegistrationStatus::Quarantined),
                "Quarantine should be monotonic, but status changed after op {}",
                i
            );
            Ok(())
        })?;
    }
}

// ── PROP-07: INV-009 — Rate-limited requests never counted ──────────────────

proptest! {
    #[test]
    fn rate_limited_requests_never_change_failure_count(
        initial_failures in 0usize..=4,
    ) {
        let state = CircuitBreakerState::new();
        let config = default_config();
        let t0 = Instant::now();
        let wf = make_wf("prop-wf-07");

        // Set up rate limiter entry 10s ago
        state.rate_limiter.insert(wf.clone(), t0);

        // Pre-load failures — assert each setup call succeeds
        let setup_ok = (0..initial_failures).into_iter().all(|i| {
            let hash = hash_from_idx(i);
            record_failure(&wf, &hash, &config, &state, t0) == Ok(None)
        });
        prop_assert!(setup_ok, "Setup record_failure should succeed with Ok(None)");

        // Get failure count before rate-limited attempt
        let count_before = state
            .failure_tracker
            .get(&wf)
            .map(|t| t.len())
            .unwrap_or(0);

        // Attempt registration within rate limit window (10s later, window is 60s)
        let now = t0 + Duration::from_secs(10);
        let request = RegistrationRequest {
            workflow_name: wf.clone(),
            binary_hash: hash_from_idx(initial_failures + 100), // new hash
            force: false,
        };
        let result = evaluate_registration(&request, &config, &state, now);

        // Should be rate-limited
        prop_assert!(
            matches!(result, Ok(RegistrationOutcome::RateLimited { .. })),
            "Expected RateLimited but got {result:?}"
        );

        // Failure count should be unchanged
        let count_after = state
            .failure_tracker
            .get(&wf)
            .map(|t| t.len())
            .unwrap_or(0);

        prop_assert_eq!(
            count_before, count_after,
            "Failure count changed from {} to {} after rate-limited request",
            count_before, count_after
        );
    }
}
