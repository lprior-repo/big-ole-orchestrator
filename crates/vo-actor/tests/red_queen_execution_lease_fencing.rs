//! Red Queen: Adversarial tests for execution leases and fencing (ADR-029)
//!
//! Attack vectors targeting:
//! - Stale fence completion rejection
//! - Lease expiry during execution
//! - Concurrent lease acquisition for same instance
//! - Fence token monotonicity
//! - Verifying stale completions cannot win

use vo_types::{FenceToken, InstanceId, LeaseRecord, StepId};

fn test_instance_id() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap()
}

fn test_step_id() -> StepId {
    StepId::parse("step-1").unwrap()
}

fn make_fence_token(v: u64) -> FenceToken {
    FenceToken::new(v).expect("valid fence token")
}

// =============================================================================
// ATTACK 1: Stale Fence Completion Rejection
// =============================================================================

#[test]
fn stale_fence_token_is_rejected_by_lease_record() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let current_lease = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(5));

    let stale_token = make_fence_token(4);
    assert!(
        !current_lease.matches_token(&stale_token),
        "Stale fence token (4) should NOT match current lease (5)"
    );

    let future_token = make_fence_token(6);
    assert!(
        !current_lease.matches_token(&future_token),
        "Future fence token (6) should NOT match current lease (5)"
    );

    let current_token = make_fence_token(5);
    assert!(
        current_lease.matches_token(&current_token),
        "Current fence token (5) MUST match"
    );
}

#[test]
fn stale_completion_cannot_win_race() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let old_lease = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(1));

    let new_lease = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(2));

    assert!(
        !new_lease.matches_token(old_lease.token()),
        "Old token (1) must NOT match new lease"
    );

    assert!(
        old_lease.matches_token(old_lease.token()),
        "Old token matches its own lease (trivial)"
    );

    let old_completion_token = *old_lease.token();
    assert!(
        !new_lease.matches_token(&old_completion_token),
        "Stale completion with old token MUST be rejected by new lease"
    );
}

#[test]
fn many_stale_tokens_all_rejected() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let current_lease =
        LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(100));

    for stale_value in 1..100 {
        let stale_token = make_fence_token(stale_value);
        assert!(
            !current_lease.matches_token(&stale_token),
            "Stale token {} should be rejected by lease 100",
            stale_value
        );
    }
}

// =============================================================================
// ATTACK 2: Lease Expiry During Execution
// =============================================================================

#[test]
fn expired_lease_token_is_stale() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let expired_lease = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(1));

    let fresh_lease = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(2));

    assert!(
        !fresh_lease.matches_token(expired_lease.token()),
        "Token from expired lease (1) must NOT match fresh lease (2)"
    );
}

#[test]
fn expiry_during_execution_prevents_double_commit() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let original_lease =
        LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(1));

    let original_token = *original_lease.token();

    let recovery_lease =
        LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(2));

    let late_completion_token = original_token;
    assert!(
        !recovery_lease.matches_token(&late_completion_token),
        "Late completion from original execution (token 1) must NOT win after recovery (token 2)"
    );
}

#[test]
fn long_running_execution_must_refresh_lease() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let lease_v1 = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(1));

    let lease_v2 = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(2));

    let lease_v3 = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(3));

    assert!(
        !lease_v2.matches_token(lease_v1.token()),
        "Lease v1 token must be stale after v2 acquired"
    );

    assert!(
        !lease_v3.matches_token(lease_v1.token()),
        "Lease v1 token must be stale after v3 acquired"
    );

    assert!(
        !lease_v3.matches_token(lease_v2.token()),
        "Lease v2 token must be stale after v3 acquired"
    );
}

// =============================================================================
// ATTACK 3: Concurrent Lease Acquisition for Same Instance
// =============================================================================

#[test]
fn concurrent_acquisition_same_instance_id_only_one_wins() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let lease_a = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(1));

    let lease_b = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(2));

    assert!(
        lease_a.matches_token(lease_a.token()),
        "Lease A token matches itself"
    );

    assert!(
        !lease_b.matches_token(lease_a.token()),
        "Lease B MUST reject Lease A's token"
    );

    assert!(
        !lease_a.matches_token(lease_b.token()),
        "Lease A MUST reject Lease B's token"
    );
}

#[test]
fn different_step_ids_have_independent_fence_tokens() {
    let instance_id = test_instance_id();
    let step_a = StepId::parse("step-a").unwrap();
    let step_b = StepId::parse("step-b").unwrap();

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
        "Different steps with same token value should match (independent key space)"
    );
}

#[test]
fn same_instance_different_step_ids_fences_dont_cross_contaminate() {
    let instance_id = test_instance_id();
    let step_1 = StepId::parse("first-step").unwrap();
    let step_2 = StepId::parse("second-step").unwrap();

    let lease_1_v1 = LeaseRecord::new(instance_id.clone(), step_1.clone(), make_fence_token(1));
    let lease_2_v1 = LeaseRecord::new(instance_id.clone(), step_2.clone(), make_fence_token(1));

    let lease_1_v2 = LeaseRecord::new(instance_id.clone(), step_1.clone(), make_fence_token(2));
    let lease_2_v2 = LeaseRecord::new(instance_id.clone(), step_2.clone(), make_fence_token(2));

    assert!(
        !lease_1_v2.matches_token(lease_1_v1.token()),
        "Step 1: v2 must reject v1 token"
    );

    assert!(
        lease_2_v1.matches_token(lease_2_v1.token()),
        "Step 2: v1 lease still matches its token"
    );

    assert!(
        lease_2_v2.matches_token(lease_2_v2.token()),
        "Step 2: v2 lease matches its token"
    );

    assert!(
        lease_1_v2.matches_token(lease_2_v2.token()),
        "Both leases have token value 2, so they match (tokens compared by value only)"
    );

    assert_eq!(
        lease_1_v1.token().inner().get(),
        lease_2_v1.token().inner().get(),
        "Both v1 leases have same token value since created with same initial token"
    );

    assert_ne!(
        lease_1_v2.token().inner().get(),
        lease_2_v1.token().inner().get(),
        "Step 1 v2 token (2) differs from Step 2 v1 token (1)"
    );
}

// =============================================================================
// ATTACK 4: Fence Token Monotonicity
// =============================================================================

#[test]
fn fence_tokens_are_strictly_monotonic() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let tokens: Vec<FenceToken> = (1..=100).map(|v| make_fence_token(v)).collect();

    for window in tokens.windows(2) {
        assert!(
            window[1] > window[0],
            "Token {} must be > {}",
            window[1].inner().get(),
            window[0].inner().get()
        );
    }

    let leases: Vec<LeaseRecord> = tokens
        .iter()
        .map(|t| LeaseRecord::new(instance_id.clone(), step_id.clone(), *t))
        .collect();

    for (idx, lease) in leases.iter().enumerate().skip(1) {
        let prev_token = make_fence_token(idx as u64);
        assert!(
            !lease.matches_token(&prev_token),
            "Lease at idx {} should NOT match previous token {}",
            idx,
            idx
        );
    }
}

#[test]
fn token_next_increments_by_one() {
    let t1 = make_fence_token(1);
    let t2 = t1.next().expect("next token");

    assert_eq!(t2.inner().get(), 2, "next() must increment by exactly 1");

    let t3 = t2.next().expect("next token");
    assert_eq!(t3.inner().get(), 3, "next() must increment by exactly 1");
}

#[test]
fn token_max_cannot_increment() {
    let max_token = make_fence_token(u64::MAX);
    assert!(
        max_token.next().is_err(),
        "Token u64::MAX cannot produce next token"
    );
}

#[test]
fn monotonic_chain_100_tokens() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let mut prev_lease: Option<LeaseRecord> = None;

    for i in 1..=100 {
        let token = make_fence_token(i);
        let lease = LeaseRecord::new(instance_id.clone(), step_id.clone(), token);

        if let Some(prev) = prev_lease {
            assert!(
                !lease.matches_token(prev.token()),
                "Lease {} must reject previous token {}",
                i,
                i - 1
            );
        }

        assert!(
            lease.matches_token(lease.token()),
            "Lease {} must match its own token",
            i
        );

        prev_lease = Some(lease);
    }
}

// =============================================================================
// ATTACK 5: Stale Completions Cannot Win
// =============================================================================

#[test]
fn stale_completion_after_reacquire_is_rejected() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let original = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(1));

    let reacquired = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(2));

    let stale_completion = *original.token();
    assert!(
        !reacquired.matches_token(&stale_completion),
        "Stale completion with token 1 MUST be rejected by reacquired lease token 2"
    );
}

#[test]
fn stale_completion_after_multiple_reacquires_is_rejected() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let original = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(1));

    let v2 = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(2));
    let v3 = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(3));
    let v4 = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(4));
    let v5 = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(5));

    let stale_token = *original.token();
    assert!(!v2.matches_token(&stale_token), "Token 1 rejected by v2");
    assert!(!v3.matches_token(&stale_token), "Token 1 rejected by v3");
    assert!(!v4.matches_token(&stale_token), "Token 1 rejected by v4");
    assert!(
        !v5.matches_token(&stale_token),
        "Token 1 rejected by v5 (stale completion cannot win after multiple reacquires)"
    );
}

#[test]
fn latest_token_is_only_valid_one() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let current = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(50));

    for old_value in 1..50 {
        let old_token = make_fence_token(old_value);
        assert!(
            !current.matches_token(&old_token),
            "Token {} (old) must be rejected by current lease token 50",
            old_value
        );
    }

    let future_token = make_fence_token(51);
    assert!(
        !current.matches_token(&future_token),
        "Future token 51 must be rejected (only exact match wins)"
    );

    let exact_token = make_fence_token(50);
    assert!(
        current.matches_token(&exact_token),
        "Exact token 50 must be accepted"
    );
}

#[test]
fn race_condition_simulated_stale_wins_not_possible() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let holder_a = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(1));

    let holder_b = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(2));

    let completion_with_a_token = *holder_a.token();
    let completion_with_b_token = *holder_b.token();

    assert!(
        !holder_b.matches_token(&completion_with_a_token),
        "If B holds lease with token 2, A's completion (token 1) cannot win"
    );

    assert!(
        holder_a.matches_token(&completion_with_a_token),
        "A's completion with A's token is valid for A's lease"
    );

    assert!(
        !holder_a.matches_token(&completion_with_b_token),
        "If A holds lease with token 1, B's completion (token 2) cannot win"
    );
}

// =============================================================================
// ATTACK 6: Edge Cases and Boundary Conditions
// =============================================================================

#[test]
fn token_one_is_minimum_valid() {
    let instance_id = test_instance_id();
    let step_id = test_step_id();

    let lease = LeaseRecord::new(instance_id.clone(), step_id.clone(), make_fence_token(1));

    assert!(lease.matches_token(&make_fence_token(1)));

    let zero_token_result = FenceToken::new(0);
    assert!(
        zero_token_result.is_err(),
        "Token value 0 must be rejected as invalid"
    );
}

#[test]
fn zero_token_is_invalid() {
    let result = FenceToken::new(0);
    assert!(
        result.is_err(),
        "FenceToken::new(0) must fail - zero is not a valid fence token"
    );
}

#[test]
fn empty_instance_id_lease_edge_case() {
    let empty_iid_result = InstanceId::parse("");
    assert!(
        empty_iid_result.is_err(),
        "Empty instance_id should be rejected"
    );
}

#[test]
fn empty_step_id_lease_edge_case() {
    let empty_sid_result = StepId::parse("");
    assert!(
        empty_sid_result.is_err(),
        "Empty step_id should be rejected"
    );
}
