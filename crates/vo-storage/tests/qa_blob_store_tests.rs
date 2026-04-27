//! QA tests for vo-storage: Blob storage via FsBlobStore.
//!
//! All tests use real FsBlobStore instances in temp directories. No mocks.

use vo_storage::blob_store::{BlobRecord, BlobStore, BlobStoreError, ContentAddress};
use vo_storage::fs_store::FsBlobStore;
use vo_types::BlobStatus;

// ══════════════════════════════════════════════════════════════════════════════
// Section 4: Blob Storage — write/read/large blobs via FsBlobStore
// ══════════════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread")]
async fn blob_store_write_read_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsBlobStore::new(dir.path());

    let data = b"hello, veloxide storage!";
    let addr = store.store(data).expect("store");

    let retrieved = store.retrieve(&addr).expect("retrieve");
    assert_eq!(retrieved, data);
}

#[tokio::test(flavor = "multi_thread")]
async fn blob_store_retrieve_missing_returns_content_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsBlobStore::new(dir.path());

    let missing =
        ContentAddress::new("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
            .unwrap();

    let result = store.retrieve(&missing);
    assert!(matches!(
        result,
        Err(BlobStoreError::ContentNotFound { .. })
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn blob_store_duplicate_content_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsBlobStore::new(dir.path());

    let data = b"duplicate me";
    store.store(data).expect("first store");

    let result = store.store(data);
    assert!(matches!(
        result,
        Err(BlobStoreError::DuplicateContent { .. })
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn blob_store_contains_works() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsBlobStore::new(dir.path());

    let addr = store.store(b"check me").expect("store");
    assert!(store.contains(&addr).expect("contains"));

    let missing =
        ContentAddress::new("0000000000000000000000000000000000000000000000000000000000000000")
            .unwrap();
    assert!(!store.contains(&missing).expect("contains missing"));
}

#[tokio::test(flavor = "multi_thread")]
async fn blob_store_content_address_is_sha256() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsBlobStore::new(dir.path());

    let data = b"predictable hash input";
    let addr = store.store(data).expect("store");

    assert_eq!(addr.as_str().len(), 64);
    assert!(addr.as_str().chars().all(|c| c.is_ascii_hexdigit()));
}

#[tokio::test(flavor = "multi_thread")]
async fn blob_store_ref_count_increment_decrement() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsBlobStore::new(dir.path());

    let addr = store.store(b"refcount").expect("store");

    let count = store.increment_ref_count(&addr).expect("increment");
    assert_eq!(count, 2);

    let meta = store.get_metadata(&addr).expect("metadata");
    assert_eq!(meta.reference_count(), 2);

    let count = store.decrement_ref_count(&addr).expect("decrement");
    assert_eq!(count, 1);

    let meta = store.get_metadata(&addr).expect("metadata");
    assert_eq!(meta.reference_count(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn blob_store_get_metadata_returns_correct_record() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsBlobStore::new(dir.path());

    let data = b"metadata test data here";
    let addr = store.store(data).expect("store");

    let meta = store.get_metadata(&addr).expect("metadata");
    assert_eq!(meta.size_bytes(), data.len() as u64);
    assert_eq!(meta.reference_count(), 1);
    assert_eq!(meta.status(), BlobStatus::DurablyStored);
}

#[tokio::test(flavor = "multi_thread")]
async fn blob_store_large_blob_1mb_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsBlobStore::new(dir.path());

    let data: Vec<u8> = (0..1_048_576)
        .map(|i| (i as u8).wrapping_mul(31).wrapping_add(17))
        .collect();

    let addr = store.store(&data).expect("store 1MB");
    let retrieved = store.retrieve(&addr).expect("retrieve 1MB");
    assert_eq!(retrieved.len(), data.len());
    assert_eq!(retrieved, data);
}

#[tokio::test(flavor = "multi_thread")]
async fn blob_store_large_blob_4mb_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsBlobStore::new(dir.path());

    let data: Vec<u8> = (0..4_194_304)
        .map(|i| (i as u8).wrapping_mul(53).wrapping_add(7))
        .collect();

    let addr = store.store(&data).expect("store 4MB");
    let retrieved = store.retrieve(&addr).expect("retrieve 4MB");
    assert_eq!(retrieved.len(), 4_194_304);
    assert_eq!(retrieved, data);
}

#[tokio::test(flavor = "multi_thread")]
async fn blob_store_gc_collects_expired_zero_ref_blobs() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsBlobStore::new(dir.path());

    let addr = store.store(b"gc me please").expect("store");
    store.decrement_ref_count(&addr).expect("decrement to 0");

    let meta = store.get_metadata(&addr).expect("meta");
    let expired_record = BlobRecord::with_status(
        meta.content_addr().clone(),
        meta.size_bytes(),
        0,
        meta.created_at_ms(),
        Some(1),
        meta.status(),
    );
    let meta_path = dir
        .path()
        .join("meta")
        .join(format!("{}.json", addr.as_str()));
    let encoded = vo_storage::blob_store::encode_blob_record(&expired_record).expect("encode");
    tokio::fs::write(&meta_path, &encoded)
        .await
        .expect("write meta");

    let collected = store.run_gc(u64::MAX).expect("gc");
    assert_eq!(collected, 1);

    assert!(!store.contains(&addr).expect("should be deleted"));
}

#[tokio::test(flavor = "multi_thread")]
async fn blob_store_gc_does_not_collect_active_blobs() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsBlobStore::new(dir.path());

    store.store(b"keep me alive").expect("store");

    let collected = store.run_gc(u64::MAX).expect("gc");
    assert_eq!(collected, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn blob_store_streaming_store_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsBlobStore::new(dir.path());

    let data = b"streaming blob content test";
    let cursor = std::io::Cursor::new(data.to_vec());
    let reader = tokio::io::BufReader::new(cursor);

    let addr = store.store_streaming(reader).expect("store streaming");
    let retrieved = store.retrieve(&addr).expect("retrieve");
    assert_eq!(retrieved, data);
}