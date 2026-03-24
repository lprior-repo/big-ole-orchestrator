pub mod definition;
pub mod handlers;
pub mod state;
#[cfg(test)]
mod tests;
pub mod types;

pub use definition::{parse_fsm, FsmDefinition, ParseFsmError};
pub use state::FsmActorState;
pub use types::*;

use wtf_common::WorkflowEvent;

/// Apply a single `WorkflowEvent` to the FSM actor state.
pub fn apply_event(
    state: &FsmActorState,
    event: &WorkflowEvent,
    seq: u64,
    phase: ExecutionPhase,
) -> Result<(FsmActorState, ApplyResult), ApplyError> {
    if state.applied_seq.contains(&seq) {
        return Ok((state.clone(), ApplyResult::AlreadyApplied));
    }

    let result = match event {
        WorkflowEvent::TransitionApplied {
            to_state, effects, ..
        } => handlers::handle_transition(state, seq, phase, to_state.clone(), effects),

        WorkflowEvent::ActivityDispatched {
            activity_id,
            activity_type,
            ..
        } => handlers::handle_activity_dispatched(state, seq, activity_id, activity_type),

        WorkflowEvent::ActivityCompleted {
            activity_id,
            result,
            ..
        } => handlers::handle_activity_completed(state, seq, activity_id, result),

        WorkflowEvent::ActivityFailed {
            activity_id,
            retries_exhausted,
            ..
        } => handlers::handle_activity_failed(state, seq, activity_id, *retries_exhausted),

        WorkflowEvent::SnapshotTaken { .. } => handlers::handle_snapshot(state, seq),

        _ => handlers::handle_generic_event(state, seq),
    };

    Ok(result)
}

/// Compute the plan for an FSM signal (pure — no I/O). Returns `None` if no transition applies.
#[must_use]
pub fn plan_fsm_signal(
    definition: &FsmDefinition,
    state: &FsmActorState,
    signal_name: &str,
) -> Option<FsmTransitionPlan> {
    let (to_state, effects) = definition.transition(&state.current_state, signal_name)?;
    let transition_event = WorkflowEvent::TransitionApplied {
        from_state: state.current_state.clone(),
        event_name: signal_name.to_owned(),
        to_state: to_state.to_owned(),
        effects: effects.to_vec(),
    };
    let (next_state, _) = apply_event(state, &transition_event, 0, ExecutionPhase::Live).ok()?;
    Some(FsmTransitionPlan {
        transition_event,
        next_state,
    })
}
