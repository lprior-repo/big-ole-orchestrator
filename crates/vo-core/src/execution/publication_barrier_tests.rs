//! Publication barrier state transition tests (TDD RED phase).
//!
//! These tests define the contract for the publication barrier state machine.
//! They MUST fail until the PendingPublication state and transitions are implemented.
//!
//! Contract from ve-s08ri / ADR-040:
//! - WHEN a step attempts to complete with a blob reference:
//!   THE SYSTEM SHALL transition the step to PendingPublication until the blob is verified
//! - IF the blob has not been verified durable:
//!   THE SYSTEM SHALL NOT emit an output_ref to downstream consumers

#[cfg(test)]
mod tests {
    use vo_types::state::{LifecycleState, TransitionEvent};

    // ------------------------------------------------------------------------
    // Helper: Build a simple NodeName for test contexts
    // ------------------------------------------------------------------------

    #[allow(dead_code)]
    fn node(name: &str) -> vo_types::NodeName {
        use vo_types::NodeName;
        NodeName::parse(name).expect("valid node name")
    }

    // ------------------------------------------------------------------------
    // RED-01: Step yielding a blob enters PendingPublication
    // ------------------------------------------------------------------------

    #[test]
    fn red_01_step_yielding_blob_enters_pending_publication() {
        // GIVEN a step in StepExecuting state
        let state = LifecycleState::StepExecuting;

        // WHEN the step yields a blob (barrier enter event)
        let result = vo_types::state::apply(state, TransitionEvent::YieldWithBlob);

        // THEN the state transitions to PendingPublication
        assert!(
            matches!(result, Ok(LifecycleState::PendingPublication)),
            "RED-01: Step yielding blob should transition to PendingPublication, got {:?}",
            result
        );
    }

    // ------------------------------------------------------------------------
    // RED-02: Confirming publication transitions step to Success with output_ref
    // ------------------------------------------------------------------------

    #[test]
    fn red_02_confirming_publication_transitions_to_completed() {
        // GIVEN a step in PendingPublication state
        let state = LifecycleState::PendingPublication;

        // WHEN publication is confirmed
        let result = vo_types::state::apply(state, TransitionEvent::ConfirmPublication);

        // THEN the state transitions to Completed
        assert!(
            matches!(result, Ok(LifecycleState::Completed)),
            "RED-02: Confirming publication should transition to Completed, got {:?}",
            result
        );
    }

    // ------------------------------------------------------------------------
    // RED-03: Attempting to emit output_ref while in PendingPublication is rejected
    // ------------------------------------------------------------------------

    #[test]
    fn red_03_emit_output_ref_while_pending_publication_is_rejected() {
        // GIVEN a step in PendingPublication state
        let state = LifecycleState::PendingPublication;

        // WHEN attempting to emit output_ref (barrier not yet lifted)
        let result = vo_types::state::apply(state, TransitionEvent::EmitOutputRef);

        // THEN the transition is rejected with InvalidTransition or barrier error
        assert!(
            result.is_err(),
            "RED-03: EmitOutputRef while PendingPublication should be rejected, got {:?}",
            result
        );
    }

    // ------------------------------------------------------------------------
    // RED-04: PendingPublication is NOT a terminal state
    // ------------------------------------------------------------------------

    #[test]
    fn red_04_pending_publication_is_not_terminal() {
        // GIVEN a step in PendingPublication state
        let state = LifecycleState::PendingPublication;

        // THEN it is NOT terminal
        assert!(
            !state.is_terminal(),
            "RED-04: PendingPublication should not be terminal, got is_terminal={}",
            state.is_terminal()
        );
    }

    // ------------------------------------------------------------------------
    // RED-05: Cannot transition from PendingPublication to any terminal state directly
    // ------------------------------------------------------------------------

    #[test]
    fn red_05_cannot_complete_without_publication_confirmation() {
        // GIVEN a step in PendingPublication state
        let state = LifecycleState::PendingPublication;

        // WHEN attempting to complete without publication confirmation
        let result = vo_types::state::apply(state, TransitionEvent::CompleteStep);

        // THEN the transition is rejected (barrier blocks premature completion)
        assert!(
            result.is_err(),
            "RED-05: CompleteStep while PendingPublication should be rejected, got {:?}",
            result
        );
    }

    // ------------------------------------------------------------------------
    // RED-06: PendingPublication accepts Cancel
    // ------------------------------------------------------------------------

    #[test]
    fn red_06_pending_publication_accepts_cancel() {
        // GIVEN a step in PendingPublication state
        let state = LifecycleState::PendingPublication;

        // WHEN Cancel is triggered
        let result = vo_types::state::apply(state, TransitionEvent::Cancel);

        // THEN the state transitions to Cancelled
        assert!(
            matches!(result, Ok(LifecycleState::Cancelled)),
            "RED-06: Cancel while PendingPublication should transition to Cancelled, got {:?}",
            result
        );
    }

    // ------------------------------------------------------------------------
    // RED-07: PendingPublication accepts Fail
    // ------------------------------------------------------------------------

    #[test]
    fn red_07_pending_publication_accepts_fail() {
        // GIVEN a step in PendingPublication state
        let state = LifecycleState::PendingPublication;

        // WHEN Fail is triggered
        let result = vo_types::state::apply(state, TransitionEvent::Fail);

        // THEN the state transitions to Failed
        assert!(
            matches!(result, Ok(LifecycleState::Failed)),
            "RED-07: Fail while PendingPublication should transition to Failed, got {:?}",
            result
        );
    }

    // ------------------------------------------------------------------------
    // RED-08: Terminal states reject EmitOutputRef (invariant enforcement)
    // ------------------------------------------------------------------------

    #[test]
    fn red_08_terminal_states_reject_emit_output_ref() {
        // GIVEN terminal states
        let terminal_states = [
            LifecycleState::Completed,
            LifecycleState::Failed,
            LifecycleState::Cancelled,
        ];

        for state in terminal_states {
            // WHEN attempting EmitOutputRef from terminal state
            let result = vo_types::state::apply(state, TransitionEvent::EmitOutputRef);

            // THEN it should be rejected (invariant: output_ref only after publication confirmed)
            assert!(
                result.is_err(),
                "RED-08: EmitOutputRef from {:?} should be rejected, got {:?}",
                state,
                result
            );
        }
    }

    // ------------------------------------------------------------------------
    // RED-09: Concurrent barrier access - only one PendingPublication per step
    // ------------------------------------------------------------------------

    #[test]
    fn red_09_second_barrier_enter_on_same_step_is_rejected() {
        // GIVEN a step already in PendingPublication state
        let state = LifecycleState::PendingPublication;

        // WHEN a second YieldWithBlob is attempted (concurrent barrier access)
        let result = vo_types::state::apply(state, TransitionEvent::YieldWithBlob);

        // THEN it should be rejected (already in barrier)
        assert!(
            result.is_err(),
            "RED-09: Second YieldWithBlob while PendingPublication should be rejected, got {:?}",
            result
        );
    }

    // ------------------------------------------------------------------------
    // RED-10: Full happy path - step → PendingPublication → Completed
    // ------------------------------------------------------------------------

    #[test]
    fn red_10_full_barrier_lifecycle() {
        // GIVEN a step in StepExecuting
        let s0 = LifecycleState::StepExecuting;

        // WHEN it yields a blob
        let s1 = vo_types::state::apply(s0, TransitionEvent::YieldWithBlob);
        assert!(
            matches!(s1, Ok(LifecycleState::PendingPublication)),
            "RED-10a: Should enter PendingPublication, got {:?}",
            s1
        );

        // AND publication is confirmed
        let s2 = vo_types::state::apply(s1.unwrap(), TransitionEvent::ConfirmPublication);
        assert!(
            matches!(s2, Ok(LifecycleState::Completed)),
            "RED-10b: Should transition to Completed after confirmation, got {:?}",
            s2
        );

        // THEN output_ref can be emitted from Completed
        let s3 = vo_types::state::apply(s2.unwrap(), TransitionEvent::EmitOutputRef);
        assert!(
            s3.is_ok(),
            "RED-10c: EmitOutputRef from Completed should succeed, got {:?}",
            s3
        );
    }

    // ------------------------------------------------------------------------
    // RED-11: Full cancellation path - step → PendingPublication → Cancelled
    // ------------------------------------------------------------------------

    #[test]
    fn red_11_barrier_cancellation_lifecycle() {
        // GIVEN a step in StepExecuting
        let s0 = LifecycleState::StepExecuting;

        // WHEN it yields a blob
        let s1 = vo_types::state::apply(s0, TransitionEvent::YieldWithBlob);
        assert!(
            matches!(s1, Ok(LifecycleState::PendingPublication)),
            "RED-11a: Should enter PendingPublication, got {:?}",
            s1
        );

        // AND Cancel is triggered
        let s2 = vo_types::state::apply(s1.unwrap(), TransitionEvent::Cancel);
        assert!(
            matches!(s2, Ok(LifecycleState::Cancelled)),
            "RED-11b: Should transition to Cancelled, got {:?}",
            s2
        );
    }

    // ------------------------------------------------------------------------
    // RED-12: Invalid transition from PendingPublication to non-barrier states
    // ------------------------------------------------------------------------

    #[test]
    fn red_12_cannot_jump_out_of_barrier_to_running() {
        // GIVEN a step in PendingPublication state
        let state = LifecycleState::PendingPublication;

        // WHEN attempting to go back to StepExecuting
        let result = vo_types::state::apply(state, TransitionEvent::ExecuteStep);

        // THEN it should be rejected (cannot exit barrier except via confirm/cancel/fail)
        assert!(
            result.is_err(),
            "RED-12: ExecuteStep from PendingPublication should be rejected, got {:?}",
            result
        );
    }

    // ------------------------------------------------------------------------
    // RED-13: get_valid_transitions includes new barrier events
    // ------------------------------------------------------------------------

    #[test]
    fn red_13_pending_publication_has_correct_valid_transitions() {
        // GIVEN PendingPublication state
        let state = LifecycleState::PendingPublication;

        // WHEN we query valid transitions
        let valid = state.get_valid_transitions();

        // THEN it should include ConfirmPublication, Cancel, and Fail
        // AND NOT include CompleteStep, EmitOutputRef, YieldWithBlob
        assert!(
            valid.contains(&TransitionEvent::ConfirmPublication),
            "RED-13a: Valid transitions should include ConfirmPublication, got {:?}",
            valid
        );
        assert!(
            valid.contains(&TransitionEvent::Cancel),
            "RED-13b: Valid transitions should include Cancel, got {:?}",
            valid
        );
        assert!(
            valid.contains(&TransitionEvent::Fail),
            "RED-13c: Valid transitions should include Fail, got {:?}",
            valid
        );
        assert!(
            !valid.contains(&TransitionEvent::CompleteStep),
            "RED-13d: Valid transitions should NOT include CompleteStep, got {:?}",
            valid
        );
        assert!(
            !valid.contains(&TransitionEvent::EmitOutputRef),
            "RED-13e: Valid transitions should NOT include EmitOutputRef, got {:?}",
            valid
        );
    }
}
