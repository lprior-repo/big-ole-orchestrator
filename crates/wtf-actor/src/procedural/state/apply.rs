//! Event application logic for Procedural workflow state (ADR-017).

#![allow(clippy::missing_errors_doc, clippy::too_many_lines)]

use super::{Checkpoint, ProceduralActorState};
use bytes::Bytes;
use wtf_common::{ActivityId, WorkflowEvent};

/// Result of applying a single event to Procedural state.
#[derive(Debug, Clone)]
pub enum ProceduralApplyResult {
    /// Event was already applied (duplicate delivery) — state unchanged.
    AlreadyApplied,
    /// No meaningful change (informational event).
    None,
    /// Activity dispatched — `operation_id` now in `in_flight`.
    ActivityDispatched {
        operation_id: u32,
        activity_id: ActivityId,
    },
    /// Activity completed — checkpoint recorded, workflow code may resume.
    ActivityCompleted { operation_id: u32, result: Bytes },
    /// Activity permanently failed (retries exhausted).
    ActivityFailed { operation_id: u32 },
}

/// Error applying an event to Procedural state.
#[derive(Debug, thiserror::Error)]
pub enum ProceduralApplyError {
    #[error("ActivityCompleted for unknown activity_id {0}; no matching in_flight entry")]
    UnknownActivityId(String),
}

/// Apply a single `WorkflowEvent` to the Procedural actor state.
pub fn apply_event(
    state: &ProceduralActorState,
    event: &WorkflowEvent,
    seq: u64,
) -> Result<(ProceduralActorState, ProceduralApplyResult), ProceduralApplyError> {
    if state.applied_seq.contains(&seq) {
        return Ok((state.clone(), ProceduralApplyResult::AlreadyApplied));
    }

    let result = match event {
        WorkflowEvent::ActivityDispatched { activity_id, .. } => {
            let mut next = state.clone();
            let op_id = next.operation_counter;
            next.operation_counter += 1;
            next.in_flight.insert(op_id, ActivityId::new(activity_id));
            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;

            (
                next,
                ProceduralApplyResult::ActivityDispatched {
                    operation_id: op_id,
                    activity_id: ActivityId::new(activity_id),
                },
            )
        }

        WorkflowEvent::ActivityCompleted {
            activity_id,
            result,
            ..
        } => {
            let aid = ActivityId::new(activity_id);

            let op_id = state
                .in_flight
                .iter()
                .find(|(_, v)| *v == &aid)
                .map(|(k, _)| *k)
                .ok_or_else(|| ProceduralApplyError::UnknownActivityId(activity_id.clone()))?;

            let mut next = state.clone();
            next.in_flight.remove(&op_id);
            next.checkpoint_map.insert(
                op_id,
                Checkpoint {
                    result: result.clone(),
                    completed_seq: seq,
                },
            );
            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;

            (
                next,
                ProceduralApplyResult::ActivityCompleted {
                    operation_id: op_id,
                    result: result.clone(),
                },
            )
        }

        WorkflowEvent::ActivityFailed {
            activity_id,
            retries_exhausted,
            ..
        } => {
            let aid = ActivityId::new(activity_id);
            let mut next = state.clone();

            if *retries_exhausted {
                let op_id = state
                    .in_flight
                    .iter()
                    .find(|(_, v)| *v == &aid)
                    .map(|(k, _)| *k);

                if let Some(id) = op_id {
                    next.in_flight.remove(&id);
                    next.applied_seq.insert(seq);
                    next.events_since_snapshot += 1;
                    return Ok((
                        next,
                        ProceduralApplyResult::ActivityFailed { operation_id: id },
                    ));
                }
            }

            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;
            (next, ProceduralApplyResult::None)
        }

        WorkflowEvent::NowSampled { operation_id, ts } => {
            let mut next = state.clone();
            let result_bytes = Bytes::copy_from_slice(&ts.timestamp_millis().to_le_bytes());
            next.checkpoint_map.insert(
                *operation_id,
                Checkpoint {
                    result: result_bytes,
                    completed_seq: seq,
                },
            );
            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;
            (next, ProceduralApplyResult::None)
        }

        WorkflowEvent::RandomSampled {
            operation_id,
            value,
        } => {
            let mut next = state.clone();
            let result_bytes = Bytes::copy_from_slice(&value.to_le_bytes());
            next.checkpoint_map.insert(
                *operation_id,
                Checkpoint {
                    result: result_bytes,
                    completed_seq: seq,
                },
            );
            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;
            (next, ProceduralApplyResult::None)
        }

        // Timer sleep: increment counter so the op_id slot is reserved.
        // timer_id format for procedural sleeps: "{instance_id}:t:{op_id}".
        WorkflowEvent::TimerScheduled { timer_id, .. } => {
            let mut next = state.clone();
            let op_id = next.operation_counter;
            next.operation_counter += 1;
            next.in_flight_timers.insert(timer_id.clone(), op_id);
            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;
            (next, ProceduralApplyResult::None)
        }

        // Timer fired: create checkpoint so ctx.sleep() replays without re-scheduling.
        WorkflowEvent::TimerFired { timer_id } => {
            let mut next = state.clone();
            if let Some(op_id) = next.in_flight_timers.remove(timer_id.as_str()) {
                next.checkpoint_map.insert(
                    op_id,
                    Checkpoint {
                        result: Bytes::new(),
                        completed_seq: seq,
                    },
                );
            }
            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;
            (next, ProceduralApplyResult::None)
        }

        WorkflowEvent::SnapshotTaken { .. } => {
            let mut next = state.clone();
            next.applied_seq.insert(seq);
            next.events_since_snapshot = 0;
            (next, ProceduralApplyResult::None)
        }

        WorkflowEvent::SignalReceived {
            signal_name: _,
            payload,
        } => {
            let mut next = state.clone();
            let op_id = next.operation_counter;
            next.operation_counter += 1;
            next.checkpoint_map.insert(
                op_id,
                Checkpoint {
                    result: payload.clone(),
                    completed_seq: seq,
                },
            );
            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;
            (next, ProceduralApplyResult::None)
        }

        _ => {
            let mut next = state.clone();
            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;
            (next, ProceduralApplyResult::None)
        }
    };

    Ok(result)
}
