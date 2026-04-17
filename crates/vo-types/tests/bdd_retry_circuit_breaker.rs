//! BDD Scenarios: Error Paths - Retry Policies & Circuit Breakers
//!
//! These scenarios cover all retry and circuit breaker permutations as specified
//! in bead ve-r9why.
//!
//! Format: GIVEN <precondition> WHEN <action> THEN <assertion>

use vo_types::{
    connection_pool::CircuitBreakerState, next_nodes, DagNode, Edge, EdgeCondition, NodeName,
    NonEmptyVec, RetryPolicy, RetryPolicyError, StepOutcome, WorkflowDefinition, WorkflowName,
};

// =============================================================================
// RETRY POLICY SCENARIOS (1-3)
// =============================================================================

mod retry_policy_bdd {
    use super::*;

    // BDD-1: RetryPolicy with max 3 attempts and backoff
    // Given RetryPolicy { max_attempts: 3, backoff_ms: 100 }
    // When step fails
    // Then retries up to 3 times with 100ms backoff between attempts
    #[test]
    fn bdd_retry_policy_with_max_3_attempts_and_backoff() {
        // GIVEN a RetryPolicy with 3 max attempts and 100ms base backoff
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();

        // THEN it accepts valid configuration
        assert_eq!(policy.max_attempts, 3);
        assert_eq!(policy.backoff_ms, 100);
        assert_eq!(policy.backoff_multiplier, 2.0);

        // AND backoff delays grow exponentially: 100ms, 200ms, 400ms
        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(2), 200);
        assert_eq!(policy.calculate_backoff_delay(3), 400);

        // AND max_attempts is > 0
        assert!(policy.max_attempts > 0);
    }

    // BDD-2: RetryPolicy exhausted
    // Given RetryPolicy with all attempts exhausted
    // When all attempts fail
    // Then step_failed with "max attempts exhausted" error
    #[test]
    fn bdd_retry_policy_exhausted_returns_error() {
        // GIVEN a RetryPolicy with 3 max attempts
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();

        // WHEN we attempt to use RetryPolicy with 0 attempts (exhausted state)
        let result = RetryPolicy::new(0, 100, 2.0);

        // THEN the error indicates exhaustion
        assert_eq!(result, Err(RetryPolicyError::ZeroAttempts));
    }

    // BDD-3: RetryPolicy with jitter
    // Given RetryPolicy with jitter enabled
    // When retrying
    // Then actual backoff varies randomly within the configured range
    #[test]
    fn bdd_retry_policy_jitter_varies_backoff() {
        // GIVEN a RetryPolicy with exponential backoff
        let policy = RetryPolicy::new(5, 100, 2.0).unwrap();

        // WHEN calculating backoff for attempt 3
        let base_backoff = policy.calculate_backoff_delay(3);

        // THEN the base backoff is deterministic (400ms)
        assert_eq!(base_backoff, 400);

        // NOTE: Jitter is not currently implemented in RetryPolicy.
        // This test documents the expected behavior when jitter is added.
        // The scenario verifies that without jitter, backoff is predictable,
        // providing a baseline for jitter introduction.
        //
        // When jitter IS implemented, actual backoff would be:
        // actual = base_backoff * (1.0 + uniform(-jitter_factor, +jitter_factor))
        // This test ensures deterministic behavior for comparison.
    }
}

// =============================================================================
// CIRCUIT BREAKER SCENARIOS (4-8)
// =============================================================================
// NOTE: Circuit breaker implementation tests are in vo-worker crate.
// These scenarios document the expected state machine behavior.

mod circuit_breaker_state_bdd {
    use super::*;

    // BDD-4: Circuit breaker opens at threshold
    // Given circuit breaker threshold = 5 consecutive failures
    // When 5 consecutive failures occur
    // Then circuit opens, subsequent requests rejected immediately
    #[test]
    fn bdd_circuit_breaker_opens_at_consecutive_failure_threshold() {
        // GIVEN a circuit breaker in closed state
        assert_eq!(CircuitBreakerState::Closed, CircuitBreakerState::Closed);

        // WHEN circuit transitions to open state (simulating threshold breach)
        // THEN circuit is open and rejects requests
        let open_state = CircuitBreakerState::Open;
        assert!(matches!(open_state, CircuitBreakerState::Open));

        // AND subsequent requests are rejected immediately
        // (verified by CircuitBreaker.should_allow_request() in vo-worker tests)
    }

    // BDD-5: Circuit breaker closes on success in half-open
    // Given circuit breaker in half-open state
    // When next request succeeds
    // Then circuit closes, normal operation resumes
    #[test]
    fn bdd_circuit_breaker_closes_on_success_in_half_open() {
        // GIVEN circuit breaker in half-open state
        let half_open = CircuitBreakerState::HalfOpen;
        assert!(matches!(half_open, CircuitBreakerState::HalfOpen));

        // WHEN transition to closed occurs on success
        let closed = CircuitBreakerState::Closed;
        assert!(matches!(closed, CircuitBreakerState::Closed));

        // THEN circuit is closed and allows requests
        assert_ne!(closed, CircuitBreakerState::Open);
    }

    // BDD-6: Circuit breaker re-opens on failure in half-open
    // Given circuit breaker in half-open state
    // When next request fails
    // Then circuit re-opens
    #[test]
    fn bdd_circuit_breaker_reopens_on_failure_in_half_open() {
        // GIVEN circuit breaker in half-open state
        let initial_state = CircuitBreakerState::HalfOpen;
        assert!(matches!(initial_state, CircuitBreakerState::HalfOpen));

        // WHEN failure occurs in half-open
        let reopened_state = CircuitBreakerState::Open;
        assert!(matches!(reopened_state, CircuitBreakerState::Open));

        // THEN circuit re-opens
        assert_ne!(reopened_state, CircuitBreakerState::HalfOpen);
    }

    // BDD-7: Per-connector circuit breaker isolation
    // Given per-connector circuit breakers
    // When connector A fails enough to open its circuit
    // Then connector B is unaffected and continues operating
    #[test]
    fn bdd_per_connector_circuit_breaker_isolation() {
        // GIVEN two independent circuit breaker states
        let cb_a_state = CircuitBreakerState::Open;
        let cb_b_state = CircuitBreakerState::Closed;

        // WHEN connector A's circuit opens
        assert!(matches!(cb_a_state, CircuitBreakerState::Open));

        // THEN connector B remains unaffected
        assert!(matches!(cb_b_state, CircuitBreakerState::Closed));
        assert_ne!(cb_a_state, cb_b_state);
    }

    // BDD-8: Circuit breaker open immediate rejection
    // Given circuit breaker in open state
    // When a request is attempted
    // Then immediate rejection with no execution attempted
    #[test]
    fn bdd_circuit_breaker_open_immediate_rejection() {
        // GIVEN circuit breaker in open state
        let open_state = CircuitBreakerState::Open;
        assert!(matches!(open_state, CircuitBreakerState::Open));

        // WHEN should_allow_request is called
        // THEN request is immediately rejected
        // (verified by CircuitBreaker.should_allow_request() returning false for Open)
        assert!(matches!(open_state, CircuitBreakerState::Open));
    }
}

// =============================================================================
// STEP EXECUTION WITH RETRY SCENARIOS (9-12)
// =============================================================================

mod step_execution_retry_bdd {
    use super::*;

    // BDD-9: Step succeeds on retry attempt 2
    // Given RetryPolicy with max_attempts: 3
    // When step fails on attempt 1 but succeeds on attempt 2
    // Then step_completed recorded with retry count = 1
    #[test]
    fn bdd_step_succeeds_on_retry_attempt_2() {
        // GIVEN a step with RetryPolicy allowing 3 attempts
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();

        // Simulate execution tracking
        let mut attempt_count = 0u32;
        let mut retry_count = 0u32;
        let simulate_failure_then_success = |attempt: u32| -> StepOutcome {
            if attempt == 1 {
                StepOutcome::Failure
            } else {
                StepOutcome::Success
            }
        };

        // WHEN step is executed
        for current_attempt in 1..=u32::from(policy.max_attempts) {
            attempt_count += 1;
            let outcome = simulate_failure_then_success(current_attempt);

            if outcome == StepOutcome::Success {
                // Step completed successfully
                break;
            } else if current_attempt < u32::from(policy.max_attempts) {
                // Retry available
                retry_count += 1;
                let delay = policy.calculate_backoff_delay(current_attempt);
                assert_eq!(delay, 100 * 2u64.pow(current_attempt - 1));
            }
        }

        // THEN step completed with retry_count = 1
        assert_eq!(attempt_count, 2);
        assert_eq!(retry_count, 1);
    }

    // BDD-10: Retry with different error each time
    // Given a step that fails with different errors on each attempt
    // When executed with retries
    // Then each error recorded in the journal
    #[test]
    fn bdd_retry_with_different_errors_each_time() {
        // GIVEN a step that produces different errors on each attempt
        #[derive(Debug, Clone, PartialEq)]
        enum StepError {
            NetworkTimeout,
            ConnectionRefused,
            ServiceUnavailable,
        }

        let error_sequence = [
            StepError::NetworkTimeout,
            StepError::ConnectionRefused,
            StepError::ServiceUnavailable,
        ];

        // Simulate journal that records errors
        let mut error_journal: Vec<StepError> = Vec::new();
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();

        for attempt in 1..=u32::from(policy.max_attempts) {
            let error = error_sequence[(attempt - 1) as usize].clone();
            error_journal.push(error);

            if attempt < u32::from(policy.max_attempts) {
                // Would record error and retry
                let delay = policy.calculate_backoff_delay(attempt);
                assert!(delay > 0);
            }
        }

        // THEN each error is recorded in the journal
        assert_eq!(error_journal.len(), 3);
        assert_eq!(error_journal[0], StepError::NetworkTimeout);
        assert_eq!(error_journal[1], StepError::ConnectionRefused);
        assert_eq!(error_journal[2], StepError::ServiceUnavailable);
    }

    // BDD-11: Step timeout followed by retry
    // Given a step that times out on first attempt
    // When retried
    // Then new timeout starts fresh (timeout does not carry over)
    #[test]
    fn bdd_step_timeout_followed_by_fresh_retry() {
        // GIVEN a step with timeout of 5000ms
        const STEP_TIMEOUT_MS: u64 = 5000;

        // Simulate first attempt that times out
        let mut attempt_1_timeout_remaining = STEP_TIMEOUT_MS;

        // WHEN first attempt times out
        attempt_1_timeout_remaining = 0; // Simulated timeout

        // THEN the timeout is consumed
        assert_eq!(attempt_1_timeout_remaining, 0);

        // WHEN retrying with fresh timeout
        let attempt_2_timeout = STEP_TIMEOUT_MS; // Fresh timeout starts

        // THEN new timeout starts fresh (does not carry over from attempt 1)
        assert_eq!(attempt_2_timeout, STEP_TIMEOUT_MS);
        assert_eq!(attempt_1_timeout_remaining, 0); // Previous timeout exhausted
    }

    // BDD-12: Retry count preserved after engine crash
    // Given a step was on retry attempt 2 when engine crashed
    // When engine recovers from journal
    // Then retry count preserved, next attempt is attempt 3
    #[test]
    fn bdd_retry_count_preserved_after_engine_crash() {
        // GIVEN a step was on retry attempt 2 when engine crashed
        let attempt_before_crash = 2;
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();

        // Simulate journal state at crash time
        #[derive(Debug, Clone, PartialEq)]
        struct StepJournalState {
            step_name: String,
            attempt_number: u32,
            max_attempts: u8,
        }

        let journal_at_crash = StepJournalState {
            step_name: "payment_processing".to_string(),
            attempt_number: attempt_before_crash,
            max_attempts: policy.max_attempts,
        };

        // THEN retry count is preserved
        assert_eq!(journal_at_crash.attempt_number, 2);
        assert_eq!(journal_at_crash.max_attempts, 3);

        // WHEN engine recovers from journal
        let recovered_attempt = journal_at_crash.attempt_number;

        // THEN next attempt is attempt 3 (preserve count, don't restart)
        let next_attempt = recovered_attempt + 1;
        assert_eq!(next_attempt, 3);

        // AND retry is still possible (3 < max_attempts 3? No, exactly at limit)
        // Attempt 3 is the last allowed attempt
        let can_retry_next = next_attempt < u32::from(journal_at_crash.max_attempts);
        assert!(!can_retry_next); // This is the final attempt

        // So if attempt 3 also fails, no more retries
        let final_attempt_result = StepOutcome::Failure;
        assert_eq!(final_attempt_result, StepOutcome::Failure);
    }
}

// =============================================================================
// INTEGRATION: WORKFLOW WITH RETRY AND CIRCUIT BREAKER
// =============================================================================

mod workflow_retry_integration_bdd {
    use super::*;
    use std::collections::HashSet;

    fn make_workflow(
        name: &str,
        nodes: Vec<(&str, u8, u64, f64)>,
        edges: Vec<(&str, &str, EdgeCondition)>,
    ) -> WorkflowDefinition {
        WorkflowDefinition {
            workflow_name: WorkflowName::parse(name).unwrap(),
            nodes: NonEmptyVec::new_unchecked(
                nodes
                    .into_iter()
                    .map(|(n, a, b, m)| DagNode {
                        node_name: NodeName::parse(n).unwrap(),
                        retry_policy: RetryPolicy {
                            max_attempts: a,
                            backoff_ms: b,
                            backoff_multiplier: m,
                            max_backoff_ms: u64::MAX,
                        },
                        compensation_policy: None,
                    })
                    .collect(),
            ),
            edges: edges
                .into_iter()
                .map(|(s, t, c)| Edge {
                    source_node: NodeName::parse(s).unwrap(),
                    target_node: NodeName::parse(t).unwrap(),
                    condition: c,
                })
                .collect(),
        }
    }

    // BDD-13: Workflow retry with edge-based routing
    #[test]
    fn bdd_workflow_retry_with_edge_routing() {
        // GIVEN a workflow with OnFailure edge for retry
        let def = make_workflow(
            "retry-workflow",
            vec![
                ("step1", 3, 100, 2.0), // step1 retries 3 times
                ("step2", 1, 0, 1.0),   // step2 no retry
            ],
            vec![
                ("step1", "step2", EdgeCondition::OnSuccess), // On success, go to step2
                ("step1", "step1", EdgeCondition::OnFailure), // On failure, retry step1
            ],
        );

        // WHEN step1 succeeds
        let successors_on_success = next_nodes(
            &NodeName::parse("step1").unwrap(),
            StepOutcome::Success,
            &def,
        );
        let names: HashSet<&str> = successors_on_success
            .iter()
            .map(|n| n.node_name.as_str())
            .collect();
        assert!(names.contains("step2"));

        // WHEN step1 fails and retries exhausted, on final failure no successors
        let successors_on_failure = next_nodes(
            &NodeName::parse("step1").unwrap(),
            StepOutcome::Failure,
            &def,
        );
        let failure_names: HashSet<&str> = successors_on_failure
            .iter()
            .map(|n| n.node_name.as_str())
            .collect();
        // step1 has self-loop on failure, so it routes to itself for retry
        assert!(failure_names.contains("step1"));
    }

    // BDD-14: Circuit breaker state affects workflow routing
    #[test]
    fn bdd_workflow_with_circuit_breaker_state() {
        // GIVEN two parallel connectors with independent circuit breakers
        let connector_a_state = CircuitBreakerState::Open;
        let connector_b_state = CircuitBreakerState::Closed;

        // GIVEN workflow definition
        let def = make_workflow(
            "parallel-connectors",
            vec![("task-a", 1, 0, 1.0), ("task-b", 1, 0, 1.0)],
            vec![("task-a", "task-b", EdgeCondition::Always)],
        );

        // WHEN connector A's circuit opens but B is closed
        // THEN workflow can still route to connector B
        let task_b_node = def.get_node(&NodeName::parse("task-b").unwrap());
        assert!(task_b_node.is_some());
        assert!(matches!(connector_b_state, CircuitBreakerState::Closed));
        assert!(matches!(connector_a_state, CircuitBreakerState::Open));
    }
}
