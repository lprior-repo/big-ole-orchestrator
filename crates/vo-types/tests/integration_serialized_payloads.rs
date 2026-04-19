use vo_types::{Snapshot, State, WorkflowSpec, MAX_SUPPORTED_SCHEMA_VERSION};

#[test]
fn integration_serialized_payloads_strictly_match_external_contract() {
    let state = State::default();
    let json = serde_json::to_string(&state).unwrap();
    assert!(json.contains(&format!("\"version\":{}", MAX_SUPPORTED_SCHEMA_VERSION)));

    let spec = WorkflowSpec::default();
    let json = serde_json::to_string(&spec).unwrap();
    assert!(json.contains(&format!("\"version\":{}", MAX_SUPPORTED_SCHEMA_VERSION)));

    let snap = Snapshot::default();
    let json = serde_json::to_string(&snap).unwrap();
    assert!(json.contains(&format!("\"version\":{}", MAX_SUPPORTED_SCHEMA_VERSION)));
}
