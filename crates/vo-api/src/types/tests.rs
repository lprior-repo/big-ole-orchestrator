use super::names::*;
use super::v1::*;
use crate::types::errors::ParseError;
use rstest::rstest;

#[rstest]
#[case("a")]
#[case("checkout")]
#[case("order_2_process")]
#[case("abc123")]
fn workflow_name_valid(#[case] name: &str) {
    WorkflowName::new(name).unwrap();
}

#[rstest]
#[case("")]
#[case("Invalid")]
#[case("1order")]
#[case("order-name")]
fn workflow_name_invalid(#[case] name: &str) {
    let result = WorkflowName::new(name);
    assert!(result, Err(_)));}

#[rstest]
#[case("payment_approved")]
#[case("cancel")]
#[case("signal_2")]
fn signal_name_valid(#[case] name: &str) {
    SignalName::new(name).unwrap();
}

#[rstest]
#[case("")]
#[case("a")]
#[case("Invalid")]
#[case("signal-name")]
fn signal_name_invalid(#[case] name: &str) {
    let result = SignalName::new(name);
    assert!(result, Err(_)));}

#[test]
fn invocation_id_valid() {
    let result = InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FAV");
    assert!(matches!(result, Ok(_)));
}

#[rstest]
#[case("")]
#[case("x")]
#[case("01ARZ3NDEKTSV4RRFFQ69G5FA")]
#[case("01ARZ3NDEKTSV4RRFFQ69G5FAVX")]
#[case("INVALID123")]
fn invocation_id_invalid(#[case] id: &str) {
    let result = InvocationId::from_str(id);
    assert!(result, Err(_)));}

#[test]
fn retry_after_seconds_valid() -> Result<(), crate::types::errors::ValidationError> {
    let result = RetryAfterSeconds::new(5)?;
    assert_eq!(result.get(), 5);
    Ok(())
}

#[test]
fn retry_after_seconds_zero_invalid() {
    let result = RetryAfterSeconds::new(0);
    assert!(matches!(result, Err(crate::types::errors::ValidationError::InvalidRetryAfterSeconds)));}

#[rstest]
#[case("2024-01-15T10:30:00Z")]
#[case("2024-01-15T10:30:00+05:00")]
fn timestamp_valid(#[case] ts: &str) {
    let result = Timestamp::new(ts);
    assert!(matches!(result, Ok(_)));
}

#[rstest]
#[case("invalid")]
#[case("2024-13-45T99:99:99Z")]
fn timestamp_invalid(#[case] ts: &str) {
    let result = Timestamp::new(ts);
    assert!(result, Err(_)));}

#[test]
fn workflow_status_validate_timestamps() -> anyhow::Result<()> {
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
    assert!(matches!(status_before.validate(), Err(crate::types::errors::InvariantViolation::UpdatedBeforeStarted)));
    let status_after = WorkflowStatus {
        invocation_id: InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FAV")
            .map_err(|e| anyhow::anyhow!(e))?,
        workflow_name: "test".to_string(),
        status: WorkflowStatusValue::Running,
        current_step: 0,
        started_at: started,
        updated_at: updated_after,
    };
    status_after.validate().unwrap();
    Ok(())
}

#[test]
fn journal_response_validate_sorted() -> anyhow::Result<()> {
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
    assert!(matches!(unsorted.validate(), Err(crate::types::errors::InvariantViolation::EntriesNotSorted)));
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
    sorted.validate().unwrap();
    Ok(())
}

#[test]
fn error_response_retryable_validation() -> anyhow::Result<()> {
    let retry = RetryAfterSeconds::new(5).map_err(|e| anyhow::anyhow!(e))?;

    let err = ErrorResponse::new("at_capacity", "Capacity reached", Some(retry.clone()));    assert!(matches!(err, Ok(_)));

    let err = ErrorResponse::new("at_capacity", "Capacity reached", None);
    assert!(err, Err(_)));
    let err = ErrorResponse::new("not_found", "Not found", None);
    assert!(matches!(err, Ok(_)));

    let err = ErrorResponse::new("not_found", "Not found", Some(retry));
    assert!(err, Err(_)));    Ok(())
}

#[test]
fn start_workflow_response_validate() -> anyhow::Result<()> {
    let resp = StartWorkflowResponse {
        invocation_id: InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FAV")
            .map_err(|e| anyhow::anyhow!(e))?,
        workflow_name: "checkout".to_string(),
        status: WorkflowStatusValue::Running,
        started_at: Timestamp::new("2024-01-15T10:30:00Z").map_err(|e| anyhow::anyhow!(e))?,
    };
    resp.validate().unwrap();

    let resp = StartWorkflowResponse {
        invocation_id: InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FAV")
            .map_err(|e| anyhow::anyhow!(e))?,
        workflow_name: "checkout".to_string(),
        status: WorkflowStatusValue::Completed,
        started_at: Timestamp::new("2024-01-15T10:30:00Z").map_err(|e| anyhow::anyhow!(e))?,
    };
    assert!(matches!(resp.validate(), Err(crate::types::errors::InvariantViolation::InvalidStatusForResponse)));    Ok(())
}

#[test]
fn serde_roundtrip_start_workflow_request() -> anyhow::Result<()> {
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
fn serde_roundtrip_signal_request() -> anyhow::Result<()> {
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
