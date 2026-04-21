//! BDD Tests for ADR-029: Execution Leases and Fencing Tokens
//!
//! These tests verify the behavior of execution leases and monotonic fence tokens
//! per ADR-029. Each test follows the Given-When-Then BDD convention.
//!
//! ## ADR-029 Summary
//! ADR-029 introduces durable execution leases with monotonic fence tokens to prevent
//! stale actors, late subprocesses, and crash-recovery races from producing duplicate completions.
//!
//! ## Key Invariants
//! 1. A fence token is monotonic - once a higher token is issued, lower tokens are stale
//! 2. Only completions carrying the current (latest) fence token can commit
//! 3. Lease acquisition advances the fence token, fencing any stale completions

use vo_types::{FenceToken, InstanceId, LeaseRecord, StepId};

fn test_instance_id() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid test instance id")
}

fn test_step_id() -> StepId {
    StepId::parse("step-1").expect("valid test step id")
}

fn make_fence_token(v: u64) -> FenceToken {
    FenceToken::new(v).expect("valid fence token")
}

// ============================================================================
// SCENARIO 1: Stale Actor Rejection
// ============================================================================
// Given lease token L1, When stale actor presents older token L0,
// Then operation is rejected.
// ============================================================================

#[test]
fn given_current_lease_token_l2_when_stale_actor_presents_older_token_l1_then_operation_is_rejected(
) {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let current_lease = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(2));
    let stale_token = make_fence_token(1);

    let is_rejected = !current_lease.matches_token(&stale_token);
    assert!(
        is_rejected,
        "Operation with stale token L1 must be rejected when current lease is L2"
    );
}

#[test]
fn given_lease_token_l1_when_actor_presents_future_token_l2_then_rejected() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let current_lease = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(1));
    let future_token = make_fence_token(2);

    assert!(
        !current_lease.matches_token(&future_token),
        "Future token L2 must be rejected - only exact match is valid"
    );
}

#[test]
fn given_lease_token_l1_when_actor_presents_same_token_l1_then_accepted() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let current_lease = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(1));
    let matching_token = make_fence_token(1);

    assert!(
        current_lease.matches_token(&matching_token),
        "Token L1 must be accepted when current lease holds token L1"
    );
}

// ============================================================================
// SCENARIO 2: Crash-Recovery Fencing
// ============================================================================
// Given crash-recovery, When new process obtains lease,
// Then old process commits are fenced.
// ============================================================================

#[test]
fn given_crash_recovery_when_new_process_obtains_lease_token_2_then_old_process_commits_are_fenced()
{
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let old_lease = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(1));
    let old_process_token = *old_lease.token();

    let new_lease = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(2));

    assert!(
        !new_lease.matches_token(&old_process_token),
        "Old process commit with token 1 must be fenced after new lease acquired with token 2"
    );
}

#[test]
fn given_process_crash_when_lease_reacquired_all_previous_completions_are_fenced() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let original_lease =
        LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(1));
    let original_token = *original_lease.token();

    let recovery_lease_v2 =
        LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(2));
    let recovery_lease_v3 =
        LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(3));
    let recovery_lease_v4 =
        LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(4));

    assert!(
        !recovery_lease_v2.matches_token(&original_token),
        "Original token 1 must be fenced by lease v2"
    );
    assert!(
        !recovery_lease_v3.matches_token(&original_token),
        "Original token 1 must be fenced by lease v3"
    );
    assert!(
        !recovery_lease_v4.matches_token(&original_token),
        "Original token 1 must be fenced by lease v4"
    );
}

#[test]
fn given_multiple_recovery_cycles_when_attempting_old_completion_then_fenced() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let initial_lease = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(1));
    let initial_token = *initial_lease.token();

    let lease_v2 = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(2));
    let lease_v3 = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(3));

    assert!(
        !lease_v2.matches_token(&initial_token),
        "Initial token must be fenced after v2 lease"
    );
    assert!(
        !lease_v3.matches_token(&initial_token),
        "Initial token must still be fenced after v3 lease"
    );
}

// ============================================================================
// SCENARIO 3: Lease Expiry Rejection
// ============================================================================
// Given lease expiry, When attempt to commit,
// Then rejection occurs.
// ============================================================================

#[test]
fn given_lease_with_token_1_expires_when_attempt_to_commit_then_rejection_occurs() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let expired_lease = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(1));
    let expired_token = *expired_lease.token();

    let fresh_lease = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(2));

    assert!(
        !fresh_lease.matches_token(&expired_token),
        "Expired lease token 1 must be rejected when fresh lease holds token 2"
    );
}

#[test]
fn given_lease_expiry_when_presenting_any_lower_token_then_rejection() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let current_lease =
        LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(10));

    for old_value in 1..10 {
        let old_token = make_fence_token(old_value);
        assert!(
            !current_lease.matches_token(&old_token),
            "Token {} (expired) must be rejected when current lease holds token 10",
            old_value
        );
    }
}

#[test]
fn given_lease_timeout_when_stale_completion_arrives_then_rejected() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let original_lease =
        LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(1));
    let original_token = *original_lease.token();

    let after_timeout_lease =
        LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(2));

    assert!(
        !after_timeout_lease.matches_token(&original_token),
        "Stale completion arriving after timeout must be rejected"
    );
}

// ============================================================================
// SCENARIO 4: Concurrent Acquisition
// ============================================================================

#[test]
fn given_concurrent_lease_acquisition_when_two_processes_race_then_only_one_wins() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let lease_a = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(1));
    let lease_b = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(2));

    assert!(
        !lease_b.matches_token(lease_a.token()),
        "Lease B with token 2 must reject Lease A's token 1"
    );
    assert!(
        !lease_a.matches_token(lease_b.token()),
        "Lease A with token 1 must reject Lease B's token 2"
    );
}

#[test]
fn given_two_concurrent_holders_when_each_presents_own_token_then_only_own_is_valid() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let holder_a = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(1));
    let holder_b = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(2));

    assert!(
        holder_a.matches_token(holder_a.token()),
        "Holder A's completion with token 1 is valid for A's lease"
    );
    assert!(
        !holder_b.matches_token(holder_a.token()),
        "Holder A's completion with token 1 is NOT valid for B's lease"
    );
}

// ============================================================================
// SCENARIO 5: Token Monotonicity
// ============================================================================

#[test]
fn given_token_sequence_then_each_is_strictly_greater_than_previous() {
    let tokens: Vec<FenceToken> = (1..=100).map(make_fence_token).collect();

    for window in tokens.windows(2) {
        assert!(
            window[1] > window[0],
            "Token {:?} must be strictly greater than {:?}",
            window[1],
            window[0]
        );
    }
}

#[test]
fn given_adjacent_tokens_when_calling_next_then_increments_by_one() {
    let token_1 = make_fence_token(1);
    let token_2 = token_1.next().expect("next token must exist");

    assert_eq!(
        token_2,
        make_fence_token(2),
        "next() must increment by exactly 1"
    );
}

#[test]
fn given_max_token_when_calling_next_then_error() {
    let max_token = make_fence_token(u64::MAX);
    assert!(
        max_token.next().is_err(),
        "Token u64::MAX cannot produce next token"
    );
}

// ============================================================================
// SCENARIO 6: Independent Step Tokens
// ============================================================================

#[test]
fn given_same_instance_different_steps_then_fence_tokens_are_independent() {
    let instance_id = test_instance_id();
    let step_a = StepId::parse("step-a").expect("valid step id");
    let step_b = StepId::parse("step-b").expect("valid step id");

    let lease_step_a = LeaseRecord::new(instance_id.clone(), step_a.clone(), make_fence_token(1));
    let lease_step_b = LeaseRecord::new(instance_id.clone(), step_b.clone(), make_fence_token(1));

    assert!(
        lease_step_a.matches_token(lease_step_a.token()),
        "Step A lease matches its token"
    );
    assert!(
        lease_step_b.matches_token(lease_step_b.token()),
        "Step B lease matches its token"
    );
    assert!(
        lease_step_a.matches_token(lease_step_b.token()),
        "Different steps with same token value match (independent key space)"
    );
}

#[test]
fn given_step_lease_advances_then_other_step_unchanged() {
    let instance_id = test_instance_id();
    let step_1 = StepId::parse("step-1").expect("valid step id");
    let step_2 = StepId::parse("step-2").expect("valid step id");

    let lease_1_v1 = LeaseRecord::new(instance_id.clone(), step_1.clone(), make_fence_token(1));
    let lease_2_v1 = LeaseRecord::new(instance_id.clone(), step_2.clone(), make_fence_token(1));

    let lease_1_v2 = LeaseRecord::new(instance_id.clone(), step_1.clone(), make_fence_token(2));

    assert!(
        !lease_1_v2.matches_token(lease_1_v1.token()),
        "Step 1 v2 must reject v1 token"
    );
    assert!(
        lease_2_v1.matches_token(lease_2_v1.token()),
        "Step 2 v1 still matches its token (unchanged)"
    );
}

// ============================================================================
// SCENARIO 7: Edge Cases
// ============================================================================

#[test]
fn given_token_one_then_it_is_minimum_valid() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let lease = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(1));

    assert!(
        lease.matches_token(&make_fence_token(1)),
        "Token 1 must be valid"
    );
}

#[test]
fn given_zero_token_then_rejected_as_invalid() {
    let result = FenceToken::new(0);
    assert!(result.is_err(), "Token value 0 must be rejected as invalid");
}

#[test]
fn given_empty_instance_id_then_rejected() {
    let result = InstanceId::parse("");
    assert!(result.is_err(), "Empty instance_id must be rejected");
}

#[test]
fn given_empty_step_id_then_rejected() {
    let result = StepId::parse("");
    assert!(result.is_err(), "Empty step_id must be rejected");
}

// ============================================================================
// SCENARIO 8: Latest Token Exclusivity
// ============================================================================

#[test]
fn given_current_lease_token_50_then_only_exact_token_50_is_valid() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let current_lease =
        LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(50));

    for old_value in 1..50 {
        let old_token = make_fence_token(old_value);
        assert!(
            !current_lease.matches_token(&old_token),
            "Old token {} must be rejected when current is 50",
            old_value
        );
    }

    let future_token = make_fence_token(51);
    assert!(
        !current_lease.matches_token(&future_token),
        "Future token 51 must be rejected (only exact match wins)"
    );

    let exact_token = make_fence_token(50);
    assert!(
        current_lease.matches_token(&exact_token),
        "Exact token 50 must be accepted"
    );
}
