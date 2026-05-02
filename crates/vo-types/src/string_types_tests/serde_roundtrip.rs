use std::collections::HashMap;

use crate::*;

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

#[test]
fn serde_instance_id_map_key_sort_order_and_roundtrip() {
    let base = "01H5JYV4XHGSR2F8KZ9BWNRFMA";
    let mut instance_ids = Vec::with_capacity(100);
    for i in 0..100u64 {
        let ts = 1_700_000_000_000u64 + i * 1000;
        let entropy = (i as u128) << 64;
        let ulid_str = ulid::Ulid::new_with(ts, entropy).to_string();
        instance_ids.push(InstanceId::parse(&ulid_str).expect("valid ULID"));
    }

    let mut map: HashMap<InstanceId, String> = HashMap::new();
    for (i, id) in instance_ids.iter().enumerate() {
        map.insert(*id, format!("value_{i}"));
    }

    let json = serde_json::to_value(&map).expect("serialize HashMap<InstanceId, String>");

    let json_obj = json.as_object().expect("JSON must be an object");

    let keys: Vec<&str> = json_obj.keys().map(|k| k.as_str()).collect();
    let sorted_keys: Vec<&str> = {
        let mut s = keys.clone();
        s.sort();
        s
    };
    assert_eq!(keys, sorted_keys, "JSON keys must be sorted lexicographically by ULID timestamp");

    let restored: HashMap<InstanceId, String> =
        serde_json::from_value(json).expect("deserialize HashMap back");

    assert_eq!(restored.len(), map.len(), "all entries must be preserved");
    for (id, value) in &map {
        assert_eq!(
            restored.get(id), Some(value),
            "entry for {:?} must roundtrip correctly",
            id
        );
    }
    assert_eq!(restored, map, "HashMap must be identical after roundtrip");
}
