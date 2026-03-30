//! Helper functions for parsing event payload JSON fields.
//!
//! Extracted from `events.rs` to eliminate primitive obsession patterns
//! and centralize the "parse, don't validate" extraction logic.

use crate::events::Error;

/// Extract a required `String` field from a JSON object.
///
/// Returns `InvalidPayloadField("<field> is required")` if absent,
/// `InvalidPayloadField("<field> must be a string")` if not a string.
/// This matches the `workflow_id`/`step_id` error convention in events.rs.
pub(crate) fn require_string_field(
    obj: &serde_json::Map<String, serde_json::Value>,
    field: &'static str,
) -> Result<String, Error> {
    obj.get(field)
        .ok_or_else(|| Error::InvalidPayloadField(format!("{field} is required")))?
        .as_str()
        .ok_or_else(|| Error::InvalidPayloadField(format!("{field} must be a string")))
        .map(|s| s.to_string())
}

/// Extract a required `String` field that uses `MissingPayloadField` for absence.
///
/// Returns `MissingPayloadField("<field>")` if absent,
/// `InvalidPayloadField("<field> must be a string")` if not a string.
pub(crate) fn require_string(
    obj: &serde_json::Map<String, serde_json::Value>,
    field: &'static str,
) -> Result<String, Error> {
    obj.get(field)
        .ok_or_else(|| Error::MissingPayloadField(field.to_string()))?
        .as_str()
        .ok_or_else(|| Error::InvalidPayloadField(format!("{field} must be a string")))
        .map(|s| s.to_string())
}

/// Extract a required `u64` field from a JSON object.
///
/// Returns `MissingPayloadField("<field>")` if absent,
/// `InvalidPayloadField("<field> must be an integer")` if not an integer.
pub(crate) fn require_u64(
    obj: &serde_json::Map<String, serde_json::Value>,
    field: &'static str,
) -> Result<u64, Error> {
    obj.get(field)
        .ok_or_else(|| Error::MissingPayloadField(field.to_string()))?
        .as_u64()
        .ok_or_else(|| Error::InvalidPayloadField(format!("{field} must be an integer")))
}

/// Extract an optional `u64` field with a default value.
///
/// Returns the default (typically 0) if the key is absent or not an integer.
pub(crate) fn optional_u64(
    obj: &serde_json::Map<String, serde_json::Value>,
    field: &str,
    default: u64,
) -> u64 {
    obj.get(field).and_then(|v| v.as_u64()).unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(pairs: &[(&str, serde_json::Value)]) -> serde_json::Map<String, serde_json::Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn require_string_field_ok() {
        let obj = obj(&[("wf", serde_json::json!("abc"))]);
        assert_eq!(require_string_field(&obj, "wf"), Ok("abc".to_string()));
    }

    #[test]
    fn require_string_field_absent() {
        let obj = obj(&[]);
        assert_eq!(
            require_string_field(&obj, "wf"),
            Err(Error::InvalidPayloadField("wf is required".to_string()))
        );
    }

    #[test]
    fn require_string_field_not_string() {
        let obj = obj(&[("wf", serde_json::json!(123))]);
        assert_eq!(
            require_string_field(&obj, "wf"),
            Err(Error::InvalidPayloadField(
                "wf must be a string".to_string()
            ))
        );
    }

    #[test]
    fn require_string_ok() {
        let obj = obj(&[("reason", serde_json::json!("timeout"))]);
        assert_eq!(require_string(&obj, "reason"), Ok("timeout".to_string()));
    }

    #[test]
    fn require_string_absent() {
        let obj = obj(&[]);
        assert!(matches!(
            require_string(&obj, "reason"),
            Err(Error::MissingPayloadField(_))
        ));
    }

    #[test]
    fn require_u64_ok() {
        let obj = obj(&[("ts", serde_json::json!(42))]);
        assert_eq!(require_u64(&obj, "ts"), Ok(42));
    }

    #[test]
    fn require_u64_absent() {
        let obj = obj(&[]);
        assert!(matches!(
            require_u64(&obj, "ts"),
            Err(Error::MissingPayloadField(_))
        ));
    }

    #[test]
    fn require_u64_not_integer() {
        let obj = obj(&[("ts", serde_json::json!("bad"))]);
        assert!(matches!(
            require_u64(&obj, "ts"),
            Err(Error::InvalidPayloadField(_))
        ));
    }

    #[test]
    fn optional_u64_present() {
        let obj = obj(&[("v", serde_json::json!(1))]);
        assert_eq!(optional_u64(&obj, "v", 0), 1);
    }

    #[test]
    fn optional_u64_absent() {
        let obj = obj(&[]);
        assert_eq!(optional_u64(&obj, "v", 0), 0);
    }

    #[test]
    fn optional_u64_not_integer() {
        let obj = obj(&[("v", serde_json::json!("bad"))]);
        assert_eq!(optional_u64(&obj, "v", 0), 0);
    }
}
