use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::types::helpers::is_retryable_error;
use crate::types::names::RetryAfterSeconds;

/// Parse errors for invalid input format
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParseError {
    #[error("workflow_name is empty string")]
    EmptyWorkflowName,
    #[error("workflow_name does not match pattern [a-z][a-z0-9_]*")]
    InvalidWorkflowNameFormat,
    #[error("signal_name is empty string")]
    EmptySignalName,
    #[error("signal_name does not match pattern [a-z][a-z0-9_]+")]
    InvalidSignalNameFormat,
    #[error("invocation_id is not valid 26-char Crockford base32")]
    InvalidUlidFormat,
    #[error("timestamp is not valid RFC3339")]
    InvalidTimestampFormat,
    #[error("unknown status variant")]
    UnknownStatusVariant,
    #[error("internal error: {0}")]
    InternalError(String),
}

/// Validation errors for business rule violations
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ValidationError {
    #[error("retry_after_seconds must be > 0")]
    InvalidRetryAfterSeconds,
    #[error("invalid status transition")]
    InvalidStatusTransition,
    #[error("current_step is inconsistent with status")]
    InvalidCurrentStep,
}

/// Invariant violations for postcondition failures
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvariantViolation {
    #[error("updated_at timestamp precedes started_at")]
    UpdatedBeforeStarted,
    #[error("journal entries not in ascending sort order")]
    EntriesNotSorted,
    #[error("retry_after_seconds set for non-retryable error")]
    InvalidRetryForErrorType,
    #[error("invocation_id is immutable")]
    InvocationIdModified,
    #[error("status must be 'running' for StartWorkflowResponse")]
    InvalidStatusForResponse,
}

/// Workload class rejection errors for fairness/budget violations (ADR-033).
///
/// These errors are returned when a workflow start or resume request is rejected
/// due to workload budget exhaustion or fairness constraints. They map to
/// HTTP 429 (Too Many Requests) to indicate the client should retry.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorkloadRejectionError {
    /// Class budget exhausted - too many concurrent instances of this class.
    #[error("budget exhausted for {class}: requested {requested}, available {available}. Consider reducing submission rate or waiting for active instances to complete.")]
    BudgetExhausted {
        class: String,
        requested: u32,
        available: u32,
    },
    /// Per-workflow concurrency cap exceeded.
    #[error("per-workflow cap exceeded for {class}: {workflow_id} has too many active instances.")]
    WorkflowCapExceeded { class: String, workflow_id: String },
    /// Global concurrency limit reached across all classes.
    #[error("global concurrency limit reached: {current}/{max} total instances. System is under heavy load.")]
    GlobalConcurrencyLimit { current: u32, max: u32 },
}

impl WorkloadRejectionError {
    /// Returns the appropriate HTTP status code for this rejection error.
    #[must_use]
    pub fn status_code(&self) -> u16 {
        match self {
            WorkloadRejectionError::BudgetExhausted { .. } => 429,
            WorkloadRejectionError::WorkflowCapExceeded { .. } => 429,
            WorkloadRejectionError::GlobalConcurrencyLimit { .. } => 503,
        }
    }

    /// Returns the error code string for API responses.
    #[must_use]
    pub fn error_code(&self) -> &'static str {
        match self {
            WorkloadRejectionError::BudgetExhausted { .. } => "budget_exhausted",
            WorkloadRejectionError::WorkflowCapExceeded { .. } => "workflow_cap_exceeded",
            WorkloadRejectionError::GlobalConcurrencyLimit { .. } => "global_concurrency_limit",
        }
    }
}

/// Generic API error response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<RetryAfterSeconds>,
}

impl ApiError {
    #[must_use]
    pub fn new(error: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            message: message.into(),
            retry_after_seconds: None,
        }
    }

    pub fn with_retry(
        error: impl Into<String>,
        message: impl Into<String>,
        retry_after: RetryAfterSeconds,
    ) -> Self {
        Self {
            error: error.into(),
            message: message.into(),
            retry_after_seconds: Some(retry_after),
        }
    }

    pub fn new_with_retry_validation(
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
mod tests {
    use super::*;

    #[test]
    fn parse_error_variants_can_be_constructed() {
        assert!(matches!(
            ParseError::EmptyWorkflowName,
            ParseError::EmptyWorkflowName
        ));
        assert!(matches!(
            ParseError::InvalidWorkflowNameFormat,
            ParseError::InvalidWorkflowNameFormat
        ));
        assert!(matches!(
            ParseError::EmptySignalName,
            ParseError::EmptySignalName
        ));
        assert!(matches!(
            ParseError::InvalidSignalNameFormat,
            ParseError::InvalidSignalNameFormat
        ));
        assert!(matches!(
            ParseError::InvalidUlidFormat,
            ParseError::InvalidUlidFormat
        ));
        assert!(matches!(
            ParseError::InvalidTimestampFormat,
            ParseError::InvalidTimestampFormat
        ));
        assert!(matches!(
            ParseError::UnknownStatusVariant,
            ParseError::UnknownStatusVariant
        ));
        assert!(matches!(
            ParseError::InternalError("x".to_string()),
            ParseError::InternalError(_)
        ));
    }

    #[test]
    fn validation_error_variants_can_be_constructed() {
        assert!(matches!(
            ValidationError::InvalidRetryAfterSeconds,
            ValidationError::InvalidRetryAfterSeconds
        ));
        assert!(matches!(
            ValidationError::InvalidStatusTransition,
            ValidationError::InvalidStatusTransition
        ));
        assert!(matches!(
            ValidationError::InvalidCurrentStep,
            ValidationError::InvalidCurrentStep
        ));
    }

    #[test]
    fn invariant_violation_variants_can_be_constructed() {
        assert!(matches!(
            InvariantViolation::UpdatedBeforeStarted,
            InvariantViolation::UpdatedBeforeStarted
        ));
        assert!(matches!(
            InvariantViolation::EntriesNotSorted,
            InvariantViolation::EntriesNotSorted
        ));
        assert!(matches!(
            InvariantViolation::InvalidRetryForErrorType,
            InvariantViolation::InvalidRetryForErrorType
        ));
        assert!(matches!(
            InvariantViolation::InvocationIdModified,
            InvariantViolation::InvocationIdModified
        ));
        assert!(matches!(
            InvariantViolation::InvalidStatusForResponse,
            InvariantViolation::InvalidStatusForResponse
        ));
    }

    #[test]
    fn workload_rejection_error_budget_exhausted() {
        let err = WorkloadRejectionError::BudgetExhausted {
            class: "Standard".to_string(),
            requested: 1,
            available: 0,
        };
        assert!(err.to_string().contains("budget exhausted"));
        assert!(err.to_string().contains("Standard"));
        assert_eq!(err.status_code(), 429);
        assert_eq!(err.error_code(), "budget_exhausted");
    }

    #[test]
    fn workload_rejection_error_workflow_cap_exceeded() {
        let err = WorkloadRejectionError::WorkflowCapExceeded {
            class: "UnsafeBulk".to_string(),
            workflow_id: "test-workflow".to_string(),
        };
        assert!(err.to_string().contains("per-workflow cap exceeded"));
        assert!(err.to_string().contains("UnsafeBulk"));
        assert_eq!(err.status_code(), 429);
        assert_eq!(err.error_code(), "workflow_cap_exceeded");
    }

    #[test]
    fn workload_rejection_error_global_concurrency_limit() {
        let err = WorkloadRejectionError::GlobalConcurrencyLimit {
            current: 100,
            max: 100,
        };
        assert!(err.to_string().contains("global concurrency limit reached"));
        assert_eq!(err.status_code(), 503);
        assert_eq!(err.error_code(), "global_concurrency_limit");
    }

    #[test]
    fn budget_tracker_rejection_maps_to_correct_status_code_and_json_payload() {
        let err = WorkloadRejectionError::BudgetExhausted {
            class: "recovery".to_string(),
            requested: 1,
            available: 0,
        };
        assert_eq!(err.status_code(), 429);
        assert_eq!(err.error_code(), "budget_exhausted");
        let msg = err.to_string();
        assert!(msg.contains("budget exhausted"));
        assert!(msg.contains("recovery"));
        assert!(msg.contains("requested"));
        assert!(msg.contains("available"));
    }

    #[test]
    fn unknown_rejection_type_falls_back_to_standard_503() {
        let err = WorkloadRejectionError::GlobalConcurrencyLimit {
            current: 50,
            max: 100,
        };
        assert_eq!(err.status_code(), 503);
        assert_eq!(err.error_code(), "global_concurrency_limit");
    }

    #[test]
    fn workload_rejection_error_all_variants_have_stable_status_codes() {
        assert_eq!(
            WorkloadRejectionError::BudgetExhausted {
                class: "test".to_string(),
                requested: 1,
                available: 0,
            }
            .status_code(),
            429
        );
        assert_eq!(
            WorkloadRejectionError::WorkflowCapExceeded {
                class: "test".to_string(),
                workflow_id: "wf".to_string(),
            }
            .status_code(),
            429
        );
        assert_eq!(
            WorkloadRejectionError::GlobalConcurrencyLimit {
                current: 1,
                max: 10,
            }
            .status_code(),
            503
        );
    }

    #[test]
    fn workload_rejection_error_all_variants_have_stable_error_codes() {
        assert_eq!(
            WorkloadRejectionError::BudgetExhausted {
                class: "test".to_string(),
                requested: 1,
                available: 0,
            }
            .error_code(),
            "budget_exhausted"
        );
        assert_eq!(
            WorkloadRejectionError::WorkflowCapExceeded {
                class: "test".to_string(),
                workflow_id: "wf".to_string(),
            }
            .error_code(),
            "workflow_cap_exceeded"
        );
        assert_eq!(
            WorkloadRejectionError::GlobalConcurrencyLimit {
                current: 1,
                max: 10,
            }
            .error_code(),
            "global_concurrency_limit"
        );
    }
}
