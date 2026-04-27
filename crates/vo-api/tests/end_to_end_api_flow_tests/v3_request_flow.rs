use super::*;

#[test]
fn minimal_start_request_serializes_correctly() {
    let req = V3StartRequest {
        namespace: "payments".to_string(),
        workflow_type: "checkout".to_string(),
        paradigm: "fsm".to_string(),
        input: json!({"order_id": "ord_123"}),
        instance_id: None,
        dedupe_key: None,
        workflow_binary_hash: None,
    };
    let json_str = serde_json::to_string(&req).unwrap();
    assert!(json_str.contains(r#""namespace":"payments""#));
    assert!(json_str.contains(r#""workflow_type":"checkout""#));
    assert!(json_str.contains(r#""paradigm":"fsm""#));
    assert!(!json_str.contains("instance_id"));
    assert!(!json_str.contains("dedupe_key"));
}

#[test]
fn full_start_request_with_all_fields() {
    let req = V3StartRequest {
        namespace: "orders".to_string(),
        workflow_type: "process_order".to_string(),
        paradigm: "dag".to_string(),
        input: json!({"items": ["a", "b"], "priority": 1}),
        instance_id: Some("01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string()),
        dedupe_key: Some("dedupe-abc-123".to_string()),
        workflow_binary_hash: None,
    };
    let json_str = serde_json::to_string(&req).unwrap();
    assert!(json_str.contains(r#""instance_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV""#));
    assert!(json_str.contains(r#""dedupe_key":"dedupe-abc-123""#));
}

#[test]
fn start_request_deserializes_from_json() {
    let json_val = json!({
        "namespace": "inventory",
        "workflow_type": "stock_check",
        "paradigm": "procedural",
        "input": {"sku": "ABC123"}
    });
    let req: V3StartRequest = serde_json::from_value(json_val).unwrap();
    assert_eq!(req.namespace, "inventory");
    assert_eq!(req.workflow_type, "stock_check");
    assert_eq!(req.paradigm, "procedural");
    assert_eq!(req.input["sku"], "ABC123");
}

#[test]
fn start_request_with_instance_id_roundtrip() {
    let req = V3StartRequest {
        namespace: "test".to_string(),
        workflow_type: "wf".to_string(),
        paradigm: "fsm".to_string(),
        input: json!({}),
        instance_id: Some("01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string()),
        dedupe_key: None,
        workflow_binary_hash: None,
    };
    let serialized = serde_json::to_string(&req).unwrap();
    let deserialized: V3StartRequest = serde_json::from_str(&serialized).unwrap();
    assert_eq!(req.instance_id, deserialized.instance_id);
}

#[test]
fn signal_request_serializes() {
    let req = V3SignalRequest {
        signal_name: "payment_approved".to_string(),
        payload: json!({"amount": 100, "currency": "USD"}),
    };
    let json_str = serde_json::to_string(&req).unwrap();
    assert!(json_str.contains(r#""signal_name":"payment_approved""#));
    assert!(json_str.contains(r#""amount":100"#));
}

#[test]
fn signal_request_roundtrip() {
    let req = V3SignalRequest {
        signal_name: "cancel".to_string(),
        payload: json!({"reason": "user_requested", "cancelled_at": "2024-01-15T10:30:00Z"}),
    };
    let serialized = serde_json::to_string(&req).unwrap();
    let deserialized: V3SignalRequest = serde_json::from_str(&serialized).unwrap();
    assert_eq!(req.signal_name, deserialized.signal_name);
    assert_eq!(req.payload, deserialized.payload);
}

#[test]
fn signal_request_with_complex_payload() {
    let payload = json!({
        "nested": {
            "deep": {
                "value": [1, 2, 3]
            }
        },
        "null_field": null,
        "bool_field": true
    });
    let req = V3SignalRequest {
        signal_name: "complex_signal".to_string(),
        payload,
    };
    let serialized = serde_json::to_string(&req).unwrap();
    let deserialized: V3SignalRequest = serde_json::from_str(&serialized).unwrap();
    assert_eq!(
        deserialized.payload["nested"]["deep"]["value"],
        json!([1, 2, 3])
    );
}
