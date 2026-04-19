//! Idempotency and duplicate rejection tests (Fjall-backed).

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
fn fjall_store_same_receipt_twice_is_idempotent() {
    let (_dir, keyspace) = create_keyspace();
    let store = FjallReceiptStore::open(&keyspace).unwrap();

    let receipt = Receipt::new(
        sample_effect_id("fx-dup-1").as_str().to_string(),
        "stripe-connector".to_string(),
        ConnectorResult::Success,
        1713000000,
        None,
    )
    .unwrap();

    let result1 = store.store(receipt.clone());
    assert_eq!(result1, Ok(()));

    let result2 = store.store(receipt);
    assert_eq!(result2, Ok(()), "second store must be idempotent Ok");

    let eid = sample_effect_id("fx-dup-1");
    let retrieved = store.get(&eid).unwrap().unwrap();
    assert_eq!(retrieved.connector_id(), "stripe-connector");
    assert_eq!(retrieved.committed_at_ms(), 1713000000);
}

#[test]
fn fjall_store_different_receipts_same_effect_id_keeps_first() {
    let (_dir, keyspace) = create_keyspace();
    let store = FjallReceiptStore::open(&keyspace).unwrap();

    let eid = sample_effect_id("fx-dup-2");

    let receipt_first = Receipt::new(
        eid.as_str().to_string(),
        "conn-original".to_string(),
        ConnectorResult::Success,
        100,
        Some(serde_json::json!({"v": 1})),
    )
    .unwrap();

    let receipt_second = Receipt::new(
        eid.as_str().to_string(),
        "conn-overwrite".to_string(),
        ConnectorResult::Failure,
        200,
        Some(serde_json::json!({"v": 2})),
    )
    .unwrap();

    store.store(receipt_first).unwrap();
    store.store(receipt_second).unwrap();

    let retrieved = store.get(&eid).unwrap().unwrap();
    assert_eq!(retrieved.connector_id(), "conn-original");
    assert_eq!(retrieved.committed_at_ms(), 100);
    assert_eq!(retrieved.result(), ConnectorResult::Success);
    assert_eq!(
        retrieved.payload_json().cloned(), Some(serde_json::json!({"v": 1})),
        "first write wins, second must be no-op"
    );
}

#[test]
fn fjall_contains_true_after_idempotent_store() {
    let (_dir, keyspace) = create_keyspace();
    let store = FjallReceiptStore::open(&keyspace).unwrap();

    let receipt = Receipt::new(
        sample_effect_id("fx-dup-3").as_str().to_string(),
        "conn".to_string(),
        ConnectorResult::Success,
        0,
        None,
    )
    .unwrap();

    assert!(!store.contains(&sample_effect_id("fx-dup-3")).unwrap());

    store.store(receipt).unwrap();
    store
        .store(
            Receipt::new(
                sample_effect_id("fx-dup-3").as_str().to_string(),
                "conn".to_string(),
                ConnectorResult::Success,
                999,
                None,
            )
            .unwrap(),
        )
        .unwrap();

    assert!(store.contains(&sample_effect_id("fx-dup-3")).unwrap());
}

#[test]
fn fjall_receipt_enforces_exact_once_boundary() {
    let (_dir, keyspace) = create_keyspace();
    let store = FjallReceiptStore::open(&keyspace).unwrap();

    let receipt = Receipt::new(
        sample_effect_id("fx-once").as_str().to_string(),
        "payment-connector".to_string(),
        ConnectorResult::Success,
        1713000000,
        Some(serde_json::json!({"charge_id": "ch_once"})),
    )
    .unwrap();

    store.store(receipt).unwrap();

    let overwrite_attempt = Receipt::new(
        sample_effect_id("fx-once").as_str().to_string(),
        "payment-connector".to_string(),
        ConnectorResult::Failure,
        1713000001,
        Some(serde_json::json!({"charge_id": "ch_once_retry"})),
    )
    .unwrap();

    store.store(overwrite_attempt).unwrap();

    let retrieved = store.get(&sample_effect_id("fx-once")).unwrap().unwrap();
    assert_eq!(retrieved.result(), ConnectorResult::Success);
    assert_eq!(
        retrieved.payload_json().cloned(), Some(serde_json::json!({"charge_id": "ch_once"})),
        "receipts enforce exact-once: first write must win"
    );
}
