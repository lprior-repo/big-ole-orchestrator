use super::*;

#[test]
fn serde_round_trip_instance_id_inline() {
    let original = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid");
    let json = serde_json::to_value(&original).expect("serialize");
    let restored: InstanceId = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_workflow_name_inline() {
    let original = WorkflowName::parse("deploy-prod").expect("valid");
    let json = serde_json::to_value(&original).expect("serialize");
    let restored: WorkflowName = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_node_name_inline() {
    let original = NodeName::parse("compile-artifact").expect("valid");
    let json = serde_json::to_value(&original).expect("serialize");
    let restored: NodeName = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_binary_hash_inline() {
    let original = BinaryHash::parse("abcdef0123456789").expect("valid");
    let json = serde_json::to_value(&original).expect("serialize");
    let restored: BinaryHash = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_timer_id_inline() {
    let original = TimerId::parse("timer-123").expect("valid");
    let json = serde_json::to_value(&original).expect("serialize");
    let restored: TimerId = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_idempotency_key_inline() {
    let original = IdempotencyKey::parse("key-abc").expect("valid");
    let json = serde_json::to_value(&original).expect("serialize");
    let restored: IdempotencyKey = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}