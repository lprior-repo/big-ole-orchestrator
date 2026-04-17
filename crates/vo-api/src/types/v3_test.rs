#[allow(clippy::unwrap_used, clippy::expect_used)]
use super::errors::ApiError;
use super::v3::*;

#[test]
fn timeline_entry_serializes_with_all_fields() {
    let entry = TimelineEntry {
        sequence: 1,
        timestamp_ms: 1_714_000_000_000,
        event_type: "workflow_started".to_string(),
        payload: serde_json::json!({"workflow_id": "wf-1"}),
    };
    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("\"sequence\":1"));
    assert!(json.contains("\"event_type\":\"workflow_started\""));
    assert!(json.contains("\"timestamp_ms\":1714000000000"));
}

#[test]
fn v3_start_request_skip_none_fields() {
    let req = V3StartRequest {
        namespace: "ns".to_string(),
        workflow_type: "wf".to_string(),
        paradigm: "fsm".to_string(),
        input: serde_json::json!({}),
        instance_id: None,
        dedupe_key: None,
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(!json.contains("instance_id"));
    assert!(!json.contains("dedupe_key"));
}

#[test]
fn v3_start_request_with_optional_fields() {
    let req = V3StartRequest {
        namespace: "ns".to_string(),
        workflow_type: "wf".to_string(),
        paradigm: "dag".to_string(),
        input: serde_json::json!({"key": "val"}),
        instance_id: Some("custom-id".to_string()),
        dedupe_key: Some("dedupe-1".to_string()),
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("custom-id"));
    assert!(json.contains("dedupe-1"));
}

#[test]
fn v3_start_request_roundtrip() {
    let req = V3StartRequest {
        namespace: "payments".to_string(),
        workflow_type: "charge".to_string(),
        paradigm: "procedural".to_string(),
        input: serde_json::json!({"amount": 100}),
        instance_id: None,
        dedupe_key: None,
    };
    let json = serde_json::to_string(&req).unwrap();
    let back: V3StartRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(back.namespace, "payments");
    assert_eq!(back.workflow_type, "charge");
}

#[test]
fn v3_start_response_roundtrip() {
    let resp = V3StartResponse {
        instance_id: "abc123".to_string(),
        namespace: "ns".to_string(),
        workflow_type: "wf".to_string(),
    };
    let json = serde_json::to_string(&resp).unwrap();
    let back: V3StartResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(back.instance_id, "abc123");
}

#[test]
fn v3_status_response_roundtrip() {
    let resp = V3StatusResponse {
        instance_id: "id".to_string(),
        namespace: "ns".to_string(),
        workflow_type: "wf".to_string(),
        paradigm: "fsm".to_string(),
        phase: "live".to_string(),
        events_applied: 42,
    };
    let json = serde_json::to_string(&resp).unwrap();
    let back: V3StatusResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(back.events_applied, 42);
    assert_eq!(back.phase, "live");
}

#[test]
fn v3_signal_request_roundtrip() {
    let req = V3SignalRequest {
        signal_name: "approve".to_string(),
        payload: serde_json::json!({"approved": true}),
    };
    let json = serde_json::to_string(&req).unwrap();
    let back: V3SignalRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(back.signal_name, "approve");
}

#[test]
fn api_error_new() {
    let err = ApiError::new("not_found", "workflow missing");
    assert_eq!(err.error, "not_found");
    assert_eq!(err.message, "workflow missing");
}

#[test]
fn effect_semantics_serde() {
    let exact = EffectSemantics::Exact;
    assert_eq!(serde_json::to_string(&exact).unwrap(), "\"exact\"");

    let unsafe_sem = EffectSemantics::Unsafe;
    assert_eq!(serde_json::to_string(&unsafe_sem).unwrap(), "\"unsafe\"");

    let back: EffectSemantics = serde_json::from_str("\"exact\"").unwrap();
    assert_eq!(back, EffectSemantics::Exact);
}

#[test]
fn timeline_entry_roundtrip() {
    let entry = TimelineEntry {
        sequence: 1,
        timestamp_ms: 1700000000000,
        event_type: "workflow_started".to_string(),
        payload: serde_json::json!({}),
    };
    let json = serde_json::to_string(&entry).unwrap();
    let back: TimelineEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(back.sequence, 1);
    assert_eq!(back.event_type, "workflow_started");
}

#[test]
fn history_entry_skip_none_fields() {
    let entry = HistoryEntry {
        sequence: 1,
        timestamp_ms: 1700000000000,
        event_type: "step_completed".to_string(),
        step_id: None,
        error: None,
        output: None,
    };
    let json = serde_json::to_string(&entry).unwrap();
    assert!(!json.contains("step_id"));
    assert!(!json.contains("error"));
    assert!(!json.contains("output"));
}

#[test]
fn history_entry_with_all_fields() {
    let entry = HistoryEntry {
        sequence: 1,
        timestamp_ms: 1700000000000,
        event_type: "step_failed".to_string(),
        step_id: Some("step-1".to_string()),
        error: Some("timeout".to_string()),
        output: None,
    };
    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("step-1"));
    assert!(json.contains("timeout"));
}

#[test]
fn effect_journal_entry_roundtrip() {
    let entry = EffectJournalEntry {
        sequence: 1,
        timestamp_ms: 1700000000000,
        event_type: "effect_committed".to_string(),
        semantics: EffectSemantics::Exact,
        payload: serde_json::json!({"key": "val"}),
    };
    let json = serde_json::to_string(&entry).unwrap();
    let back: EffectJournalEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(back.semantics, EffectSemantics::Exact);
}

#[test]
fn workflow_version_response_null_fields() {
    let resp = WorkflowVersionResponse {
        instance_id: "id".to_string(),
        schema_version: 1,
        event_count: 10,
        last_sequence: None,
        last_timestamp_ms: None,
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("null"));
}
