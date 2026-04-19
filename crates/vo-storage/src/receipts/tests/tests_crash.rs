//! Crash recovery and durability tests (Fjall-backed).

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

#[test]
fn fjall_receipt_survives_keyspace_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.path().to_path_buf();

    {
        let keyspace = fjall::Config::new(&dir_path).open().expect("keyspace");
        let store = FjallReceiptStore::open(&keyspace).expect("store");

        let receipt = Receipt::new(
            sample_effect_id("fx-crash-1").as_str().to_string(),
            "stripe-connector".to_string(),
            ConnectorResult::Success,
            1713000000,
            Some(serde_json::json!({"charge_id": "ch_crash_1"})),
        )
        .unwrap();

        store.store(receipt).expect("store receipt");
    }

    let keyspace = fjall::Config::new(&dir_path)
        .open()
        .expect("keyspace reopen");
    let store = FjallReceiptStore::open(&keyspace).expect("store reopen");

    let eid = sample_effect_id("fx-crash-1");
    let retrieved = store.get(&eid).expect("get after reopen").unwrap();

    assert_eq!(retrieved.effect_id(), eid.as_str());
    assert_eq!(retrieved.connector_id(), "stripe-connector");
    assert_eq!(retrieved.result(), ConnectorResult::Success);
    assert_eq!(retrieved.committed_at_ms(), 1713000000);
    assert_eq!(
        retrieved.payload_json().cloned(), Some(serde_json::json!({"charge_id": "ch_crash_1"}))
    );
}

#[test]
fn fjall_receipt_idempotency_survives_keyspace_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.path().to_path_buf();

    {
        let keyspace = fjall::Config::new(&dir_path).open().expect("keyspace");
        let store = FjallReceiptStore::open(&keyspace).expect("store");

        let receipt = Receipt::new(
            sample_effect_id("fx-crash-dup").as_str().to_string(),
            "conn".to_string(),
            ConnectorResult::Success,
            100,
            None,
        )
        .unwrap();

        store.store(receipt).expect("first store");
    }

    let keyspace = fjall::Config::new(&dir_path)
        .open()
        .expect("keyspace reopen");
    let store = FjallReceiptStore::open(&keyspace).expect("store reopen");

    let retry_receipt = Receipt::new(
        sample_effect_id("fx-crash-dup").as_str().to_string(),
        "conn-overwrite".to_string(),
        ConnectorResult::Failure,
        999,
        Some(serde_json::json!({"retry": true})),
    )
    .unwrap();

    store
        .store(retry_receipt)
        .expect("idempotent retry after reopen");

    let eid = sample_effect_id("fx-crash-dup");
    let retrieved = store.get(&eid).expect("get after reopen").unwrap();

    assert_eq!(retrieved.connector_id(), "conn");
    assert_eq!(retrieved.committed_at_ms(), 100);
    assert_eq!(
        retrieved.result(),
        ConnectorResult::Success,
        "first write must survive crash and win over retry"
    );
    assert!(retrieved.payload_json().is_none());
}

#[test]
fn fjall_multiple_receipts_survive_keyspace_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.path().to_path_buf();

    let intents: Vec<&str> = vec!["fx-multi-0", "fx-multi-1", "fx-multi-2", "fx-multi-3"];

    {
        let keyspace = fjall::Config::new(&dir_path).open().expect("keyspace");
        let store = FjallReceiptStore::open(&keyspace).expect("store");

        for intent in &intents {
            let receipt = Receipt::new(
                sample_effect_id(intent).as_str().to_string(),
                format!("connector-{intent}"),
                ConnectorResult::Success,
                1713000000 + intents.iter().position(|&i| i == *intent).unwrap() as u64,
                Some(serde_json::json!({"intent": intent})),
            )
            .unwrap();
            store.store(receipt).expect("store receipt");
        }
    }

    let keyspace = fjall::Config::new(&dir_path)
        .open()
        .expect("keyspace reopen");
    let store = FjallReceiptStore::open(&keyspace).expect("store reopen");

    for intent in &intents {
        let eid = sample_effect_id(intent);
        let retrieved = store.get(&eid).expect("get after reopen").unwrap();
        assert_eq!(
            retrieved.connector_id(),
            format!("connector-{intent}"),
            "receipt for {intent} must survive crash"
        );
    }

    for intent in &intents {
        let eid = sample_effect_id(intent);
        assert!(
            store.contains(&eid).expect("contains after reopen"),
            "contains must return true for {intent} after crash"
        );
    }
}

#[test]
fn fjall_receipt_contains_survives_keyspace_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.path().to_path_buf();

    {
        let keyspace = fjall::Config::new(&dir_path).open().expect("keyspace");
        let store = FjallReceiptStore::open(&keyspace).expect("store");

        let receipt = Receipt::new(
            sample_effect_id("fx-contains-crash").as_str().to_string(),
            "conn".to_string(),
            ConnectorResult::Success,
            0,
            None,
        )
        .unwrap();

        store.store(receipt).expect("store");
    }

    let keyspace = fjall::Config::new(&dir_path)
        .open()
        .expect("keyspace reopen");
    let store = FjallReceiptStore::open(&keyspace).expect("store reopen");

    let eid = sample_effect_id("fx-contains-crash");
    assert!(store.contains(&eid).expect("contains after reopen"));

    let missing_eid = sample_effect_id("fx-never-stored");
    assert!(!store
        .contains(&missing_eid)
        .expect("contains missing after reopen"));
}
