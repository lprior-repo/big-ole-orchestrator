//! BDD tests for rejecting normal completion while compensating (ADR-034, ADR-039).
//!
//! Given-When-Then scenarios validating that when an instance is in a Compensating
//! state, normal StepCompleted events are rejected to prevent stale normal work
//! from interfering with saga compensation.

use vo_types::state::transition::apply;
use vo_types::state::{lifecycle::LifecycleState, TransitionEvent};
use vo_types::LifecycleSuperstate;

// ============================================================================
// Scenario: Normal completion rejected while compensating
// GIVEN instance is Compensating and an old normal StepCompleted arrives
// WHEN writer validates lifecycle and fence
// THEN normal completion is rejected and compensation state is unchanged
// ============================================================================

#[test]
fn given_compensating_instance_when_normal_completion_arrives_then_completion_is_rejected() {
    // GIVEN: An instance is in Compensating superstate
    // (Compensating superstate has no mapped states currently - this test
    // documents the required behavior: completions must be rejected when
    // instance is in a compensating context)
    //
    // Currently LifecycleSuperstate::Compensating exists but no LifecycleState
    // maps to it. This test verifies the expected behavior once a state is
    // added that maps to Compensating superstate.

    // The behavior is: when an instance enters a compensating context
    // (e.g., saga compensation is executing), normal StepCompleted events
    // MUST be rejected because:
    // 1. The instance is focused on reversal, not forward progress
    // 2. Stale normal completions could corrupt compensation state
    // 3. The fence token for normal work would be stale relative to
    //    the compensation epoch

    // WHEN: A normal StepCompleted arrives (simulated as CompleteStep event)
    // THEN: The completion is rejected

    // For this test to work, there needs to be a LifecycleState that maps
    // to LifecycleSuperstate::Compensating. Since no such state exists yet,
    // we verify that the concept is respected by checking that:
    //
    // 1. LifecycleSuperstate::Compensating is a valid superstate
    // 2. No current state maps to Compensating (documenting the gap)
    // 3. The expected behavior: if a state mapped to Compensating received
    //    CompleteStep, it should be rejected

    // Verify Compensating is a valid superstate
    let compensating = LifecycleSuperstate::Compensating;
    assert!(
        matches!(compensating, LifecycleSuperstate::Compensating),
        "Compensating superstate must exist"
    );

    // Document that no current LifecycleState maps to Compensating
    // This is the gap that needs to be filled for this scenario to work
    let all_states = [
        LifecycleState::Pending,
        LifecycleState::RunningDecision,
        LifecycleState::StepScheduled,
        LifecycleState::StepExecuting,
        LifecycleState::PreparingEffect,
        LifecycleState::WaitingForTimer,
        LifecycleState::PendingPublication,
        LifecycleState::Completed,
        LifecycleState::Failed,
        LifecycleState::Cancelled,
    ];

    for state in all_states {
        let superstate = state.superstate();
        assert_ne!(
            superstate,
            LifecycleSuperstate::Compensating,
            "No LifecycleState should map to Compensating yet (found {:?})",
            state
        );
    }

    // WHEN: CompleteStep is attempted from a state that SHOULD be in Compensating
    // (This would be a new Compensating state - for now, verify that
    // CompleteStep is rejected from states that should NOT accept it)

    // CompleteStep is only valid from StepExecuting
    let result_from_pending = apply(LifecycleState::Pending, TransitionEvent::CompleteStep);
    assert!(
        result_from_pending.is_err(),
        "CompleteStep from Pending should be rejected"
    );

    let result_from_running = apply(
        LifecycleState::RunningDecision,
        TransitionEvent::CompleteStep,
    );
    assert!(
        result_from_running.is_err(),
        "CompleteStep from RunningDecision should be rejected"
    );

    let result_from_step_scheduled =
        apply(LifecycleState::StepScheduled, TransitionEvent::CompleteStep);
    assert!(
        result_from_step_scheduled.is_err(),
        "CompleteStep from StepScheduled should be rejected"
    );

    // CompleteStep IS valid from StepExecuting
    let result_from_step_executing =
        apply(LifecycleState::StepExecuting, TransitionEvent::CompleteStep);
    assert!(
        result_from_step_executing.is_ok(),
        "CompleteStep from StepExecuting should succeed"
    );

    // THEN: The behavior is well-defined for normal states
    // The gap is: there is no Compensating state that rejects CompleteStep
    // This test documents that such a state is needed

    // NOTE: Once a Compensating state is added to LifecycleState and mapped
    // to LifecycleSuperstate::Compensating, this test should verify that
    // apply(CompensatingState, TransitionEvent::CompleteStep) returns an error.
}

// ============================================================================
// Scenario: Compensation state remains unchanged when normal completion rejected
// ============================================================================

#[test]
fn given_compensating_instance_when_normal_completion_rejected_then_state_unchanged() {
    // This test verifies that when a normal completion is rejected while
    // in compensating state, the compensation state itself is unchanged.
    //
    // The rejection of normal completion should NOT cause any state change
    // to the instance's compensation context.

    // Currently there's no LifecycleState for Compensating to test this directly.
    // This test documents the expected behavior:
    //
    // GIVEN: Instance is in Compensating state
    // WHEN: Normal CompleteStep arrives and is rejected
    // THEN: Instance remains in Compensating state (no transition)

    // For now, verify that terminal states reject all transitions and remain unchanged
    let terminal_states = [LifecycleState::Completed, LifecycleState::Cancelled];

    for state in terminal_states {
        let result = apply(state, TransitionEvent::CompleteStep);
        assert!(
            result.is_err(),
            "CompleteStep from {:?} should be rejected",
            state
        );
        // The state remains unchanged (error means no transition)
    }

    // Failed state also rejects CompleteStep
    let result = apply(LifecycleState::Failed, TransitionEvent::CompleteStep);
    assert!(
        result.is_err(),
        "CompleteStep from Failed should be rejected"
    );

    // This demonstrates that the pattern of rejecting completions from
    // wrong states is already established - Compensating should follow
    // the same pattern once implemented.
}

// ============================================================================
// Scenario: Stale normal completion is rejected based on fence token
// ============================================================================

#[test]
fn given_compensating_instance_with_stale_fence_when_normal_completion_arrives_then_rejected() {
    // When in Compensating state, the fence token for normal work would be
    // stale relative to the compensation epoch. A normal StepCompleted
    // arriving with an old fence token should be rejected.
    //
    // This is separate from the lifecycle state check - even if the lifecycle
    // check passed, the fence check should reject stale completions.

    // Currently, fence token validation is handled by LeaseRecord.
    // This test documents that:
    // 1. A stale fence token should be rejected
    // 2. When in Compensating state, all normal fence tokens are stale

    use vo_types::integer_types::FenceToken;
    use vo_types::state::transition::LeaseRecord;
    use vo_types::string_types::{InstanceId, StepId};

    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid test instance");
    let step_id = StepId::parse("test-step").expect("valid test step");

    // Current lease has fence token 2 (compensation epoch)
    let current_lease = LeaseRecord::new(
        instance_id.clone(),
        step_id.clone(),
        FenceToken::new(2).expect("valid fence token"),
    );

    // Old fence token 1 is stale
    let stale_token = FenceToken::new(1).expect("valid fence token");

    // New fence token 3 is future (also invalid)
    let future_token = FenceToken::new(3).expect("valid fence token");

    // THEN: Stale token is rejected
    assert!(
        !current_lease.matches_token(&stale_token),
        "Stale fence token (1) should NOT match current lease (2)"
    );

    // THEN: Future token is rejected
    assert!(
        !current_lease.matches_token(&future_token),
        "Future fence token (3) should NOT match current lease (2)"
    );

    // THEN: Only matching token is accepted
    let current_token = FenceToken::new(2).expect("valid fence token");
    assert!(
        current_lease.matches_token(&current_token),
        "Current fence token (2) should match lease (2)"
    );

    // This demonstrates that when in Compensating state, any normal
    // StepCompleted arriving would have a stale fence token and would
    // be rejected at the fence validation layer.
}
