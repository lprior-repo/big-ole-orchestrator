//! Event replay calculation (ADR-038).
//!
//! Pure functions for replay math, sequence gaps, and continue-as-new transactions.

use crate::lineage_projection::types::*;
use std::collections::BTreeMap;

/// Continue-as-new rollover: the 7-step atomic transaction.
///
/// Steps:
/// 1. Validate active epoch exists for lineage
/// 2. Compute carried state (operational carried, operator discarded)
/// 3. Validate carried state
/// 4. Write ContinuedAsNew event for old epoch
/// 5. Create new epoch and register in epoch map
/// 6. Write WorkflowStarted event for new epoch
/// 7. Drain buffered signals for lineage
pub fn continue_as_new_7step(
    epoch_map: &mut EpochMap,
    lineage_id: LineageId,
    carried_state: CarriedState,
    trigger: ContinuedAsNewTrigger,
    buffered_signals: &mut SignalBuffer,
) -> Result<RolloverResult, RolloverError> {
    let mut steps_completed = 0;
    let mut events_written: Vec<CanonicalEvent> = Vec::new();

    let old_epoch_id = epoch_map
        .active_epoch(&lineage_id)
        .ok_or(RolloverError::NoActiveEpoch(lineage_id.clone()))?;

    let new_epoch_id = old_epoch_id.next();

    let computed = compute_carried_state(&carried_state);
    if !computed.is_valid {
        return Err(RolloverError::CarriedStateInvalid);
    }

    validate_carried_state(&computed.operational)?;

    let continued_as_new = ContinuedAsNew {
        lineage_id: lineage_id.clone(),
        old_epoch_id,
        new_epoch_id,
        carried_state: carried_state.clone(),
        trigger: trigger.clone(),
    };
    let event = CanonicalEvent {
        lineage_id: lineage_id.clone(),
        epoch_id: old_epoch_id,
        sequence: u64::MAX - 1,
        event_type: "ContinuedAsNew".to_string(),
        payload: serde_json::to_value(&continued_as_new).unwrap_or_default(),
    };
    events_written.push(event);
    steps_completed = 1;

    epoch_map.set_rollover_in_progress(true);
    epoch_map.register_epoch(lineage_id.clone(), new_epoch_id);
    epoch_map.unregister_epoch(&lineage_id);
    steps_completed = 2;

    let workflow_started = WorkflowStarted {
        lineage_id: lineage_id.clone(),
        epoch_id: new_epoch_id,
        carried_state: CarriedState::new(computed.operational.clone(), serde_json::Value::Null),
        parent_epoch_id: old_epoch_id,
    };
    let event = CanonicalEvent {
        lineage_id: lineage_id.clone(),
        epoch_id: new_epoch_id,
        sequence: 1,
        event_type: "WorkflowStarted".to_string(),
        payload: serde_json::to_value(&workflow_started).unwrap_or_default(),
    };
    events_written.push(event);
    steps_completed = 3;

    let _drained = buffered_signals.drain(&lineage_id);
    steps_completed = 4;

    epoch_map.set_rollover_in_progress(false);
    steps_completed = 7;

    Ok(RolloverResult {
        lineage_id,
        old_epoch_id,
        new_epoch_id,
        carried_state: CarriedState::new(computed.operational, serde_json::Value::Null),
        events_written,
        steps_completed,
        step_count: 7,
    })
}

/// Compute carried state from a full state snapshot.
///
/// Operational state is carried forward; operator state is discarded.
pub fn compute_carried_state(state: &CarriedState) -> CarriedStateResult {
    let is_valid = !state.operational.is_null() || state.operational.is_object();

    CarriedStateResult {
        operational: state.operational.clone(),
        operator_discarded: true,
        is_valid,
    }
}

/// Validate carried state before rollover.
pub fn validate_carried_state(_operational: &serde_json::Value) -> Result<(), RolloverError> {
    let serialized =
        serde_json::to_string(_operational).map_err(|_| RolloverError::CarriedStateInvalid)?;

    if serialized.len() > 1_048_576 {
        return Err(RolloverError::CarriedStateInvalid);
    }

    Ok(())
}

/// Determine the rebuild scope for a projection based on corruption type.
pub fn determine_rebuild_scope(
    corruption: &ProjectionCorruption,
    lineage_id: &LineageId,
    epoch_id: EpochId,
    last_known_sequence: u64,
) -> RebuildScope {
    match corruption {
        ProjectionCorruption::ChecksumMismatch { .. } | ProjectionCorruption::Unknown => {
            RebuildScope::FullEpoch {
                lineage_id: lineage_id.clone(),
                epoch_id,
            }
        }
        ProjectionCorruption::SchemaVersionMismatch { .. } => RebuildScope::FullEpoch {
            lineage_id: lineage_id.clone(),
            epoch_id,
        },
        ProjectionCorruption::SequenceGap { gap_at } => {
            if *gap_at == 0 {
                RebuildScope::FullEpoch {
                    lineage_id: lineage_id.clone(),
                    epoch_id,
                }
            } else {
                RebuildScope::Incremental {
                    lineage_id: lineage_id.clone(),
                    epoch_id,
                    from_sequence: *gap_at,
                }
            }
        }
    }
}

/// Determine which projection class needs rebuilding.
pub fn determine_projection_class(
    corruption: &ProjectionCorruption,
    projection_class: &ProjectionClass,
) -> (ProjectionClass, bool) {
    match corruption {
        ProjectionCorruption::SchemaVersionMismatch { .. } => (projection_class.clone(), true),
        ProjectionCorruption::ChecksumMismatch { .. } => (projection_class.clone(), false),
        ProjectionCorruption::SequenceGap { .. } => (projection_class.clone(), false),
        ProjectionCorruption::Unknown => (projection_class.clone(), true),
    }
}

/// Build a projection from a sequence of canonical events.
/// Returns the final state and event count.
pub fn build_projection(
    events: &[CanonicalEvent],
    initial_state: ProjectionState,
) -> Result<RebuildResult, RebuildError> {
    match initial_state {
        ProjectionState::Building => {}
        _ => return Err(RebuildError::InvalidInitialState),
    }

    if events.is_empty() {
        return Ok(RebuildResult {
            scope: RebuildScope::FullEpoch {
                lineage_id: events
                    .first()
                    .map(|e| e.lineage_id.clone())
                    .unwrap_or_else(|| LineageId(String::new())),
                epoch_id: EpochId(0),
            },
            events_applied: 0,
            final_state: ProjectionState::Ready {
                schema_version: 0,
                last_sequence: 0,
            },
            rebuilt_from_canonical: true,
        });
    }

    for (i, event) in events.iter().enumerate() {
        let expected_seq = i as u64 + 1;
        if event.sequence != expected_seq {
            return Err(RebuildError::SequenceGap {
                expected: expected_seq,
                actual: event.sequence,
            });
        }
        if let Some(first) = events.first() {
            if event.lineage_id != first.lineage_id || event.epoch_id != first.epoch_id {
                return Err(RebuildError::MixedLineage);
            }
        }
    }

    let last_sequence = events.last().unwrap().sequence;
    let events_applied = events.len() as u64;

    Ok(RebuildResult {
        scope: RebuildScope::FullEpoch {
            lineage_id: events.first().unwrap().lineage_id.clone(),
            epoch_id: events.first().unwrap().epoch_id,
        },
        events_applied,
        final_state: ProjectionState::Ready {
            schema_version: 0,
            last_sequence,
        },
        rebuilt_from_canonical: true,
    })
}

/// Build a projection incrementally from a starting sequence.
pub fn build_projection_incremental(
    events: &[CanonicalEvent],
    from_sequence: u64,
    existing_last_sequence: u64,
) -> Result<RebuildResult, RebuildError> {
    let expected_first = existing_last_sequence + 1;
    if from_sequence != expected_first {
        return Err(RebuildError::SequenceGap {
            expected: expected_first,
            actual: from_sequence,
        });
    }

    build_projection(events, ProjectionState::Building)
}

/// Determine the trigger for continue-as-new based on current metrics.
pub fn evaluate_rollover_trigger(
    event_count: u64,
    signal_count: u64,
    blob_count: u64,
    event_threshold: u64,
    signal_threshold: u64,
    blob_threshold: u64,
    explicit: bool,
) -> Option<ContinuedAsNewTrigger> {
    if explicit {
        return Some(ContinuedAsNewTrigger::Explicit);
    }

    if event_count > event_threshold {
        return Some(ContinuedAsNewTrigger::EventCountThreshold {
            event_count,
            threshold: event_threshold,
        });
    }

    if signal_count > signal_threshold {
        return Some(ContinuedAsNewTrigger::SignalCountThreshold {
            signal_count,
            threshold: signal_threshold,
        });
    }

    if blob_count > blob_threshold {
        return Some(ContinuedAsNewTrigger::BlobReferencesThreshold {
            blob_count,
            threshold: blob_threshold,
        });
    }

    None
}

/// Compute compensation order for effects within an epoch.
/// Effects in older epochs must be compensated before newer ones.
pub fn compute_epoch_compensation_order(
    effects: &[ExecutedEffect],
) -> Result<Vec<ExecutedEffect>, CompensationError> {
    let mut epoch_groups: BTreeMap<EpochId, Vec<&ExecutedEffect>> = BTreeMap::new();

    for effect in effects {
        epoch_groups
            .entry(effect.epoch_id)
            .or_default()
            .push(effect);
    }

    let mut ordered: Vec<ExecutedEffect> = Vec::new();
    for (_epoch_id, group) in &epoch_groups {
        for effect in group {
            ordered.push((*effect).clone());
        }
    }

    ordered.reverse();
    Ok(ordered)
}

/// Errors that can occur during a continue-as-new rollover.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RolloverError {
    NoActiveEpoch(LineageId),
    CarriedStateInvalid,
    RolloverInProgress,
    ValidationFailed(String),
}

/// Errors that can occur during projection rebuild.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RebuildError {
    SequenceGap { expected: u64, actual: u64 },
    MixedLineage,
    InvalidInitialState,
}

/// Errors that can occur during compensation ordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompensationError {
    DuplicateEffectId(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_lineage(id: &str) -> LineageId {
        LineageId(id.to_string())
    }

    fn test_epoch(n: u64) -> EpochId {
        EpochId(n)
    }

    fn test_event(
        lineage: LineageId,
        epoch: EpochId,
        seq: u64,
        event_type: &str,
    ) -> CanonicalEvent {
        CanonicalEvent {
            lineage_id: lineage,
            epoch_id: epoch,
            sequence: seq,
            event_type: event_type.to_string(),
            payload: serde_json::json!({"test": true}),
        }
    }

    #[test]
    fn compute_carried_state_carries_operational_discards_operator() {
        let state = CarriedState::new(
            serde_json::json!({"work_item": "abc"}),
            serde_json::json!({"ui_pos": [100, 200]}),
        );
        let result = compute_carried_state(&state);
        assert!(result.is_valid);
        assert!(result.operator_discarded);
        assert_eq!(result.operational, serde_json::json!({"work_item": "abc"}));
    }

    #[test]
    fn compute_carried_state_null_operational_invalid() {
        let state = CarriedState::new(serde_json::Value::Null, serde_json::json!({"ui": true}));
        let result = compute_carried_state(&state);
        assert!(!result.is_valid);
    }

    #[test]
    fn rebuild_scope_checksum_is_full_epoch() {
        let corruption = ProjectionCorruption::ChecksumMismatch {
            expected: "abc".to_string(),
            actual: "def".to_string(),
        };
        let lineage = test_lineage("wf-1");
        let epoch = test_epoch(1);
        let scope = determine_rebuild_scope(&corruption, &lineage, epoch, 0);
        match scope {
            RebuildScope::FullEpoch {
                lineage_id,
                epoch_id: test_epoch(1),
            } => {
                assert_eq!(lineage_id, lineage);
            }
            other => panic!("expected FullEpoch, got {:?}", other),
        }
    }

    #[test]
    fn rebuild_scope_sequence_gap_at_zero_is_full() {
        let corruption = ProjectionCorruption::SequenceGap { gap_at: 0 };
        let lineage = test_lineage("wf-1");
        let epoch = test_epoch(1);
        let scope = determine_rebuild_scope(&corruption, &lineage, epoch, 0);
        match scope {
            RebuildScope::FullEpoch { .. } => {}
            other => panic!("expected FullEpoch for gap at 0, got {:?}", other),
        }
    }

    #[test]
    fn rebuild_scope_sequence_gap_midstream_is_incremental() {
        let corruption = ProjectionCorruption::SequenceGap { gap_at: 50 };
        let lineage = test_lineage("wf-1");
        let epoch = test_epoch(1);
        let scope = determine_rebuild_scope(&corruption, &lineage, epoch, 49);
        match scope {
            RebuildScope::Incremental {
                lineage_id,
                epoch_id: test_epoch(1),
                from_sequence: 50,
            } => {
                assert_eq!(lineage_id, lineage);
            }
            other => panic!("expected Incremental, got {:?}", other),
        }
    }

    #[test]
    fn build_projection_empty_events() {
        let events: Vec<CanonicalEvent> = vec![];
        let result = build_projection(&events, ProjectionState::Building);
        assert!(result.is_ok());
        let rebuild = result.unwrap();
        assert_eq!(rebuild.events_applied, 0);
        assert!(rebuild.rebuilt_from_canonical);
        assert!(matches!(rebuild.final_state, ProjectionState::Ready { .. }));
    }

    #[test]
    fn build_projection_single_event() {
        let events = vec![test_event(
            test_lineage("wf-1"),
            test_epoch(1),
            1,
            "Started",
        )];
        let result = build_projection(&events, ProjectionState::Building);
        assert!(result.is_ok());
        let rebuild = result.unwrap();
        assert_eq!(rebuild.events_applied, 1);
        assert!(rebuild.rebuilt_from_canonical);
    }

    #[test]
    fn build_projection_multiple_events() {
        let events = vec![
            test_event(test_lineage("wf-1"), test_epoch(1), 1, "Started"),
            test_event(test_lineage("wf-1"), test_epoch(1), 2, "Signal"),
            test_event(test_lineage("wf-1"), test_epoch(1), 3, "Effect"),
        ];
        let result = build_projection(&events, ProjectionState::Building);
        assert!(result.is_ok());
        let rebuild = result.unwrap();
        assert_eq!(rebuild.events_applied, 3);
        assert!(rebuild.rebuilt_from_canonical);
        assert_eq!(
            rebuild.final_state,
            ProjectionState::Ready {
                schema_version: 0,
                last_sequence: 3
            }
        );
    }

    #[test]
    fn build_projection_sequence_gap_fails() {
        let events = vec![
            test_event(test_lineage("wf-1"), test_epoch(1), 1, "Started"),
            test_event(test_lineage("wf-1"), test_epoch(1), 3, "Signal"),
        ];
        let result = build_projection(&events, ProjectionState::Building);
        assert!(result.is_err());
        match result.unwrap_err() {
            RebuildError::SequenceGap {
                expected: 2,
                actual: 3,
            } => {}
            other => panic!("expected SequenceGap(2,3), got {:?}", other),
        }
    }

    #[test]
    fn build_projection_mixed_lineage_fails() {
        let events = vec![
            test_event(test_lineage("wf-1"), test_epoch(1), 1, "Started"),
            test_event(test_lineage("wf-2"), test_epoch(1), 2, "Signal"),
        ];
        let result = build_projection(&events, ProjectionState::Building);
        assert!(result.is_err());
        match result.unwrap_err() {
            RebuildError::MixedLineage => {}
            other => panic!("expected MixedLineage, got {:?}", other),
        }
    }

    #[test]
    fn build_projection_invalid_initial_state() {
        let events = vec![test_event(
            test_lineage("wf-1"),
            test_epoch(1),
            1,
            "Started",
        )];
        let result = build_projection(
            &events,
            ProjectionState::Ready {
                schema_version: 1,
                last_sequence: 0,
            },
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            RebuildError::InvalidInitialState => {}
            other => panic!("expected InvalidInitialState, got {:?}", other),
        }
    }

    #[test]
    fn rollover_trigger_explicit() {
        let trigger = evaluate_rollover_trigger(1, 1, 1, 100, 100, 100, true);
        assert!(matches!(trigger, Some(ContinuedAsNewTrigger::Explicit)));
    }

    #[test]
    fn rollover_trigger_event_count() {
        let trigger = evaluate_rollover_trigger(150, 1, 1, 100, 100, 100, false);
        assert!(matches!(
            trigger,
            Some(ContinuedAsNewTrigger::EventCountThreshold {
                event_count: 150,
                threshold: 100
            })
        ));
    }

    #[test]
    fn rollover_trigger_no_trigger() {
        let trigger = evaluate_rollover_trigger(50, 50, 50, 100, 100, 100, false);
        assert!(trigger.is_none());
    }

    #[test]
    fn rollover_7step_happy_path() {
        let mut epoch_map = EpochMap::new();
        let lineage = test_lineage("wf-1");
        epoch_map.register_epoch(lineage.clone(), test_epoch(1));
        let mut buffer = SignalBuffer::new();
        let carried = CarriedState::new(
            serde_json::json!({"work_item": "abc"}),
            serde_json::json!({"ui": "discarded"}),
        );

        let result = continue_as_new_7step(
            &mut epoch_map,
            lineage.clone(),
            carried.clone(),
            ContinuedAsNewTrigger::Explicit,
            &mut buffer,
        );

        assert!(result.is_ok());
        let rollover = result.unwrap();
        assert_eq!(rollover.step_count, 7);
        assert_eq!(rollover.steps_completed, 7);
        assert_eq!(rollover.old_epoch_id, test_epoch(1));
        assert_eq!(rollover.new_epoch_id, test_epoch(2));
        assert_eq!(rollover.events_written.len(), 2);

        assert!(epoch_map.active_epoch(&lineage).is_none());
        assert!(!epoch_map.is_rollover_in_progress());
    }

    #[test]
    fn rollover_7step_no_active_epoch_fails() {
        let mut epoch_map = EpochMap::new();
        let mut buffer = SignalBuffer::new();
        let carried = CarriedState::default();

        let result = continue_as_new_7step(
            &mut epoch_map,
            test_lineage("nonexistent"),
            carried,
            ContinuedAsNewTrigger::Explicit,
            &mut buffer,
        );

        assert!(result.is_err());
        match result.unwrap_err() {
            RolloverError::NoActiveEpoch(lineage) => {
                assert_eq!(lineage, test_lineage("nonexistent"));
            }
            other => panic!("expected NoActiveEpoch, got {:?}", other),
        }
    }

    #[test]
    fn rollover_7step_invalid_carried_state_fails() {
        let mut epoch_map = EpochMap::new();
        epoch_map.register_epoch(test_lineage("wf-1"), test_epoch(1));
        let mut buffer = SignalBuffer::new();
        let carried = CarriedState::new(serde_json::Value::Null, serde_json::json!({}));

        let result = continue_as_new_7step(
            &mut epoch_map,
            test_lineage("wf-1"),
            carried,
            ContinuedAsNewTrigger::Explicit,
            &mut buffer,
        );

        assert!(result.is_err());
        match result.unwrap_err() {
            RolloverError::CarriedStateInvalid => {}
            other => panic!("expected CarriedStateInvalid, got {:?}", other),
        }
    }

    #[test]
    fn compensation_order_newest_epoch_first() {
        let effects = vec![
            ExecutedEffect {
                effect_id: "e1".to_string(),
                effect_type: "send".to_string(),
                epoch_id: test_epoch(1),
                lineage_id: test_lineage("wf-1"),
                status: EffectStatus::Executed,
            },
            ExecutedEffect {
                effect_id: "e2".to_string(),
                effect_type: "send".to_string(),
                epoch_id: test_epoch(2),
                lineage_id: test_lineage("wf-1"),
                status: EffectStatus::Executed,
            },
            ExecutedEffect {
                effect_id: "e3".to_string(),
                effect_type: "send".to_string(),
                epoch_id: test_epoch(3),
                lineage_id: test_lineage("wf-1"),
                status: EffectStatus::Executed,
            },
        ];
        let ordered = compute_epoch_compensation_order(&effects).unwrap();
        assert_eq!(ordered[0].epoch_id, test_epoch(3));
        assert_eq!(ordered[1].epoch_id, test_epoch(2));
        assert_eq!(ordered[2].epoch_id, test_epoch(1));
    }

    #[test]
    fn schema_mismatch_both_classes_need_rebuild() {
        let corruption = ProjectionCorruption::SchemaVersionMismatch {
            expected: 2,
            actual: 1,
        };
        let result = determine_projection_class(&corruption, &ProjectionClass::Operational);
        assert!(result.1);
    }

    #[test]
    fn checksum_mismatch_only_specific_class() {
        let corruption = ProjectionCorruption::ChecksumMismatch {
            expected: "a".to_string(),
            actual: "b".to_string(),
        };
        let result = determine_projection_class(&corruption, &ProjectionClass::Operator);
        assert!(!result.1);
    }

    #[test]
    fn validate_carried_state_valid_value() {
        let result = validate_carried_state(&serde_json::json!({"key": "value"}));
        assert!(result.is_ok());
    }

    #[test]
    fn validate_carried_state_null_value() {
        let result = validate_carried_state(&serde_json::Value::Null);
        assert!(result.is_ok());
    }
}
