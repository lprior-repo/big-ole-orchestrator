//! QA static analysis validation tests for vo-common (ve-hf48p.2).

use vo_common::{EventId, InstanceId, NamespaceId, TimerId, VoError, WorkflowEvent};

#[test]
fn type_alias_instance_id_roundtrip() {
    let id: InstanceId = "inst-42".into();
    assert_eq!(id.as_str(), "inst-42");
    let s: String = id.to_string();
    assert_eq!(s, "inst-42");
}

#[test]
fn type_alias_namespace_id_roundtrip() {
    let ns: NamespaceId = "ns/prod".into();
    let s: String = ns.to_string();
    assert_eq!(s, "ns/prod");
}

#[test]
fn type_alias_timer_id_roundtrip() {
    let t: TimerId = "timer-abc".into();
    let s: String = t.to_string();
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
    assert_eq!(VoError::internal("x").to_string(), "internal error: x");
}

#[test]
fn error_not_found_display() {
    assert_eq!(VoError::not_found("x").to_string(), "not found: x");
}

#[test]
fn error_validation_display() {
    assert_eq!(VoError::validation("x").to_string(), "validation failed: x");
}

#[test]
fn error_timeout_display() {
    assert_eq!(VoError::timeout("x").to_string(), "operation timed out: x");
}

#[test]
fn error_config_ne_internal() {
    assert_ne!(VoError::config("m"), VoError::internal("m"));
}

#[test]
fn error_same_variant_equality() {
    assert_eq!(VoError::config("a"), VoError::config("a"));
}

#[test]
fn error_is_std_error_send_sync_clone() {
    fn check<E: std::error::Error + Send + Sync + Clone>(_e: E) {}
    check(VoError::config("x"));
}

#[test]
fn workflow_event_json_roundtrip() {
    let event = WorkflowEvent::TimerFired {
        timer_id: "t1".into(),
        timestamp_ms: 999,
    };
    let json = serde_json::to_string(&event).unwrap();
    assert_eq!(event, serde_json::from_str(&json).unwrap());
}

#[test]
fn workflow_event_json_structure() {
    let val = serde_json::to_value(&WorkflowEvent::TimerFired {
        timer_id: "s".into(),
        timestamp_ms: 0,
    })
    .unwrap();
    assert!(val.as_object().unwrap().contains_key("TimerFired"));
}

#[test]
fn workflow_event_u64_max_roundtrip() {
    let e = WorkflowEvent::TimerFired {
        timer_id: "x".into(),
        timestamp_ms: u64::MAX,
    };
    assert_eq!(
        e,
        serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap()
    );
}

#[test]
fn workflow_event_rejects_null() {
    assert!(serde_json::from_str::<WorkflowEvent>("null").is_err());
}

#[test]
fn workflow_event_rejects_unknown_variant() {
    assert!(serde_json::from_str::<WorkflowEvent>(r#"{"Unknown":{}}"#).is_err());
}

#[test]
fn workflow_event_unicode_roundtrip() {
    let e = WorkflowEvent::TimerFired {
        timer_id: "计时🚀".into(),
        timestamp_ms: 1,
    };
    assert_eq!(
        e,
        serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap()
    );
}
