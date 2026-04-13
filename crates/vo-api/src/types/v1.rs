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

// ---------------------------------------------------------------------------
// Lineage API types (ADR-038)
// ---------------------------------------------------------------------------

/// Response containing lineage information for a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageInfoResponse {
    /// Stable lineage identifier (persists across epoch rollovers).
    pub lineage_id: String,
    /// Currently active epoch number.
    pub active_epoch: u64,
    /// Instance ID backing the currently active epoch.
    pub active_instance_id: String,
    /// Total number of epochs in this lineage (including current).
    pub epoch_count: u64,
    /// Instance IDs for previous epochs (most recent first).
    pub previous_epochs: Vec<EpochSummary>,
}

/// Summary of a single epoch within a lineage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpochSummary {
    /// Epoch number.
    pub epoch: u64,
    /// Instance ID backing this epoch.
    pub instance_id: String,
    /// Whether this epoch is the currently active one.
    pub is_active: bool,
}

/// Request to trigger continue-as-new on a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinueAsNewRequest {
    /// The instance ID to roll over.
    pub instance_id: String,
}

/// Response after a successful continue-as-new rollover.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinueAsNewResponse {
    /// The stable lineage ID (unchanged across rollovers).
    pub lineage_id: String,
    /// The new epoch number.
    pub new_epoch: u64,
    /// The previous epoch number.
    pub old_epoch: u64,
    /// The new instance ID backing the new epoch.
    pub new_instance_id: String,
}
