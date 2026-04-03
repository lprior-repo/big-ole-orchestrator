#[allow(unused_imports)]
use super::*;
use crate::events::Error;
use crate::types::{
    extract_schema_version, Snapshot, State, WorkflowSpec, MAX_SUPPORTED_SCHEMA_VERSION,
};

#[test]
fn state_version_returns_current_schema_version_for_default() {
    let state = State::default();
    assert_eq!(state.version(), MAX_SUPPORTED_SCHEMA_VERSION);
}

#[test]
fn state_version_returns_inner_field_value() {
    let state = State { version: 99 };
    assert_eq!(state.version(), 99);
}

#[test]
fn workflow_spec_version_returns_current_schema_version_for_default() {
    let spec = WorkflowSpec::default();
    assert_eq!(spec.version(), MAX_SUPPORTED_SCHEMA_VERSION);
}

#[test]
fn workflow_spec_version_returns_inner_field_value() {
    let spec = WorkflowSpec { version: 99 };
    assert_eq!(spec.version(), 99);
}

#[test]
fn snapshot_version_returns_current_schema_version_for_default() {
    let snap = Snapshot::default();
    assert_eq!(snap.version(), MAX_SUPPORTED_SCHEMA_VERSION);
}

#[test]
fn snapshot_version_returns_inner_field_value() {
    let snap = Snapshot { version: 99 };
    assert_eq!(snap.version(), 99);
}

#[test]
fn state_version_returns_0_for_legacy_payload() {
    let payload = serde_json::json!({});
    // Should successfully deserialize using legacy fallback policy (not implemented yet, will fail)
    let state: State = serde_json::from_value(payload).unwrap();
    assert_eq!(state.version(), 0);
}

#[test]
fn state_serializes_with_explicit_schema_version_one() {
    let state = State::default();
    let json = serde_json::to_value(&state).unwrap();
    assert_eq!(json.get("version").unwrap().as_u64().unwrap(), 1);
}

#[test]
fn state_serializes_with_dynamic_inner_version() {
    let state = State { version: 99 };
    let json = serde_json::to_value(&state).unwrap();
    assert_eq!(json.get("version").unwrap().as_u64().unwrap(), 99);
}

#[test]
fn workflow_spec_serializes_with_explicit_schema_version_one() {
    let spec = WorkflowSpec::default();
    let json = serde_json::to_value(&spec).unwrap();
    assert_eq!(json.get("version").unwrap().as_u64().unwrap(), 1);
}

#[test]
fn workflow_spec_serializes_with_dynamic_inner_version() {
    let spec = WorkflowSpec { version: 99 };
    let json = serde_json::to_value(&spec).unwrap();
    assert_eq!(json.get("version").unwrap().as_u64().unwrap(), 99);
}

#[test]
fn snapshot_serializes_with_explicit_schema_version_one() {
    let snap = Snapshot::default();
    let json = serde_json::to_value(&snap).unwrap();
    assert_eq!(json.get("version").unwrap().as_u64().unwrap(), 1);
}

#[test]
fn snapshot_serializes_with_dynamic_inner_version() {
    let snap = Snapshot { version: 99 };
    let json = serde_json::to_value(&snap).unwrap();
    assert_eq!(json.get("version").unwrap().as_u64().unwrap(), 99);
}

#[test]
fn state_deserializes_successfully_when_version_is_explicitly_one() {
    let payload = serde_json::json!({ "version": 1 });
    let state: State = serde_json::from_value(payload).unwrap();
    assert_eq!(state, State { version: 1 });
}

#[test]
fn workflow_spec_deserializes_successfully_when_version_is_explicitly_one() {
    let payload = serde_json::json!({ "version": 1 });
    let spec: WorkflowSpec = serde_json::from_value(payload).unwrap();
    assert_eq!(spec, WorkflowSpec { version: 1 });
}

#[test]
fn snapshot_deserializes_successfully_when_version_is_explicitly_one() {
    let payload = serde_json::json!({ "version": 1 });
    let snap: Snapshot = serde_json::from_value(payload).unwrap();
    assert_eq!(snap, Snapshot { version: 1 });
}

#[test]
fn state_deserialization_fails_on_future_version() {
    let payload = serde_json::json!({ "version": 2 });
    let result: Result<State, _> = serde_json::from_value(payload);
    let err_str = result.unwrap_err().to_string();
    assert_eq!(err_str, "Unsupported schema version: 2");
}

#[test]
fn state_deserialization_fails_on_invalid_format() {
    let payload = serde_json::json!({ "version": "1" });
    let result: Result<State, _> = serde_json::from_value(payload);
    assert_eq!(
        result.unwrap_err().to_string(),
        "Invalid schema version format"
    );
}

#[test]
fn workflow_spec_deserialization_fails_on_future_version() {
    let payload = serde_json::json!({ "version": 2 });
    let result: Result<WorkflowSpec, _> = serde_json::from_value(payload);
    assert_eq!(
        result.unwrap_err().to_string(),
        "Unsupported schema version: 2"
    );
}

#[test]
fn workflow_spec_deserialization_fails_on_invalid_format() {
    let payload = serde_json::json!({ "version": "1" });
    let result: Result<WorkflowSpec, _> = serde_json::from_value(payload);
    assert_eq!(
        result.unwrap_err().to_string(),
        "Invalid schema version format"
    );
}

#[test]
fn workflow_spec_deserialization_fails_when_missing() {
    let payload = serde_json::json!({});
    let result: Result<WorkflowSpec, _> = serde_json::from_value(payload);
    assert_eq!(result.unwrap_err().to_string(), "Missing schema version");
}

#[test]
fn snapshot_deserialization_fails_on_future_version() {
    let payload = serde_json::json!({ "version": 2 });
    let result: Result<Snapshot, _> = serde_json::from_value(payload);
    assert_eq!(
        result.unwrap_err().to_string(),
        "Unsupported schema version: 2"
    );
}

#[test]
fn snapshot_deserialization_fails_on_invalid_format() {
    let payload = serde_json::json!({ "version": "1" });
    let result: Result<Snapshot, _> = serde_json::from_value(payload);
    assert_eq!(
        result.unwrap_err().to_string(),
        "Invalid schema version format"
    );
}

#[test]
fn snapshot_deserialization_fails_when_missing() {
    let payload = serde_json::json!({});
    let result: Result<Snapshot, _> = serde_json::from_value(payload);
    assert_eq!(result.unwrap_err().to_string(), "Missing schema version");
}

#[test]
fn extract_schema_version_prioritizes_payload_over_fallback() {
    let payload = serde_json::json!({ "version": 1 });
    assert_eq!(extract_schema_version(&payload, Some(0)), Ok(1));
}

#[test]
fn schema_extraction_returns_fallback_version_when_missing_and_policy_is_zero() {
    let payload = serde_json::json!({});
    assert_eq!(extract_schema_version(&payload, Some(0)), Ok(0));
}

#[test]
fn schema_extraction_returns_fallback_version_when_missing_and_policy_is_max_supported() {
    let payload = serde_json::json!({});
    assert_eq!(extract_schema_version(&payload, Some(1)), Ok(1));
}

#[test]
fn schema_extraction_returns_fallback_version_when_missing_and_policy_is_future() {
    let payload = serde_json::json!({});
    assert_eq!(extract_schema_version(&payload, Some(2)), Ok(2));
}

#[test]
fn schema_extraction_returns_unsupported_error_for_future_version_despite_fallback() {
    let payload = serde_json::json!({ "version": 2 });
    assert_eq!(
        extract_schema_version(&payload, Some(0)),
        Err(Error::UnsupportedSchemaVersion(2))
    );
}

#[test]
fn schema_extraction_returns_invalid_format_error_for_string_despite_fallback() {
    let payload = serde_json::json!({ "version": "1" });
    assert_eq!(
        extract_schema_version(&payload, Some(0)),
        Err(Error::InvalidSchemaVersionFormat)
    );
}

#[test]
fn schema_extraction_succeeds_when_version_is_exactly_max_supported() {
    let payload = serde_json::json!({ "version": 1 });
    assert_eq!(extract_schema_version(&payload, None), Ok(1));
}

#[test]
fn schema_extraction_succeeds_when_version_is_exactly_zero() {
    let payload = serde_json::json!({ "version": 0 });
    assert_eq!(extract_schema_version(&payload, None), Ok(0));
}

#[test]
fn schema_extraction_returns_unsupported_version_error_for_future_version() {
    let payload = serde_json::json!({ "version": 2 });
    assert_eq!(
        extract_schema_version(&payload, None),
        Err(Error::UnsupportedSchemaVersion(2))
    );
}

#[test]
fn schema_extraction_returns_unsupported_version_error_for_version_256() {
    let payload = serde_json::json!({ "version": 256 });
    assert_eq!(
        extract_schema_version(&payload, None),
        Err(Error::UnsupportedSchemaVersion(256))
    );
}

#[test]
fn schema_extraction_returns_unsupported_version_error_for_version_65535() {
    let payload = serde_json::json!({ "version": 65535 });
    assert_eq!(
        extract_schema_version(&payload, None),
        Err(Error::UnsupportedSchemaVersion(65535))
    );
}

#[test]
fn schema_extraction_returns_invalid_format_error_when_version_is_string() {
    let payload = serde_json::json!({ "version": "1" });
    assert_eq!(
        extract_schema_version(&payload, None),
        Err(Error::InvalidSchemaVersionFormat)
    );
}

#[test]
fn schema_extraction_returns_invalid_format_error_when_version_is_float() {
    let payload = serde_json::json!({ "version": 1.5 });
    assert_eq!(
        extract_schema_version(&payload, None),
        Err(Error::InvalidSchemaVersionFormat)
    );
}

#[test]
fn schema_extraction_returns_invalid_format_error_when_version_is_float_integer() {
    let payload = serde_json::json!({ "version": 1.0 });
    // serde_json might parse 1.0 as u64 or f64. If it treats as f64, we should reject it.
    assert_eq!(
        extract_schema_version(&payload, None),
        Err(Error::InvalidSchemaVersionFormat)
    );
}

#[test]
fn schema_extraction_returns_invalid_format_error_when_version_is_null() {
    let payload = serde_json::json!({ "version": null });
    assert_eq!(
        extract_schema_version(&payload, None),
        Err(Error::InvalidSchemaVersionFormat)
    );
}

#[test]
fn schema_extraction_returns_invalid_format_error_when_version_is_negative() {
    let payload = serde_json::json!({ "version": -1 });
    assert_eq!(
        extract_schema_version(&payload, None),
        Err(Error::InvalidSchemaVersionFormat)
    );
}

#[test]
fn schema_extraction_returns_invalid_format_error_when_version_is_boolean() {
    let payload = serde_json::json!({ "version": true });
    assert_eq!(
        extract_schema_version(&payload, None),
        Err(Error::InvalidSchemaVersionFormat)
    );
}

#[test]
fn schema_extraction_returns_invalid_format_error_when_version_exceeds_u16() {
    let payload = serde_json::json!({ "version": 65536 });
    assert_eq!(
        extract_schema_version(&payload, None),
        Err(Error::InvalidSchemaVersionFormat)
    );
}

#[test]
fn schema_extraction_returns_missing_schema_version_error_when_no_fallback_exists() {
    let payload = serde_json::json!({ "other": 1 });
    assert_eq!(
        extract_schema_version(&payload, None),
        Err(Error::MissingSchemaVersion)
    );
}

#[test]
fn schema_extraction_returns_missing_schema_version_error_for_empty_object() {
    let payload = serde_json::json!({});
    assert_eq!(
        extract_schema_version(&payload, None),
        Err(Error::MissingSchemaVersion)
    );
}

#[test]
fn schema_extraction_returns_invalid_format_error_when_payload_is_not_an_object() {
    let payload = serde_json::json!([]);
    assert_eq!(
        extract_schema_version(&payload, None),
        Err(Error::InvalidSchemaVersionFormat)
    );
}

#[cfg(feature = "proptest")]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn extract_schema_version_logical_properties(val in proptest::num::u64::ANY) {
            // Just verify basic roundtrip and bounds
            if val <= 1 {
                let payload = serde_json::json!({ "version": val });
                prop_assert_eq!(extract_schema_version(&payload, None), Ok(val as u16));
            } else if val <= u16::MAX as u64 {
                let payload = serde_json::json!({ "version": val });
                prop_assert_eq!(extract_schema_version(&payload, None), Err(Error::UnsupportedSchemaVersion(val as u16)));
            } else {
                let payload = serde_json::json!({ "version": val });
                prop_assert_eq!(extract_schema_version(&payload, None), Err(Error::InvalidSchemaVersionFormat));
            }
        }

        #[test]
        fn state_serialization_roundtrip(version in 0..=1u16) {
            let state = State { version };
            let serialized = serde_json::to_string(&state).unwrap();
            let deserialized: State = serde_json::from_str(&serialized).unwrap();
            prop_assert_eq!(state, deserialized);
        }

        #[test]
        fn workflow_spec_serialization_roundtrip(version in 0..=1u16) {
            let spec = WorkflowSpec { version };
            let serialized = serde_json::to_string(&spec).unwrap();
            let deserialized: WorkflowSpec = serde_json::from_str(&serialized).unwrap();
            prop_assert_eq!(spec, deserialized);
        }

        #[test]
        fn snapshot_serialization_roundtrip(version in 0..=1u16) {
            let snap = Snapshot { version };
            let serialized = serde_json::to_string(&snap).unwrap();
            let deserialized: Snapshot = serde_json::from_str(&serialized).unwrap();
            prop_assert_eq!(snap, deserialized);
        }
    }
}
