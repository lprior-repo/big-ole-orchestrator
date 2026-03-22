//! FsmActor — FSM paradigm actor state and event application (ADR-017).
//!
//! The FSM paradigm records every state transition as `TransitionApplied` in JetStream.
//! On replay: apply transitions in order, skip re-executing effects (they're in the event).
//! The replay produces `current_state` identical to what it was at crash time.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use wtf_common::{ActivityId, EffectDeclaration, WorkflowEvent};

/// In-memory state for an FSM workflow actor.
///
/// This is a pure cache of the JetStream event log. Every field is derivable
/// by replaying `WorkflowEvent` records from the stream (ADR-016).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsmActorState {
    /// Current FSM state name (e.g. `"Pending"`, `"Authorized"`).
    pub current_state: String,

    /// Set of JetStream sequence numbers already applied.
    /// Used to skip duplicate events during replay (idempotency guard — ADR-016).
    pub applied_seq: HashSet<u64>,

    /// Activities currently dispatched but not yet completed.
    /// Key: `ActivityId`. Value: activity type name.
    pub in_flight: HashMap<ActivityId, String>,

    /// Number of events processed since the last snapshot.
    pub events_since_snapshot: u32,
}

impl FsmActorState {
    /// Create a new FSM state starting in `initial_state`.
    #[must_use]
    pub fn new(initial_state: impl Into<String>) -> Self {
        Self {
            current_state: initial_state.into(),
            applied_seq: HashSet::new(),
            in_flight: HashMap::new(),
            events_since_snapshot: 0,
        }
    }
}

/// Apply a single `WorkflowEvent` to the FSM actor state.
///
/// Returns the effects that should be executed (non-empty only in Live Phase).
/// During Replay Phase, effects in `TransitionApplied` are SKIPPED — they already happened.
///
/// # Idempotency
/// If `seq` is already in `applied_seq`, returns `ApplyResult::AlreadyApplied` without
/// mutating state. This handles duplicate deliveries from JetStream.
///
/// # Errors
/// Returns [`ApplyError::UnexpectedEvent`] for events that are valid in the log but
/// not applicable to FSM state (e.g., DAG-specific events in an FSM stream).
pub fn apply_event(
    state: &FsmActorState,
    event: &WorkflowEvent,
    seq: u64,
    phase: ExecutionPhase,
) -> Result<(FsmActorState, ApplyResult), ApplyError> {
    // Idempotency: skip already-applied sequences.
    if state.applied_seq.contains(&seq) {
        return Ok((state.clone(), ApplyResult::AlreadyApplied));
    }

    let result = match event {
        WorkflowEvent::TransitionApplied {
            to_state, effects, ..
        } => {
            let mut next = state.clone();
            next.current_state = to_state.clone();
            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;

            // In Replay Phase: effects already happened — do not re-execute.
            // In Live Phase: return effects to the actor for execution.
            let to_execute = match phase {
                ExecutionPhase::Replay => vec![],
                ExecutionPhase::Live => effects.clone(),
            };

            (next, ApplyResult::Effects(to_execute))
        }

        WorkflowEvent::ActivityDispatched {
            activity_id,
            activity_type,
            ..
        } => {
            let mut next = state.clone();
            next.in_flight
                .insert(ActivityId::new(activity_id), activity_type.clone());
            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;
            (next, ApplyResult::None)
        }

        WorkflowEvent::ActivityCompleted {
            activity_id,
            result,
            ..
        } => {
            let mut next = state.clone();
            next.in_flight.remove(&ActivityId::new(activity_id));
            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;
            (
                next,
                ApplyResult::ActivityResult(activity_id.clone(), result.clone()),
            )
        }

        WorkflowEvent::ActivityFailed {
            activity_id,
            retries_exhausted,
            ..
        } => {
            let mut next = state.clone();
            if *retries_exhausted {
                next.in_flight.remove(&ActivityId::new(activity_id));
            }
            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;
            (next, ApplyResult::None)
        }

        WorkflowEvent::SignalReceived { .. }
        | WorkflowEvent::TimerFired { .. }
        | WorkflowEvent::TimerScheduled { .. }
        | WorkflowEvent::TimerCancelled { .. }
        | WorkflowEvent::InstanceStarted { .. }
        | WorkflowEvent::InstanceCompleted { .. }
        | WorkflowEvent::InstanceFailed { .. }
        | WorkflowEvent::InstanceCancelled { .. }
        | WorkflowEvent::ChildStarted { .. }
        | WorkflowEvent::ChildCompleted { .. }
        | WorkflowEvent::ChildFailed { .. }
        | WorkflowEvent::ActivityHeartbeat { .. } => {
            let mut next = state.clone();
            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;
            (next, ApplyResult::None)
        }

        WorkflowEvent::SnapshotTaken { .. } => {
            // SnapshotTaken resets the event counter — it's the checkpoint marker.
            let mut next = state.clone();
            next.applied_seq.insert(seq);
            next.events_since_snapshot = 0;
            (next, ApplyResult::None)
        }

        WorkflowEvent::NowSampled { .. } | WorkflowEvent::RandomSampled { .. } => {
            let mut next = state.clone();
            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;
            (next, ApplyResult::None)
        }
    };

    Ok(result)
}

/// FSM workflow definition (bead wtf-tzjw).
#[derive(Debug, Clone, Default)]
pub struct FsmDefinition {
    transitions: HashMap<(String, String), (String, Vec<EffectDeclaration>)>,
    terminal_states: HashSet<String>,
}

impl FsmDefinition {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare `state` as a terminal state (workflow ends when entered).
    pub fn add_terminal_state(&mut self, state: impl Into<String>) {
        self.terminal_states.insert(state.into());
    }

    /// Returns `true` if `state` is a declared terminal state.
    #[must_use]
    pub fn is_terminal(&self, state: &str) -> bool {
        self.terminal_states.contains(state)
    }

    pub fn add_transition(
        &mut self,
        from: impl Into<String>,
        event: impl Into<String>,
        to: impl Into<String>,
        effects: Vec<EffectDeclaration>,
    ) {
        self.transitions
            .insert((from.into(), event.into()), (to.into(), effects));
    }

    #[must_use]
    pub fn transition(
        &self,
        current_state: &str,
        event_name: &str,
    ) -> Option<(&str, &[EffectDeclaration])> {
        self.transitions
            .get(&(current_state.to_owned(), event_name.to_owned()))
            .map(|(to, effects)| (to.as_str(), effects.as_slice()))
    }
}

/// Output of [`plan_fsm_signal`].
#[derive(Debug, Clone)]
pub struct FsmTransitionPlan {
    pub transition_event: WorkflowEvent,
    pub next_state: FsmActorState,
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

/// Which phase the actor is in — determines whether effects are executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionPhase {
    /// Replaying the event log — effects are skipped (they already happened).
    Replay,
    /// Processing new events in real-time — effects must be executed.
    Live,
}

/// Result of applying a single event to FSM state.
#[derive(Debug, Clone)]
pub enum ApplyResult {
    /// Event was already applied (duplicate delivery) — state unchanged.
    AlreadyApplied,
    /// No effect to execute (informational event).
    None,
    /// Effects to execute in Live Phase (from `TransitionApplied`).
    Effects(Vec<EffectDeclaration>),
    /// Activity completed — caller should deliver result to pending waiter.
    ActivityResult(String, Bytes),
}

/// Error applying an event.
#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error("event type not applicable to FSM actor: {0}")]
    UnexpectedEvent(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use wtf_common::EffectDeclaration;

    fn transition(
        from: &str,
        event: &str,
        to: &str,
        effects: Vec<EffectDeclaration>,
    ) -> WorkflowEvent {
        WorkflowEvent::TransitionApplied {
            from_state: from.into(),
            event_name: event.into(),
            to_state: to.into(),
            effects,
        }
    }

    fn no_effect_transition(from: &str, event: &str, to: &str) -> WorkflowEvent {
        transition(from, event, to, vec![])
    }

    #[test]
    fn new_state_starts_in_initial_state() {
        let state = FsmActorState::new("Pending");
        assert_eq!(state.current_state, "Pending");
    }

    #[test]
    fn apply_transition_updates_current_state() {
        let state = FsmActorState::new("Pending");
        let event = no_effect_transition("Pending", "Authorize", "Authorized");
        let (next, _) = apply_event(&state, &event, 1, ExecutionPhase::Live).expect("apply");
        assert_eq!(next.current_state, "Authorized");
    }

    #[test]
    fn apply_transition_adds_seq_to_applied_set() {
        let state = FsmActorState::new("Pending");
        let event = no_effect_transition("Pending", "Authorize", "Authorized");
        let (next, _) = apply_event(&state, &event, 42, ExecutionPhase::Live).expect("apply");
        assert!(next.applied_seq.contains(&42));
    }

    #[test]
    fn apply_duplicate_seq_returns_already_applied() {
        let state = FsmActorState::new("Pending");
        let event = no_effect_transition("Pending", "Authorize", "Authorized");
        let (s1, _) = apply_event(&state, &event, 1, ExecutionPhase::Live).expect("first apply");
        let (s2, result) = apply_event(&s1, &event, 1, ExecutionPhase::Live).expect("second apply");
        assert!(matches!(result, ApplyResult::AlreadyApplied));
        // State unchanged — still Authorized (not re-applied)
        assert_eq!(s2.current_state, "Authorized");
    }

    #[test]
    fn replay_phase_returns_no_effects() {
        let effect = EffectDeclaration {
            effect_type: "CallPayment".into(),
            payload: Bytes::from_static(b"{}"),
        };
        let state = FsmActorState::new("Pending");
        let event = transition("Pending", "Charge", "Charged", vec![effect]);
        let (_, result) = apply_event(&state, &event, 1, ExecutionPhase::Replay).expect("apply");
        match result {
            ApplyResult::Effects(e) => assert!(e.is_empty(), "replay should not return effects"),
            _ => {}
        }
    }

    #[test]
    fn live_phase_returns_effects() {
        let effect = EffectDeclaration {
            effect_type: "CallPayment".into(),
            payload: Bytes::from_static(b"{}"),
        };
        let state = FsmActorState::new("Pending");
        let event = transition("Pending", "Charge", "Charged", vec![effect.clone()]);
        let (_, result) = apply_event(&state, &event, 1, ExecutionPhase::Live).expect("apply");
        match result {
            ApplyResult::Effects(e) => assert_eq!(e.len(), 1),
            _ => panic!("expected Effects"),
        }
    }

    #[test]
    fn snapshot_taken_resets_events_since_snapshot() {
        let mut state = FsmActorState::new("Pending");
        state.events_since_snapshot = 99;
        let event = WorkflowEvent::SnapshotTaken {
            seq: 10,
            checksum: 0,
        };
        let (next, _) = apply_event(&state, &event, 11, ExecutionPhase::Replay).expect("apply");
        assert_eq!(next.events_since_snapshot, 0);
    }

    #[test]
    fn activity_dispatched_adds_to_in_flight() {
        let state = FsmActorState::new("Authorized");
        let event = WorkflowEvent::ActivityDispatched {
            activity_id: "act-1".into(),
            activity_type: "charge".into(),
            payload: Bytes::new(),
            retry_policy: wtf_common::RetryPolicy::default(),
            attempt: 1,
        };
        let (next, _) = apply_event(&state, &event, 1, ExecutionPhase::Live).expect("apply");
        assert!(next.in_flight.contains_key(&ActivityId::new("act-1")));
    }

    #[test]
    fn activity_completed_removes_from_in_flight() {
        let mut state = FsmActorState::new("Charged");
        state
            .in_flight
            .insert(ActivityId::new("act-1"), "charge".into());
        let event = WorkflowEvent::ActivityCompleted {
            activity_id: "act-1".into(),
            result: Bytes::from_static(b"ok"),
            duration_ms: 50,
        };
        let (next, _) = apply_event(&state, &event, 2, ExecutionPhase::Live).expect("apply");
        assert!(!next.in_flight.contains_key(&ActivityId::new("act-1")));
    }

    #[test]
    fn multiple_transitions_accumulate_correctly() {
        let s0 = FsmActorState::new("Pending");
        let e1 = no_effect_transition("Pending", "Authorize", "Authorized");
        let e2 = no_effect_transition("Authorized", "Charge", "Charged");
        let e3 = no_effect_transition("Charged", "Fulfill", "Fulfilled");

        let (s1, _) = apply_event(&s0, &e1, 1, ExecutionPhase::Replay).expect("e1");
        let (s2, _) = apply_event(&s1, &e2, 2, ExecutionPhase::Replay).expect("e2");
        let (s3, _) = apply_event(&s2, &e3, 3, ExecutionPhase::Replay).expect("e3");

        assert_eq!(s3.current_state, "Fulfilled");
        assert_eq!(s3.applied_seq.len(), 3);
        assert_eq!(s3.events_since_snapshot, 3);
    }

    #[test]
    fn fsm_definition_transition_returns_some_when_valid() {
        let mut def = FsmDefinition::new();
        def.add_transition("Pending", "Authorize", "Authorized", vec![]);
        let result = def.transition("Pending", "Authorize");
        assert!(result.is_some());
    }

    #[test]
    fn fsm_definition_transition_returns_none_for_unknown_event() {
        let mut def = FsmDefinition::new();
        def.add_transition("Pending", "Authorize", "Authorized", vec![]);
        assert!(def.transition("Pending", "Bogus").is_none());
    }

    /// Bead contract: duplicate event returns None (no second TransitionApplied).
    #[test]
    fn duplicate_fsm_event_returns_none_from_new_state() {
        let mut def = FsmDefinition::new();
        def.add_transition("Pending", "Authorize", "Authorized", vec![]);
        let state = FsmActorState::new("Pending");
        let plan1 = plan_fsm_signal(&def, &state, "Authorize");
        assert!(plan1.is_some());
        let next = plan1.unwrap().next_state;
        // Same event from the new state — no transition defined.
        assert!(plan_fsm_signal(&def, &next, "Authorize").is_none());
    }
}
