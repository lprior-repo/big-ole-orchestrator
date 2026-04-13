#[test]
fn timeline_entry_serializes_with_all_fields() {
    let entry = super::v3::TimelineEntry {
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
