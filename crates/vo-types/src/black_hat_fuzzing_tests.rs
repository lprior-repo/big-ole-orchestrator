//! BLACK-HAT: Adversarial fuzzing for vo-types type system.
//!
//! Attack vectors:
//! 1. Newtype boundary violations (bypass constructors)
//! 2. State machine transition table inconsistencies
//! 3. Serialization round-trip failures with adversarial inputs
//! 4. Data structure invariant breaks
//! 5. Proptest-based invariant fuzzing

use crate::state::{apply, LifecycleState, TransitionError, TransitionEvent};

/// CRITICAL: Exhaustive check that every transition listed as valid by
/// get_valid_transitions() is accepted by apply(). Found inconsistency:
/// PendingPublication + {Cancel, ConfirmPublication, PublicationFailed} and
/// StepExecuting + YieldWithBlob are declared valid but rejected by apply().
#[test]
fn bh_all_valid_transitions_must_be_accepted_by_apply() {
    let all_states = [
        LifecycleState::Pending,
        LifecycleState::RunningDecision,
        LifecycleState::StepScheduled,
        LifecycleState::StepExecuting,
        LifecycleState::WaitingForTimer,
        LifecycleState::PendingPublication,
        LifecycleState::Completed,
        LifecycleState::Failed,
        LifecycleState::Cancelled,
    ];

    let mut failures = Vec::new();
    for state in &all_states {
        for event in state.get_valid_transitions() {
            let result = transition::apply(*state, event);
            if result.is_err() {
                failures.push(format!("{:?} + {:?} = {:?}", state, event, result));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "These valid transitions are rejected by apply():\n{}",
        failures.join("\n")
    );
}
