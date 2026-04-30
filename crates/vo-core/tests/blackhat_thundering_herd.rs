#![allow(clippy::redundant_pattern_matching)]
//! BLACKHAT: Thundering Herd Attack Tests for Rate Limiting
//!
//! These tests demonstrate the thundering herd vulnerability in the rate limiter.
//! When multiple clients are rate-limited simultaneously, they receive the SAME
//! retry_after value. Without jitter, they all retry at the same moment,
//! overwhelming the system.
//!
//! ATTACK VECTOR: DoS via synchronized retry storms
//! BEAD: ve-oly4d

use std::time::{Duration, Instant};

use vo_core::circuit_breaker::{
    check_rate_limit, evaluate_registration, update_rate_limit, CircuitBreakerConfig,
    CircuitBreakerState, RegistrationOutcome, RegistrationRequest,
};
use vo_types::{BinaryHash, WorkflowName};

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

fn make_request(wf: &str, hash: &str, force: bool) -> RegistrationRequest {
    RegistrationRequest {
        workflow_name: make_wf(wf),
        binary_hash: make_hash(hash),
        force,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// THUNDERING HERD: All rate-limited clients get the SAME retry value
// ═══════════════════════════════════════════════════════════════════════════════

/// BLACKHAT-TH-01: Thundering herd - all concurrent rate-limited clients
/// receive the EXACT SAME retry_after value.
///
/// Attack scenario:
/// 1. 1000 clients hit the rate limiter simultaneously
/// 2. All receive retry_after_secs = 60 (same value)
/// 3. All wait exactly 60 seconds
/// 4. All retry simultaneously → system overwhelmed
///
/// This is a VULNERABILITY because the rate limiter returns a deterministic
/// ceiling value without any jitter.
#[test]
fn blackhat_thundering_herd_all_clients_get_same_retry_value() {
    let config = default_config();
    let state = CircuitBreakerState::new();
    let t0 = Instant::now();
    let wf = make_wf("thundering-wf");

    // Client 1 registers at t0
    let req1 = make_request("thundering-wf", "aaaa0001", false);
    let result1 = evaluate_registration(&req1, &config, &state, t0);
    assert_eq!(result1, Ok(RegistrationOutcome::Allowed));

    // 1000 clients arrive 1ms later - all should be rate limited
    let t1ms_later = t0 + Duration::from_millis(1);
    let mut retry_values = Vec::new();

    for i in 0..1000 {
        let client_id = format!("{:08x}", i);
        let req = make_request("thundering-wf", &client_id, false);
        let result = evaluate_registration(&req, &config, &state, t1ms_later);
        match result {
            Ok(RegistrationOutcome::RateLimited { retry_after_secs }) => {
                retry_values.push(retry_after_secs);
            }
            Ok(RegistrationOutcome::Allowed) => {
                panic!(
                    "Client {i} should be rate limited but got Allowed - rate limit window may be too large"
                );
            }
            other => {
                panic!("Unexpected outcome for client {i}: {other:?}");
            }
        }
    }

    // VULNERABILITY: ALL 1000 clients got the EXACT SAME retry value
    let unique_retry_values: std::collections::HashSet<u64> =
        retry_values.iter().copied().collect();

    assert_eq!(
        unique_retry_values.len(),
        1,
        "BLACKHAT THUNDERING HERD: All 1000 clients received the same retry_after value. \
         This confirms the vulnerability - they will all retry simultaneously after waiting \
         the same duration, overwhelming the system. \
         Retry values seen: {unique_retry_values:?}"
    );

    // The retry value should be ~60 seconds (ceiling of ~59.999s remaining)
    let retry_value = retry_values.first().expect("should have at least one");
    assert!(
        *retry_value >= 59 && *retry_value <= 60,
        "Expected retry value around 60s, got {retry_value}"
    );
}

/// BLACKHAT-TH-02: No jitter in rate limit response
///
/// Demonstrates that check_rate_limit() returns deterministic values
/// without any randomization. This is exploitable because attackers
/// can predict exactly when rate limits will expire.
#[test]
fn blackhat_no_jitter_in_rate_limit_response() {
    let t0 = Instant::now();
    let window = Duration::from_secs(60);

    // Multiple calls at the same instant should return identical results
    let result1 = check_rate_limit(Some(t0), window, t0 + Duration::from_millis(1));
    let result2 = check_rate_limit(Some(t0), window, t0 + Duration::from_millis(1));
    let result3 = check_rate_limit(Some(t0), window, t0 + Duration::from_millis(1));

    assert_eq!(result1, result2, "Results should be identical - no jitter");
    assert_eq!(result2, result3, "Results should be identical - no jitter");

    // This is the vulnerability: all clients get the same value
    assert_eq!(
        result1,
        Some(60),
        "At t0+1ms, retry should be ~60s (ceiling of 59.999s)"
    );
}

/// BLACKHAT-TH-03: Rate limit is per-workflow, not per-client
///
/// Demonstrates that multiple malicious clients can coordinate to
/// flood the same workflow, sharing the rate limit.
#[test]
fn blackhat_rate_limit_is_per_workflow_not_per_client() {
    let config = default_config();
    let state = CircuitBreakerState::new();
    let t0 = Instant::now();
    let wf = make_wf("shared-workflow");

    // Client A registers
    let req_a = make_request("shared-workflow", "aaaa0001", false);
    let result_a = evaluate_registration(&req_a, &config, &state, t0);
    assert_eq!(result_a, Ok(RegistrationOutcome::Allowed));

    // Client B registers (same workflow - should set NEW rate limit)
    let req_b = make_request("shared-workflow", "bbbb0001", false);
    let result_b = evaluate_registration(&req_b, &config, &state, t0);
    // BOTH succeed because rate limit is keyed by (workflow), not (workflow, client)
    assert_eq!(result_b, Ok(RegistrationOutcome::Allowed));

    // Now Client A tries again - it can succeed because Client B's registration
    // reset the rate limit for the shared workflow
    let req_a2 = make_request("shared-workflow", "aaaa0002", false);
    let result_a2 = evaluate_registration(&req_a2, &config, &state, t0 + Duration::from_millis(1));

    // VULNERABILITY: Client A is NOT rate limited because Client B's registration
    // updated the shared workflow's rate limit timestamp.
    //
    // This means:
    // 1. Multiple clients can effectively bypass the rate limit by coordinating
    // 2. Legitimate clients can be pushed out by malicious clients resetting the window
    assert_eq!(
        result_a2,
        Ok(RegistrationOutcome::Allowed),
        "BLACKHAT: Client A is not rate limited because Client B's registration \
         reset the shared workflow's rate limit. This is a vulnerability - \
         malicious clients can coordinate to bypass per-workflow rate limiting."
    );
}

/// BLACKHAT-TH-04: Deterministic retry timing allows prediction
///
/// Shows that retry_after is deterministic and can be predicted by attackers.
#[test]
fn blackhat_deterministic_retry_timing_is_predictable() {
    let t0 = Instant::now();
    let window = Duration::from_secs(60);

    // Calculate what the retry value will be at various future times
    let check = |elapsed_ms: u64| -> Option<u64> {
        check_rate_limit(Some(t0), window, t0 + Duration::from_millis(elapsed_ms))
    };

    // At 0ms: retry = 60s
    assert_eq!(check(0), Some(60));

    // At 100ms: retry = 60s (ceiling of 59.9s)
    assert_eq!(check(100), Some(60));

    // At 1000ms: retry = 59s (ceiling of 59s)
    assert_eq!(check(1000), Some(59));

    // At 59000ms: retry = 1s (ceiling of 1s)
    assert_eq!(check(59000), Some(1));

    // At 60000ms: retry = None (window expired)
    assert_eq!(check(60000), None);

    // An attacker can trivially calculate when to send requests to maximize throughput
    // They just need to wait (60 - retry_value) seconds before retrying.
}

/// BLACKHAT-TH-05: Sliding window expiry creates synchronized retry moment
///
/// Shows that when the rate limit window expires, ALL waiting clients
/// become eligible at the exact same instant.
#[test]
fn blackhat_window_expiry_synchronizes_all_waiting_clients() {
    let config = default_config();
    let state = CircuitBreakerState::new();
    let t0 = Instant::now();
    let wf = make_wf("expiry-test-wf");

    // First registration sets rate limit
    let req1 = make_request("expiry-test-wf", "aaaa0001", false);
    let result1 = evaluate_registration(&req1, &config, &state, t0);
    assert_eq!(result1, Ok(RegistrationOutcome::Allowed));

    // 1000 clients arrive immediately after
    let t_immediate = t0 + Duration::from_millis(1);
    let mut rate_limited_clients = Vec::new();

    for i in 0..1000 {
        let req = make_request("expiry-test-wf", &format!("{i:08x}"), false);
        match evaluate_registration(&req, &config, &state, t_immediate) {
            Ok(RegistrationOutcome::RateLimited { retry_after_secs }) => {
                rate_limited_clients.push(retry_after_secs);
            }
            Ok(RegistrationOutcome::Allowed) => {
                panic!("Should have been rate limited");
            }
            other => panic!("Unexpected: {other:?}"),
        }
    }

    // ALL 1000 clients have the SAME retry value
    assert!(
        rate_limited_clients
            .iter()
            .all(|&v| v == rate_limited_clients[0]),
        "All clients should have same retry value"
    );

    // At exactly 60 seconds later, ALL become eligible simultaneously
    let t_expiry = t0 + Duration::from_secs(60);
    let mut success_count = 0;
    for i in 0..1000 {
        let req = make_request("expiry-test-wf", &format!("{i:08x}"), false);
        match evaluate_registration(&req, &config, &state, t_expiry) {
            Ok(RegistrationOutcome::Allowed) => success_count += 1,
            Ok(RegistrationOutcome::RateLimited { .. }) => {}
            other => panic!("Unexpected: {other:?}"),
        }
    }

    // BLACKHAT VULNERABILITY CONFIRMED: All 1000 clients succeeded at the exact same instant
    assert_eq!(
        success_count, 1000,
        "BLACKHAT THUNDERING HERD: At window expiry, all 1000 rate-limited clients \
         succeeded simultaneously. This is the thundering herd attack vector. \
         Expected: 1000, Got: {success_count}"
    );
}
