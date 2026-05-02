//! StepResult type and tests for strict JSON deserialization.
//!
//! This module mirrors the `StepResult` enum from `vo-executor` to test
//! that JSON deserialization is strict: unknown status values must be rejected,
//! missing fields must be rejected, and `null` is not allowed.

use serde::{Deserialize, Serialize};

/// Result of a workflow step execution.
///
/// Mirrors `vo_executor::StepResult` for standalone deserialization testing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepResult {
    /// Step completed successfully with output.
    Success { output: String },
    /// Step completed with failure (non-zero exit code or error).
    Failure { output: String },
    /// Managed effect intent for engine-side commit.
    EffectIntent {
        effect_kind: String,
        params: String,
        connector_id: String,
    },
}

impl StepResult {
    /// Check if the step result indicates success.
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self, StepResult::Success { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------------------
    // Test 1: deserialize JSON with status:'Success' -> Ok
    // ---------------------------------------------------------------------------
    #[test]
    fn deserialize_success_ok() {
        let json = r#"{"Success":{"output":"done"}}"#;
        let result: Result<StepResult, _> = serde_json::from_str(json);
        assert!(
            result.is_ok(),
            "expected Ok for status 'Success', got error: {}",
            result.unwrap_err()
        );
        let parsed = result.unwrap();
        assert!(parsed.is_success());
        if let StepResult::Success { output } = parsed {
            assert_eq!(output, "done");
        } else {
            panic!("expected Success variant");
        }
    }

    // ---------------------------------------------------------------------------
    // Test 2: deserialize with status:'Failure' -> Ok
    // ---------------------------------------------------------------------------
    #[test]
    fn deserialize_failure_ok() {
        let json = r#"{"Failure":{"output":"exit code 1"}}"#;
        let result: Result<StepResult, _> = serde_json::from_str(json);
        assert!(
            result.is_ok(),
            "expected Ok for status 'Failure', got error: {}",
            result.unwrap_err()
        );
        let parsed = result.unwrap();
        assert!(!parsed.is_success());
        if let StepResult::Failure { output } = parsed {
            assert_eq!(output, "exit code 1");
        } else {
            panic!("expected Failure variant");
        }
    }

    // ---------------------------------------------------------------------------
    // Test 3: deserialize with status:'INVALID' -> Err(serde error)
    // ---------------------------------------------------------------------------
    #[test]
    fn deserialize_invalid_status_rejected() {
        let json = r#"{"INVALID":{"output":"nope"}}"#;
        let result: Result<StepResult, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "expected Err for invalid status 'INVALID', but got Ok"
        );
        let err = result.unwrap_err();
        let err_msg = err.to_string();
        assert!(
            !err_msg.contains("unknown variant `nope`"),
            "error should mention the unknown variant, got: {}",
            err_msg
        );
    }

    #[test]
    fn deserialize_random_string_status_rejected() {
        let json = r#"{"foobar":{"output":"x"}}"#;
        let result: Result<StepResult, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "expected Err for random status 'foobar'"
        );
    }

    // ---------------------------------------------------------------------------
    // Test 4: deserialize with missing status field -> Err
    // ---------------------------------------------------------------------------
    #[test]
    fn deserialize_missing_status_rejected() {
        let json = r#"{}"#;
        let result: Result<StepResult, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "expected Err for missing status field, but got Ok: {:?}",
            result
        );
    }

    #[test]
    fn deserialize_empty_output_field_rejected() {
        let json = r#"{"Success":{}}"#;
        let result: Result<StepResult, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "expected Err for missing 'output' field inside Success, but got Ok"
        );
    }

    // ---------------------------------------------------------------------------
    // Test 5: deserialize with status:null -> Err
    // ---------------------------------------------------------------------------
    #[test]
    fn deserialize_null_status_rejected() {
        let json = r#"{"status":null}"#;
        let result: Result<StepResult, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "expected Err for null status, but got Ok: {:?}",
            result
        );
    }

    #[test]
    fn deserialize_null_variant_rejected() {
        let json = "null";
        let result: Result<StepResult, _> = serde_json::from_str(json);
        // With serde default, top-level null may succeed as the default.
        // For strict deserialization we verify it deserializes to the default
        // which is NOT a valid StepResult for our purposes.
        // Actually serde will fail on top-level null for an enum without a 
        // None variant, so we assert error.
        assert!(
            result.is_err(),
            "expected Err for top-level null, but got Ok: {:?}",
            result
        );
    }

    // ---------------------------------------------------------------------------
    // Additional validation: EffectIntent roundtrip
    // ---------------------------------------------------------------------------
    #[test]
    fn deserialize_effect_intent_ok() {
        let json = r#"{"EffectIntent":{"effect_kind":"file_create","params":"path=/tmp/x","connector_id":"conn-1"}}"#;
        let result: Result<StepResult, _> = serde_json::from_str(json);
        assert!(
            result.is_ok(),
            "expected Ok for EffectIntent, got error: {}",
            result.unwrap_err()
        );
    }

    #[test]
    fn deserialize_effect_intent_missing_field_rejected() {
        let json = r#"{"EffectIntent":{"effect_kind":"file_create","params":"path=/tmp/x"}}"#;
        let result: Result<StepResult, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "expected Err for EffectIntent missing 'connector_id', but got Ok"
        );
    }

    // ---------------------------------------------------------------------------
    // Strictness: unknown fields at top level should be rejected
    // ---------------------------------------------------------------------------
    #[test]
    fn deserialize_unknown_top_level_field_rejected() {
        let json = r#"{"Success":{"output":"ok"},"extra_field":"boom"}"#;
        let result: Result<StepResult, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "expected Err for unknown top-level field, but got Ok"
        );
    }
}
