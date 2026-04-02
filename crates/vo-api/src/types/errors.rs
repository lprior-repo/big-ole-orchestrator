use thiserror::Error;

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
    #[error("journal entries not in ascending seq order")]
    EntriesNotSorted,
    #[error("retry_after_seconds set for non-retryable error")]
    InvalidRetryForErrorType,
    #[error("invocation_id is immutable")]
    InvocationIdModified,
    #[error("status must be 'running' for StartWorkflowResponse")]
    InvalidStatusForResponse,
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
}
