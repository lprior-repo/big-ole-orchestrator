//! Receipt read/query tests (Fjall-backed).

use super::super::*;
use crate::effect_journal::EffectId;
use vo_types::ConnectorResult;
use vo_types::InstanceId;

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

fn sample_effect_id(intent: &str) -> EffectId {
    EffectId::new(&sample_instance_id(), intent).unwrap()
}

fn create_keyspace() -> (tempfile::TempDir, fjall::Keyspace) {
    let dir = tempfile::tempdir().expect("tempdir");
    let ks = fjall::Config::new(dir.path()).open().expect("keyspace");
    (dir, ks)
}

#[test]
fn fjall_get_nonexistent_receipt_returns_none() {
    let (_dir, keyspace) = create_keyspace();
    let store = FjallReceiptStore::open(&keyspace).unwrap();
    let eid = sample_effect_id("fx-nonexistent");

    let result = store.get(&eid).unwrap();
    assert!(result.is_none());
}

#[test]
fn fjall_contains_returns_false_for_nonexistent() {
    let (_dir, keyspace) = create_keyspace();
    let store = FjallReceiptStore::open(&keyspace).unwrap();
    let eid = sample_effect_id("fx-missing");

    let result = store.contains(&eid).unwrap();
    assert!(!result);
}

#[test]
fn fjall_contains_returns_true_after_store() {
    let (_dir, keyspace) = create_keyspace();
    let store = FjallReceiptStore::open(&keyspace).unwrap();

    let receipt = Receipt::new(
        sample_effect_id("fx-exists").as_str().to_string(),
        "conn".to_string(),
        ConnectorResult::Success,
        0,
        None,
    )
    .unwrap();

    store.store(receipt).unwrap();

    let eid = sample_effect_id("fx-exists");
    assert!(store.contains(&eid).unwrap());
}

#[test]
fn fjall_get_returns_correct_receipt_fields() {
    let (_dir, keyspace) = create_keyspace();
    let store = FjallReceiptStore::open(&keyspace).unwrap();

    let receipt = Receipt::new(
        sample_effect_id("fx-fields").as_str().to_string(),
        "kafka-connector".to_string(),
        ConnectorResult::Success,
        1713999999,
        Some(serde_json::json!({"topic": "orders", "partition": 3})),
    )
    .unwrap();

    store.store(receipt).unwrap();

    let eid = sample_effect_id("fx-fields");
    let retrieved = store.get(&eid).unwrap().unwrap();
    assert_eq!(retrieved.effect_id(), eid.as_str());
    assert_eq!(retrieved.connector_id(), "kafka-connector");
    assert_eq!(retrieved.result(), ConnectorResult::Success);
    assert_eq!(retrieved.committed_at_ms(), 1713999999);
    assert_eq!(
        retrieved.payload_json().cloned(), Some(serde_json::json!({"topic": "orders", "partition": 3}))
    );
}

#[test]
fn fjall_get_is_isolated_by_effect_id() {
    let (_dir, keyspace) = create_keyspace();
    let store = FjallReceiptStore::open(&keyspace).unwrap();

    let r1 = Receipt::new(
        sample_effect_id("fx-iso-a").as_str().to_string(),
        "conn-a".to_string(),
        ConnectorResult::Success,
        100,
        None,
    )
    .unwrap();

    let r2 = Receipt::new(
        sample_effect_id("fx-iso-b").as_str().to_string(),
        "conn-b".to_string(),
        ConnectorResult::Failure,
        200,
        None,
    )
    .unwrap();

    store.store(r1).unwrap();
    store.store(r2).unwrap();

    let eid_a = sample_effect_id("fx-iso-a");
    let eid_b = sample_effect_id("fx-iso-b");

    assert_eq!(store.get(&eid_a).unwrap().unwrap().connector_id(), "conn-a");
    assert_eq!(store.get(&eid_b).unwrap().unwrap().connector_id(), "conn-b");
}
