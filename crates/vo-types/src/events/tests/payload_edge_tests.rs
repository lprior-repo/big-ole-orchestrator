
use crate::events::error::Error;
use crate::events::payload::EventPayload;

// -------------------------------------------------------------------------
// ADR-038: ContinuedAsNew tests
// -------------------------------------------------------------------------

#[test]
fn payload_try_from_json_returns_continued_as_new_when_type_matches() {
    let json = serde_json::json!({
        "type": "ContinuedAsNew",
        "workflow_id": "wf-1",
        "lineage_id": "lin-abc-123",
        "old_epoch": 0,
        "new_epoch": 1,
        "version": 1
    });
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Ok(EventPayload::ContinuedAsNew {
            workflow_id: "wf-1".to_string(),
            lineage_id: "lin-abc-123".to_string(),
            old_epoch: 0,
            new_epoch: 1,
        })
    );
}

#[test]
fn payload_try_from_json_returns_missing_payload_field_when_lineage_id_absent() {
    let json = serde_json::json!({
        "type": "ContinuedAsNew",
        "workflow_id": "wf-1",
        "old_epoch": 0,
        "new_epoch": 1,
        "version": 1
    });
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Err(Error::MissingPayloadField("lineage_id".to_string()))
    );
}

#[test]
fn payload_try_from_json_returns_missing_payload_field_when_old_epoch_absent() {
    let json = serde_json::json!({
        "type": "ContinuedAsNew",
        "workflow_id": "wf-1",
        "lineage_id": "lin-1",
        "new_epoch": 1,
        "version": 1
    });
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Err(Error::MissingPayloadField("old_epoch".to_string()))
    );
}

#[test]
fn payload_try_from_json_returns_missing_payload_field_when_new_epoch_absent() {
    let json = serde_json::json!({
        "type": "ContinuedAsNew",
        "workflow_id": "wf-1",
        "lineage_id": "lin-1",
        "old_epoch": 0,
        "version": 1
    });
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Err(Error::MissingPayloadField("new_epoch".to_string()))
    );
}

#[test]
fn payload_try_from_json_returns_invalid_payload_field_when_old_epoch_not_integer() {
    let json = serde_json::json!({
        "type": "ContinuedAsNew",
        "workflow_id": "wf-1",
        "lineage_id": "lin-1",
        "old_epoch": "bad",
        "new_epoch": 1,
        "version": 1
    });
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Err(Error::InvalidPayloadField(
            "old_epoch must be an integer".to_string()
        ))
    );
}

#[test]
fn payload_try_from_json_returns_invalid_payload_field_when_new_epoch_not_integer() {
    let json = serde_json::json!({
        "type": "ContinuedAsNew",
        "workflow_id": "wf-1",
        "lineage_id": "lin-1",
        "old_epoch": 0,
        "new_epoch": "bad",
        "version": 1
    });
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Err(Error::InvalidPayloadField(
            "new_epoch must be an integer".to_string()
        ))
    );
}

#[test]
fn payload_try_from_json_returns_unknown_payload_type_when_type_is_unrecognized() {
    let json = serde_json::json!({"type": "UnknownType", "workflow_id": "wf-123", "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Err(Error::UnknownPayloadType("UnknownType".to_string()))
    );
}

#[test]
fn payload_try_from_json_returns_unsupported_payload_version_when_version_exceeds_max() {
    let json =
        serde_json::json!({"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 2});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(result, Err(Error::UnsupportedPayloadVersion(2)));
}

#[test]
fn payload_try_from_json_returns_missing_payload_field_when_type_is_absent() {
    let json = serde_json::json!({"workflow_id": "wf-123", "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(result, Err(Error::MissingPayloadField("type".to_string())));
}

#[test]
fn payload_try_from_json_returns_invalid_payload_field_when_variant_field_is_absent() {
    let json = serde_json::json!({"type": "WorkflowStarted", "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert!(matches!(result, Err(Error::InvalidPayloadField(_))));
}

#[test]
fn payload_try_from_json_returns_invalid_payload_format_when_json_is_malformed() {
    let json = serde_json::Value::String("not an object".to_string());
    let result = EventPayload::try_from_json(&json);
    assert_eq!(result, Err(Error::InvalidPayloadFormat));
}

#[test]
fn payload_is_version_supported_returns_true_when_version_is_zero() {
    assert!(EventPayload::is_version_supported(0));
}

#[test]
fn payload_is_version_supported_returns_true_when_version_is_one() {
    assert!(EventPayload::is_version_supported(1));
}

#[test]
fn payload_is_version_supported_returns_false_when_version_is_two() {
    assert!(!EventPayload::is_version_supported(2));
}

#[test]
fn payload_is_version_supported_returns_false_when_version_is_u8_max() {
    assert!(!EventPayload::is_version_supported(u8::MAX));
}
