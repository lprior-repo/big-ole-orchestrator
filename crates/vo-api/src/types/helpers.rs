use itertools::Itertools;

pub const MAX_JSON_PAYLOAD_SIZE: usize = 1024 * 1024;

#[must_use]
pub fn is_retryable_error(error: &str) -> bool {
    matches!(
        error,
        "at_capacity" | "internal_error" | "timeout" | "service_unavailable" | "rate_limited"
    )
}

#[must_use]
pub fn is_sorted<T: PartialOrd + Clone>(iter: impl Iterator<Item = T>) -> bool {
    iter.tuple_windows().all(|(prev, curr)| prev <= curr)
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum JsonPayloadError {
    #[error("JSON payload exceeds maximum size of {max_size} bytes")]
    ExceedsMaxSize { max_size: usize, actual_size: usize },
}

#[must_use]
pub fn validate_json_payload_size(value: &serde_json::Value) -> Option<JsonPayloadError> {
    match serde_json::to_vec(value) {
        Ok(bytes) => {
            let len = bytes.len();
            if len > MAX_JSON_PAYLOAD_SIZE {
                Some(JsonPayloadError::ExceedsMaxSize {
                    max_size: MAX_JSON_PAYLOAD_SIZE,
                    actual_size: len,
                })
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_retryable_error_at_capacity() {
        assert!(is_retryable_error("at_capacity"));
    }

    #[test]
    fn is_retryable_error_other_errors() {
        assert!(!is_retryable_error(""));
        assert!(!is_retryable_error("AT_CAPACITY"));
    }

    #[test]
    fn is_sorted_empty() {
        let v: Vec<u32> = vec![];
        assert!(is_sorted(v.into_iter()));
    }

    #[test]
    fn is_sorted_single() {
        assert!(is_sorted(vec![42].into_iter()));
    }

    #[test]
    fn is_sorted_ascending() {
        assert!(is_sorted(vec![1, 2, 3, 4, 5].into_iter()));
    }

    #[test]
    fn is_sorted_equal_elements() {
        assert!(is_sorted(vec![3, 3, 3].into_iter()));
    }

    #[test]
    fn is_sorted_descending() {
        assert!(!is_sorted(vec![5, 4, 3, 2, 1].into_iter()));
    }

    #[test]
    fn is_sorted_partially_sorted() {
        assert!(!is_sorted(vec![1, 3, 2, 4].into_iter()));
    }

    #[test]
    fn validate_json_payload_size_small_payload() {
        let value = serde_json::json!({"key": "value"});
        assert!(validate_json_payload_size(&value).is_none());
    }

    #[test]
    fn validate_json_payload_size_empty_object() {
        let value = serde_json::json!({});
        assert!(validate_json_payload_size(&value).is_none());
    }

    #[test]
    fn validate_json_payload_size_large_payload_exceeds_limit() {
        let large_obj: serde_json::Value = serde_json::json!({
            "data": "x".repeat(MAX_JSON_PAYLOAD_SIZE + 1)
        });
        let result = validate_json_payload_size(&large_obj);
        assert!(result.is_some());
        let err = result.unwrap();
        assert!(
            matches!(err, JsonPayloadError::ExceedsMaxSize { max_size, actual_size } 
            if max_size == MAX_JSON_PAYLOAD_SIZE && actual_size > MAX_JSON_PAYLOAD_SIZE)
        );
    }

    #[test]
    fn validate_json_payload_size_exactly_at_limit() {
        let value = serde_json::json!({"data": "x".repeat(MAX_JSON_PAYLOAD_SIZE)});
        assert!(validate_json_payload_size(&value).is_none());
    }

    #[test]
    fn validate_json_payload_size_nested_object() {
        let value = serde_json::json!({
            "outer": {
                "inner": {
                    "key": "value"
                }
            }
        });
        assert!(validate_json_payload_size(&value).is_none());
    }

    #[test]
    fn validate_json_payload_size_array() {
        let value: serde_json::Value = serde_json::json!([1, 2, 3, 4, 5]);
        assert!(validate_json_payload_size(&value).is_none());
    }
}
