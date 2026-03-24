#![allow(clippy::expect_used)]
//! Tests for `WorkflowEvent` serialization.

use super::*;
use bytes::Bytes;

#[test]
fn roundtrip_msgpack_now_sampled() {
    let event = WorkflowEvent::NowSampled {
        operation_id: 3,
        ts: DateTime::parse_from_rfc3339("2026-01-01T12:00:00Z")
            .expect("parse")
            .with_timezone(&Utc),
    };
    let bytes = event.to_msgpack().expect("encode");
    let decoded = WorkflowEvent::from_msgpack(&bytes).expect("decode");
    assert_eq!(event, decoded);
}

#[test]
fn roundtrip_msgpack_activity_completed() {
    let event = WorkflowEvent::ActivityCompleted {
        activity_id: "act-001".into(),
        result: Bytes::from_static(b"ok"),
        duration_ms: 42,
    };
    let bytes = event.to_msgpack().expect("encode");
    let decoded = WorkflowEvent::from_msgpack(&bytes).expect("decode");
    assert_eq!(event, decoded);
}

#[test]
fn roundtrip_msgpack_snapshot_taken() {
    let event = WorkflowEvent::SnapshotTaken {
        seq: 99,
        checksum: 0xDEAD_BEEF,
    };
    let bytes = event.to_msgpack().expect("encode");
    let decoded = WorkflowEvent::from_msgpack(&bytes).expect("decode");
    assert_eq!(event, decoded);
}

#[test]
fn roundtrip_msgpack_transition_applied() {
    let event = WorkflowEvent::TransitionApplied {
        from_state: "Pending".into(),
        event_name: "Authorize".into(),
        to_state: "Authorized".into(),
        effects: vec![EffectDeclaration {
            effect_type: "CallAuthorizationService".into(),
            payload: Bytes::from_static(b"{}"),
        }],
    };
    let bytes = event.to_msgpack().expect("encode");
    let decoded = WorkflowEvent::from_msgpack(&bytes).expect("decode");
    assert_eq!(event, decoded);
}

#[test]
fn roundtrip_msgpack_timer_scheduled() {
    let event = WorkflowEvent::TimerScheduled {
        timer_id: "tmr-001".into(),
        fire_at: DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .expect("parse")
            .with_timezone(&Utc),
    };
    let bytes = event.to_msgpack().expect("encode");
    let decoded = WorkflowEvent::from_msgpack(&bytes).expect("decode");
    assert_eq!(event, decoded);
}

#[test]
fn serde_json_tag_is_snake_case() {
    let event = WorkflowEvent::SnapshotTaken {
        seq: 1,
        checksum: 0,
    };
    let json = serde_json::to_string(&event).expect("json");
    assert!(json.contains("\"type\":\"snapshot_taken\""), "got: {json}");
}

#[test]
fn serde_json_tag_activity_dispatched() {
    let event = WorkflowEvent::ActivityDispatched {
        activity_id: "a".into(),
        activity_type: "fetch".into(),
        payload: Bytes::new(),
        retry_policy: RetryPolicy::default(),
        attempt: 1,
    };
    let json = serde_json::to_string(&event).expect("json");
    assert!(
        json.contains("\"type\":\"activity_dispatched\""),
        "got: {json}"
    );
}

#[test]
fn retry_policy_default_has_three_attempts() {
    assert_eq!(RetryPolicy::default().max_attempts, 3);
}

#[test]
fn effect_declaration_roundtrips_msgpack() {
    let decl = EffectDeclaration {
        effect_type: "SendEmail".into(),
        payload: Bytes::from_static(b"payload"),
    };
    let bytes = rmp_serde::to_vec_named(&decl).expect("encode");
    let decoded: EffectDeclaration = rmp_serde::from_slice(&bytes).expect("decode");
    assert_eq!(decl, decoded);
}
