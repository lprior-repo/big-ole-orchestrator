//! Idempotency and duplicate rejection tests (Fjall-backed).

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

#[test]
fn fjall_store_same_receipt_twice_is_idempotent() {
    let (_dir, db) = create_db();
    let store = FjallReceiptStore::open(&db).unwrap();
    let id = sample_instance_id();
    let receipt = ExecutionReceipt::new(
        format!("{id}::fx-dup-1"),
        id.to_string(),
        EffectKind::HttpCall,
        1_713_000_000,
        "Success".to_string(),
    )
    .unwrap();

    let result1 = store.store_receipt(receipt.clone());
    assert_eq!(result1, Ok(()));

    let result2 = store.store_receipt(receipt);
    assert_eq!(result2, Ok(()), "second store must be idempotent Ok");

    let retrieved = store
        .get_receipt(&format!("{id}::fx-dup-1"))
        .unwrap()
        .unwrap();
    assert_eq!(retrieved.committed_at_ms(), 1_713_000_000);
}

#[test]
fn fjall_store_different_receipts_same_effect_id_keeps_first() {
    let (_dir, db) = create_db();
    let store = FjallReceiptStore::open(&db).unwrap();
    let id = sample_instance_id();
    let eid = format!("{id}::fx-dup-2");

    let receipt_first = ExecutionReceipt::new(
        eid.clone(),
        id.to_string(),
        EffectKind::HttpCall,
        100,
        "Success".to_string(),
    )
    .unwrap();

    let receipt_second = ExecutionReceipt::new(
        eid.clone(),
        id.to_string(),
        EffectKind::HttpCall,
        200,
        "Failure".to_string(),
    )
    .unwrap();

    store.store_receipt(receipt_first).unwrap();
    store.store_receipt(receipt_second).unwrap();

    let retrieved = store.get_receipt(&eid).unwrap().unwrap();
    assert_eq!(retrieved.committed_at_ms(), 100);
    assert_eq!(retrieved.connector_result(), "Success");
}

#[test]
fn fjall_has_receipt_true_after_idempotent_store() {
    let (_dir, db) = create_db();
    let store = FjallReceiptStore::open(&db).unwrap();
    let id = sample_instance_id();
    let eid = format!("{id}::fx-dup-3");

    assert!(!store.has_receipt(&eid).unwrap());

    let receipt = ExecutionReceipt::new(
        eid.clone(),
        id.to_string(),
        EffectKind::HttpCall,
        0,
        "Success".to_string(),
    )
    .unwrap();
    store.store_receipt(receipt).unwrap();

    let receipt2 = ExecutionReceipt::new(
        eid.clone(),
        id.to_string(),
        EffectKind::HttpCall,
        999,
        "Success".to_string(),
    )
    .unwrap();
    store.store_receipt(receipt2).unwrap();

    assert!(store.has_receipt(&eid).unwrap());
}

#[test]
fn fjall_receipt_enforces_exact_once_boundary() {
    let (_dir, db) = create_db();
    let store = FjallReceiptStore::open(&db).unwrap();
    let id = sample_instance_id();
    let eid = format!("{id}::fx-once");

    let receipt = ExecutionReceipt::new(
        eid.clone(),
        id.to_string(),
        EffectKind::HttpCall,
        1_713_000_000,
        "Success".to_string(),
    )
    .unwrap();

    store.store_receipt(receipt).unwrap();

    let overwrite_attempt = ExecutionReceipt::new(
        eid.clone(),
        id.to_string(),
        EffectKind::HttpCall,
        1_713_000_001,
        "Failure".to_string(),
    )
    .unwrap();

    store.store_receipt(overwrite_attempt).unwrap();

    let retrieved = store.get_receipt(&eid).unwrap().unwrap();
    assert_eq!(retrieved.connector_result(), "Success");
    assert_eq!(retrieved.committed_at_ms(), 1_713_000_000);
}
