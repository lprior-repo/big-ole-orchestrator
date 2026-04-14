use crate::types::errors::InvariantViolation;
use crate::types::helpers::{is_retryable_error, is_sorted};
use crate::types::names::{InvocationId, RetryAfterSeconds, SignalName, Timestamp, WorkflowName};
use serde::{Deserialize, Serialize};

/// Request to start a new workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartWorkflowRequest {
    pub workflow_name: WorkflowName,
    pub input: serde_json::Value,
}

/// Request to send a signal to a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalRequest {
    pub signal_name: SignalName,
    pub payload: serde_json::Value,
}

/// Workflow status value enum
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowStatusValue {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Response after starting a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartWorkflowResponse {
    pub invocation_id: InvocationId,
    pub workflow_name: String,
    pub status: WorkflowStatusValue,
    pub started_at: Timestamp,
}

impl StartWorkflowResponse {
    /// Validate the response postconditions.
    ///
    /// # Errors
    /// Returns `InvariantViolation` if the status is not 'running'.
    pub fn validate(&self) -> Result<(), InvariantViolation> {
        if self.status != WorkflowStatusValue::Running {
            return Err(InvariantViolation::InvalidStatusForResponse);
        }
        Ok(())
    }
}

/// Detailed workflow status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStatus {
    pub invocation_id: InvocationId,
    pub workflow_name: String,
    pub status: WorkflowStatusValue,
    pub current_step: u32,
    pub started_at: Timestamp,
    pub updated_at: Timestamp,
}

impl WorkflowStatus {
    /// Validate the status postconditions.
    ///
    /// # Errors
    /// Returns `InvariantViolation` if updated_at precedes started_at.
    pub fn validate(&self) -> Result<(), InvariantViolation> {
        let chronologically_invalid =
            match (self.updated_at.as_datetime(), self.started_at.as_datetime()) {
                (Some(updated), Some(started)) => updated < started,
                _ => true,
            };
        if chronologically_invalid {
            return Err(InvariantViolation::UpdatedBeforeStarted);
        }
        Ok(())
    }
}

/// Response to a signal request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalResponse {
    pub acknowledged: bool,
}

/// Journal entry type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum JournalEntryType {
    Run,
    Wait,
}

/// Journal entry for workflow history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub seq: u32,
    #[serde(flatten)]
    pub entry_type: JournalEntryType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fire_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// Response containing workflow journal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalResponse {
    pub invocation_id: InvocationId,
    pub entries: Vec<JournalEntry>,
}

impl JournalResponse {
    /// Validate the journal response postconditions.
    ///
    /// # Errors
    /// Returns `InvariantViolation` if entries are not sorted by seq.
    pub fn validate(&self) -> Result<(), InvariantViolation> {
        let seqs = self.entries.iter().map(|e| e.seq);
        if !is_sorted(seqs) {
            return Err(InvariantViolation::EntriesNotSorted);
        }
        Ok(())
    }
}

/// Response containing list of running workflows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListWorkflowsResponse {
    pub workflows: Vec<WorkflowStatus>,
}

/// API error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<RetryAfterSeconds>,
}

impl ErrorResponse {
    /// Create a new `ErrorResponse` with validation.
    ///
    /// # Errors
    /// Returns `InvariantViolation` if retry_after_seconds is missing for retryable errors
    /// or present for non-retryable errors.
    pub fn new(
        error: impl Into<String>,
        message: impl Into<String>,
        retry_after: Option<RetryAfterSeconds>,
    ) -> Result<Self, InvariantViolation> {
        let error_str = error.into();
        let is_retryable = is_retryable_error(&error_str);
        let has_retry = retry_after.is_some();
        if is_retryable && !has_retry {
            return Err(InvariantViolation::InvalidRetryForErrorType);
        }
        if !is_retryable && has_retry {
            return Err(InvariantViolation::InvalidRetryForErrorType);
        }
        Ok(Self {
            error: error_str,
            message: message.into(),
            retry_after_seconds: retry_after,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::types::names::{InvocationId, Timestamp};

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
}
