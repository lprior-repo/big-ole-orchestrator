//! Stateless, deterministic replay engine (ADR-027).
//!
//! Applies events through the pure `apply()` state machine to reconstruct
//! `LifecycleState` from an event history.

use vo_types::events::{EventEnvelope, EventPayload};
use vo_types::state::{self, LifecycleState, TransitionEvent};

use crate::upcaster::UpcasterRegistry;

use super::types::{ReplayError, ReplayResult};

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
                position: super::types::ReplayPosition {
                    last_applied_sequence: None,
                    last_applied_timestamp_ms: None,
                },
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
            let next_expected = expected_seq.checked_add(1).ok_or_else(|| ReplayError::SequenceGap {
                    expected: 0,
                    actual: event.sequence,
                    at_index: i,
                })?;
            if event.sequence != next_expected {
                return Err(ReplayError::SequenceGap {
                    expected: next_expected,
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

            // EffectPrepared and EffectCommitted are managed effect lifecycle events.
            // They are checkpoint markers in the ADR-027 managed effect sequence:
            // StepScheduled -> StepStarted -> EffectPrepared -> EffectCommitted -> StepCompleted
            // Count them as applied but do not change state.
            // WorkflowQuarantined is an operational circuit breaker event (ADR-026),
            // not a state machine transition.
            if matches!(
                payload,
                EventPayload::EffectPrepared { .. }
                    | EventPayload::EffectCommitted { .. }
                    | EventPayload::WorkflowQuarantined { .. }
            ) {
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

        let last_applied_sequence = if events_applied > 0 {
            Some(events[events_applied - 1].sequence)
        } else {
            None
        };
        let last_applied_timestamp_ms = if events_applied > 0 {
            Some(events[events_applied - 1].timestamp_ms)
        } else {
            None
        };

        Ok(ReplayResult {
            final_state: current_state,
            events_applied,
            position: super::types::ReplayPosition {
                last_applied_sequence,
                last_applied_timestamp_ms,
            },
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
                position: super::types::ReplayPosition {
                    last_applied_sequence: None,
                    last_applied_timestamp_ms: None,
                },
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
pub(super) fn payload_to_transition(
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
        EventPayload::WorkflowQuarantined { .. } => Ok(TransitionEvent::Fail),
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
        EventPayload::EffectPrepared { .. } => {
            // Handled as a no-op in the replay loop before calling this function.
            // This branch should never be reached.
            Err(ReplayError::UnexpectedEventType {
                payload_type: "EffectPrepared".to_string(),
                sequence,
            })
        }
        EventPayload::EffectCommitted { .. } => {
            // Handled as a no-op in the replay loop before calling this function.
            // This branch should never be reached.
            Err(ReplayError::UnexpectedEventType {
                payload_type: "EffectCommitted".to_string(),
                sequence,
            })
        }
    }
}
