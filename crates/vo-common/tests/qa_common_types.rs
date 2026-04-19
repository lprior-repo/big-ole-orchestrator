//! QA static analysis validation tests for vo-common (ve-hf48p.2).

use vo_common::{NamespaceId, VoError, WorkflowEvent};
use vo_types::{InstanceId, TimerId};

#[test]
fn type_alias_instance_id_roundtrip() {
    let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID");
    assert_eq!(id.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
    let s: String = id.into_inner();
    assert_eq!(s, "01H5JYV4XHGSR2F8KZ9BWNRFMA");
}

#[test]
fn type_alias_namespace_id_roundtrip() {
    let ns = NamespaceId::new("ns/prod").unwrap();
    let s: String = ns.into_inner();
    assert_eq!(s, "ns/prod");
}

#[test]
fn type_alias_timer_id_roundtrip() {
    let t = TimerId::new("timer-abc").unwrap();
    let s: String = t.into_inner();
    assert_eq!(s, "timer-abc");
}

#[test]
fn type_aliases_are_zero_cost() {
    assert_eq!(
        std::mem::size_of::<InstanceId>(),
        std::mem::size_of::<String>()
    );
    assert_eq!(
        std::mem::size_of::<NamespaceId>(),
        std::mem::size_of::<String>()
    );
    assert_eq!(
        std::mem::size_of::<TimerId>(),
        std::mem::size_of::<String>()
    );
}

#[test]
fn error_config_display() {
    assert_eq!(VoError::config("x").to_string(), "configuration error: x");
}

#[test]
fn error_internal_display() {
    assert_eq!(VoError::internal("y").to_string(), "internal error: y");
}

#[test]
fn error_not_found_display() {
    assert_eq!(VoError::not_found("z").to_string(), "not found: z");
}

#[test]
fn error_validation_display() {
    assert_eq!(VoError::validation("w").to_string(), "validation failed: w");
}

#[test]
fn error_timeout_display() {
    assert_eq!(VoError::timeout("v").to_string(), "operation timed out: v");
}

#[test]
fn namespace_id_validation_rejects_empty() {
    let result = NamespaceId::new("");
    assert!(result.is_err());
}

#[test]
fn namespace_id_validation_rejects_control_chars() {
    let result = NamespaceId::new("ns\x00with\0null");
    assert!(result.is_err());
}

#[test]
fn workflow_event_serialization() {
    let event = WorkflowEvent::WorkflowStarted {
        workflow_id: "wf-123".into(),
        input_json: r#"{"key":"value"}"#.into(),
    };
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: WorkflowEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, deserialized);
}
