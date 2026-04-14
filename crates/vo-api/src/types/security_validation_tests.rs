use crate::types::errors::*;
use crate::types::helpers::*;
use crate::types::ApiError;

#[cfg(test)]
mod security_validation_tests {
    use super::*;

    #[test]
    fn test_is_retryable_error_classification() {
        assert!(is_retryable_error("at_capacity"));
        assert!(is_retryable_error("internal_error"));
        assert!(is_retryable_error("timeout"));
        assert!(is_retryable_error("service_unavailable"));
        assert!(is_retryable_error("rate_limited"));

        assert!(!is_retryable_error("not_found"));
        assert!(!is_retryable_error("invalid_id"));
        assert!(!is_retryable_error("unauthorized"));
        assert!(!is_retryable_error("forbidden"));
        assert!(!is_retryable_error("invalid_request"));
        assert!(!is_retryable_error("conflict"));
        assert!(!is_retryable_error("precondition_failed"));
        assert!(!is_retryable_error("unsupported_media_type"));
        assert!(!is_retryable_error("validation_failed"));
        assert!(!is_retryable_error("rate_limit_exceeded"));
    }

    #[test]
    fn test_is_retryable_error_empty_string() {
        assert!(!is_retryable_error(""));
    }

    #[test]
    fn test_is_retryable_error_unknown_error() {
        assert!(!is_retryable_error("some_unknown_error"));
        assert!(!is_retryable_error("malformed_request"));
        assert!(!is_retryable_error("database_error"));
    }

    #[test]
    fn test_is_sorted_empty_iterator() {
        let empty: Vec<i32> = vec![];
        assert!(is_sorted(empty.into_iter()));
    }

    #[test]
    fn test_is_sorted_single_element() {
        assert!(is_sorted(std::iter::once(42)));
    }

    #[test]
    fn test_is_sorted_ascending_sequence() {
        assert!(is_sorted(vec![1, 2, 3, 4, 5].into_iter()));
        assert!(is_sorted(vec![0, 10, 20, 30].into_iter()));
        assert!(is_sorted(vec![-5, -3, -1, 0].into_iter()));
    }

    #[test]
    fn test_is_sorted_descending_sequence() {
        assert!(!is_sorted(vec![5, 4, 3, 2, 1].into_iter()));
        assert!(!is_sorted(vec![30, 20, 10, 0].into_iter()));
    }

    #[test]
    fn test_is_sorted_unsorted_sequence() {
        assert!(!is_sorted(vec![1, 3, 2, 4].into_iter()));
        assert!(!is_sorted(vec![1, 5, 2, 6, 3].into_iter()));
    }

    #[test]
    fn test_is_sorted_with_duplicates_ascending() {
        assert!(is_sorted(vec![1, 1, 2, 2, 3].into_iter()));
        assert!(is_sorted(vec![0, 0, 0].into_iter()));
        assert!(is_sorted(vec![-1, -1, 0, 0].into_iter()));
    }

    #[test]
    fn test_is_sorted_with_duplicates_unsorted() {
        assert!(!is_sorted(vec![1, 2, 1, 3].into_iter()));
        assert!(!is_sorted(vec![2, 1, 2, 1].into_iter()));
    }

    #[test]
    fn test_parse_error_empty_workflow_name() {
        let err = ParseError::EmptyWorkflowName;
        let msg = err.to_string();
        assert!(msg.contains("workflow_name"));
        assert!(msg.contains("empty"));
    }

    #[test]
    fn test_parse_error_invalid_workflow_name_format() {
        let err = ParseError::InvalidWorkflowNameFormat;
        let msg = err.to_string();
        assert!(msg.contains("pattern"));
        assert!(msg.contains("workflow_name"));
    }

    #[test]
    fn test_parse_error_empty_signal_name() {
        let err = ParseError::EmptySignalName;
        let msg = err.to_string();
        assert!(msg.contains("signal_name"));
    }

    #[test]
    fn test_parse_error_invalid_signal_name_format() {
        let err = ParseError::InvalidSignalNameFormat;
        let msg = err.to_string();
        assert!(msg.contains("signal_name"));
    }

    #[test]
    fn test_parse_error_invalid_ulid_format() {
        let err = ParseError::InvalidUlidFormat;
        let msg = err.to_string();
        assert!(msg.contains("Crockford"));
        assert!(msg.contains("base32"));
    }

    #[test]
    fn test_parse_error_invalid_timestamp_format() {
        let err = ParseError::InvalidTimestampFormat;
        let msg = err.to_string();
        assert!(msg.contains("RFC3339"));
    }

    #[test]
    fn test_parse_error_internal_error_with_message() {
        let err = ParseError::InternalError("regex compilation failed".to_string());
        let msg = err.to_string();
        assert!(msg.contains("regex compilation failed"));
    }

    #[test]
    fn test_validation_error_invalid_retry_after_seconds() {
        let err = ValidationError::InvalidRetryAfterSeconds;
        let msg = err.to_string();
        assert!(msg.contains("retry_after_seconds"));
    }

    #[test]
    fn test_validation_error_invalid_status_transition() {
        let err = ValidationError::InvalidStatusTransition;
        let msg = err.to_string();
        assert!(msg.contains("transition"));
    }

    #[test]
    fn test_validation_error_invalid_current_step() {
        let err = ValidationError::InvalidCurrentStep;
        let msg = err.to_string();
        assert!(msg.contains("current_step"));
    }

    #[test]
    fn test_invariant_violation_updated_before_started() {
        let err = InvariantViolation::UpdatedBeforeStarted;
        let msg = err.to_string();
        assert!(msg.contains("updated_at"));
    }

    #[test]
    fn test_invariant_violation_entries_not_sorted() {
        let err = InvariantViolation::EntriesNotSorted;
        let msg = err.to_string();
        assert!(msg.contains("ascending"));
        assert!(msg.contains("sort"));
    }

    #[test]
    fn test_invariant_violation_invalid_retry_for_error_type() {
        let err = InvariantViolation::InvalidRetryForErrorType;
        let msg = err.to_string();
        assert!(msg.contains("retry"));
    }

    #[test]
    fn test_invariant_violation_invocation_id_modified() {
        let err = InvariantViolation::InvocationIdModified;
        let msg = err.to_string();
        assert!(msg.contains("immutable"));
    }

    #[test]
    fn test_invariant_violation_invalid_status_for_response() {
        let err = InvariantViolation::InvalidStatusForResponse;
        let msg = err.to_string();
        assert!(msg.contains("running"));
    }

    #[test]
    fn test_api_error_creation_with_strings() {
        let err = ApiError::new("test_code", "Test error message");
        assert_eq!(err.error, "test_code");
        assert_eq!(err.message, "Test error message");
    }

    #[test]
    fn test_api_error_creation_with_string_types() {
        let err = ApiError::new(String::from("code"), String::from("Message"));
        assert_eq!(err.error, "code");
        assert_eq!(err.message, "Message");
    }

    #[test]
    fn test_api_error_serialization() {
        let err = ApiError::new("server_error", "Something went wrong");
        let json = serde_json::to_value(&err).expect("serializable");

        assert_eq!(json["error"], "server_error");
        assert_eq!(json["message"], "Something went wrong");
    }

    #[test]
    fn test_api_error_deserialization() {
        let json = serde_json::json!({
            "error": "not_found",
            "message": "Resource not found"
        });
        let err: ApiError = serde_json::from_value(json).expect("deserializable");

        assert_eq!(err.error, "not_found");
        assert_eq!(err.message, "Resource not found");
    }

    #[test]
    fn test_api_error_roundtrip() {
        let original = ApiError::new("custom_code", "Custom message");
        let serialized = serde_json::to_string(&original).expect("serializable");
        let deserialized: ApiError = serde_json::from_str(&serialized).expect("deserializable");

        assert_eq!(original.error, deserialized.error);
        assert_eq!(original.message, deserialized.message);
    }
}
