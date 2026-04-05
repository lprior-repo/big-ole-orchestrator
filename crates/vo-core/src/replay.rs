//! Deterministic event-sourced replay engine (ADR-027).
//!
//! Replays event sequences through the pure `apply()` state machine
//! to reconstruct `LifecycleState` from event history.

use vo_types::events::{EventEnvelope, EventPayload};
use vo_types::state::{self, LifecycleState, TransitionEvent};

use crate::upcaster::UpcasterRegistry;

// ============================================================================
// Result & Error Types
// ============================================================================

/// Result of replaying events through the state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayResult {
    /// Final reconstructed lifecycle state. `None` if no events were applied.
    pub final_state: Option<LifecycleState>,
    /// Number of events successfully applied.
    pub events_applied: usize,
}

/// Errors that can occur during event replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    /// Events have different instance_ids.
    InstanceMismatch { expected: String, actual: String },
    /// Sequence numbers have a gap.
    SequenceGap {
        expected: u64,
        actual: u64,
        at_index: usize,
    },
    /// Duplicate sequence number found.
    SequenceDuplicate {
        sequence: u64,
        first_at_index: usize,
        second_at_index: usize,
    },
    /// Event payload could not be decoded.
    PayloadDecodeFailed { sequence: u64, source: String },
    /// State machine rejected a transition.
    TransitionFailed {
        sequence: u64,
        state: LifecycleState,
        reason: String,
    },
    /// Event payload variant has no mapping to a TransitionEvent.
    UnexpectedEventType { payload_type: String, sequence: u64 },
    /// Upcasting failed during replay_with_upcaster.
    UpcastingFailed { sequence: u64, reason: String },
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayError::InstanceMismatch { expected, actual } => {
                write!(
                    f,
                    "Instance ID mismatch: expected '{expected}', got '{actual}'"
                )
            }
            ReplayError::SequenceGap {
                expected,
                actual,
                at_index,
            } => {
                write!(
                    f,
                    "Sequence gap at index {at_index}: expected {expected}, got {actual}"
                )
            }
            ReplayError::SequenceDuplicate {
                sequence,
                first_at_index,
                second_at_index,
            } => {
                write!(
                    f,
                    "Duplicate sequence {sequence} at indices {first_at_index} and {second_at_index}"
                )
            }
            ReplayError::PayloadDecodeFailed { sequence, source } => {
                write!(f, "Payload decode failed at sequence {sequence}: {source}")
            }
            ReplayError::TransitionFailed {
                sequence,
                state,
                reason,
            } => {
                write!(
                    f,
                    "Transition failed at sequence {sequence} in state {state:?}: {reason}"
                )
            }
            ReplayError::UnexpectedEventType {
                payload_type,
                sequence,
            } => {
                write!(
                    f,
                    "Unexpected event type '{payload_type}' at sequence {sequence}"
                )
            }
            ReplayError::UpcastingFailed { sequence, reason } => {
                write!(f, "Upcasting failed at sequence {sequence}: {reason}")
            }
        }
    }
}

impl std::error::Error for ReplayError {}

// ============================================================================
// ReplayEngine
// ============================================================================

/// Stateless, deterministic replay engine.
///
/// Applies events through the pure `apply()` state machine to reconstruct
/// `LifecycleState` from an event history.
pub struct ReplayEngine;

impl ReplayEngine {
    /// Create a new ReplayEngine.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Replay a sequence of events to reconstruct the final `LifecycleState`.
    ///
    /// # Arguments
    /// * `events` - Ordered slice of `EventEnvelope` (must be sorted by sequence,
    ///   same instance_id, upcast to current schema version)
    ///
    /// # Returns
    /// * `Ok(ReplayResult)` with final state and count of applied events
    /// * `Err(ReplayError)` with specific failure reason
    ///
    /// # Errors
    /// See `ReplayError` variants.
    pub fn replay(&self, events: &[EventEnvelope]) -> Result<ReplayResult, ReplayError> {
        if events.is_empty() {
            return Ok(ReplayResult {
                final_state: None,
                events_applied: 0,
            });
        }

        // Validate instance_id consistency
        let expected_instance_id = &events[0].instance_id;
        for event in events.iter().skip(1) {
            if event.instance_id != *expected_instance_id {
                return Err(ReplayError::InstanceMismatch {
                    expected: expected_instance_id.clone(),
                    actual: event.instance_id.clone(),
                });
            }
        }

        // Validate sequence ordering
        let mut expected_seq = events[0].sequence;
        for (i, event) in events.iter().enumerate() {
            if i == 0 {
                expected_seq = event.sequence;
                continue;
            }
            if event.sequence == expected_seq {
                return Err(ReplayError::SequenceDuplicate {
                    sequence: event.sequence,
                    first_at_index: i - 1,
                    second_at_index: i,
                });
            }
            if event.sequence != expected_seq + 1 {
                return Err(ReplayError::SequenceGap {
                    expected: expected_seq + 1,
                    actual: event.sequence,
                    at_index: i,
                });
            }
            expected_seq = event.sequence;
        }

        // Apply events through state machine
        let mut current_state: Option<LifecycleState> = None;
        let mut events_applied: usize = 0;

        for event in events {
            let payload = EventPayload::try_from_json(&event.payload).map_err(|e| {
                ReplayError::PayloadDecodeFailed {
                    sequence: event.sequence,
                    source: e.to_string(),
                }
            })?;

            // ContinuedAsNew is a lineage tracking event, not a state transition.
            // Count it as applied but do not change state.
            if matches!(payload, EventPayload::ContinuedAsNew { .. }) {
                events_applied += 1;
                continue;
            }

            let transition = payload_to_transition(&payload, event.sequence)?;

            // First event starts from Pending; subsequent events use accumulated state
            let state_for_apply = current_state.unwrap_or(LifecycleState::Pending);

            match state::apply(state_for_apply, transition) {
                Ok(new_state) => {
                    current_state = Some(new_state);
                    events_applied += 1;

                    // Stop processing after truly terminal states.
                    // Failed is NOT a stopping point — InstanceResumed can recover it.
                    if matches!(
                        new_state,
                        LifecycleState::Completed | LifecycleState::Cancelled
                    ) {
                        break;
                    }
                }
                Err(e) => {
                    return Err(ReplayError::TransitionFailed {
                        sequence: event.sequence,
                        state: state_for_apply,
                        reason: e.to_string(),
                    });
                }
            }
        }

        Ok(ReplayResult {
            final_state: current_state,
            events_applied,
        })
    }

    /// Replay a sequence of events with schema version upcasting.
    ///
    /// This method upcasts each envelope to the current schema version before
    /// applying the replay logic. If upcasting fails for any envelope, the
    /// entire replay fails with `ReplayError::UpcastingFailed`.
    ///
    /// # Arguments
    /// * `registry` - The upcaster registry to use for version transformations
    /// * `events` - Ordered slice of `EventEnvelope` (must be sorted by sequence,
    ///   same instance_id, may have older schema versions)
    ///
    /// # Returns
    /// * `Ok(ReplayResult)` with final state and count of applied events
    /// * `Err(ReplayError)` with specific failure reason
    ///
    /// # Errors
    /// See `ReplayError` variants.
    pub fn replay_with_upcaster(
        &self,
        registry: &dyn UpcasterRegistry,
        events: &[EventEnvelope],
    ) -> Result<ReplayResult, ReplayError> {
        if events.is_empty() {
            return Ok(ReplayResult {
                final_state: None,
                events_applied: 0,
            });
        }

        // Upcast all envelopes to the current schema version before replay
        let upcasted_events: Result<Vec<EventEnvelope>, ReplayError> = events
            .iter()
            .map(|envelope| {
                registry.upcast_envelope(envelope.clone()).map_err(|e| {
                    ReplayError::UpcastingFailed {
                        sequence: envelope.sequence,
                        reason: e.to_string(),
                    }
                })
            })
            .collect();

        let upcasted_events = upcasted_events?;

        // Delegate to the standard replay logic with upcasted events
        self.replay(&upcasted_events)
    }
}

impl Default for ReplayEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Map an `EventPayload` variant to a `TransitionEvent` for replay.
fn payload_to_transition(
    payload: &EventPayload,
    sequence: u64,
) -> Result<TransitionEvent, ReplayError> {
    match payload {
        EventPayload::WorkflowStarted { .. } => Ok(TransitionEvent::AssignToNode),
        EventPayload::StepScheduled { .. } => Ok(TransitionEvent::StepScheduled),
        EventPayload::StepStarted { .. } => Ok(TransitionEvent::ExecuteStep),
        EventPayload::StepCompleted { .. } => Ok(TransitionEvent::CompleteStep),
        EventPayload::StepFailed { .. } => Ok(TransitionEvent::Fail),
        EventPayload::TimerSet { .. } => Ok(TransitionEvent::WaitForTimer),
        EventPayload::TimerFired { .. } => Ok(TransitionEvent::TimerFired),
        EventPayload::WorkflowCompleted { .. } => Ok(TransitionEvent::CompleteStep),
        EventPayload::WorkflowFailed { .. } => Ok(TransitionEvent::Fail),
        EventPayload::WorkflowCancelled { .. } => Ok(TransitionEvent::Cancel),
        EventPayload::CancelRequested { .. } => Ok(TransitionEvent::Cancel),
        EventPayload::InstanceResumed { .. } => Ok(TransitionEvent::InstanceResumed),
        EventPayload::ContinuedAsNew { .. } => {
            // Handled as a no-op in the replay loop before calling this function.
            // This branch should never be reached.
            Err(ReplayError::UnexpectedEventType {
                payload_type: "ContinuedAsNew".to_string(),
                sequence,
            })
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Helper: create a valid EventEnvelope for testing.
    fn make_event(instance_id: &str, sequence: u64, payload: serde_json::Value) -> EventEnvelope {
        EventEnvelope {
            schema_version: 1,
            instance_id: instance_id.to_string(),
            sequence,
            timestamp_ms: 1000 * sequence,
            payload,
            metadata: json!({}),
        }
    }

    fn workflow_started_payload(workflow_id: &str) -> serde_json::Value {
        json!({
            "type": "WorkflowStarted",
            "workflow_id": workflow_id,
            "binary_hash": "sha256abc",
            "version": 1
        })
    }

    fn step_scheduled_payload(workflow_id: &str, step_id: &str) -> serde_json::Value {
        json!({
            "type": "StepScheduled",
            "workflow_id": workflow_id,
            "step_id": step_id,
            "attempt": 1,
            "execution_id": "exec-1",
            "version": 1
        })
    }

    fn step_started_payload(workflow_id: &str, step_id: &str) -> serde_json::Value {
        json!({
            "type": "StepStarted",
            "workflow_id": workflow_id,
            "step_id": step_id,
            "started_at_ms": 2000,
            "version": 1
        })
    }

    fn step_completed_payload(workflow_id: &str, step_id: &str) -> serde_json::Value {
        json!({
            "type": "StepCompleted",
            "workflow_id": workflow_id,
            "step_id": step_id,
            "completed_at_ms": 3000,
            "version": 1
        })
    }

    fn step_failed_payload(workflow_id: &str, step_id: &str) -> serde_json::Value {
        json!({
            "type": "StepFailed",
            "workflow_id": workflow_id,
            "step_id": step_id,
            "failure_reason": "error",
            "attempt": 1,
            "version": 1
        })
    }

    fn timer_set_payload(workflow_id: &str, timer_id: &str) -> serde_json::Value {
        json!({
            "type": "TimerSet",
            "workflow_id": workflow_id,
            "timer_id": timer_id,
            "fire_at_ms": 5000,
            "version": 1
        })
    }

    fn timer_fired_payload(workflow_id: &str, timer_id: &str) -> serde_json::Value {
        json!({
            "type": "TimerFired",
            "workflow_id": workflow_id,
            "timer_id": timer_id,
            "fired_at_ms": 5000,
            "version": 1
        })
    }

    fn workflow_cancelled_payload(workflow_id: &str) -> serde_json::Value {
        json!({
            "type": "WorkflowCancelled",
            "workflow_id": workflow_id,
            "cancelled_by": "user",
            "version": 1
        })
    }

    fn cancel_requested_payload(workflow_id: &str) -> serde_json::Value {
        json!({
            "type": "CancelRequested",
            "workflow_id": workflow_id,
            "requested_by": "user",
            "version": 1
        })
    }

    fn workflow_failed_payload(workflow_id: &str) -> serde_json::Value {
        json!({
            "type": "WorkflowFailed",
            "workflow_id": workflow_id,
            "failure_reason": "fatal",
            "version": 1
        })
    }

    fn instance_resumed_payload(workflow_id: &str) -> serde_json::Value {
        json!({
            "type": "InstanceResumed",
            "workflow_id": workflow_id,
            "resumed_at_ms": 6000,
            "version": 1
        })
    }

    fn continued_as_new_payload(workflow_id: &str) -> serde_json::Value {
        json!({
            "type": "ContinuedAsNew",
            "workflow_id": workflow_id,
            "lineage_id": "lin-1",
            "old_epoch": 0,
            "new_epoch": 1,
            "version": 1
        })
    }

    // =========================================================================
    // Behavior 1: ReplayEngine::new()
    // =========================================================================

    #[test]
    fn replay_engine_new_creates_instance() {
        let _engine = ReplayEngine::new();
    }

    #[test]
    fn replay_engine_default_creates_instance() {
        let _engine = ReplayEngine;
    }

    // =========================================================================
    // Behavior 2: Empty event list
    // =========================================================================

    #[test]
    fn replay_returns_empty_result_when_event_list_is_empty() {
        let engine = ReplayEngine::new();
        let result = engine.replay(&[]).expect("empty replay should succeed");
        assert_eq!(result.final_state, None);
        assert_eq!(result.events_applied, 0);
    }

    // =========================================================================
    // Behavior 3: WorkflowStarted maps to AssignToNode
    // =========================================================================

    #[test]
    fn replay_maps_workflow_started_to_assign_to_node_transition() {
        let engine = ReplayEngine::new();
        let events = [make_event("inst-1", 1, workflow_started_payload("wf-1"))];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
        assert_eq!(result.events_applied, 1);
    }

    // =========================================================================
    // Behavior 4: StepScheduled maps correctly
    // =========================================================================

    #[test]
    fn replay_maps_step_scheduled_to_step_scheduled_transition() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
        assert_eq!(result.events_applied, 2);
    }

    // =========================================================================
    // Behavior 5: StepStarted maps to ExecuteStep
    // =========================================================================

    #[test]
    fn replay_maps_step_started_to_execute_step_transition() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::StepExecuting));
        assert_eq!(result.events_applied, 3);
    }

    // =========================================================================
    // Behavior 6: StepCompleted maps to CompleteStep
    // =========================================================================

    #[test]
    fn replay_maps_step_completed_to_complete_step_transition() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Completed));
        assert_eq!(result.events_applied, 4);
    }

    // =========================================================================
    // Behavior 7: StepFailed maps to Fail
    // =========================================================================

    #[test]
    fn replay_maps_step_failed_to_fail_transition() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Failed));
        assert_eq!(result.events_applied, 4);
    }

    // =========================================================================
    // Behavior 8: TimerSet maps to WaitForTimer
    // =========================================================================

    #[test]
    fn replay_maps_timer_set_to_wait_for_timer_transition() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, timer_set_payload("wf-1", "timer-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::WaitingForTimer));
        assert_eq!(result.events_applied, 4);
    }

    // =========================================================================
    // Behavior 9: TimerFired maps correctly
    // =========================================================================

    #[test]
    fn replay_maps_timer_fired_to_timer_fired_transition() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, timer_set_payload("wf-1", "timer-1")),
            make_event("inst-1", 5, timer_fired_payload("wf-1", "timer-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::StepExecuting));
        assert_eq!(result.events_applied, 5);
    }

    // =========================================================================
    // Behavior 10: Cancel transitions
    // =========================================================================

    #[test]
    fn replay_maps_workflow_cancelled_to_cancel_transition() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, workflow_cancelled_payload("wf-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Cancelled));
        assert_eq!(result.events_applied, 2);
    }

    #[test]
    fn replay_maps_cancel_requested_to_cancel_transition() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, cancel_requested_payload("wf-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Cancelled));
        assert_eq!(result.events_applied, 4);
    }

    // =========================================================================
    // Behavior 11: WorkflowFailed maps to Fail
    // =========================================================================

    #[test]
    fn replay_maps_workflow_failed_to_fail_transition() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, workflow_failed_payload("wf-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Failed));
        assert_eq!(result.events_applied, 2);
    }

    // =========================================================================
    // Behavior 12: InstanceResumed maps correctly
    // =========================================================================

    #[test]
    fn replay_maps_instance_resumed_to_instance_resumed_transition() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, workflow_failed_payload("wf-1")),
            make_event("inst-1", 3, instance_resumed_payload("wf-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
        assert_eq!(result.events_applied, 3);
    }

    // =========================================================================
    // Behavior 13: ContinuedAsNew is no-op
    // =========================================================================

    #[test]
    fn replay_treats_continued_as_new_as_noop() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, continued_as_new_payload("wf-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
        assert_eq!(result.events_applied, 2);
    }

    // =========================================================================
    // Behavior 14: Instance mismatch error
    // =========================================================================

    #[test]
    fn replay_returns_instance_mismatch_when_events_have_different_instance_ids() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-2", 2, step_scheduled_payload("wf-1", "step-1")),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert_eq!(
            err,
            ReplayError::InstanceMismatch {
                expected: "inst-1".to_string(),
                actual: "inst-2".to_string(),
            }
        );
    }

    // =========================================================================
    // Behavior 15: Sequence gap error
    // =========================================================================

    #[test]
    fn replay_returns_sequence_gap_when_sequence_numbers_have_gap() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 3, step_scheduled_payload("wf-1", "step-1")),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert_eq!(
            err,
            ReplayError::SequenceGap {
                expected: 2,
                actual: 3,
                at_index: 1,
            }
        );
    }

    // =========================================================================
    // Behavior 16: Sequence duplicate error
    // =========================================================================

    #[test]
    fn replay_returns_sequence_duplicate_when_duplicate_sequence_found() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 1, step_scheduled_payload("wf-1", "step-1")),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert_eq!(
            err,
            ReplayError::SequenceDuplicate {
                sequence: 1,
                first_at_index: 0,
                second_at_index: 1,
            }
        );
    }

    // =========================================================================
    // Behavior 17: Payload decode failure
    // =========================================================================

    #[test]
    fn replay_returns_payload_decode_failed_when_payload_is_invalid() {
        let engine = ReplayEngine::new();
        let events = [make_event("inst-1", 1, json!({"type": "UnknownType"}))];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            ReplayError::PayloadDecodeFailed { sequence: 1, .. }
        ));
    }

    // =========================================================================
    // Behavior 18: Transition failure
    // =========================================================================

    #[test]
    fn replay_returns_transition_failed_when_apply_rejects_transition() {
        let engine = ReplayEngine::new();
        // StepCompleted from Pending is invalid (no prior StepScheduled/StepStarted)
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_completed_payload("wf-1", "step-1")),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            ReplayError::TransitionFailed {
                sequence: 2,
                state: LifecycleState::RunningDecision,
                ..
            }
        ));
    }

    // =========================================================================
    // Behavior 20: Terminal state stops processing
    // =========================================================================

    #[test]
    fn replay_stops_processing_after_reaching_completed_state() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
            // This event should be ignored (terminal state reached)
            make_event("inst-1", 5, timer_set_payload("wf-1", "timer-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Completed));
        assert_eq!(result.events_applied, 4);
    }

    #[test]
    fn replay_stops_processing_after_reaching_cancelled_state() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, workflow_cancelled_payload("wf-1")),
            make_event("inst-1", 3, step_scheduled_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Cancelled));
        assert_eq!(result.events_applied, 2);
    }

    // =========================================================================
    // Behavior 21: Determinism
    // =========================================================================

    #[test]
    fn replay_is_deterministic_same_events_produce_same_result() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        ];
        let result1 = engine.replay(&events).expect("first replay");
        let result2 = engine.replay(&events).expect("second replay");
        assert_eq!(result1, result2);
    }

    // =========================================================================
    // Behavior 22: events_applied count
    // =========================================================================

    #[test]
    fn replay_reports_correct_events_applied_count() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.events_applied, 3);
    }

    // =========================================================================
    // Integration: Real EventEnvelope wiring
    // =========================================================================

    #[test]
    fn replay_works_with_real_event_envelope_serialization() {
        let engine = ReplayEngine::new();
        let json = serde_json::json!({
            "version": 1,
            "instance_id": "inst-real",
            "sequence": 1,
            "timestamp_ms": 1000,
            "payload": {
                "type": "WorkflowStarted",
                "workflow_id": "wf-real",
                "binary_hash": "sha256abc",
                "version": 1
            },
            "metadata": {}
        });
        let bytes = serde_json::to_vec(&json).expect("serialize");
        let envelope = EventEnvelope::from_bytes(&bytes).expect("parse envelope");
        let result = engine.replay(&[envelope]).expect("replay");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
        assert_eq!(result.events_applied, 1);
    }

    // =========================================================================
    // Full lifecycle integration test
    // =========================================================================

    #[test]
    fn replay_full_lifecycle_pending_to_completed() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Completed));
        assert_eq!(result.events_applied, 4);
    }

    #[test]
    fn replay_full_lifecycle_with_timer_round_trip() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, timer_set_payload("wf-1", "timer-1")),
            make_event("inst-1", 5, timer_fired_payload("wf-1", "timer-1")),
            make_event("inst-1", 6, step_completed_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Completed));
        assert_eq!(result.events_applied, 6);
    }

    #[test]
    fn replay_failure_recovery_cycle() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
            make_event("inst-1", 5, instance_resumed_payload("wf-1")),
            make_event("inst-1", 6, step_scheduled_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
        assert_eq!(result.events_applied, 6);
    }

    // =========================================================================
    // Kani harnesses
    // =========================================================================

    #[cfg(kani)]
    #[kani::proof]
    fn kani_replay_never_panics() {
        let engine = ReplayEngine::new();
        let seq: u64 = kani::any();
        if seq >= 1 {
            let event = EventEnvelope {
                schema_version: 1,
                instance_id: "inst-1".to_string(),
                sequence: seq,
                timestamp_ms: 1000,
                payload: json!({"type": "WorkflowStarted", "workflow_id": "wf-1", "binary_hash": "abc", "version": 1}),
                metadata: json!({}),
            };
            let _ = engine.replay(&[event]);
        }
    }

    #[cfg(kani)]
    #[kani::proof]
    fn kani_replay_determinism() {
        let engine = ReplayEngine::new();
        let event = EventEnvelope {
            schema_version: 1,
            instance_id: "inst-1".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: json!({"type": "WorkflowStarted", "workflow_id": "wf-1", "binary_hash": "abc", "version": 1}),
            metadata: json!({}),
        };
        let events = [event.clone(), event.clone()];
        // Clone is not available on EventEnvelope, so we construct two identical ones
        let r1 = engine.replay(&[EventEnvelope {
            schema_version: 1,
            instance_id: "inst-1".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: json!({"type": "WorkflowStarted", "workflow_id": "wf-1", "binary_hash": "abc", "version": 1}),
            metadata: json!({}),
        }]);
        let r2 = engine.replay(&[EventEnvelope {
            schema_version: 1,
            instance_id: "inst-1".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: json!({"type": "WorkflowStarted", "workflow_id": "wf-1", "binary_hash": "abc", "version": 1}),
            metadata: json!({}),
        }]);
        assert_eq!(r1, r2);
    }

    // =========================================================================
    // Proptest invariants
    // =========================================================================

    #[cfg(test)]
    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn replay_events_applied_never_exceeds_input_len(
                seq in 1u64..=10u64,
            ) {
                let engine = ReplayEngine::new();
                let event = make_event("inst-1", seq, workflow_started_payload("wf-1"));
                let events = vec![event];
                let result = engine.replay(&events).expect("replay");
                prop_assert!(result.events_applied <= events.len());
            }
        }

        #[test]
        fn replay_empty_always_returns_none_state() {
            let engine = ReplayEngine::new();
            let result = engine.replay(&[]).expect("replay");
            assert_eq!(result.final_state, None);
            assert_eq!(result.events_applied, 0);
        }
    }

    // =========================================================================
    // replay_with_upcaster tests
    // =========================================================================

    #[cfg(test)]
    mod replay_with_upcaster_tests {
        use super::*;
        use crate::upcaster::{Upcaster, UpcasterError, UpcasterRegistry};

        /// A simple upcaster that transforms version 0 JSON to version 1.
        struct Version0To1Upcaster;

        impl Upcaster for Version0To1Upcaster {
            fn source_version(&self) -> u8 {
                0
            }

            fn upcast(&self, input: &[u8]) -> Result<Vec<u8>, UpcasterError> {
                let mut value: serde_json::Value = serde_json::from_slice(input)
                    .map_err(|e| UpcasterError::UpcastingFailed(e.to_string()))?;
                value["version"] = serde_json::json!(1);
                serde_json::to_vec(&value)
                    .map_err(|e| UpcasterError::UpcastingFailed(e.to_string()))
            }
        }

        /// An upcaster that fails to parse its input.
        struct FailingUpcaster;

        impl Upcaster for FailingUpcaster {
            fn source_version(&self) -> u8 {
                0
            }

            fn upcast(&self, _input: &[u8]) -> Result<Vec<u8>, UpcasterError> {
                Err(UpcasterError::UpcastingFailed(
                    "cannot parse input JSON".to_string(),
                ))
            }
        }

        /// Helper to create an EventEnvelope at version 0
        fn make_v0_event(
            instance_id: &str,
            sequence: u64,
            payload: serde_json::Value,
        ) -> EventEnvelope {
            EventEnvelope {
                schema_version: 0,
                instance_id: instance_id.to_string(),
                sequence,
                timestamp_ms: 1000 * sequence,
                payload,
                metadata: json!({}),
            }
        }

        /// Helper to create a registry with a Version0To1Upcaster
        fn make_registry_with_upcaster() -> crate::upcaster::UpcasterRegistryImpl {
            let registry = crate::upcaster::UpcasterRegistryImpl::new(1);
            let _ = registry.register(Box::new(Version0To1Upcaster));
            registry
        }

        /// Helper to create a registry with a failing upcaster
        fn make_registry_with_failing_upcaster() -> crate::upcaster::UpcasterRegistryImpl {
            let registry = crate::upcaster::UpcasterRegistryImpl::new(1);
            let _ = registry.register(Box::new(FailingUpcaster));
            registry
        }

        // =====================================================================
        // Behavior: Empty event list with upcaster
        // =====================================================================

        #[test]
        fn replay_with_upcaster_returns_empty_result_when_event_list_is_empty() {
            let engine = ReplayEngine::new();
            let registry = make_registry_with_upcaster();
            let result = engine
                .replay_with_upcaster(&registry, &[])
                .expect("empty replay should succeed");
            assert_eq!(result.final_state, None);
            assert_eq!(result.events_applied, 0);
        }

        // =====================================================================
        // Behavior: Version 0 event is upcast to version 1 and replay succeeds
        // =====================================================================

        #[test]
        fn replay_with_upcaster_upcasts_v0_event_and_replays_successfully() {
            let engine = ReplayEngine::new();
            let registry = make_registry_with_upcaster();
            let events = [make_v0_event("inst-1", 1, workflow_started_payload("wf-1"))];
            let result = engine
                .replay_with_upcaster(&registry, &events)
                .expect("replay should succeed");
            assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
            assert_eq!(result.events_applied, 1);
        }

        // =====================================================================
        // Behavior: Full lifecycle with version 0 events upcast correctly
        // =====================================================================

        #[test]
        fn replay_with_upcaster_full_lifecycle_v0_to_v1() {
            let engine = ReplayEngine::new();
            let registry = make_registry_with_upcaster();
            let events = [
                make_v0_event("inst-1", 1, workflow_started_payload("wf-1")),
                make_v0_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
                make_v0_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
                make_v0_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
            ];
            let result = engine
                .replay_with_upcaster(&registry, &events)
                .expect("replay should succeed");
            assert_eq!(result.final_state, Some(LifecycleState::Completed));
            assert_eq!(result.events_applied, 4);
        }

        // =====================================================================
        // Behavior: Upcasting failure propagates as ReplayError::UpcastingFailed
        // =====================================================================

        #[test]
        fn replay_with_upcaster_returns_upcasting_failed_when_upcaster_errors() {
            let engine = ReplayEngine::new();
            let registry = make_registry_with_failing_upcaster();
            let events = [make_v0_event("inst-1", 1, workflow_started_payload("wf-1"))];
            let result = engine.replay_with_upcaster(&registry, &events);
            let err = result.expect_err("replay should fail");
            assert!(matches!(
                err,
                ReplayError::UpcastingFailed { sequence: 1, .. }
            ));
        }

        // =====================================================================
        // Behavior: Events already at max version pass through unchanged
        // =====================================================================

        #[test]
        fn replay_with_upcaster_preserves_v1_events() {
            let engine = ReplayEngine::new();
            let registry = make_registry_with_upcaster();
            let events = [make_event("inst-1", 1, workflow_started_payload("wf-1"))];
            let result = engine
                .replay_with_upcaster(&registry, &events)
                .expect("replay should succeed");
            assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
            assert_eq!(result.events_applied, 1);
        }

        // =====================================================================
        // Behavior: Mixed version events all get upcast
        // =====================================================================

        #[test]
        fn replay_with_upcaster_handles_mixed_version_events() {
            let engine = ReplayEngine::new();
            let registry = make_registry_with_upcaster();
            let events = [
                make_v0_event("inst-1", 1, workflow_started_payload("wf-1")),
                make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
                make_v0_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
                make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
            ];
            let result = engine
                .replay_with_upcaster(&registry, &events)
                .expect("replay should succeed");
            assert_eq!(result.final_state, Some(LifecycleState::Completed));
            assert_eq!(result.events_applied, 4);
        }
    }
}
