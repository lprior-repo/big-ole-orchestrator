//! Lifecycle logic for WorkflowInstance actors (ADR-016).
//!
//! Follows "Functional Rust" Data -> Calc -> Actions:
//! - Data: [`ParadigmState`].
//! - Calc: [`compute_live_transition`] — pure function determining re-dispatches.
//! - Actions: [`execute_transition_actions`] — side effects in the actor shell.

use std::collections::HashMap;
use wtf_common::{ActivityId, InstanceId, WorkflowEvent, WtfError};
use crate::messages::{WorkflowParadigm, InstancePhase};
use crate::procedural::ProceduralActorState;
use crate::fsm::{FsmActorState, ExecutionPhase};
use crate::dag::DagActorState;
use wtf_storage::NatsClient;
use async_nats::jetstream::kv::Store;

/// Unified error for event application across all paradigms.
#[derive(Debug, thiserror::Error)]
pub enum ParadigmApplyError {
    #[error("Procedural error: {0}")]
    Procedural(#[from] crate::procedural::ProceduralApplyError),
    #[error("DAG error: {0}")]
    Dag(#[from] crate::dag::DagApplyError),
    #[error("FSM error: {0}")]
    Fsm(#[from] crate::fsm::ApplyError),
}

/// Unified state for the three execution paradigms.
#[derive(Debug, Clone)]
pub enum ParadigmState {
    Fsm(FsmActorState),
    Dag(DagActorState),
    Procedural(ProceduralActorState),
}

impl ParadigmState {
    /// Return the operation counter for procedural paradigm, or 0.
    #[must_use]
    pub fn operation_counter(&self) -> u32 {
        match self {
            ParadigmState::Procedural(s) => s.operation_counter,
            _ => 0,
        }
    }

    /// Apply a single event to the paradigm state.
    ///
    /// # Errors
    /// Returns an error if the event is invalid for the current paradigm or state.
    pub fn apply_event(
        &self,
        event: &WorkflowEvent,
        seq: u64,
        phase: InstancePhase,
    ) -> Result<Self, ParadigmApplyError> {
        match self {
            ParadigmState::Procedural(s) => {
                let (next, _) = crate::procedural::apply_event(s, event, seq)?;
                Ok(ParadigmState::Procedural(next))
            }
            ParadigmState::Dag(s) => {
                let (next, _) = crate::dag::apply_event(s, event, seq)?;
                Ok(ParadigmState::Dag(next))
            }
            ParadigmState::Fsm(s) => {
                let fsm_phase = match phase {
                    InstancePhase::Replay => ExecutionPhase::Replay,
                    InstancePhase::Live => ExecutionPhase::Live,
                };
                let (next, _) = crate::fsm::apply_event(s, event, seq, fsm_phase)?;
                Ok(ParadigmState::Fsm(next))
            }
        }
    }
}

/// Actions to be performed when transitioning from Replay to Live phase.
#[derive(Debug, Clone, Default)]
pub struct TransitionActions {
    /// Activities that were in-flight at crash time and need re-dispatching.
    pub re_dispatch: Vec<WorkflowEvent>,
    /// Timers that were pending and need re-registration in the wtf-timers KV.
    pub re_register_timers: Vec<WorkflowEvent>,
}

/// PURE CALC: Compute the actions needed to transition to Live phase.
///
/// Scans the paradigm state for in-flight activities and pending timers
/// that were not completed/fired during replay.
#[must_use]
pub fn compute_live_transition(
    _instance_id: &InstanceId,
    _paradigm: WorkflowParadigm,
    state: &ParadigmState,
    event_log: &[WorkflowEvent],
) -> TransitionActions {
    let mut actions = TransitionActions::default();

    // 1. Re-dispatch in-flight activities
    match state {
        ParadigmState::Procedural(s) => {
            for activity_id in s.in_flight.values() {
                if let Some(event) = find_original_dispatch(event_log, activity_id) {
                    actions.re_dispatch.push(event);
                }
            }
        }
        ParadigmState::Dag(s) => {
            for node_id in &s.in_flight {
                let activity_id = ActivityId::new(node_id.as_str());
                if let Some(event) = find_original_dispatch(event_log, &activity_id) {
                    actions.re_dispatch.push(event);
                }
            }
        }
        ParadigmState::Fsm(s) => {
            for activity_id in s.in_flight.keys() {
                if let Some(event) = find_original_dispatch(event_log, activity_id) {
                    actions.re_dispatch.push(event);
                }
            }
        }
    }

    // 2. Re-register pending timers
    actions.re_register_timers = find_pending_timers(event_log);

    actions
}

/// ACTION: Execute the side effects determined by the transition calculation.
///
/// # Errors
/// Returns [`WtfError::NatsPublish`] if re-dispatching or timer registration fails.
pub async fn execute_transition_actions(
    nats: &NatsClient,
    timers_kv: &Store,
    actions: TransitionActions,
) -> Result<(), WtfError> {
    // 1. Re-publish in-flight activities to wtf-work
    for event in actions.re_dispatch {
        if let WorkflowEvent::ActivityDispatched { activity_type, payload, .. } = event {
            let subject = format!("wtf.work.{}", activity_type);
            nats.jetstream().publish(subject, payload).await
                .map_err(|e| WtfError::nats_publish(format!("re-dispatch failed: {e}")))?;
        }
    }

    // 2. Re-register pending timers in wtf-timers KV
    for event in actions.re_register_timers {
        if let WorkflowEvent::TimerScheduled { timer_id, fire_at } = event {
            let payload = serde_json::to_vec(&fire_at)
                .map_err(|e| WtfError::nats_publish(format!("serialize timer: {e}")))?;
            timers_kv.put(&timer_id, payload.into()).await
                .map_err(|e| WtfError::nats_publish(format!("re-register timer {timer_id}: {e}")))?;
        }
    }

    Ok(())
}

fn find_original_dispatch(log: &[WorkflowEvent], activity_id: &ActivityId) -> Option<WorkflowEvent> {
    log.iter().find(|e| match e {
        WorkflowEvent::ActivityDispatched { activity_id: id, .. } => id == activity_id.as_str(),
        _ => false,
    }).cloned()
}

fn find_pending_timers(log: &[WorkflowEvent]) -> Vec<WorkflowEvent> {
    let mut pending = HashMap::new();
    for event in log {
        match event {
            WorkflowEvent::TimerScheduled { timer_id, .. } => {
                pending.insert(timer_id.clone(), event.clone());
            }
            WorkflowEvent::TimerFired { timer_id } | WorkflowEvent::TimerCancelled { timer_id } => {
                pending.remove(timer_id);
            }
            _ => {}
        }
    }
    pending.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use wtf_common::RetryPolicy;

    #[test]
    fn compute_transition_re_dispatches_in_flight_procedural() {
        let inst = InstanceId::new("inst-1");
        let aid = ActivityId::procedural(&inst, 0);
        let mut proc_state = ProceduralActorState::new();
        proc_state.in_flight.insert(0, aid.clone());
        proc_state.operation_counter = 1;

        let dispatch_event = WorkflowEvent::ActivityDispatched {
            activity_id: aid.to_string(),
            activity_type: "work".into(),
            payload: Bytes::new(),
            retry_policy: RetryPolicy::default(),
            attempt: 1,
        };

        let actions = compute_live_transition(
            &inst,
            WorkflowParadigm::Procedural,
            &ParadigmState::Procedural(proc_state),
            &[dispatch_event.clone()],
        );

        assert_eq!(actions.re_dispatch.len(), 1);
        assert_eq!(actions.re_dispatch[0], dispatch_event);
    }

    #[test]
    fn find_pending_timers_returns_only_unfired() {
        let t1 = "timer-1".to_string();
        let t2 = "timer-2".to_string();
        let log = vec![
            WorkflowEvent::TimerScheduled { timer_id: t1.clone(), fire_at: chrono::Utc::now() },
            WorkflowEvent::TimerScheduled { timer_id: t2.clone(), fire_at: chrono::Utc::now() },
            WorkflowEvent::TimerFired { timer_id: t1 },
        ];

        let pending = find_pending_timers(&log);
        assert_eq!(pending.len(), 1);
        if let WorkflowEvent::TimerScheduled { timer_id, .. } = &pending[0] {
            assert_eq!(timer_id, &t2);
        } else {
            panic!("Expected TimerScheduled");
        }
    }
}
