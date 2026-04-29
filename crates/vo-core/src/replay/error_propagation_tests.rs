//! Error propagation tests for vo-core (Behaviors EP-01 through EP-15).
//!
//! Tests that errors correctly propagate and convert across all layers:
//! - ReplayEngine -> ReplayError
//! - ProjectionError conversions from StorageError, ReplayError
//! - UpcasterError -> ReplayError::UpcastFailed
//! - State machine errors -> ReplayError::TransitionFailed

use crate::replay::engine::ReplayEngine;
use crate::replay::projection::error::{
    ProjectionError, ProjectionStateError, ProjectionVersionError, StorageError,
};
use crate::replay::test_helpers::*;
use crate::replay::types::ReplayError;
use vo_types::state::LifecycleState;

// =========================================================================
// EP-01: ReplayError::InstanceMismatch propagates correctly
// =========================================================================

#[test]
fn ep_01_instance_mismatch_error_contains_correct_fields() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-2", 2, step_scheduled_payload("wf-1", "step-1")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    assert!(matches!(
        err,
        ReplayError::InstanceMismatch {
            expected: _,
            actual: _
        }
    ));
}

#[test]
fn ep_01_instance_mismatch_error_message_is_descriptive() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-2", 2, step_scheduled_payload("wf-1", "step-1")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    let display = format!("{}", err);
    assert!(display.contains("inst-1"));
    assert!(display.contains("inst-2"));
}

// =========================================================================
// EP-02: ReplayError::SequenceGap propagates correctly
// =========================================================================

#[test]
fn ep_02_sequence_gap_error_contains_gap_details() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 3, step_scheduled_payload("wf-1", "step-1")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    assert!(matches!(
        err,
        ReplayError::SequenceGap {
            expected: _,
            actual: _,
            at_index: _
        }
    ));
}

#[test]
fn ep_02_sequence_gap_error_shows_expected_vs_actual() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 5, step_scheduled_payload("wf-1", "step-1")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    let display = format!("{}", err);
    assert!(display.contains("2")); // expected
    assert!(display.contains("5")); // actual
}

// =========================================================================
// EP-03: ReplayError::SequenceDuplicate propagates correctly
// =========================================================================

#[test]
fn ep_03_sequence_duplicate_error_identifies_both_indices() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 1, step_scheduled_payload("wf-1", "step-1")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    assert!(matches!(
        err,
        ReplayError::SequenceDuplicate {
            sequence: 1,
            first_at_index: _,
            second_at_index: _
        }
    ));
}

// =========================================================================
// EP-04: ReplayError::PayloadDecodeFailed propagates correctly
// =========================================================================

#[test]
fn ep_04_payload_decode_error_contains_sequence() {
    let engine = ReplayEngine::new();
    let events = [make_event(
        "inst-1",
        1,
        serde_json::json!({"type": "UnknownType"}),
    )];
    let err = engine.replay(&events).expect_err("should fail");
    assert!(matches!(
        err,
        ReplayError::PayloadDecodeFailed { sequence: 1, .. }
    ));
}

#[test]
fn ep_04_payload_decode_error_preserves_source_message() {
    let engine = ReplayEngine::new();
    let events = [make_event(
        "inst-1",
        1,
        serde_json::json!({"type": "UnknownType"}),
    )];
    let err = engine.replay(&events).expect_err("should fail");
    let display = format!("{}", err);
    assert!(display.contains("sequence 1"));
}

// =========================================================================
// EP-05: ReplayError::TransitionFailed propagates with state context
// =========================================================================

#[test]
fn ep_05_transition_failed_error_contains_sequence_and_state() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_completed_payload("wf-1", "step-1")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    assert!(matches!(
        err,
        ReplayError::TransitionFailed {
            sequence: 2,
            state: _,
            ..
        }
    ));
}

#[test]
fn ep_05_transition_failed_error_shows_invalid_transition() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_completed_payload("wf-1", "step-1")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    let display = format!("{}", err);
    assert!(display.contains("sequence 2"));
    assert!(display.contains("RunningDecision")); // Invalid from RunningDecision - CompleteStep not allowed
}

// =========================================================================
// EP-06: ReplayError::UnexpectedEventType propagates for no-op events
// =========================================================================

#[test]
fn ep_06_continued_as_new_is_noop_in_replay() {
    let engine = ReplayEngine::new();
    use vo_types::events::EventEnvelope;
    let continued_as_new_event = EventEnvelope {
        schema_version: 1,
        instance_id: "inst-1".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({
            "type": "ContinuedAsNew",
            "workflow_id": "wf-1",
            "lineage_id": "lin-1",
            "old_epoch": 1,
            "new_epoch": 2,
            "version": 1
        }),
        metadata: Default::default(),
    };
    let events = [continued_as_new_event];
    let result = engine
        .replay(&events)
        .expect("ContinuedAsNew should be no-op");
    assert_eq!(result.events_applied, 1);
    assert_eq!(result.final_state, None);
}

// =========================================================================
// EP-07: StorageError -> ProjectionError conversion via From trait
// =========================================================================

#[test]
fn ep_07_storage_error_record_not_found_converts_to_projection_not_found() {
    let storage_err = StorageError::RecordNotFound("test-projection".to_string());
    let projection_err: ProjectionError = storage_err.into();
    assert!(matches!(
        projection_err,
        ProjectionError::ProjectionNotFound(id) if id == "test-projection"
    ));
}

#[test]
fn ep_07_storage_error_serialization_failed_converts_to_storage() {
    let storage_err = StorageError::SerializationFailed("json error".to_string());
    let projection_err: ProjectionError = storage_err.into();
    assert!(matches!(
        projection_err,
        ProjectionError::Storage(msg) if msg.contains("json error")
    ));
}

#[test]
fn ep_07_storage_error_deserialization_failed_converts_to_storage() {
    let storage_err = StorageError::DeserializationFailed("invalid bytes".to_string());
    let projection_err: ProjectionError = storage_err.into();
    assert!(matches!(
        projection_err,
        ProjectionError::Storage(msg) if msg.contains("invalid bytes")
    ));
}

#[test]
fn ep_07_storage_error_corrupt_record_converts_to_storage() {
    let storage_err = StorageError::CorruptRecord("checksum mismatch".to_string());
    let projection_err: ProjectionError = storage_err.into();
    assert!(matches!(
        projection_err,
        ProjectionError::Storage(msg) if msg.contains("checksum mismatch")
    ));
}

#[test]
fn ep_07_storage_error_write_failed_converts_to_storage() {
    let storage_err = StorageError::WriteFailed("io error".to_string());
    let projection_err: ProjectionError = storage_err.into();
    assert!(matches!(
        projection_err,
        ProjectionError::Storage(msg) if msg.contains("io error")
    ));
}

#[test]
fn ep_07_storage_error_batch_full_converts_to_storage() {
    let storage_err = StorageError::BatchFull {
        class: "test".to_string(),
        depth: 10,
        capacity: 5,
    };
    let projection_err: ProjectionError = storage_err.into();
    assert!(matches!(projection_err, ProjectionError::Storage(_)));
}

#[test]
fn ep_07_storage_error_budget_exceeded_converts_to_storage() {
    let storage_err = StorageError::BudgetExceeded {
        class: "test".to_string(),
        item_size: 1000,
        remaining: 500,
    };
    let projection_err: ProjectionError = storage_err.into();
    assert!(matches!(projection_err, ProjectionError::Storage(_)));
}

// =========================================================================
// EP-08: ProjectionError::is_retryable correctly identifies retryable errors
// =========================================================================

#[test]
fn ep_08_projection_error_throttle_exceeded_is_retryable() {
    let err = ProjectionError::ThrottleExceeded(100);
    assert!(err.is_retryable());
}

#[test]
fn ep_08_projection_error_concurrency_conflict_is_retryable() {
    let err = ProjectionError::ConcurrencyConflict("lock timeout".to_string());
    assert!(err.is_retryable());
}

#[test]
fn ep_08_projection_error_storage_is_retryable() {
    let err = ProjectionError::Storage("io error".to_string());
    assert!(err.is_retryable());
}

#[test]
fn ep_08_projection_error_projection_not_found_is_not_retryable() {
    let err = ProjectionError::ProjectionNotFound("test".to_string());
    assert!(!err.is_retryable());
}

#[test]
fn ep_08_projection_error_build_failed_is_not_retryable() {
    let err = ProjectionError::BuildFailed("test".to_string());
    assert!(!err.is_retryable());
}

#[test]
fn ep_08_projection_error_incompatible_schema_is_not_retryable() {
    let err = ProjectionError::IncompatibleSchemaVersion {
        expected: 1,
        actual: 2,
    };
    assert!(!err.is_retryable());
}

// =========================================================================
// EP-09: ProjectionStateError variants have correct error messages
// =========================================================================

#[test]
fn ep_09_projection_state_error_invalid_transition_message() {
    let err = ProjectionStateError::InvalidTransition {
        from: "Ready".to_string(),
        to: "Building".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("Ready"));
    assert!(display.contains("Building"));
}

#[test]
fn ep_09_projection_state_error_unexpected_state_message() {
    let err = ProjectionStateError::UnexpectedState("Failed".to_string());
    let display = format!("{}", err);
    assert!(display.contains("Failed"));
}

#[test]
fn ep_09_projection_state_error_still_building_message() {
    let err = ProjectionStateError::StillBuilding;
    let display = format!("{}", err);
    assert!(display.contains("building"));
}

#[test]
fn ep_09_projection_state_error_still_rebuilding_message() {
    let err = ProjectionStateError::StillRebuilding;
    let display = format!("{}", err);
    assert!(display.contains("rebuilding"));
}

// =========================================================================
// EP-10: ProjectionVersionError variants have correct error messages
// =========================================================================

#[test]
fn ep_10_projection_version_error_stale_version_message() {
    let err = ProjectionVersionError::StaleVersion(1);
    let display = format!("{}", err);
    assert!(display.contains("1"));
    assert!(display.contains("stale"));
}

#[test]
fn ep_10_projection_version_error_exceeds_max_supported_message() {
    let err = ProjectionVersionError::ExceedsMaxSupported(10, 5);
    let display = format!("{}", err);
    assert!(display.contains("10"));
    assert!(display.contains("5"));
}

#[test]
fn ep_10_projection_version_error_invalid_version_message() {
    let err = ProjectionVersionError::InvalidVersion(0);
    let display = format!("{}", err);
    assert!(display.contains("0"));
    assert!(display.contains("invalid"));
}

#[test]
fn ep_10_projection_version_error_missing_upcaster_message() {
    let err = ProjectionVersionError::MissingUpcaster(3);
    let display = format!("{}", err);
    assert!(display.contains("3"));
    assert!(display.contains("missing"));
}

// =========================================================================
// EP-11: ProjectionError variants have correct error messages
// =========================================================================

#[test]
fn ep_11_projection_error_not_found_message() {
    let err = ProjectionError::ProjectionNotFound("my-projection".to_string());
    let display = format!("{}", err);
    assert!(display.contains("my-projection"));
}

#[test]
fn ep_11_projection_error_already_exists_message() {
    let err = ProjectionError::ProjectionAlreadyExists("my-projection".to_string());
    let display = format!("{}", err);
    assert!(display.contains("my-projection"));
}

#[test]
fn ep_11_projection_error_invalid_state_message() {
    let err = ProjectionError::InvalidState("lock poisoned".to_string());
    let display = format!("{}", err);
    assert!(display.contains("lock poisoned"));
}

#[test]
fn ep_11_projection_error_build_failed_message() {
    let err = ProjectionError::BuildFailed("test error".to_string());
    let display = format!("{}", err);
    assert!(display.contains("test error"));
}

#[test]
fn ep_11_projection_error_rebuild_failed_message() {
    let err = ProjectionError::RebuildFailed("rebuild error".to_string());
    let display = format!("{}", err);
    assert!(display.contains("rebuild error"));
}

#[test]
fn ep_11_projection_error_upcasting_failed_message() {
    let err = ProjectionError::UpcastingFailed("version mismatch".to_string());
    let display = format!("{}", err);
    assert!(display.contains("version mismatch"));
}

#[test]
fn ep_11_projection_error_incompatible_schema_version_message() {
    let err = ProjectionError::IncompatibleSchemaVersion {
        expected: 2,
        actual: 1,
    };
    let display = format!("{}", err);
    assert!(display.contains("2"));
    assert!(display.contains("1"));
}

#[test]
fn ep_11_projection_error_sequence_gap_message() {
    let err = ProjectionError::SequenceGap(100);
    let display = format!("{}", err);
    assert!(display.contains("100"));
}

#[test]
fn ep_11_projection_error_checksum_mismatch_message() {
    let err = ProjectionError::ChecksumMismatch {
        expected: 12345,
        actual: 54321,
    };
    let display = format!("{}", err);
    assert!(display.contains("12345"));
    assert!(display.contains("54321"));
}

#[test]
fn ep_11_projection_error_concurrency_conflict_message() {
    let err = ProjectionError::ConcurrencyConflict("optimistic lock".to_string());
    let display = format!("{}", err);
    assert!(display.contains("optimistic lock"));
}

#[test]
fn ep_11_projection_error_throttle_exceeded_message() {
    let err = ProjectionError::ThrottleExceeded(500);
    let display = format!("{}", err);
    assert!(display.contains("500"));
}

#[test]
fn ep_11_projection_error_failed_state_message() {
    let err = ProjectionError::FailedState("upstream error".to_string());
    let display = format!("{}", err);
    assert!(display.contains("upstream error"));
}

// =========================================================================
// EP-12: ReplayError Display implementation is correct
// =========================================================================

#[test]
fn ep_12_replay_error_instance_mismatch_display() {
    let err = ReplayError::InstanceMismatch {
        expected: "inst-a".to_string(),
        actual: "inst-b".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("Instance ID mismatch"));
    assert!(display.contains("inst-a"));
    assert!(display.contains("inst-b"));
}

#[test]
fn ep_12_replay_error_sequence_gap_display() {
    let err = ReplayError::SequenceGap {
        expected: 5,
        actual: 10,
        at_index: 2,
    };
    let display = format!("{}", err);
    assert!(display.contains("Sequence gap"));
    assert!(display.contains("5"));
    assert!(display.contains("10"));
    assert!(display.contains("2"));
}

#[test]
fn ep_12_replay_error_sequence_duplicate_display() {
    let err = ReplayError::SequenceDuplicate {
        sequence: 7,
        first_at_index: 1,
        second_at_index: 3,
    };
    let display = format!("{}", err);
    assert!(display.contains("Duplicate sequence 7"));
    assert!(display.contains("1"));
    assert!(display.contains("3"));
}

#[test]
fn ep_12_replay_error_payload_decode_failed_display() {
    let err = ReplayError::PayloadDecodeFailed {
        sequence: 42,
        source: "invalid json".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("Payload decode failed"));
    assert!(display.contains("42"));
    assert!(display.contains("invalid json"));
}

#[test]
fn ep_12_replay_error_transition_failed_display() {
    let err = ReplayError::TransitionFailed {
        sequence: 5,
        state: LifecycleState::Pending,
        reason: "invalid transition".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("Transition failed"));
    assert!(display.contains("5"));
    assert!(display.contains("Pending"));
    assert!(display.contains("invalid transition"));
}

#[test]
fn ep_12_replay_error_unexpected_event_type_display() {
    let err = ReplayError::UnexpectedEventType {
        payload_type: "Unknown".to_string(),
        sequence: 3,
    };
    let display = format!("{}", err);
    assert!(display.contains("Unknown"));
    assert!(display.contains("3"));
}

#[test]
fn ep_12_replay_error_upcasting_failed_display() {
    let err = ReplayError::UpcastingFailed {
        sequence: 10,
        reason: "missing upcaster".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("Upcasting failed"));
    assert!(display.contains("10"));
    assert!(display.contains("missing upcaster"));
}

// =========================================================================
// EP-13: Empty events list returns correct result
// =========================================================================

#[test]
fn ep_13_empty_events_returns_empty_result() {
    let engine = ReplayEngine::new();
    let result = engine.replay(&[]).expect("empty replay should succeed");
    assert_eq!(result.final_state, None);
    assert_eq!(result.events_applied, 0);
}

// =========================================================================
// EP-14: Terminal states stop processing correctly
// =========================================================================

#[test]
fn ep_14_completed_state_stops_processing() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        make_event("inst-1", 5, timer_set_payload("wf-1", "timer-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Completed));
    assert_eq!(result.events_applied, 4);
}

#[test]
fn ep_14_cancelled_state_stops_processing() {
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
// EP-15: Error propagation through multiple failure points
// =========================================================================

#[test]
fn ep_15_first_event_failure_stops_before_second_event() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, serde_json::json!({"type": "UnknownType"})),
        make_event("inst-1", 2, workflow_started_payload("wf-1")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    assert!(matches!(
        err,
        ReplayError::PayloadDecodeFailed { sequence: 1, .. }
    ));
}

#[test]
fn ep_15_mid_sequence_failure_reports_correct_sequence() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_completed_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_scheduled_payload("wf-1", "step-2")),
    ];
    let err = engine
        .replay(&events)
        .expect_err("should fail at sequence 2");
    assert!(matches!(
        err,
        ReplayError::TransitionFailed { sequence: 2, .. }
    ));
}
