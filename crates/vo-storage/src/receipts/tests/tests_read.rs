//! Receipt read/query tests (Fjall-backed).

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
fn fjall_get_nonexistent_receipt_returns_none() {
    let (_dir, db) = create_db();
    let store = FjallReceiptStore::open(&db).unwrap();
    let result = store.get_receipt("fx-nonexistent").unwrap();
    assert!(result.is_none());
}

#[test]
fn fjall_has_receipt_returns_false_for_nonexistent() {
    let (_dir, db) = create_db();
    let store = FjallReceiptStore::open(&db).unwrap();
    let result = store.has_receipt("fx-missing").unwrap();
    assert!(!result);
}

#[test]
fn fjall_has_receipt_returns_true_after_store() {
    let (_dir, db) = create_db();
    let store = FjallReceiptStore::open(&db).unwrap();
    let id = sample_instance_id();
    let receipt = ExecutionReceipt::new(
        format!("{id}::fx-exists"),
        id.to_string(),
        EffectKind::HttpCall,
        0,
        "Success".to_string(),
    )
    .unwrap();

    store.store_receipt(receipt).unwrap();
    assert!(store.has_receipt(&format!("{id}::fx-exists")).unwrap());
}

#[test]
fn fjall_get_returns_correct_receipt_fields() {
    let (_dir, db) = create_db();
    let store = FjallReceiptStore::open(&db).unwrap();
    let id = sample_instance_id();
    let receipt = ExecutionReceipt::new(
        format!("{id}::fx-fields"),
        id.to_string(),
        EffectKind::HttpCall,
        1_713_999_999,
        "Success".to_string(),
    )
    .unwrap();

    store.store_receipt(receipt.clone()).unwrap();

    let retrieved = store.get_receipt(receipt.effect_id()).unwrap().unwrap();
    assert_eq!(retrieved.effect_id(), receipt.effect_id());
    assert_eq!(retrieved.instance_id(), receipt.instance_id());
    assert_eq!(retrieved.kind(), EffectKind::HttpCall);
    assert_eq!(retrieved.committed_at_ms(), 1_713_999_999);
}

#[test]
fn fjall_get_is_isolated_by_effect_id() {
    let (_dir, db) = create_db();
    let store = FjallReceiptStore::open(&db).unwrap();
    let id = sample_instance_id();

    let r1 = ExecutionReceipt::new(
        format!("{id}::fx-iso-a"),
        id.to_string(),
        EffectKind::HttpCall,
        100,
        "Success".to_string(),
    )
    .unwrap();

    let r2 = ExecutionReceipt::new(
        format!("{id}::fx-iso-b"),
        id.to_string(),
        EffectKind::HttpCall,
        200,
        "Failure".to_string(),
    )
    .unwrap();

    store.store_receipt(r1.clone()).unwrap();
    store.store_receipt(r2.clone()).unwrap();

    assert_eq!(
        store
            .get_receipt(r1.effect_id())
            .unwrap()
            .unwrap()
            .committed_at_ms(),
        100
    );
    assert_eq!(
        store
            .get_receipt(r2.effect_id())
            .unwrap()
            .unwrap()
            .committed_at_ms(),
        200
    );
}
