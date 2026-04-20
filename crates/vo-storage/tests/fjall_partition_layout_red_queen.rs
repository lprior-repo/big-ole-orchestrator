//! Red Queen adversarial integration tests for Fjall partition layout (ve-3zrs)
//!
//! Tests the Fjall keyspace partition layout against:
//! - Concurrent partition access (multiple partitions accessed simultaneously)
//! - Partition corruption recovery (malformed data, truncated keys/values)
//! - Schema migration across partitions (old format data read after schema changes)
//!
//! Target: vo-storage (ADR-002)

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::thread;

use tempfile::tempdir;
use vo_storage::dedupe_partition::{
    AdmissionResult, DedupeStore, DedupeStoreError, FjallDedupeStore,
};
use vo_storage::lease_partition::{FjallLeaseStore, LeaseStore};
use vo_types::{DedupeKey, FenceToken, InstanceId, StepId};

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

fn alternate_instance_id() -> InstanceId {
    InstanceId::from_bytes([2u8; 16])
}

fn sample_step_id() -> StepId {
    StepId::parse("step-1").unwrap()
}

fn alternate_step_id() -> StepId {
    StepId::parse("step-2").unwrap()
}

// ========================================================================
// DIMENSION: concurrent partition access
// Tests that multiple partitions can be accessed simultaneously without interference
// ========================================================================

#[test]
fn red_queen_fjall_concurrent_dedupe_and_lease_access() {
    let dir = tempdir().unwrap();
    let database = fjall::Database::builder(dir.path()).open().unwrap();
    let dedupe_store = FjallDedupeStore::open(&database).unwrap();
    let lease_store = FjallLeaseStore::open(&database).unwrap();

    let dedupe_store = Arc::new(dedupe_store);
    let lease_store = Arc::new(lease_store);

    let num_threads = 16;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

    let dedupe_handles: Vec<_> = (0..8)
        .map(|i| {
            let ds = Arc::clone(&dedupe_store);
            let ls = Arc::clone(&lease_store);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let key = DedupeKey::parse(&format!("rq-part-{i}-ve-3zrs")).unwrap();
                let iid = InstanceId::from_bytes([i as u8; 16]);
                ds.check_and_insert(&key, &iid, 60_000)
            })
        })
        .collect();

    let lease_handles: Vec<_> = (0..8)
        .map(|i| {
            let ds = Arc::clone(&dedupe_store);
            let ls = Arc::clone(&lease_store);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let iid = InstanceId::from_bytes([(i + 8) as u8; 16]);
                let step_id = StepId::parse(&format!("step-{i}")).unwrap();
                ls.acquire(&iid, &step_id, 60_000)
            })
        })
        .collect();

    let dedupe_results: Vec<_> = dedupe_handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect();
    let lease_results: Vec<_> = lease_handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect();

    let dedupe_admitted = dedupe_results
        .iter()
        .filter(|r| matches!(r, Ok(AdmissionResult::Admitted)))
        .count();
    assert_eq!(
        dedupe_admitted, 8,
        "BUG: {}/8 dedupe operations admitted",
        dedupe_admitted
    );

    let lease_success = lease_results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(
        lease_success, 8,
        "BUG: {}/8 lease acquisitions succeeded",
        lease_success
    );
}

#[test]
fn red_queen_fjall_concurrent_same_partition_different_keys() {
    let dir = tempdir().unwrap();
    let database = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallDedupeStore::open(&database).unwrap();
    let store = Arc::new(store);
    let num_threads = 16;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let key = DedupeKey::parse(&format!("rq-same-part-{i}-ve-3zrs")).unwrap();
                let iid = InstanceId::from_bytes([i as u8; 16]);
                store.check_and_insert(&key, &iid, 60_000)
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let admitted_count = results
        .iter()
        .filter(|r| matches!(r, Ok(AdmissionResult::Admitted)))
        .count();
    assert_eq!(
        admitted_count, num_threads,
        "BUG: {}/{} operations admitted instead of all {}",
        admitted_count, num_threads, num_threads
    );
}

#[test]
fn red_queen_fjall_concurrent_cross_partition_key_isolation() {
    let dir = tempdir().unwrap();
    let database = fjall::Database::builder(dir.path()).open().unwrap();
    let dedupe_store = FjallDedupeStore::open(&database).unwrap();
    let lease_store = FjallLeaseStore::open(&database).unwrap();

    let iid = sample_instance_id();
    let step_id = sample_step_id();

    let dedupe_key = DedupeKey::parse("rq-isolation-key-ve-3zrs").unwrap();

    dedupe_store
        .check_and_insert(&dedupe_key, &iid, 60_000)
        .unwrap();

    let lease_result = lease_store.acquire(&iid, &step_id, 60_000);
    assert!(lease_result.is_ok(), "BUG: lease acquisition failed");

    let contains = dedupe_store.contains(&dedupe_key).unwrap();
    assert!(
        contains,
        "BUG: dedupe key disappeared after cross-partition access"
    );

    let lease_check = lease_store
        .check_stale_fence(&iid, &step_id, lease_result.unwrap().token())
        .unwrap();
    assert!(
        !lease_check,
        "BUG: fence token reported stale after fresh acquisition"
    );
}

// ========================================================================
// DIMENSION: partition corruption recovery
// Tests that corrupted keys/values are rejected gracefully without crashing
// ========================================================================

#[test]
fn red_queen_fjall_corruption_truncated_dedupe_key_rejected() {
    let dir = tempdir().unwrap();
    let database = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallDedupeStore::open(&database).unwrap();

    let partition = database
        .keyspace("dedupe", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    let truncated_key = vec![0xFF, 0xFE];
    let value = serde_json::to_vec(&serde_json::json!({
        "dedupe_key": "test",
        "instance_id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        "expires_at": u64::MAX
    }))
    .unwrap();
    let insert_result = partition.insert(&truncated_key, &value);
    assert!(
        insert_result.is_ok(),
        "BUG: partition rejected direct insert of truncated key"
    );

    let key = DedupeKey::parse("test-corrupt-ve-3zrs").unwrap();
    let result = store.check_and_insert(&key, &sample_instance_id(), 60_000);
    assert!(
        result.is_ok(),
        "BUG: check_and_insert failed on corrupted partition"
    );
}

#[test]
fn red_queen_fjall_corruption_truncated_json_value_handled() {
    let dir = tempdir().unwrap();
    let database = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallDedupeStore::open(&database).unwrap();

    let partition = database
        .keyspace("dedupe", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    let valid_key = DedupeKey::parse("rq-trunc-json-ve-3zrs").unwrap();
    let key_bytes = valid_key.as_str().as_bytes().to_vec();
    let truncated_json = b"{\"dedupe_key\":\"rq-trunc";
    let insert_result = partition.insert(&key_bytes, truncated_json);
    assert!(insert_result.is_ok());

    let contains_result = store.contains(&valid_key);
    match contains_result {
        Ok(false) => {}
        Ok(true) => panic!("BUG: contains returned true for corrupted entry"),
        Err(_) => {
            panic!("BUG: contains returned error instead of handling gracefully")
        }
    }
}

#[test]
fn red_queen_fjall_corruption_valid_json_wrong_type_rejected() {
    let dir = tempdir().unwrap();
    let database = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallDedupeStore::open(&database).unwrap();

    let partition = database
        .keyspace("dedupe", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    let valid_key = DedupeKey::parse("rq-wrong-type-ve-3zrs").unwrap();
    let key_bytes = valid_key.as_str().as_bytes().to_vec();
    let wrong_type_value = b"42";
    let insert_result = partition.insert(&key_bytes, wrong_type_value);
    assert!(insert_result.is_ok());

    let contains_result = store.contains(&valid_key);
    match contains_result {
        Ok(false) => {}
        Ok(true) => panic!("BUG: contains returned true for wrong-type entry"),
        Err(_) => {
            panic!("BUG: contains returned error instead of handling gracefully")
        }
    }
}

#[test]
fn red_queen_fjall_corruption_missing_required_fields_handled() {
    let dir = tempdir().unwrap();
    let database = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallDedupeStore::open(&database).unwrap();

    let partition = database
        .keyspace("dedupe", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    let valid_key = DedupeKey::parse("rq-missing-fields-ve-3zrs").unwrap();
    let key_bytes = valid_key.as_str().as_bytes().to_vec();
    let incomplete_json = serde_json::to_vec(&serde_json::json!({
        "dedupe_key": "rq-missing-fields-ve-3zrs"
    }))
    .unwrap();
    let insert_result = partition.insert(&key_bytes, &incomplete_json);
    assert!(insert_result.is_ok());

    let contains_result = store.contains(&valid_key);
    match contains_result {
        Ok(false) => {}
        Ok(true) => panic!("BUG: contains returned true for incomplete entry"),
        Err(_) => {}
    }
}

#[test]
fn red_queen_fjall_corruption_expired_entry_allows_reinsert() {
    let dir = tempdir().unwrap();
    let database = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallDedupeStore::open(&database).unwrap();

    let partition = database
        .keyspace("dedupe", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    let valid_key = DedupeKey::parse("rq-expired-corrupt-ve-3zrs").unwrap();
    let key_bytes = valid_key.as_str().as_bytes().to_vec();
    let expired_json = serde_json::to_vec(&serde_json::json!({
        "dedupe_key": "rq-expired-corrupt-ve-3zrs",
        "instance_id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        "expires_at": 0u64
    }))
    .unwrap();
    partition.insert(&key_bytes, &expired_json).unwrap();

    let result = store.check_and_insert(&valid_key, &sample_instance_id(), 60_000);
    assert_eq!(
        result,
        Ok(AdmissionResult::Admitted),
        "BUG: expired corrupted entry prevented reinsert"
    );
}

#[test]
fn red_queen_fjall_corruption_null_bytes_in_value_handled() {
    let dir = tempdir().unwrap();
    let database = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallDedupeStore::open(&database).unwrap();

    let partition = database
        .keyspace("dedupe", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    let valid_key = DedupeKey::parse("rq-null-bytes-ve-3zrs").unwrap();
    let key_bytes = valid_key.as_str().as_bytes().to_vec();
    let null_bytes = vec![0u8; 100];
    let insert_result = partition.insert(&key_bytes, &null_bytes);
    assert!(insert_result.is_ok());

    let contains_result = store.contains(&valid_key);
    match contains_result {
        Ok(false) => {}
        Ok(true) => panic!("BUG: contains returned true for null-bytes entry"),
        Err(_) => {}
    }
}

// ========================================================================
// DIMENSION: schema migration across partitions
// Tests that old format data can still be read correctly after schema changes
// ========================================================================

#[test]
fn red_queen_fjall_schema_migration_old_dedupe_format_still_readable() {
    let dir = tempdir().unwrap();
    let database = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallDedupeStore::open(&database).unwrap();

    let partition = database
        .keyspace("dedupe", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    let valid_key = DedupeKey::parse("rq-schema-migrate-ve-3zrs").unwrap();
    let key_bytes = valid_key.as_str().as_bytes().to_vec();

    let old_format_json = serde_json::to_vec(&serde_json::json!({
        "dedupe_key": "rq-schema-migrate-ve-3zrs",
        "instance_id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        "expires_at": u64::MAX,
        "extra_field_ignored": true
    }))
    .unwrap();
    partition.insert(&key_bytes, &old_format_json).unwrap();

    let contains = store.contains(&valid_key).unwrap();
    assert!(contains, "BUG: old format with extra fields not readable");

    let result = store.check_and_insert(&valid_key, &sample_instance_id(), 60_000);
    match result {
        Ok(AdmissionResult::Duplicate { instance_id }) => {
            assert_eq!(
                instance_id, "01ARZ3NDEKTSV4RRFFQ69G5FAV",
                "BUG: schema migration changed stored instance_id"
            );
        }
        other => panic!("BUG: expected Duplicate, got {:?}", other),
    }
}

#[test]
fn red_queen_fjall_schema_migration_reordered_fields_still_readable() {
    let dir = tempdir().unwrap();
    let database = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallDedupeStore::open(&database).unwrap();

    let partition = database
        .keyspace("dedupe", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    let valid_key = DedupeKey::parse("rq-schema-reorder-ve-3zrs").unwrap();
    let key_bytes = valid_key.as_str().as_bytes().to_vec();

    let reordered_json = serde_json::to_vec(&serde_json::json!({
        "expires_at": u64::MAX,
        "instance_id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        "dedupe_key": "rq-schema-reorder-ve-3zrs"
    }))
    .unwrap();
    partition.insert(&key_bytes, &reordered_json).unwrap();

    let contains = store.contains(&valid_key).unwrap();
    assert!(contains, "BUG: reordered JSON fields not readable");
}

#[test]
fn red_queen_fjall_schema_migration_minimal_v1_format() {
    let dir = tempdir().unwrap();
    let database = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallDedupeStore::open(&database).unwrap();

    let partition = database
        .keyspace("dedupe", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    let valid_key = DedupeKey::parse("rq-schema-minimal-ve-3zrs").unwrap();
    let key_bytes = valid_key.as_str().as_bytes().to_vec();

    let minimal_json = serde_json::to_vec(&serde_json::json!({
        "dedupe_key": "rq-schema-minimal-ve-3zrs",
        "instance_id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        "expires_at": u64::MAX
    }))
    .unwrap();
    partition.insert(&key_bytes, &minimal_json).unwrap();

    let contains = store.contains(&valid_key).unwrap();
    assert!(contains, "BUG: minimal v1 format not readable");
}

#[test]
fn red_queen_fjall_schema_migration_unknown_field_ignored() {
    let dir = tempdir().unwrap();
    let database = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallDedupeStore::open(&database).unwrap();

    let partition = database
        .keyspace("dedupe", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    let valid_key = DedupeKey::parse("rq-schema-unknown-ve-3zrs").unwrap();
    let key_bytes = valid_key.as_str().as_bytes().to_vec();

    let with_unknown = serde_json::to_vec(&serde_json::json!({
        "dedupe_key": "rq-schema-unknown-ve-3zrs",
        "instance_id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        "expires_at": u64::MAX,
        "schema_version": "2.0",
        "migration_info": {"source": "v1", "destination": "v2"}
    }))
    .unwrap();
    partition.insert(&key_bytes, &with_unknown).unwrap();

    let contains = store.contains(&valid_key).unwrap();
    assert!(contains, "BUG: entry with unknown fields not readable");

    let duplicate_result = store.check_and_insert(&valid_key, &alternate_instance_id(), 60_000);
    match duplicate_result {
        Ok(AdmissionResult::Duplicate { instance_id }) => {
            assert_eq!(
                instance_id, "01ARZ3NDEKTSV4RRFFQ69G5FAV",
                "BUG: schema migration corrupted instance_id"
            );
        }
        other => panic!("BUG: expected Duplicate, got {:?}", other),
    }
}

// ========================================================================
// DIMENSION: concurrent purge + access during schema migration
// ========================================================================

#[test]
fn red_queen_fjall_concurrent_schema_migration_and_purge() {
    let dir = tempdir().unwrap();
    let database = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallDedupeStore::open(&database).unwrap();
    let store = Arc::new(store);

    let partition = database
        .keyspace("dedupe", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    for i in 0..16usize {
        let key = DedupeKey::parse(&format!("rq-schema-purge-{i}-ve-3zrs")).unwrap();
        let key_bytes = key.as_str().as_bytes().to_vec();
        let json = serde_json::to_vec(&serde_json::json!({
            "dedupe_key": format!("rq-schema-purge-{i}-ve-3zrs"),
            "instance_id": InstanceId::from_bytes([i as u8; 16]).to_string(),
            "expires_at": u64::MAX
        }))
        .unwrap();
        partition.insert(&key_bytes, &json).unwrap();
    }

    let barrier = Arc::new(std::sync::Barrier::new(3));

    let insert_handle = {
        let store = Arc::clone(&store);
        let barrier = Arc::clone(&barrier);
        thread::spawn(move || {
            barrier.wait();
            let mut admitted = 0u64;
            for i in 16..32usize {
                let key = DedupeKey::parse(&format!("rq-schema-purge-{i}-ve-3zrs")).unwrap();
                let iid = InstanceId::from_bytes([i as u8; 16]);
                if store.check_and_insert(&key, &iid, 60_000).unwrap() == AdmissionResult::Admitted
                {
                    admitted += 1;
                }
            }
            admitted
        })
    };

    let purge_handle = {
        let store = Arc::clone(&store);
        let barrier = Arc::clone(&barrier);
        thread::spawn(move || {
            barrier.wait();
            std::thread::sleep(std::time::Duration::from_millis(5));
            store.purge_expired(u64::MAX).unwrap()
        })
    };

    let read_handle = {
        let store = Arc::clone(&store);
        let barrier = Arc::clone(&barrier);
        thread::spawn(move || {
            barrier.wait();
            let mut found = 0u64;
            for i in 0..16usize {
                let key = DedupeKey::parse(&format!("rq-schema-purge-{i}-ve-3zrs")).unwrap();
                if store.contains(&key).unwrap() {
                    found += 1;
                }
            }
            found
        })
    };

    let admitted = insert_handle.join().unwrap();
    let purged = purge_handle.join().unwrap();
    let found = read_handle.join().unwrap();

    assert!(
        admitted > 0 || purged > 0 || found > 0,
        "BUG: all operations returned zero"
    );
}

// ========================================================================
// DIMENSION: lease partition corruption and recovery
// ========================================================================

#[test]
fn red_queen_fjall_lease_corruption_truncated_key_handled() {
    let dir = tempdir().unwrap();
    let database = fjall::Database::builder(dir.path()).open().unwrap();
    let lease_store = FjallLeaseStore::open(&database).unwrap();

    let partition = database
        .keyspace("leases", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    let truncated_key = vec![0xFF, 0xFE];
    let value = serde_json::to_vec(&serde_json::json!({
        "instance_id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        "step_id": "step-1",
        "fence_token": 1u64,
        "expires_at": u64::MAX
    }))
    .unwrap();
    let insert_result = partition.insert(&truncated_key, &value);
    assert!(
        insert_result.is_ok(),
        "BUG: partition rejected direct insert of truncated key"
    );

    let result = lease_store.acquire(&sample_instance_id(), &sample_step_id(), 60_000);
    assert!(
        result.is_ok(),
        "BUG: lease acquisition failed with corrupted partition"
    );
}

#[test]
fn red_queen_fjall_lease_schema_migration_old_format() {
    let dir = tempdir().unwrap();
    let database = fjall::Database::builder(dir.path()).open().unwrap();
    let lease_store = FjallLeaseStore::open(&database).unwrap();

    let partition = database
        .keyspace("leases", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    let old_format_key = format!("{}::{}", sample_instance_id(), sample_step_id());
    let old_format_json = serde_json::to_vec(&serde_json::json!({
        "instance_id": sample_instance_id().to_string(),
        "step_id": "step-1",
        "fence_token": 1u64,
        "expires_at": 0u64,
        "legacy_field": true
    }))
    .unwrap();
    partition
        .insert(old_format_key.as_bytes(), &old_format_json)
        .unwrap();

    let iid = sample_instance_id();
    let step_id = sample_step_id();
    let new_lease = lease_store.acquire(&iid, &step_id, 60_000);
    match new_lease {
        Ok(lease) => {
            assert_eq!(
                lease.token().inner().get(),
                1,
                "BUG: lease acquisition should succeed with fence_token=1 (fence_partition starts at 0)"
            );
        }
        Err(e) => panic!("BUG: lease acquisition failed on old format: {:?}", e),
    }
}

// ========================================================================
// DIMENSION: partition isolation verification
// Tests that one partition's corruption doesn't affect another
// ========================================================================

#[test]
fn red_queen_fjall_partition_isolation_dedupe_corruption_no_effect_on_lease() {
    let dir = tempdir().unwrap();
    let database = fjall::Database::builder(dir.path()).open().unwrap();
    let dedupe_store = FjallDedupeStore::open(&database).unwrap();
    let lease_store = FjallLeaseStore::open(&database).unwrap();

    let dedupe_partition = database
        .keyspace("dedupe", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    let corrupted_key = vec![0xFF, 0xFE, 0xFD];
    let corrupted_value = vec![0x00; 100];
    dedupe_partition
        .insert(&corrupted_key, &corrupted_value)
        .unwrap();

    let lease_result = lease_store.acquire(&sample_instance_id(), &sample_step_id(), 60_000);
    assert!(
        lease_result.is_ok(),
        "BUG: lease partition affected by dedupe corruption"
    );

    let dedupe_key = DedupeKey::parse("rq-isolate-test-ve-3zrs").unwrap();
    let dedupe_result = dedupe_store.check_and_insert(&dedupe_key, &sample_instance_id(), 60_000);
    assert_eq!(
        dedupe_result,
        Ok(AdmissionResult::Admitted),
        "BUG: dedupe store corrupted by its own bad entry"
    );
}

#[test]
fn red_queen_fjall_partition_isolation_lease_corruption_no_effect_on_dedupe() {
    let dir = tempdir().unwrap();
    let database = fjall::Database::builder(dir.path()).open().unwrap();
    let dedupe_store = FjallDedupeStore::open(&database).unwrap();
    let lease_store = FjallLeaseStore::open(&database).unwrap();

    let lease_partition = database
        .keyspace("leases", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    let corrupted_key = vec![0xFF, 0xFE, 0xFD];
    let corrupted_value = vec![0x00; 100];
    lease_partition
        .insert(&corrupted_key, &corrupted_value)
        .unwrap();

    let dedupe_key = DedupeKey::parse("rq-isolate-lease-ve-3zrs").unwrap();
    let dedupe_result = dedupe_store.check_and_insert(&dedupe_key, &sample_instance_id(), 60_000);
    assert_eq!(
        dedupe_result,
        Ok(AdmissionResult::Admitted),
        "BUG: dedupe store affected by lease corruption"
    );

    let lease_result = lease_store.acquire(&alternate_instance_id(), &alternate_step_id(), 60_000);
    assert!(
        lease_result.is_ok(),
        "BUG: lease store corrupted by its own bad entry"
    );
}

// ========================================================================
// DIMENSION: high-concurrency stress on multiple partitions
// ========================================================================

#[test]
fn red_queen_fjall_high_concurrency_multi_partition_stress() {
    let dir = tempdir().unwrap();
    let database = fjall::Database::builder(dir.path()).open().unwrap();
    let dedupe_store = FjallDedupeStore::open(&database).unwrap();
    let lease_store = FjallLeaseStore::open(&database).unwrap();

    let dedupe_store = Arc::new(dedupe_store);
    let lease_store = Arc::new(lease_store);

    let num_threads = 32;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let ds = Arc::clone(&dedupe_store);
            let ls = Arc::clone(&lease_store);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                if i % 2 == 0 {
                    let key = DedupeKey::parse(&format!("rq-stress-{i}-ve-3zrs")).unwrap();
                    let iid = InstanceId::from_bytes([i as u8; 16]);
                    ds.check_and_insert(&key, &iid, 60_000).map(|_| ()).map_err(
                        |e: DedupeStoreError| -> Box<dyn std::error::Error + Send + Sync> {
                            Box::new(e)
                        },
                    )
                } else {
                    let iid = InstanceId::from_bytes([i as u8; 16]);
                    let step_id = StepId::parse(&format!("step-{}", i)).unwrap();
                    ls.acquire(&iid, &step_id, 60_000)
                        .map(|_| ())
                        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })
                }
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    let success_count = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(
        success_count, num_threads,
        "BUG: {}/{} operations succeeded under high concurrency",
        success_count, num_threads
    );
}

// ========================================================================
// DIMENSION: partition constants integrity
// Contract: partition names are non-empty, valid UTF-8, no control chars
// ========================================================================

#[test]
fn red_queen_fjall_partition_names_are_valid_utf8() {
    let partitions = [
        ("dedupe", vo_storage::dedupe_partition::DEDUPE_PARTITION),
        ("effects", vo_storage::effect_journal::EFFECTS_PARTITION),
        ("leases", vo_storage::lease_partition::LEASE_PARTITION),
    ];

    for (name, partition) in partitions {
        assert!(!partition.is_empty(), "BUG: partition '{}' is empty", name);
        assert!(
            partition.is_ascii(),
            "BUG: partition '{}' contains non-ASCII: {:?}",
            name,
            partition
        );
        assert!(
            partition.chars().all(|c| !c.is_control()),
            "BUG: partition '{}' contains control character: {:?}",
            name,
            partition
        );
    }
}
