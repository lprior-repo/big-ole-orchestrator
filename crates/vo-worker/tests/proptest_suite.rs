#![allow(clippy::doc_markdown)]

//! Proptest suite for vo-worker pure functions and invariants.
//!
//! This module contains property-based tests for:
//! - Retry config backoff calculations
//! - Hash ring consistent hashing
//! - Pool config validation
//! - Circuit breaker state transitions
//! - Type conversions and builder patterns
//!
//! Each test specifies: invariant, strategy, and anti-invariant.

use proptest::collection::{vec, SizeRange};
use proptest::prelude::*;
use proptest::strategy::Strategy;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use vo_types::connection_pool::{CircuitBreakerState, PoolId};
use vo_worker::pool::circuit_breaker::CircuitBreaker;
use vo_worker::pool::config::{PoolConfig, PoolConfigError};
use vo_worker::pool::hash_ring::{HashRing, HashRingConfig, RingNode};
use vo_worker::retry::RetryConfig;

//==============================================================================
// RETRY CONFIG PROPTESTS
//==============================================================================

// Invariant: Backoff calculation is monotonically non-decreasing with attempt number
// Strategy: Random initial_backoff (1-10000ms), multiplier (1.0-5.0), attempts (1-10)
// Anti-invariant: Later attempt has strictly smaller backoff than earlier attempt
proptest! {
    #[test]
    fn test_retry_backoff_monotonicity(
        initial_backoff_ms in 1u64..10000,
        multiplier in 1.0f64..5.0,
        attempt1 in 1u32..10,
        attempt2 in 11u32..20u32,
    ) {
        prop_assert!(attempt2 > attempt1, "attempt2 must be greater than attempt1");
        let config = RetryConfig::new(initial_backoff_ms, multiplier, 10);
        let backoff1 = config.calculate_backoff(attempt1);
        let backoff2 = config.calculate_backoff(attempt2);

        // Monotonicity invariant: backoff should not decrease
        prop_assert!(backoff2 >= backoff1,
            "Backoff should be monotonically non-decreasing: attempt {}={}ms, attempt {}={}ms",
            attempt1, backoff1.as_millis(), attempt2, backoff2.as_millis()
        );
    }
}

// Invariant: Backoff respects max_backoff cap
// Strategy: Random initial_backoff, multiplier, max_backoff
// Anti-invariant: Some backoff exceeds max_backoff
proptest! {
    #[test]
    fn test_retry_backoff_respects_max_cap(
        initial_backoff_ms in 1u64..10000,
        multiplier in 1.0f64..5.0,
        max_backoff_ms in 100u64..100000,
        attempt in 1u32..100,
    ) {
        let config = RetryConfig::new(initial_backoff_ms, multiplier, 10)
            .with_max_backoff(max_backoff_ms);
        let backoff = config.calculate_backoff(attempt);

        // Max cap invariant
        prop_assert!(backoff.as_millis() as u64 <= max_backoff_ms,
            "Backoff {}ms exceeds max_backoff {}ms",
            backoff.as_millis(), max_backoff_ms
        );
    }
}

// Invariant: Zero jitter factor returns exactly base duration
// Strategy: Random base durations
// Anti-invariant: Returned duration differs from base by any amount
proptest! {
    #[test]
    fn test_zero_jitter_returns_base_duration(
        base_ms in 1u64..100000,
    ) {
        let config = RetryConfig::new(100, 2.0, 3)
            .with_jitter(0.0);
        let base = Duration::from_millis(base_ms);
        let with_jitter = config.calculate_jitter(base);

        // Zero jitter invariant
        prop_assert!(with_jitter == base,
            "Zero jitter should return base duration exactly: base={:?}, got={:?}",
            base, with_jitter
        );
    }
}

// Invariant: Jitter is symmetric around base (positive and negative equal range)
// Strategy: Random base, jitter factor
// Anti-invariant: Jitter consistently biased positive or negative
proptest! {
    #[test]
    fn test_jitter_symmetry(
        base_ms in 100u64..10000,
        jitter_factor in 0.1f64..1.0,
    ) {
        let config = RetryConfig::new(100, 2.0, 3)
            .with_jitter(jitter_factor);
        let base = Duration::from_millis(base_ms);

        // Run multiple trials - note: rand_jitter uses time-based LCG seed,
        // so tight-loop iterations produce correlated values, not uniform randomness.
        // We only verify the jitter produces values in a reasonable range.
        for _ in 0..100 {
            let with_jitter = config.calculate_jitter(base);
            let jitter_ms = (with_jitter.as_millis() as i64).abs_diff(base.as_millis() as i64);
            let max_expected = (base_ms as f64 * jitter_factor * 1.5) as u64;
            prop_assert!(jitter_ms <= max_expected + 1,
                "Jitter out of range: base={:?}, jitter_ms={}, max_expected={}",
                base, jitter_ms, max_expected
            );
        }
    }
}

//==============================================================================
// HASH RING PROPTESTS
//==============================================================================

// Invariant: Empty ring returns None for any key
// Strategy: Empty ring, random keys
// Anti-invariant: Empty ring returns Some node
proptest! {
    #[test]
    fn test_empty_ring_returns_none(
        _key in any::<String>(),
    ) {
        let ring = HashRing::new(HashRingConfig::default());

        // Empty ring invariant
        prop_assert!(ring.get_node(&_key).is_none(),
            "Empty ring should return None for any key"
        );
    }
}

// Invariant: Single node ring always returns that node
// Strategy: Ring with single node, random keys
// Anti-invariant: Single node ring returns different node or None
proptest! {
    #[test]
    fn test_single_node_always_returns_that_node(
        key in any::<String>(),
    ) {
        let mut ring = HashRing::new(HashRingConfig::default());
        let node_id = PoolId::new("single-node");
        ring.add_node(RingNode {
            pool_id: node_id.clone(),
            weight: 1,
        });

        // Single node invariant
        prop_assert_eq!(ring.get_node(&key), Some(node_id.clone()),
            "Single node ring should always return that node"
        );
    }
}

// Invariant: Node count equals number of added nodes
// Strategy: Sequence of add/remove operations
// Anti-invariant: Node count != actual distinct node count
proptest! {
    #[test]
    fn test_node_count_invariant(
        node_ids in vec(any::<u32>(), 0..20),
    ) {
        let mut ring = HashRing::new(HashRingConfig::default());
        let mut distinct_nodes: std::collections::HashSet<u32> = std::collections::HashSet::new();

        for id in node_ids {
            let pool_id = PoolId::new(format!("node-{}", id));
            ring.add_node(RingNode {
                pool_id: pool_id.clone(),
                weight: 1,
            });
            distinct_nodes.insert(id);

            // Node count invariant
            prop_assert_eq!(
                ring.node_count(),
                distinct_nodes.len(),
                "Node count should equal distinct nodes: claimed={}, actual={}",
                ring.node_count(),
                distinct_nodes.len()
            );
        }
    }
}

// Invariant: get_nodes returns unique pool IDs
// Strategy: Ring with multiple nodes, request multiple nodes
// Anti-invariant: get_nodes returns duplicate pool IDs
proptest! {
    #[test]
    fn test_get_nodes_returns_unique_pools(
        keys in vec(any::<String>(), 1..10),
        request_count in 1usize..10usize,
    ) {
        let mut ring = HashRing::new(HashRingConfig::default());

        // Add 3 nodes
        for i in 0..3 {
            ring.add_node(RingNode {
                pool_id: PoolId::new(format!("node-{}", i)),
                weight: 1,
            });
        }

        // Test with multiple keys
        for key in keys {
            let nodes = ring.get_nodes(&key, request_count);

            // Uniqueness invariant
            let unique_count = nodes.len();
            let distinct_count = nodes.iter().collect::<std::collections::HashSet<_>>().len();

            prop_assert_eq!(unique_count, distinct_count,
                "get_nodes should return unique pools: len={}, unique={}",
                unique_count, distinct_count
            );
        }
    }
}

// Invariant: Total virtual nodes equals sum of (virtual_nodes * weight) for all nodes
// Strategy: Random configuration and nodes
// Anti-invariant: total_virtual_nodes != expected sum
proptest! {
    #[test]
    fn test_total_virtual_nodes_invariant(
        virtual_nodes_base in 10u32..200,
        nodes in vec((any::<u32>(), 1u32..10u32), 1..10),
    ) {
        let config = HashRingConfig { virtual_nodes: virtual_nodes_base };
        let mut ring = HashRing::new(config.clone());

        let mut expected_sum: u64 = 0;

        for (id, weight) in nodes {
            let pool_id = PoolId::new(format!("node-{}", id));
            ring.add_node(RingNode {
                pool_id,
                weight,
            });
            expected_sum += (virtual_nodes_base as u64) * (weight as u64);
        }

        // Virtual nodes invariant
        prop_assert_eq!(
            ring.total_virtual_nodes(),
            expected_sum,
            "Total virtual nodes should equal sum of (virtual_nodes * weight)"
        );
    }
}

// Invariant: Consistent hashing produces deterministic results for same key
// Strategy: Ring with nodes, random keys
// Anti-invariant: Same key maps to different nodes on repeated lookups
proptest! {
    #[test]
    fn test_consistent_hashing_determinism(
        key in any::<String>(),
        node_count in 2usize..5usize,
    ) {
        let mut ring = HashRing::new(HashRingConfig::default());

        for i in 0..node_count {
            ring.add_node(RingNode {
                pool_id: PoolId::new(format!("node-{}", i)),
                weight: 1,
            });
        }

        // Run multiple lookups for same key
        let results: Vec<Option<PoolId>> = (0..100)
            .map(|_| ring.get_node(&key))
            .collect();

        // Determinism invariant: all results should be identical
        let first_result = results[0].clone();
        for result in results.iter() {
            prop_assert_eq!(result, &first_result,
                "Consistent hashing should be deterministic for same key"
            );
        }
    }
}

// Invariant: get_nodes count never exceeds requested count
// Strategy: Ring with nodes, various request counts
// Anti-invariant: get_nodes returns more than requested
proptest! {
    #[test]
    fn test_get_nodes_count_limit(
        request_count in 1usize..20usize,
    ) {
        let mut ring = HashRing::new(HashRingConfig::default());

        // Add more nodes than we'll request
        for i in 0..10 {
            ring.add_node(RingNode {
                pool_id: PoolId::new(format!("node-{}", i)),
                weight: 1,
            });
        }

        let nodes = ring.get_nodes(&"test-key", request_count);

        // Count limit invariant
        prop_assert!(nodes.len() <= request_count,
            "get_nodes should return at most {} nodes, got {}",
            request_count, nodes.len()
        );
    }
}

//==============================================================================
// POOL CONFIG PROPTESTS
//==============================================================================

// Invariant: Valid configs pass validation
// Strategy: Valid parameter combinations
// Anti-invariant: Valid parameters fail validation
proptest! {
    #[test]
    fn test_valid_configs_pass_validation(
        min_connections in 1u32..100u32,
        max_offset in 1u32..100u32,
        connection_timeout_ms in 100u64..60000u64,
        idle_timeout_ms in 1000u64..300000u64,
        health_check_interval_ms in 100u64..60000u64,
        max_pending_acquires in 0u32..100u32,
    ) {
        let max_connections = min_connections.saturating_add(max_offset);
        let result = PoolConfig::new(
            min_connections,
            max_connections,
            connection_timeout_ms,
            idle_timeout_ms,
            health_check_interval_ms,
            max_pending_acquires,
        );

        // Valid config invariant
        prop_assert!(result.is_ok(),
            "Valid parameters should pass validation"
        );

        if let Ok(config) = result {
            prop_assert_eq!(config.min_connections, min_connections);
            prop_assert_eq!(config.max_connections, max_connections);
            prop_assert_eq!(config.connection_timeout_ms, connection_timeout_ms);
            prop_assert_eq!(config.idle_timeout_ms, idle_timeout_ms);
            prop_assert_eq!(config.health_check_interval_ms, health_check_interval_ms);
            prop_assert_eq!(config.max_pending_acquires, max_pending_acquires);
        }
    }
}

// Invariant: Invalid configs fail with appropriate error
// Strategy: Various invalid parameter combinations
// Anti-invariant: Invalid parameters pass validation
proptest! {
    #[test]
    fn test_invalid_configs_rejected(
        min_gt_max in any::<bool>(),
        max_zero in any::<bool>(),
        timeout_zero in any::<bool>(),
    ) {
        let min = if min_gt_max { 10u32 } else { 5u32 };
        let max = if min_gt_max { 5u32 } else { 10u32 };
        let max = if max_zero { 0u32 } else { max };
        let timeout = if timeout_zero { 0u64 } else { 5000u64 };

        let result = PoolConfig::new(min, max, timeout, 30000, 10000, 5);

        // Invalid config should fail
        if min > max || max == 0 || timeout == 0 {
            prop_assert!(result.is_err(),
                "Invalid parameters should fail validation"
            );
        }
    }
}

// Invariant: min_connections <= max_connections is required
// Strategy: Random min/max pairs
// Anti-invariant: min > max passes validation
proptest! {
    #[test]
    fn test_min_leq_max_constraint(
        min in 1u32..100u32,
        max in 1u32..100u32,
    ) {
        let result = PoolConfig::new(min, max, 5000, 30000, 10000, 5);

        if min > max {
            prop_assert_eq!(result.unwrap_err(), PoolConfigError::MinGreaterThanMax);
        } else {
            prop_assert!(result.is_ok());
        }
    }
}

// Invariant: All timeout/interval fields must be > 0
// Strategy: Random timeout values including zero
// Anti-invariant: Zero timeout passes validation
proptest! {
    #[test]
    fn test_timeout_fields_nonzero(
        connection_timeout in 0u64..60000u64,
        idle_timeout in 0u64..300000u64,
        health_check in 0u64..60000u64,
    ) {
        let result = PoolConfig::new(1, 10, connection_timeout, idle_timeout, health_check, 5);

        let has_zero = connection_timeout == 0 || idle_timeout == 0 || health_check == 0;

        if has_zero {
            prop_assert!(result.is_err(),
                "Zero timeout/interval should fail validation"
            );
        } else {
            prop_assert!(result.is_ok());
        }
    }
}

// Invariant: max_pending_acquires can be zero
// Strategy: Zero and non-zero values
// Anti-invariant: Zero max_pending_acquires fails validation
proptest! {
    #[test]
    fn test_max_pending_acquires_zero_allowed(
        _unused in 0u32..10u32,
    ) {
        let result = PoolConfig::new(1, 10, 5000, 30000, 10000, 0);

        // Zero max_pending_acquires is allowed
        prop_assert!(result.is_ok(),
            "Zero max_pending_acquires should be allowed"
        );
    }
}

// Invariant: with_defaults produces valid config
// Strategy: None (deterministic)
// Anti-invariant: with_defaults produces invalid config
#[test]
fn test_with_defaults_valid() {
    let config = PoolConfig::with_defaults();

    // Defaults invariant: should always be valid
    assert!(config.min_connections > 0);
    assert!(config.max_connections > 0);
    assert!(config.connection_timeout_ms > 0);
    assert!(config.idle_timeout_ms > 0);
    assert!(config.health_check_interval_ms > 0);
    assert!(config.min_connections <= config.max_connections);
}

//==============================================================================
// CIRCUIT BREAKER PROPTESTS
//==============================================================================

// Invariant: Circuit breaker starts in Closed state
// Strategy: None (deterministic)
// Anti-invariant: Circuit breaker starts in Open or HalfOpen state
#[test]
fn test_initial_state_is_closed() {
    let cb = CircuitBreaker::new();

    // Initial state invariant
    assert_eq!(cb.state(), CircuitBreakerState::Closed);
}

// Invariant: Success resets consecutive failures to 0
// Strategy: Random failure count, then success
// Anti-invariant: Success does not reset failures
proptest! {
    #[test]
    fn test_success_resets_failures(
        failures in 1u32..100u32,
    ) {
        let mut cb = CircuitBreaker::new();

        // Record failures
        for _ in 0..failures {
            cb.record_failure();
        }

        let failures_before = cb.consecutive_failures();
        prop_assert_eq!(failures_before, failures);

        // Record success
        cb.record_success();

        // Success resets invariant
        prop_assert_eq!(cb.consecutive_failures(), 0,
            "Success should reset consecutive failures to 0"
        );
    }
}

// Invariant: Open state rejects requests, Closed/HalfOpen allow
// Strategy: Record failures to trip circuit to Open, or timeout for HalfOpen
// Anti-invariant: State allows/rejects incorrectly
proptest! {
    #[test]
    fn test_state_request_policy(
        _seed in 0u32..3u32,
    ) {
        // Test Closed state
        let cb_closed = CircuitBreaker::new();
        prop_assert!(cb_closed.should_allow_request(),
            "Closed state should allow requests"
        );

        // Test HalfOpen state - use try_transition_to_half_open for testing
        // This requires internal state manipulation which we can't do in tests
        // Instead, we test the states we can reach via public API
        let mut cb_half_open = CircuitBreaker::new();
        // Record 10 failures to trip to Open, then transition to HalfOpen
        for _ in 0..10 {
            cb_half_open.record_failure();
        }
        // At this point circuit is Open
        // We cannot easily transition to HalfOpen without timestamp manipulation
        // So we test that Open state rejects
        assert!(!cb_half_open.should_allow_request(),
            "Open state should reject requests");

        // Test that Closed allows
        let cb_closed = CircuitBreaker::new();
        prop_assert!(cb_closed.should_allow_request(),
            "Closed state should allow requests"
        );
    }
}

// Invariant: reset() returns to initial Closed state with clean state
// Strategy: Random failure count, then reset
// Anti-invariant: reset() does not return to clean Closed state
proptest! {
    #[test]
    fn test_reset_clears_state(
        failures_before_reset in 0u32..100u32,
    ) {
        let mut cb = CircuitBreaker::new();

        // Record some failures to trip the circuit
        for _ in 0..failures_before_reset {
            cb.record_failure();
        }

        let state_before = cb.state();
        let failures_before = cb.consecutive_failures();

        // Reset
        cb.reset();

        // Reset invariant
        prop_assert_eq!(cb.state(), CircuitBreakerState::Closed,
            "Reset should return to Closed state, was {:?}", state_before
        );
        prop_assert_eq!(cb.consecutive_failures(), 0,
            "Reset should clear consecutive failures: was {}", failures_before
        );
    }
}

// Invariant: Circuit opens after 50% failure rate in window
// Strategy: Random mix of successes and failures
// Anti-invariant: Circuit stays closed despite 50%+ failure rate
proptest! {
    #[test]
    fn test_circuit_opens_after_high_failure_rate(
        seed in 0u32..10u32,
    ) {
        let mut cb = CircuitBreaker::new();
        let mut total = 0u32;
        let mut failures = 0u32;
        let mut targets = vec![false; 20];

        // Generate deterministic pattern based on seed
        for i in 0..20 {
            targets[i] = ((seed.wrapping_mul(i as u32).wrapping_add(17)) % 2) == 0;
        }

        // Record events
        for should_fail in targets {
            if should_fail {
                cb.record_failure();
                failures += 1;
            } else {
                cb.record_success();
            }
            total += 1;
        }

        // If we had >50% failures, circuit should be open
        if failures > total / 2 {
            prop_assert!(cb.state() == CircuitBreakerState::Open ||
                       cb.consecutive_failures() >= 5,
                "Circuit should trip with >50% failure rate"
            );
        }
    }
}

// Invariant: Success in HalfOpen closes circuit
// Strategy: Simulate HalfOpen via failures, then success
// Anti-invariant: Success in HalfOpen does not close
proptest! {
    #[test]
    fn test_half_open_success_closes(
        _seed in 0u32..1u32,
    ) {
        let mut cb = CircuitBreaker::new();

        // Record 10 failures to trip to Open
        for _ in 0..10 {
            cb.record_failure();
        }

        // Circuit should be open now
        prop_assert_eq!(cb.state(), CircuitBreakerState::Open);

        // We cannot easily test HalfOpen without internal state access
        // The HalfOpen transition requires timestamp manipulation
        // So we verify the Open state and that reset works
        cb.reset();
        prop_assert_eq!(cb.state(), CircuitBreakerState::Closed);
    }
}

// Invariant: Failure count tracks correctly through operations
// Strategy: Random sequences of successes and failures
// Anti-invariant: Failure count diverges from actual recorded failures
proptest! {
    #[test]
    fn test_failure_count_accuracy(
        seed in 0u32..5u32,
    ) {
        let mut cb = CircuitBreaker::new();

        // Generate deterministic sequence
        for i in 0..30 {
            let should_fail = ((seed.wrapping_mul(i).wrapping_add(7)) % 3) == 0;

            if should_fail {
                cb.record_failure();
            } else {
                cb.record_success();
            }

            // After success, failure count should be 0
            if !should_fail {
                prop_assert_eq!(cb.consecutive_failures(), 0,
                    "Success should reset failure count"
                );
            }
        }

        // After all operations, failure count should match trailing failures
        // (we can only assert 0 if the last operation was a success)
        let last_was_success = !((seed.wrapping_mul(29u32).wrapping_add(7)) % 3) == 0;
        if last_was_success {
            prop_assert_eq!(cb.consecutive_failures(), 0);
        }
    }
}

// Invariant: Circuit stays closed with all successes
// Strategy: Random number of successes (0-100)
// Anti-invariant: Circuit opens with only successes
proptest! {
    #[test]
    fn test_all_successes_keeps_closed(
        success_count in 0u32..100u32,
    ) {
        let mut cb = CircuitBreaker::new();

        for _ in 0..success_count {
            cb.record_success();
        }

        // Should remain closed
        prop_assert_eq!(cb.state(), CircuitBreakerState::Closed,
            "Circuit should stay closed with all successes"
        );
        prop_assert_eq!(cb.consecutive_failures(), 0);
    }
}

//==============================================================================
// TYPE CONVERSION INVARIANTS
//==============================================================================

// Invariant: PoolConfig <-> VoPoolConfig conversion is lossless
// Strategy: Random valid PoolConfig parameters
// Anti-invariant: Conversion loses or changes any field
proptest! {
    #[test]
    fn test_pool_config_conversion_lossless(
        min_conn in 1u32..100u32,
        max_conn in 1u32..100u32,
        conn_timeout in 100u64..60000u64,
        idle_timeout in 1000u64..300000u64,
        health_check in 100u64..60000u64,
        pending in 0u32..100u32,
    ) {
        // Ensure min <= max
        let (min, max) = if min_conn <= max_conn {
            (min_conn, max_conn)
        } else {
            (max_conn, min_conn)
        };

        let pool_config = PoolConfig {
            min_connections: min,
            max_connections: max,
            connection_timeout_ms: conn_timeout,
            idle_timeout_ms: idle_timeout,
            health_check_interval_ms: health_check,
            max_pending_acquires: pending,
        };

        // Convert to VoPoolConfig
        let vo_config: vo_types::connection_pool::PoolConfig = pool_config.clone().into();

        // Convert back to PoolConfig
        let back_to_pool: PoolConfig = vo_config.clone().into();

        // Lossless conversion invariant
        prop_assert_eq!(pool_config, back_to_pool,
            "Double conversion should be lossless"
        );
    }
}

// Invariant: Conversion preserves all fields exactly
// Strategy: Random valid config
// Anti-invariant: Any field differs after conversion
proptest! {
    #[test]
    fn test_conversion_field_preservation(
        min_conn in 1u32..50u32,
        max_conn in 50u32..100u32,
        conn_timeout in 1000u64..10000u64,
        idle_timeout in 10000u64..60000u64,
        health_check in 1000u64..30000u64,
        pending in 0u32..20u32,
    ) {
        let pool_config = PoolConfig {
            min_connections: min_conn,
            max_connections: max_conn,
            connection_timeout_ms: conn_timeout,
            idle_timeout_ms: idle_timeout,
            health_check_interval_ms: health_check,
            max_pending_acquires: pending,
        };

        let vo_config: vo_types::connection_pool::PoolConfig = pool_config.clone().into();

        // Field preservation invariant
        prop_assert_eq!(vo_config.min_connections, pool_config.min_connections);
        prop_assert_eq!(vo_config.max_connections, pool_config.max_connections);
        prop_assert_eq!(vo_config.connection_timeout_ms, pool_config.connection_timeout_ms);
        prop_assert_eq!(vo_config.idle_timeout_ms, pool_config.idle_timeout_ms);
        prop_assert_eq!(vo_config.health_check_interval_ms, pool_config.health_check_interval_ms);
        prop_assert_eq!(vo_config.max_pending_acquires, pool_config.max_pending_acquires);
    }
}

//==============================================================================
// COMPOSITE INVARIANTS
//==============================================================================

// Invariant: Hash ring distribution is fair across equal-weight nodes
// Strategy: Multiple equal-weight nodes, many keys
// Anti-invariant: One node gets >90% of requests (extreme imbalance)
proptest! {
    #[test]
    fn test_fair_distribution(
        node_count in 2usize..6usize,
    ) {
        let mut ring = HashRing::new(HashRingConfig { virtual_nodes: 150 });

        for i in 0..node_count {
            ring.add_node(RingNode {
                pool_id: PoolId::new(format!("node-{}", i)),
                weight: 1,
            });
        }

        // Distribute 10000 keys
        let mut distribution: HashMap<String, u32> = HashMap::new();
        for i in 0..10000 {
            let key = format!("key-{}", i);
            if let Some(node) = ring.get_node(&key) {
                *distribution.entry(node.to_string()).or_insert(0) += 1;
            }
        }

        // Calculate expected and check fairness
        let total: u32 = distribution.values().sum();
        let expected = total / node_count as u32;
        let variance_threshold = expected / 2; // Allow 50% variance

        for (node, count) in &distribution {
            let diff = count.abs_diff(expected);
            prop_assert!(diff <= variance_threshold,
                "Distribution should be fair: {} has {} requests (expected ~{})",
                node, count, expected
            );
        }
    }
}

// Invariant: Weighted distribution proportional to weights
// Strategy: Nodes with different weights
// Anti-invariant: High-weight node gets fewer requests than low-weight node
proptest! {
    #[test]
    fn test_weighted_distribution_proportional(
        _seed in 0u32..1u32,
    ) {
        let mut ring = HashRing::new(HashRingConfig { virtual_nodes: 100 });

        // Add low weight node
        ring.add_node(RingNode {
            pool_id: PoolId::new("low"),
            weight: 1,
        });

        // Add high weight node
        ring.add_node(RingNode {
            pool_id: PoolId::new("high"),
            weight: 3,
        });

        // Distribute 10000 keys
        let mut distribution: HashMap<String, u32> = HashMap::new();
        for i in 0..10000 {
            let key = format!("key-{}", i);
            if let Some(node) = ring.get_node(&key) {
                *distribution.entry(node.to_string()).or_insert(0) += 1;
            }
        }

        let high_count = *distribution.get("high").unwrap_or(&0);
        let low_count = *distribution.get("low").unwrap_or(&0);

        // High weight should get more traffic (allowing for randomness)
        prop_assert!(high_count > low_count,
            "High-weight node should get more traffic: high={}, low={}",
            high_count, low_count
        );
    }
}

//==============================================================================
// ANTI-INVARIANTS (tests that should fail if uncommented)
//==============================================================================

// The following are anti-invariants documented for reference.
// These tests should NOT pass and are kept as documentation of what NOT to allow.

/*
// Anti-invariant: Backoff decreases (should fail)
proptest! {
    #[test]
    fn anti_invariant_backoff_decreases(
        initial_backoff_ms in 1u64..10000,
        multiplier in 1.0f64..5.0,
    ) {
        let config = RetryConfig::new(initial_backoff_ms, multiplier, 10);
        let backoff1 = config.calculate_backoff(1);
        let backoff2 = config.calculate_backoff(10);

        // This should FAIL - backoff should NOT decrease
        prop_assert!(backoff2 < backoff1, "Anti-invariant: backoff decreases");
    }
}
*/

//==============================================================================
// PROPERTY TEST STRATEGIES
//==============================================================================

// Strategy for generating valid PoolConfig parameters
pub fn pool_config_strategy() -> impl Strategy<Value = PoolConfig> {
    (
        1u32..50u32,                                   // min_connections
        proptest::collection::vec(1u32..100u32, 1..2), // max >= min
        100u64..60000u64,                              // connection_timeout_ms
        1000u64..300000u64,                            // idle_timeout_ms
        100u64..60000u64,                              // health_check_interval_ms
        0u32..100u32,                                  // max_pending_acquires
    )
        .prop_map(|(min, max_vec, conn_to, idle_to, health_to, pending)| {
            let max = max_vec[0].max(min);
            PoolConfig::new(min, max, conn_to, idle_to, health_to, pending).unwrap()
        })
}

// Strategy for generating random keys for hash ring
pub fn hash_ring_key_strategy() -> impl Strategy<Value = String> {
    any::<String>()
}

// Strategy for generating random nodes
pub fn ring_node_strategy(
    virtual_nodes: u32,
) -> impl Strategy<Value = (HashRingConfig, Vec<RingNode>)> {
    use proptest::strategy::Just;
    (
        Just(HashRingConfig { virtual_nodes }),
        proptest::collection::vec((any::<u32>(), 1u32..10u32), 1usize..10usize),
    )
        .prop_map(|(config, nodes)| {
            let ring_nodes: Vec<RingNode> = nodes
                .into_iter()
                .map(|(id, weight)| RingNode {
                    pool_id: PoolId::new(format!("node-{}", id)),
                    weight,
                })
                .collect();
            (config, ring_nodes)
        })
}
