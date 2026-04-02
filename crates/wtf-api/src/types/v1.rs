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
