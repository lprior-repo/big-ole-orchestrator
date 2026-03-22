//! Tests for API types.

use super::*;

#[test]
fn test_workflow_name_valid() {
    let cases = ["a", "checkout", "order_2_process", "abc123"];
    for name in cases {
        assert!(WorkflowName::new(name).is_ok());
    }
}

#[test]
fn test_workflow_name_invalid() {
    let cases = ["", "Invalid", "1order", "order-name"];
    for name in cases {
        assert!(WorkflowName::new(name).is_err());
    }
}

#[test]
fn test_signal_name_valid() {
    let cases = ["payment_approved", "cancel", "signal_2"];
    for name in cases {
        assert!(SignalName::new(name).is_ok());
    }
}

#[test]
fn test_invocation_id_valid() {
    assert!(InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FAV").is_ok());
}

#[test]
fn test_retry_after_seconds_valid() {
    assert!(RetryAfterSeconds::new(5).is_ok());
}

#[test]
fn test_timestamp_valid() {
    assert!(Timestamp::new("2024-01-15T10:30:00Z").is_ok());
}

#[test]
fn test_workflow_name_rejects_uppercase() {
    assert!(WorkflowName::new("Checkout").is_err());
}

#[test]
fn test_signal_name_rejects_single_char() {
    assert!(SignalName::new("x").is_err());
}

#[test]
fn test_retry_after_seconds_rejects_zero() {
    assert!(RetryAfterSeconds::new(0).is_err());
}

#[test]
fn test_start_workflow_response_validate_running_only() {
    let response = StartWorkflowResponse {
        invocation_id: InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FAV")
            .unwrap_or_else(|_| unreachable!()),
        workflow_name: "checkout".to_owned(),
        status: WorkflowStatusValue::Completed,
        started_at: Timestamp::new("2024-01-15T10:30:00Z").unwrap_or_else(|_| unreachable!()),
    };
    assert!(response.validate().is_err());
}

#[test]
fn test_journal_response_validate_requires_sorted_entries() {
    let invocation_id = "payments/01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned();
    let entries = vec![
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
    ];
    let response = JournalResponse::new(invocation_id, entries);
    assert!(response.validate().is_err());
}

#[test]
fn test_error_response_rejects_retry_for_non_retryable_error() {
    let retry = RetryAfterSeconds::new(5).unwrap_or_else(|_| unreachable!());
    let response = ErrorResponse::new("not_found", "missing", Some(retry));
    assert!(response.is_err());
}
