//! Receipt write persistence tests (Fjall-backed).

use super::super::*;
use vo_types::EffectKind;
use vo_types::InstanceId;

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

fn create_db() -> (tempfile::TempDir, fjall::Database) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = fjall::Database::builder(dir.path()).open().expect("db");
    (dir, db)
}

fn make_receipt(effect_id: &str) -> ExecutionReceipt {
    let full_id = format!("{}::{effect_id}", sample_instance_id());
    ExecutionReceipt::new(
        full_id,
        sample_instance_id().to_string(),
        EffectKind::HttpCall,
        1713000000,
        "Success".to_string(),
    )
    .unwrap()
}

#[test]
fn fjall_store_persists_receipt_and_get_returns_it() {
    let (_dir, db) = create_db();
    let store = FjallReceiptStore::open(&db).unwrap();
    let receipt = make_receipt("fx-write-1");

    store.store_receipt(receipt.clone()).unwrap();

    let retrieved = store.get_receipt(receipt.effect_id()).unwrap();
    assert!(retrieved.is_some());
    let r = retrieved.unwrap();
    assert_eq!(r.effect_id(), receipt.effect_id());
    assert_eq!(r.instance_id(), receipt.instance_id());
}

#[test]
fn fjall_store_receipt_with_failure_result() {
    let (_dir, db) = create_db();
    let store = FjallReceiptStore::open(&db).unwrap();
    let id = sample_instance_id();
    let receipt = ExecutionReceipt::new(
        format!("{id}::fx-fail-1"),
        id.to_string(),
        EffectKind::HttpCall,
        1713000001,
        "Failure".to_string(),
    )
    .unwrap();

    store.store_receipt(receipt.clone()).unwrap();

    let retrieved = store.get_receipt(receipt.effect_id()).unwrap().unwrap();
    assert_eq!(retrieved.connector_result(), "Failure");
}

#[test]
fn fjall_store_receipt_without_payload() {
    let (_dir, db) = create_db();
    let store = FjallReceiptStore::open(&db).unwrap();
    let id = sample_instance_id();
    let receipt = ExecutionReceipt::new(
        format!("{id}::fx-nopayload"),
        id.to_string(),
        EffectKind::HttpCall,
        1713000002,
        "Success".to_string(),
    )
    .unwrap();

    store.store_receipt(receipt.clone()).unwrap();

    let retrieved = store.get_receipt(receipt.effect_id()).unwrap().unwrap();
    assert_eq!(retrieved.connector_result(), "Success");
}

#[test]
fn fjall_store_multiple_receipts_for_different_effects() {
    let (_dir, db) = create_db();
    let store = FjallReceiptStore::open(&db).unwrap();

    let id = sample_instance_id();
    for i in 0..5 {
        let receipt = ExecutionReceipt::new(
            format!("{id}::fx-batch-{i}"),
            id.to_string(),
            EffectKind::HttpCall,
            1713000000 + i as u64,
            format!("Success-{i}"),
        )
        .unwrap();
        store.store_receipt(receipt).unwrap();
    }

    for i in 0..5 {
        let eid = format!("{id}::fx-batch-{i}");
        let retrieved = store.get_receipt(&eid).unwrap().unwrap();
        assert_eq!(retrieved.committed_at_ms(), 1713000000 + i as u64);
    }
}
