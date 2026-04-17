//! Crash recovery and durability tests (Fjall-backed).

use super::super::*;
use vo_types::EffectKind;
use vo_types::InstanceId;

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

fn make_effect_id(intent: &str) -> String {
    format!("{}::{intent}", sample_instance_id())
}

fn make_receipt(effect_id: &str) -> ExecutionReceipt {
    ExecutionReceipt::new(
        effect_id.to_string(),
        sample_instance_id().to_string(),
        EffectKind::HttpCall,
        1713000000,
        "Success".to_string(),
    )
    .unwrap()
}

#[test]
fn fjall_receipt_survives_database_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.path().to_path_buf();

    {
        let db = fjall::Database::builder(&dir_path).open().expect("db");
        let store = FjallReceiptStore::open(&db).expect("store");
        let receipt = make_receipt(&make_effect_id("fx-crash-1"));
        store.store_receipt(receipt).expect("store receipt");
    }

    let db = fjall::Database::builder(&dir_path)
        .open()
        .expect("db reopen");
    let store = FjallReceiptStore::open(&db).expect("store reopen");

    let eid = make_effect_id("fx-crash-1");
    let retrieved = store.get_receipt(&eid).expect("get after reopen").unwrap();

    assert_eq!(retrieved.effect_id(), eid);
    assert_eq!(retrieved.committed_at_ms(), 1713000000);
}

#[test]
fn fjall_receipt_idempotency_survives_database_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.path().to_path_buf();

    {
        let db = fjall::Database::builder(&dir_path).open().expect("db");
        let store = FjallReceiptStore::open(&db).expect("store");
        let receipt = make_receipt(&make_effect_id("fx-crash-dup"));
        store.store_receipt(receipt).expect("first store");
    }

    let db = fjall::Database::builder(&dir_path)
        .open()
        .expect("db reopen");
    let store = FjallReceiptStore::open(&db).expect("store reopen");

    let retry_receipt = ExecutionReceipt::new(
        make_effect_id("fx-crash-dup"),
        sample_instance_id().to_string(),
        EffectKind::HttpCall,
        999,
        "Failure".to_string(),
    )
    .unwrap();

    store
        .store_receipt(retry_receipt)
        .expect("idempotent retry after reopen");

    let eid = make_effect_id("fx-crash-dup");
    let retrieved = store.get_receipt(&eid).expect("get after reopen").unwrap();

    assert_eq!(retrieved.committed_at_ms(), 1713000000);
    assert_eq!(retrieved.connector_result(), "Success");
}

#[test]
fn fjall_multiple_receipts_survive_database_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.path().to_path_buf();

    let intents: Vec<&str> = vec!["fx-multi-0", "fx-multi-1", "fx-multi-2", "fx-multi-3"];

    {
        let db = fjall::Database::builder(&dir_path).open().expect("db");
        let store = FjallReceiptStore::open(&db).expect("store");

        for (i, intent) in intents.iter().enumerate() {
            let receipt = ExecutionReceipt::new(
                make_effect_id(intent),
                sample_instance_id().to_string(),
                EffectKind::HttpCall,
                1713000000 + i as u64,
                "Success".to_string(),
            )
            .unwrap();
            store.store_receipt(receipt).expect("store receipt");
        }
    }

    let db = fjall::Database::builder(&dir_path)
        .open()
        .expect("db reopen");
    let store = FjallReceiptStore::open(&db).expect("store reopen");

    for (i, intent) in intents.iter().enumerate() {
        let eid = make_effect_id(intent);
        let retrieved = store.get_receipt(&eid).expect("get after reopen").unwrap();
        assert_eq!(
            retrieved.committed_at_ms(),
            1713000000 + i as u64,
            "receipt for {intent} must survive crash"
        );
    }

    for intent in &intents {
        let eid = make_effect_id(intent);
        assert!(
            store.has_receipt(&eid).expect("has_receipt after reopen"),
            "has_receipt must return true for {intent} after crash"
        );
    }
}

#[test]
fn fjall_receipt_has_receipt_survives_database_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.path().to_path_buf();

    {
        let db = fjall::Database::builder(&dir_path).open().expect("db");
        let store = FjallReceiptStore::open(&db).expect("store");
        let receipt = make_receipt(&make_effect_id("fx-contains-crash"));
        store.store_receipt(receipt).expect("store");
    }

    let db = fjall::Database::builder(&dir_path)
        .open()
        .expect("db reopen");
    let store = FjallReceiptStore::open(&db).expect("store reopen");

    let eid = make_effect_id("fx-contains-crash");
    assert!(store.has_receipt(&eid).expect("has_receipt after reopen"));

    let missing_eid = make_effect_id("fx-never-stored");
    assert!(!store
        .has_receipt(&missing_eid)
        .expect("has_receipt missing after reopen"));
}
