//! BDD: GDPR purge destroys DEK and blob references (ADR-025, ADR-040).
//!
//! Given a terminal instance has encrypted canonical payloads and blob refs
//! When purge executes
//! Then the DEK is destroyed and blob references/projections are removed or tombstoned by policy
//!
//! Required proof command:
//! cargo test -p vo-storage given_terminal_instance_when_purged_then_dek_and_blob_refs_are_destroyed

use vo_storage::blob_store::{decode_blob_record, encode_blob_record, ContentAddress, BLOB_RECORD_PARTITION};
use vo_storage::codec::encode_event_key;
use vo_storage::instance_index::instance_index_upsert;
use vo_storage::key_partition::DekStore;
use vo_storage::key_partition::FjallDekStore;
use vo_storage::key_partition::{decode_dek_entry, DEK_PARTITION};
use vo_storage::purge::purge_instance;
use vo_types::{BlobStatus, DekId, InstanceId, InstanceStatus, SequenceNumber, TimestampMs};

const DEK_INDEX_PARTITION: &str = "dek_index";

fn sample_instance_id() -> InstanceId {
    InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap()
}

fn sample_instance_id_string() -> String {
    "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string()
}

fn make_content_addr() -> String {
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()
}

fn make_kek() -> [u8; 32] {
    [0x42u8; 32]
}

fn setup_terminal_instance_with_event_and_blob(
    db: &fjall::Database,
    instance_id_str: &str,
    instance_id: &InstanceId,
    blob_content_addr: &str,
) {
    let ts = TimestampMs::try_from(1000u64).unwrap();

    instance_index_upsert(db, instance_id, InstanceStatus::Completed, ts, None).unwrap();

    let events_p = db
        .keyspace("events", || fjall::KeyspaceCreateOptions::default())
        .unwrap();
    let seq = SequenceNumber::try_from(1u64).unwrap();
    let key = encode_event_key(instance_id, &seq).unwrap();

    let event_json = serde_json::json!({
        "output_ref": format!("blob:{}", blob_content_addr),
        "data": "encrypted canonical payload"
    });
    events_p.insert(&key, serde_json::to_vec(&event_json).unwrap()).unwrap();
}

fn setup_dek_for_instance(db: &fjall::Database, instance_id: &InstanceId) {
    let store = FjallDekStore::open(db).unwrap();
    let kek = make_kek();
    store.generate_and_store_dek(instance_id, &kek).unwrap();
}

fn setup_blob_record(db: &fjall::Database, content_addr: &str, ref_count: u64) {
    let blob_records_p = db
        .keyspace(BLOB_RECORD_PARTITION, || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    let content_address = ContentAddress::new(content_addr).unwrap();
    let record = vo_storage::blob_store::BlobRecord::new(
        content_address,
        1024,
        ref_count,
        1000,
        None,
    )
    .unwrap();

    let encoded = encode_blob_record(&record).unwrap();
    blob_records_p.insert(content_addr.as_bytes(), encoded).unwrap();
}

fn get_dek_status(db: &fjall::Database, dek_id: &DekId) -> Option<vo_storage::key_partition::DekStatus> {
    let dek_store_p = db
        .keyspace(DEK_PARTITION, || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    let key = dek_id.as_str().as_bytes().to_vec();
    dek_store_p.get(&key).ok().flatten().map(|bytes| {
        decode_dek_entry(&bytes).map(|e| e.status()).ok()
    }).flatten()
}

fn get_dek_index_exists(db: &fjall::Database, instance_id_str: &str) -> bool {
    let dek_index_p = db
        .keyspace(DEK_INDEX_PARTITION, || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    let index_key = format!("{instance_id_str}::active").into_bytes();
    dek_index_p.get(&index_key).ok().flatten().is_some()
}

fn get_blob_record(db: &fjall::Database, content_addr: &str) -> Option<vo_storage::blob_store::BlobRecord> {
    let blob_records_p = db
        .keyspace(BLOB_RECORD_PARTITION, || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    blob_records_p.get(content_addr.as_bytes()).ok().flatten()
        .and_then(|bytes| decode_blob_record(&bytes).ok())
}

#[test]
fn given_terminal_instance_when_purged_then_dek_and_blob_refs_are_destroyed() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(temp_dir.path()).open().unwrap();

    let instance_id_str = sample_instance_id_string();
    let instance_id = sample_instance_id();
    let content_addr = make_content_addr();
    let initial_ref_count = 2u64;

    setup_terminal_instance_with_event_and_blob(&db, &instance_id_str, &instance_id, &content_addr);
    setup_dek_for_instance(&db, &instance_id);
    setup_blob_record(&db, &content_addr, initial_ref_count);

    let dek_id = {
        let store = FjallDekStore::open(&db).unwrap();
        store.get_active_dek_id(&instance_id).unwrap()
    };

    assert!(
        get_dek_index_exists(&db, &instance_id_str),
        "DEK index should exist before purge"
    );
    assert_eq!(
        get_dek_status(&db, &dek_id),
        Some(vo_storage::key_partition::DekStatus::Active),
        "DEK should be Active before purge"
    );
    let blob_before = get_blob_record(&db, &content_addr);
    assert_eq!(
        blob_before.as_ref().map(|r| r.reference_count()),
        Some(initial_ref_count),
        "Blob ref_count should be {} before purge",
        initial_ref_count
    );

    let result = purge_instance(&db, &instance_id_str);
    assert!(result.is_ok(), "purge_instance should succeed for terminal instance");

    assert!(
        !get_dek_index_exists(&db, &instance_id_str),
        "DEK index must be removed after purge"
    );
    assert_eq!(
        get_dek_status(&db, &dek_id),
        Some(vo_storage::key_partition::DekStatus::Retired),
        "DEK must be Retired (crypto-shredded) after purge"
    );

    let blob_after = get_blob_record(&db, &content_addr);
    assert!(
        blob_after.is_some(),
        "Blob record should still exist after purge (tombstoned, not deleted)"
    );
    assert_eq!(
        blob_after.as_ref().map(|r| r.reference_count()),
        Some(initial_ref_count - 1),
        "Blob ref_count should be decremented after purge"
    );
    assert_eq!(
        blob_after.as_ref().map(|r| r.status()),
        Some(BlobStatus::Published),
        "Blob status should remain Published when ref_count > 0"
    );
}

#[test]
fn given_terminal_instance_with_last_blob_ref_when_purged_then_blob_tombstoned() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(temp_dir.path()).open().unwrap();

    let instance_id_str = sample_instance_id_string();
    let instance_id = sample_instance_id();
    let content_addr = make_content_addr();
    let initial_ref_count = 1u64;

    setup_terminal_instance_with_event_and_blob(&db, &instance_id_str, &instance_id, &content_addr);
    setup_dek_for_instance(&db, &instance_id);
    setup_blob_record(&db, &content_addr, initial_ref_count);

    let dek_id = {
        let store = FjallDekStore::open(&db).unwrap();
        store.get_active_dek_id(&instance_id).unwrap()
    };

    let result = purge_instance(&db, &instance_id_str);
    assert!(result.is_ok(), "purge_instance should succeed");

    assert!(
        !get_dek_index_exists(&db, &instance_id_str),
        "DEK index must be removed after purge"
    );
    assert_eq!(
        get_dek_status(&db, &dek_id),
        Some(vo_storage::key_partition::DekStatus::Retired),
        "DEK must be Retired after purge"
    );

    let blob_after = get_blob_record(&db, &content_addr);
    assert!(
        blob_after.is_some(),
        "Blob record should exist after purge"
    );
    assert_eq!(
        blob_after.as_ref().map(|r| r.reference_count()),
        Some(0u64),
        "Blob ref_count should be 0 after purge"
    );
    assert_eq!(
        blob_after.as_ref().map(|r| r.status()),
        Some(BlobStatus::Failed),
        "Blob status must be Failed (tombstoned) when ref_count reaches 0"
    );
}

#[test]
fn given_terminal_instance_without_blob_ref_when_purged_then_dek_destroyed() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(temp_dir.path()).open().unwrap();

    let instance_id_str = sample_instance_id_string();
    let instance_id = sample_instance_id();
    let ts = TimestampMs::try_from(1000u64).unwrap();

    instance_index_upsert(&db, &instance_id, InstanceStatus::Failed, ts, None).unwrap();

    let events_p = db
        .keyspace("events", || fjall::KeyspaceCreateOptions::default())
        .unwrap();
    let seq = SequenceNumber::try_from(1u64).unwrap();
    let key = encode_event_key(&instance_id, &seq).unwrap();
    let event_json = serde_json::json!({
        "output_ref": null,
        "data": "no blob ref here"
    });
    events_p.insert(&key, serde_json::to_vec(&event_json).unwrap()).unwrap();

    setup_dek_for_instance(&db, &instance_id);

    let dek_id = {
        let store = FjallDekStore::open(&db).unwrap();
        store.get_active_dek_id(&instance_id).unwrap()
    };

    let result = purge_instance(&db, &instance_id_str);
    assert!(result.is_ok(), "purge_instance should succeed");

    assert!(
        !get_dek_index_exists(&db, &instance_id_str),
        "DEK index must be removed after purge"
    );
    assert_eq!(
        get_dek_status(&db, &dek_id),
        Some(vo_storage::key_partition::DekStatus::Retired),
        "DEK must be Retired after purge even without blob refs"
    );
}

#[test]
fn given_terminal_instance_with_multiple_blob_refs_when_purged_then_all_decremented() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(temp_dir.path()).open().unwrap();

    let instance_id_str = sample_instance_id_string();
    let instance_id = sample_instance_id();
    let content_addr1 = make_content_addr();
    let content_addr2 = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string();
    let initial_ref_count1 = 3u64;
    let initial_ref_count2 = 2u64;

    let ts = TimestampMs::try_from(1000u64).unwrap();
    instance_index_upsert(&db, &instance_id, InstanceStatus::Completed, ts, None).unwrap();

    let events_p = db
        .keyspace("events", || fjall::KeyspaceCreateOptions::default())
        .unwrap();
    let seq1 = SequenceNumber::try_from(1u64).unwrap();
    let key1 = encode_event_key(&instance_id, &seq1).unwrap();
    let event_json1 = serde_json::json!({
        "output_ref": format!("blob:{}", content_addr1),
        "data": "blob 1"
    });
    events_p.insert(&key1, serde_json::to_vec(&event_json1).unwrap()).unwrap();

    let seq2 = SequenceNumber::try_from(2u64).unwrap();
    let key2 = encode_event_key(&instance_id, &seq2).unwrap();
    let event_json2 = serde_json::json!({
        "output_ref": format!("blob:{}", content_addr2),
        "data": "blob 2"
    });
    events_p.insert(&key2, serde_json::to_vec(&event_json2).unwrap()).unwrap();

    setup_dek_for_instance(&db, &instance_id);
    setup_blob_record(&db, &content_addr1, initial_ref_count1);
    setup_blob_record(&db, &content_addr2, initial_ref_count2);

    let result = purge_instance(&db, &instance_id_str);
    assert!(result.is_ok(), "purge_instance should succeed");

    let blob1_after = get_blob_record(&db, &content_addr1);
    let blob2_after = get_blob_record(&db, &content_addr2);

    assert_eq!(
        blob1_after.as_ref().map(|r| r.reference_count()),
        Some(initial_ref_count1 - 1),
        "Blob1 ref_count should be decremented"
    );
    assert_eq!(
        blob2_after.as_ref().map(|r| r.reference_count()),
        Some(initial_ref_count2 - 1),
        "Blob2 ref_count should be decremented"
    );
}
