#![allow(clippy::unwrap_used, clippy::expect_used)]
use super::names::{InvocationId, RetryAfterSeconds, Timestamp};
use super::v1::*;
use crate::types::errors::InvariantViolation;

fn make_ulid() -> InvocationId {
    InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap()
}

fn make_timestamp() -> Timestamp {
    Timestamp::new("2026-04-13T00:00:00Z").unwrap()
}

#[test]
fn start_workflow_request_serde_roundtrip() {
    let req = StartWorkflowRequest {
        workflow_name: crate::types::names::WorkflowName::new("my_workflow").unwrap(),
        input: serde_json::json!({"key": "value"}),
    };
    let json = serde_json::to_string(&req).unwrap();
    let back: StartWorkflowRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(back.workflow_name.as_str(), "my_workflow");
}

#[test]
fn signal_request_serde_roundtrip() {
    let req = SignalRequest {
        signal_name: crate::types::names::SignalName::new("my_signal").unwrap(),
        payload: serde_json::json!({"data": 42}),
    };
    let json = serde_json::to_string(&req).unwrap();
    let back: SignalRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(back.signal_name.as_str(), "my_signal");
}

#[test]
fn workflow_status_value_serde_lowercase() {
    let val = WorkflowStatusValue::Running;
    let json = serde_json::to_string(&val).unwrap();
    assert_eq!(json, "\"running\"");

    let back: WorkflowStatusValue = serde_json::from_str(&json).unwrap();
    assert_eq!(back, WorkflowStatusValue::Running);
}

#[test]
fn workflow_status_value_all_variants() {
    let variants = [
        (WorkflowStatusValue::Pending, "\"pending\""),
        (WorkflowStatusValue::Running, "\"running\""),
        (WorkflowStatusValue::Completed, "\"completed\""),
        (WorkflowStatusValue::Failed, "\"failed\""),
        (WorkflowStatusValue::Cancelled, "\"cancelled\""),
    ];
    for (val, expected) in variants {
        assert_eq!(serde_json::to_string(&val).unwrap(), expected);
    }
}

#[test]
fn start_workflow_response_validate_running() {
    let resp = StartWorkflowResponse {
        invocation_id: make_ulid(),
        workflow_name: "test".to_string(),
        status: WorkflowStatusValue::Running,
        started_at: make_timestamp(),
    };
    assert!(resp.validate().is_ok());
}

#[test]
fn start_workflow_response_validate_non_running_fails() {
    let resp = StartWorkflowResponse {
        invocation_id: make_ulid(),
        workflow_name: "test".to_string(),
        status: WorkflowStatusValue::Pending,
        started_at: make_timestamp(),
    };
    assert!(resp.validate().is_err());
}

#[test]
fn workflow_status_validate_valid() {
    let ts = make_timestamp();
    let resp = WorkflowStatus {
        invocation_id: make_ulid(),
        workflow_name: "test".to_string(),
        status: WorkflowStatusValue::Running,
        current_step: 1,
        started_at: ts.clone(),
        updated_at: Timestamp::new("2026-04-13T01:00:00Z").unwrap(),
    };
    assert!(resp.validate().is_ok());
}

#[test]
fn workflow_status_validate_updated_before_started() {
    let resp = WorkflowStatus {
        invocation_id: make_ulid(),
        workflow_name: "test".to_string(),
        status: WorkflowStatusValue::Running,
        current_step: 1,
        started_at: Timestamp::new("2026-04-13T01:00:00Z").unwrap(),
        updated_at: Timestamp::new("2026-04-13T00:00:00Z").unwrap(),
    };
    assert!(matches!(
        resp.validate().unwrap_err(),
        InvariantViolation::UpdatedBeforeStarted
    ));
}

#[test]
fn journal_response_validate_sorted() {
    let resp = JournalResponse {
        invocation_id: make_ulid(),
        entries: vec![
            JournalEntry {
                seq: 1,
                entry_type: JournalEntryType::Run,
                name: None,
                input: None,
                output: None,
                timestamp: None,
                duration_ms: None,
                fire_at: None,
                status: None,
            },
            JournalEntry {
                seq: 2,
                entry_type: JournalEntryType::Wait,
                name: None,
                input: None,
                output: None,
                timestamp: None,
                duration_ms: None,
                fire_at: None,
                status: None,
            },
        ],
    };
    assert!(resp.validate().is_ok());
}

#[test]
fn journal_response_validate_unsorted() {
    let resp = JournalResponse {
        invocation_id: make_ulid(),
        entries: vec![
            JournalEntry {
                seq: 2,
                entry_type: JournalEntryType::Run,
                name: None,
                input: None,
                output: None,
                timestamp: None,
                duration_ms: None,
                fire_at: None,
                status: None,
            },
            JournalEntry {
                seq: 1,
                entry_type: JournalEntryType::Wait,
                name: None,
                input: None,
                output: None,
                timestamp: None,
                duration_ms: None,
                fire_at: None,
                status: None,
            },
        ],
    };
    assert!(matches!(
        resp.validate().unwrap_err(),
        InvariantViolation::EntriesNotSorted
    ));
}

#[test]
fn journal_entry_skip_none_fields() {
    let entry = JournalEntry {
        seq: 1,
        entry_type: JournalEntryType::Run,
        name: None,
        input: None,
        output: None,
        timestamp: None,
        duration_ms: None,
        fire_at: None,
        status: None,
    };
    let json = serde_json::to_string(&entry).unwrap();
    assert!(!json.contains("name"));
    assert!(!json.contains("input"));
}

#[test]
fn error_response_new_retryable_with_retry() {
    let resp = ErrorResponse::new(
        "at_capacity",
        "try again",
        Some(RetryAfterSeconds::new(30).unwrap()),
    );
    assert!(resp.is_ok());
    let r = resp.unwrap();
    assert_eq!(r.error, "at_capacity");
    assert_eq!(r.retry_after_seconds.unwrap().get(), 30);
}

#[test]
fn error_response_new_retryable_without_retry_fails() {
    let resp = ErrorResponse::new("at_capacity", "try again", None);
    assert!(matches!(
        resp.unwrap_err(),
        InvariantViolation::InvalidRetryForErrorType
    ));
}

#[test]
fn error_response_new_non_retryable_with_retry_fails() {
    let resp = ErrorResponse::new(
        "not_found",
        "oops",
        Some(RetryAfterSeconds::new(30).unwrap()),
    );
    assert!(matches!(
        resp.unwrap_err(),
        InvariantViolation::InvalidRetryForErrorType
    ));
}

#[test]
fn error_response_new_non_retryable_no_retry() {
    let resp = ErrorResponse::new("not_found", "missing", None);
    assert!(resp.is_ok());
    let r = resp.unwrap();
    assert_eq!(r.error, "not_found");
    assert!(r.retry_after_seconds.is_none());
}
