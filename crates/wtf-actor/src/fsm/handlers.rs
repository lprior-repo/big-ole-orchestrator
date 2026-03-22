use super::state::FsmActorState;
use super::types::{ApplyResult, ExecutionPhase};
use bytes::Bytes;
use wtf_common::{ActivityId, EffectDeclaration};

pub fn handle_transition(
    state: &FsmActorState,
    seq: u64,
    phase: ExecutionPhase,
    to_state: String,
    effects: &[EffectDeclaration],
) -> (FsmActorState, ApplyResult) {
    let mut next = state.clone();
    next.current_state = to_state;
    next.applied_seq.insert(seq);
    next.events_since_snapshot += 1;

    let to_execute = match phase {
        ExecutionPhase::Replay => vec![],
        ExecutionPhase::Live => effects.to_vec(),
    };

    (next, ApplyResult::Effects(to_execute))
}

pub fn handle_activity_dispatched(
    state: &FsmActorState,
    seq: u64,
    activity_id: &str,
    activity_type: &str,
) -> (FsmActorState, ApplyResult) {
    let mut next = state.clone();
    next.in_flight
        .insert(ActivityId::new(activity_id), activity_type.to_owned());
    next.applied_seq.insert(seq);
    next.events_since_snapshot += 1;
    (next, ApplyResult::None)
}

pub fn handle_activity_completed(
    state: &FsmActorState,
    seq: u64,
    activity_id: &str,
    result: &Bytes,
) -> (FsmActorState, ApplyResult) {
    let mut next = state.clone();
    next.in_flight.remove(&ActivityId::new(activity_id));
    next.applied_seq.insert(seq);
    next.events_since_snapshot += 1;
    (
        next,
        ApplyResult::ActivityResult(activity_id.to_owned(), result.clone()),
    )
}

pub fn handle_activity_failed(
    state: &FsmActorState,
    seq: u64,
    activity_id: &str,
    retries_exhausted: bool,
) -> (FsmActorState, ApplyResult) {
    let mut next = state.clone();
    if retries_exhausted {
        next.in_flight.remove(&ActivityId::new(activity_id));
    }
    next.applied_seq.insert(seq);
    next.events_since_snapshot += 1;
    (next, ApplyResult::None)
}

pub fn handle_generic_event(state: &FsmActorState, seq: u64) -> (FsmActorState, ApplyResult) {
    let mut next = state.clone();
    next.applied_seq.insert(seq);
    next.events_since_snapshot += 1;
    (next, ApplyResult::None)
}

pub fn handle_snapshot(state: &FsmActorState, seq: u64) -> (FsmActorState, ApplyResult) {
    let mut next = state.clone();
    next.applied_seq.insert(seq);
    next.events_since_snapshot = 0;
    (next, ApplyResult::None)
}
