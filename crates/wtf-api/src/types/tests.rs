use super::names::*;
use super::v1::*;
use crate::types::errors::ParseError;

#[test]
fn test_workflow_name_valid() -> Result<(), ParseError> {
    let cases = ["a", "checkout", "order_2_process", "abc123"];
    for name in cases {
        assert!(
            WorkflowName::new(name).is_ok(),
            "Expected {name} to be valid"
        );
    }
    Ok(())
}

#[test]
fn test_workflow_name_invalid() {
    let cases = ["", "Invalid", "1order", "order-name"];
    for name in cases {
        let result = WorkflowName::new(name);
        assert!(result.is_err(), "Expected {name} to be invalid");
    }
}

#[test]
fn test_signal_name_valid() -> Result<(), ParseError> {
    let cases = ["payment_approved", "cancel", "signal_2"];
    for name in cases {
        assert!(SignalName::new(name).is_ok(), "Expected {name} to be valid");
    }
    Ok(())
}

#[test]
fn test_signal_name_invalid() {
    let cases = ["", "a", "Invalid", "signal-name"];
    for name in cases {
        let result = SignalName::new(name);
        assert!(result.is_err(), "Expected {name} to be invalid");
    }
}

#[test]
fn test_invocation_id_valid() {
    let result = InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FAV");
    assert!(result.is_ok(), "Valid ULID should pass");
}

#[test]
fn test_invocation_id_invalid() {
    let cases = [
        "",
        "x",
        "01ARZ3NDEKTSV4RRFFQ69G5FA",
        "01ARZ3NDEKTSV4RRFFQ69G5FAVX",
        "INVALID123",
    ];
    for id in cases {
        let result = InvocationId::from_str(id);
        assert!(result.is_err(), "Expected {id} to be invalid");
    }
}

#[test]
fn test_retry_after_seconds_valid() -> Result<(), crate::types::errors::ValidationError> {
    let result = RetryAfterSeconds::new(5)?;
    assert_eq!(result.get(), 5);
    Ok(())
}

#[test]
fn test_retry_after_seconds_zero_invalid() {
    let result = RetryAfterSeconds::new(0);
    assert!(result.is_err());
}

#[test]
fn test_timestamp_valid() {
    let cases = ["2024-01-15T10:30:00Z", "2024-01-15T10:30:00+05:00"];
    for ts in cases {
        let result = Timestamp::new(ts);
        assert!(result.is_ok(), "Expected {ts} to be valid");
    }
}

#[test]
fn test_timestamp_invalid() {
    let cases = ["invalid", "2024-13-45T99:99:99Z"];
    for ts in cases {
        let result = Timestamp::new(ts);
        assert!(result.is_err(), "Expected {ts} to be invalid");
    }
}

#[test]
fn test_workflow_status_validate_timestamps() -> anyhow::Result<()> {
    let started = Timestamp::new("2024-01-15T10:31:00Z").map_err(|e| anyhow::anyhow!(e))?;
    let updated_before = Timestamp::new("2024-01-15T10:30:00Z").map_err(|e| anyhow::anyhow!(e))?;
    let updated_after = Timestamp::new("2024-01-15T10:32:00Z").map_err(|e| anyhow::anyhow!(e))?;

    let status_before = WorkflowStatus {
        invocation_id: InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FAV")
            .map_err(|e| anyhow::anyhow!(e))?,
        workflow_name: "test".to_string(),
        status: WorkflowStatusValue::Running,
        current_step: 0,
        started_at: started.clone(),
        updated_at: updated_before,
    };
    assert!(status_before.validate().is_err());

    let status_after = WorkflowStatus {
        invocation_id: InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FAV")
            .map_err(|e| anyhow::anyhow!(e))?,
        workflow_name: "test".to_string(),
        status: WorkflowStatusValue::Running,
        current_step: 0,
        started_at: started,
        updated_at: updated_after,
    };
    assert!(status_after.validate().is_ok());
    Ok(())
}

#[test]
fn test_journal_response_validate_sorted() -> anyhow::Result<()> {
    let invocation_id =
        InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FAV").map_err(|e| anyhow::anyhow!(e))?;

    let unsorted = JournalResponse {
        invocation_id: invocation_id.clone(),
        entries: vec![
            JournalEntry {
                seq: 1,
                entry_type: JournalEntryType::Run,
                name: Some("first".to_string()),
                input: None,
                output: None,
                timestamp: None,
                duration_ms: None,
                fire_at: None,
                status: None,
            },
            JournalEntry {
                seq: 0,
                entry_type: JournalEntryType::Run,
                name: Some("second".to_string()),
                input: None,
                output: None,
                timestamp: None,
                duration_ms: None,
                fire_at: None,
                status: None,
            },
        ],
    };
    assert!(unsorted.validate().is_err());

    let sorted = JournalResponse {
        invocation_id,
        entries: vec![
            JournalEntry {
                seq: 0,
                entry_type: JournalEntryType::Run,
                name: Some("first".to_string()),
                input: None,
                output: None,
                timestamp: None,
                duration_ms: None,
                fire_at: None,
                status: None,
            },
            JournalEntry {
                seq: 1,
                entry_type: JournalEntryType::Run,
                name: Some("second".to_string()),
                input: None,
                output: None,
                timestamp: None,
                duration_ms: None,
                fire_at: None,
                status: None,
            },
        ],
    };
    assert!(sorted.validate().is_ok());
    Ok(())
}

#[test]
fn test_error_response_retryable_validation() -> anyhow::Result<()> {
    let retry = RetryAfterSeconds::new(5).map_err(|e| anyhow::anyhow!(e))?;

    let err = ErrorResponse::new("at_capacity", "Capacity reached", Some(retry.clone()));
    assert!(err.is_ok(), "at_capacity with retry should be ok");

    let err = ErrorResponse::new("at_capacity", "Capacity reached", None);
    assert!(err.is_err(), "at_capacity without retry should fail");

    let err = ErrorResponse::new("not_found", "Not found", None);
    assert!(err.is_ok(), "not_found without retry should be ok");

    let err = ErrorResponse::new("not_found", "Not found", Some(retry));
    assert!(err.is_err(), "not_found with retry should fail");
    Ok(())
}

#[test]
fn test_start_workflow_response_validate() -> anyhow::Result<()> {
    let resp = StartWorkflowResponse {
        invocation_id: InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FAV")
            .map_err(|e| anyhow::anyhow!(e))?,
        workflow_name: "checkout".to_string(),
        status: WorkflowStatusValue::Running,
        started_at: Timestamp::new("2024-01-15T10:30:00Z").map_err(|e| anyhow::anyhow!(e))?,
    };
    assert!(resp.validate().is_ok());

    let resp = StartWorkflowResponse {
        invocation_id: InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FAV")
            .map_err(|e| anyhow::anyhow!(e))?,
        workflow_name: "checkout".to_string(),
        status: WorkflowStatusValue::Completed,
        started_at: Timestamp::new("2024-01-15T10:30:00Z").map_err(|e| anyhow::anyhow!(e))?,
    };
    assert!(resp.validate().is_err());
    Ok(())
}

#[test]
fn test_serde_roundtrip_start_workflow_request() -> anyhow::Result<()> {
    let request = StartWorkflowRequest {
        workflow_name: WorkflowName::new("checkout").map_err(|e| anyhow::anyhow!(e))?,
        input: serde_json::json!({ "order_id": "ord_123" }),
    };
    let json = serde_json::to_string(&request).map_err(|e| anyhow::anyhow!(e))?;
    let deserialized: StartWorkflowRequest =
        serde_json::from_str(&json).map_err(|e| anyhow::anyhow!(e))?;
    assert_eq!(
        request.workflow_name.as_str(),
        deserialized.workflow_name.as_str()
    );
    Ok(())
}

#[test]
fn test_serde_roundtrip_signal_request() -> anyhow::Result<()> {
    let request = SignalRequest {
        signal_name: SignalName::new("payment_approved").map_err(|e| anyhow::anyhow!(e))?,
        payload: serde_json::json!({ "approved": true }),
    };
    let json = serde_json::to_string(&request).map_err(|e| anyhow::anyhow!(e))?;
    let deserialized: SignalRequest =
        serde_json::from_str(&json).map_err(|e| anyhow::anyhow!(e))?;
    assert_eq!(
        request.signal_name.as_str(),
        deserialized.signal_name.as_str()
    );
    Ok(())
}
