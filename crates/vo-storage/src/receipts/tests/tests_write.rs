//! Receipt write persistence tests (Fjall-backed).

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

fn make_receipt(intent: &str) -> Receipt {
    Receipt::new(
        sample_effect_id(intent).as_str().to_string(),
        "stripe-connector".to_string(),
        ConnectorResult::Success,
        1713000000,
        Some(serde_json::json!({"charge_id": "ch_123"})),
    )
    .unwrap()
}

#[test]
fn fjall_store_persists_receipt_and_get_returns_it() {
    let (_dir, keyspace) = create_keyspace();
    let store = FjallReceiptStore::open(&keyspace).unwrap();
    let receipt = make_receipt("fx-write-1");
    let eid = sample_effect_id("fx-write-1");

    store.store(receipt).unwrap();

    let retrieved = store.get(&eid).unwrap();
    assert!(retrieved.is_some());
    let r = retrieved.unwrap();
    assert_eq!(r.effect_id(), eid.as_str());
    assert_eq!(r.connector_id(), "stripe-connector");
    assert_eq!(r.result(), ConnectorResult::Success);
}

#[test]
fn fjall_store_receipt_with_failure_result() {
    let (_dir, keyspace) = create_keyspace();
    let store = FjallReceiptStore::open(&keyspace).unwrap();

    let receipt = Receipt::new(
        sample_effect_id("fx-fail-1").as_str().to_string(),
        "sql-connector".to_string(),
        ConnectorResult::Failure,
        1713000001,
        Some(serde_json::json!({"error": "deadlock"})),
    )
    .unwrap();

    store.store(receipt).unwrap();

    let eid = sample_effect_id("fx-fail-1");
    let retrieved = store.get(&eid).unwrap().unwrap();
    assert_eq!(retrieved.result(), ConnectorResult::Failure);
    assert_eq!(
        retrieved.payload_json().cloned(), Some(serde_json::json!({"error": "deadlock"}))
    );
}

#[test]
fn fjall_store_receipt_without_payload() {
    let (_dir, keyspace) = create_keyspace();
    let store = FjallReceiptStore::open(&keyspace).unwrap();

    let receipt = Receipt::new(
        sample_effect_id("fx-nopayload").as_str().to_string(),
        "s3-connector".to_string(),
        ConnectorResult::Success,
        1713000002,
        None,
    )
    .unwrap();

    store.store(receipt).unwrap();

    let eid = sample_effect_id("fx-nopayload");
    let retrieved = store.get(&eid).unwrap().unwrap();
    assert!(retrieved.payload_json().is_none());
}

#[test]
fn fjall_store_multiple_receipts_for_different_effects() {
    let (_dir, keyspace) = create_keyspace();
    let store = FjallReceiptStore::open(&keyspace).unwrap();

    for i in 0..5 {
        let receipt = Receipt::new(
            sample_effect_id(&format!("fx-batch-{i}"))
                .as_str()
                .to_string(),
            format!("connector-{i}"),
            ConnectorResult::Success,
            1713000000 + i as u64,
            Some(serde_json::json!({"i": i})),
        )
        .unwrap();
        store.store(receipt).unwrap();
    }

    for i in 0..5 {
        let eid = sample_effect_id(&format!("fx-batch-{i}"));
        let retrieved = store.get(&eid).unwrap().unwrap();
        assert_eq!(retrieved.connector_id(), format!("connector-{i}"));
        assert_eq!(retrieved.committed_at_ms(), 1713000000 + i as u64);
    }
}
