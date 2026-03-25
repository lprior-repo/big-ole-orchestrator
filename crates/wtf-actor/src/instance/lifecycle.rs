//! Lifecycle logic for `WorkflowInstance` actors (ADR-016).
//!
//! Follows "Functional Rust" Data -> Calc -> Actions:
//! - Data: [`ParadigmState`].
//! - Calc: [`compute_live_transition`] — pure function determining re-dispatches.
//! - Actions: [`execute_transition_actions`] — side effects in the actor shell.

#![allow(clippy::missing_errors_doc)]

use crate::dag::DagActorState;
use crate::fsm::{ExecutionPhase, FsmActorState};
use crate::messages::InstancePhase;
use crate::procedural::ProceduralActorState;
use ractor::ActorProcessingErr;
use std::collections::HashMap;
use wtf_common::storage::{StateStore, TaskQueue};
use wtf_common::{ActivityId, InstanceId, WorkflowEvent, WorkflowParadigm, WtfError};

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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "paradigm", rename_all = "snake_case")]
pub enum ParadigmState {
    Fsm(FsmActorState),
    Dag(DagActorState),
    Procedural(ProceduralActorState),
}

impl ParadigmState {
    #[must_use]
    pub fn operation_counter(&self) -> u32 {
        match self {
            ParadigmState::Procedural(s) => s.operation_counter,
            _ => 0,
        }
    }

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

pub fn deserialize_paradigm_state(
    paradigm: WorkflowParadigm,
    bytes: &[u8],
) -> Result<ParadigmState, ActorProcessingErr> {
    match paradigm {
        WorkflowParadigm::Fsm => Ok(ParadigmState::Fsm(
            rmp_serde::from_slice(bytes).map_err(|e| ActorProcessingErr::from(Box::new(e)))?,
        )),
        WorkflowParadigm::Dag => Ok(ParadigmState::Dag(
            rmp_serde::from_slice(bytes).map_err(|e| ActorProcessingErr::from(Box::new(e)))?,
        )),
        WorkflowParadigm::Procedural => Ok(ParadigmState::Procedural(
            rmp_serde::from_slice(bytes).map_err(|e| ActorProcessingErr::from(Box::new(e)))?,
        )),
    }
}

#[derive(Debug, Clone, Default)]
pub struct TransitionActions {
    pub re_dispatch: Vec<WorkflowEvent>,
    pub re_register_timers: Vec<WorkflowEvent>,
}

#[must_use]
pub fn compute_live_transition(
    _instance_id: &InstanceId,
    _paradigm: WorkflowParadigm,
    state: &ParadigmState,
    event_log: &[WorkflowEvent],
) -> TransitionActions {
    let mut actions = TransitionActions::default();

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

    actions.re_register_timers = find_pending_timers(event_log);
    actions
}

pub async fn execute_transition_actions(
    task_queue: &dyn TaskQueue,
    state_store: &dyn StateStore,
    actions: TransitionActions,
) -> Result<(), WtfError> {
    for event in actions.re_dispatch {
        if let WorkflowEvent::ActivityDispatched {
            activity_type,
            payload,
            ..
        } = event
        {
            task_queue.dispatch(&activity_type, payload).await?;
        }
    }

    for event in actions.re_register_timers {
        if let WorkflowEvent::TimerScheduled { timer_id, fire_at } = event {
            let payload = serde_json::to_vec(&fire_at)
                .map_err(|e| WtfError::nats_publish(format!("serialize timer: {e}")))?;
            state_store.put_timer(&timer_id, payload.into()).await?;
        }
    }

    Ok(())
}

fn find_original_dispatch(
    log: &[WorkflowEvent],
    activity_id: &ActivityId,
) -> Option<WorkflowEvent> {
    log.iter()
        .find(|e| match e {
            WorkflowEvent::ActivityDispatched {
                activity_id: id, ..
            } => id == activity_id.as_str(),
            _ => false,
        })
        .cloned()
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
