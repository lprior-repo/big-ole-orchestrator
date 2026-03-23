//! API response DTOs.

use super::errors::InvariantViolation;
use super::newtypes::{InvocationId, RetryAfterSeconds, Timestamp};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
    pub fn validate(&self) -> Result<(), InvariantViolation> {
        if self.status != WorkflowStatusValue::Running {
            return Err(InvariantViolation::InvalidStatusForResponse);
        }
        Ok(())
    }
}

/// Event record for NDJSON streaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    pub seq: u64,
    pub event_type: String,
    pub data: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

/// Response to a signal request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalResponse {
    pub acknowledged: bool,
}

/// Response containing workflow journal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalResponse {
    pub invocation_id: String,
    pub entries: Vec<super::JournalEntry>,
}

impl JournalResponse {
    #[must_use]
    pub fn new(invocation_id: impl Into<String>, entries: Vec<super::JournalEntry>) -> Self {
        Self {
            invocation_id: invocation_id.into(),
            entries,
        }
    }

    pub fn validate(&self) -> Result<(), InvariantViolation> {
        let seqs = self.entries.iter().map(|e| e.seq);
        if !super::is_sorted(seqs) {
            return Err(InvariantViolation::EntriesNotSorted);
        }
        Ok(())
    }
}

/// Diagnostic from the linter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticDto {
    pub code: String,
    pub severity: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<(usize, usize)>,
}

/// Response to POST /api/v1/definitions/<type>.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefinitionResponse {
    pub valid: bool,
    pub diagnostics: Vec<DiagnosticDto>,
}

/// Response to POST /api/v1/workflows/validate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateWorkflowResponse {
    pub valid: bool,
    pub diagnostics: Vec<DiagnosticDto>,
}

/// Response to POST /api/v1/workflows on success (HTTP 201).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V3StartResponse {
    pub instance_id: String,
    pub namespace: String,
    pub workflow_type: String,
}

/// Response to GET /api/v1/workflows/:id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V3StatusResponse {
    pub instance_id: String,
    pub namespace: String,
    pub workflow_type: String,
    pub paradigm: String,
    pub phase: String,
    pub events_applied: u64,
}

/// Generic API error response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
    pub message: String,
}

impl ApiError {
    #[must_use]
    pub fn new(error: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            message: message.into(),
        }
    }
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
    pub fn new(
        error: impl Into<String>,
        message: impl Into<String>,
        retry_after: Option<RetryAfterSeconds>,
    ) -> Result<Self, InvariantViolation> {
        let error_str = error.into();
        let is_retryable = super::is_retryable_error(&error_str);
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
