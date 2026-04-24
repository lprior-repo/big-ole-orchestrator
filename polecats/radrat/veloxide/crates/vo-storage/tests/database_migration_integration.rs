//! Database migration integration tests for vo-storage (ve-yv21).
//!
//! Tests the database migration process and data integrity across schema
//! evolution scenarios. Validates that:
//!
//! 1. **Partition layout migration** — new partitions can be added without
//!    corrupting existing data.
//! 2. **Codec forward/backward compatibility** — binary-encoded data from
//!    older versions can be decoded by the current codec, and vice versa.
//! 3. **Projection schema versioning** — the `projection_compat` module
//!    correctly identifies Fresh, NeedsUpcast, and Stale records.
//! 4. **Keyspace recreation** — a keyspace can be closed and reopened, with
//!    all partition data intact.
//! 5. **Cross-partition data integrity** — writing to multiple partitions
//!    and then migrating (reopening) preserves all data across partitions.

#![allow(clippy::unwrap_used)]

use tempfile::tempdir;
use vo_storage::codec::{decode_event_key, encode_event_key};
use vo_storage::dedupe_partition::{
    decode_dedupe_entry, encode_dedupe_entry, AdmissionResult, DedupeEntry, DedupeStore,
    FjallDedupeStore,
};
use vo_storage::lease_partition::{FjallLeaseStore, LeaseStore};
use vo_storage::partitions::{
    create_partition_layout, get_partition_config, open_all_partitions, PartitionClass,
    PartitionConfig, PartitionInfo, StorageConfig, ALL_PARTITIONS, BLOB_PACK_INDEX_PARTITION,
    BLOB_PARTITIONS, BLOB_RECORDS_PARTITION, COLD_PARTITIONS, DEDUPE_PARTITION, EFFECTS_PARTITION,
    EVENTS_PARTITION, HOT_PARTITIONS, INSTANCES_PARTITION, LEASE_PARTITION,
    PAYLOAD_BLOBS_PARTITION, SNAPSHOTS_PARTITION, TIMERS_PARTITION, WORKFLOW_VERSIONS_PARTITION,
};
use vo_storage::projection_compat::{
    check_projection_compat, is_projection_compatible, projection_compat_window,
    validate_projection_batch, validate_projection_payload, window_max_supported,
    CompatibleProjectionIterator, ProjectionCompat, ProjectionCompatibilityWindow, ProjectionError,
    ProjectionRecord,
};
use vo_types::{DedupeKey, InstanceId, SequenceNumber, StepId};

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

fn alternate_instance_id() -> InstanceId {
    InstanceId::from_bytes([2u8; 16])
}

fn sample_step_id() -> StepId {
    StepId::parse("step-migration-1").unwrap()
}

// ========================================================================
// SUITE 1: Keyspace recreation preserves data
// ========================================================================

#[test]
fn migration_keyspace_reopen_preserves_dedupe_entries() {
    let dir = tempdir().unwrap();

    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let store = FjallDedupeStore::open(&keyspace).unwrap();

    let key = DedupeKey::parse("migration-reopen-ve-yv21").unwrap();
    let iid = sample_instance_id();
    store.check_and_insert(&key, &iid, 60_000).unwrap();
    drop(store);
    drop(keyspace);

    let keyspace2 = fjall::Config::new(dir.path()).open().unwrap();
    let store2 = FjallDedupeStore::open(&keyspace2).unwrap();

    assert!(
        store2.contains(&key).unwrap(),
        "BUG: dedupe entry lost after keyspace reopen"
    );

    let result = store2
        .check_and_insert(&key, &alternate_instance_id(), 60_000)
        .unwrap();
    assert!(
        matches!(result, AdmissionResult::Duplicate { .. }),
        "BUG: reopened store did not detect duplicate"
    );
}

#[test]
fn migration_keyspace_reopen_preserves_lease_entries() {
    let dir = tempdir().unwrap();

    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let store = FjallLeaseStore::open(&keyspace).unwrap();

    let iid = sample_instance_id();
    let step_id = sample_step_id();
    let lease = store.acquire(&iid, &step_id, 60_000).unwrap();
    let token = lease.token().clone();
    drop(store);
    drop(keyspace);

    let keyspace2 = fjall::Config::new(dir.path()).open().unwrap();
    let store2 = FjallLeaseStore::open(&keyspace2).unwrap();

    let is_stale = store2.check_stale_fence(&iid, &step_id, &token).unwrap();
    assert!(
        !is_stale,
        "BUG: lease fence token reported stale after keyspace reopen"
    );
}

#[test]
fn migration_keyspace_reopen_preserves_cross_partition_data() {
    let dir = tempdir().unwrap();

    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let dedupe = FjallDedupeStore::open(&keyspace).unwrap();
    let lease = FjallLeaseStore::open(&keyspace).unwrap();

    let dk = DedupeKey::parse("migration-cross-ve-yv21").unwrap();
    let iid = sample_instance_id();
    let step = sample_step_id();

    dedupe.check_and_insert(&dk, &iid, 60_000).unwrap();
    let l = lease.acquire(&iid, &step, 60_000).unwrap();
    let token = l.token().clone();
    drop(dedupe);
    drop(lease);
    drop(keyspace);

    let keyspace2 = fjall::Config::new(dir.path()).open().unwrap();
    let dedupe2 = FjallDedupeStore::open(&keyspace2).unwrap();
    let lease2 = FjallLeaseStore::open(&keyspace2).unwrap();

    assert!(
        dedupe2.contains(&dk).unwrap(),
        "BUG: dedupe lost after reopen"
    );
    let is_stale = lease2.check_stale_fence(&iid, &step, &token).unwrap();
    assert!(!is_stale, "BUG: lease lost after reopen");
}

// ========================================================================
// SUITE 2: Codec forward/backward compatibility
// ========================================================================

#[test]
fn migration_codec_binary_dedupe_entry_roundtrip() {
    let entry = DedupeEntry::new(
        "migration-codec-ve-yv21".to_string(),
        "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
        u64::MAX,
    )
    .unwrap();

    let encoded = encode_dedupe_entry(&entry).unwrap();
    let decoded = decode_dedupe_entry(&encoded).unwrap();

    assert_eq!(decoded.dedupe_key(), entry.dedupe_key());
    assert_eq!(decoded.instance_id(), entry.instance_id());
    assert_eq!(decoded.expires_at(), entry.expires_at());
}

#[test]
fn migration_codec_binary_dedupe_entry_with_trailing_bytes_still_decodes() {
    let entry = DedupeEntry::new(
        "migration-trailing-ve-yv21".to_string(),
        "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
        999_999,
    )
    .unwrap();

    let mut encoded = encode_dedupe_entry(&entry).unwrap();
    encoded.extend_from_slice(b"future_schema_metadata_v2");
    let decoded = decode_dedupe_entry(&encoded).unwrap();

    assert_eq!(decoded.dedupe_key(), entry.dedupe_key());
    assert_eq!(decoded.instance_id(), entry.instance_id());
    assert_eq!(decoded.expires_at(), entry.expires_at());
}

#[test]
fn migration_codec_event_key_roundtrip() {
    let id = InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    let seq = SequenceNumber::try_from(42u64).unwrap();

    let encoded = encode_event_key(&id, &seq).unwrap();
    let (decoded_id, decoded_seq) = decode_event_key(&encoded).unwrap();

    assert_eq!(decoded_id, id);
    assert_eq!(decoded_seq, seq);
}

#[test]
fn migration_codec_event_key_survives_write_to_fjall_and_read_back() {
    let dir = tempdir().unwrap();
    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let partition = keyspace
        .open_partition(EVENTS_PARTITION, fjall::PartitionCreateOptions::default())
        .unwrap();

    let id = InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    let seq = SequenceNumber::try_from(7u64).unwrap();
    let key = encode_event_key(&id, &seq).unwrap();
    let value = b"test-event-payload-v1";

    partition.insert(&key, value).unwrap();

    let read_back = partition.get(&key).unwrap().unwrap();
    assert_eq!(&*read_back, value);

    let (decoded_id, decoded_seq) = decode_event_key(&key).unwrap();
    assert_eq!(decoded_id, id);
    assert_eq!(decoded_seq, seq);
}

#[test]
fn migration_codec_rejects_truncated_binary_dedupe_entry() {
    let truncated = vec![0x00, 0x05];
    let result = decode_dedupe_entry(&truncated);
    assert!(result.is_err(), "BUG: truncated entry should be rejected");
}

#[test]
fn migration_codec_rejects_empty_binary_dedupe_entry() {
    let result = decode_dedupe_entry(&[]);
    assert!(result.is_err(), "BUG: empty entry should be rejected");
}

// ========================================================================
// SUITE 3: Projection schema versioning migration
// ========================================================================

#[test]
fn migration_projection_v1_to_v3_needs_upcast() {
    let window = projection_compat_window(1, 3).unwrap();
    let result = check_projection_compat(1, &window).unwrap();
    assert_eq!(result, ProjectionCompat::NeedsUpcast { from: 1, to: 3 });
}

#[test]
fn migration_projection_v2_to_v3_needs_upcast() {
    let window = projection_compat_window(1, 3).unwrap();
    let result = check_projection_compat(2, &window).unwrap();
    assert_eq!(result, ProjectionCompat::NeedsUpcast { from: 2, to: 3 });
}

#[test]
fn migration_projection_v3_is_fresh() {
    let window = projection_compat_window(1, 3).unwrap();
    let result = check_projection_compat(3, &window).unwrap();
    assert_eq!(result, ProjectionCompat::Fresh);
}

#[test]
fn migration_projection_v0_always_stale() {
    let window = projection_compat_window(1, 3).unwrap();
    let result = check_projection_compat(0, &window).unwrap();
    assert_eq!(result, ProjectionCompat::StaleZeroVersion);
}

#[test]
fn migration_projection_window_widening_preserves_old_data() {
    let old_window = projection_compat_window(3, 5).unwrap();
    assert!(
        !is_projection_compatible(2, &old_window),
        "v2 should be stale under old window [3,5]"
    );

    let new_window = projection_compat_window(1, 5).unwrap();
    assert!(
        is_projection_compatible(2, &new_window),
        "v2 should be compatible after widening window to [1,5]"
    );
    assert!(
        is_projection_compatible(3, &new_window),
        "v3 should still be compatible after widening"
    );
}

#[test]
fn migration_projection_batch_validates_mixed_versions() {
    let window = projection_compat_window(1, 3).unwrap();
    let p1 = br#"{"version": 3, "data": "fresh"}"#.to_vec();
    let p2 = br#"{"version": 2, "data": "needs-upcast"}"#.to_vec();
    let p3 = br#"{"version": 1, "data": "oldest-supported"}"#.to_vec();
    let payloads: Vec<&[u8]> = vec![&p1, &p2, &p3];
    let result = validate_projection_batch(payloads, &window);
    assert!(result.is_ok(), "BUG: valid mixed-version batch rejected");
}

#[test]
fn migration_projection_batch_rejects_stale_in_middle() {
    let window = projection_compat_window(2, 4).unwrap();
    let p1 = br#"{"version": 4}"#.to_vec();
    let p2 = br#"{"version": 1}"#.to_vec();
    let p3 = br#"{"version": 3}"#.to_vec();
    let payloads: Vec<&[u8]> = vec![&p1, &p2, &p3];
    let result = validate_projection_batch(payloads, &window);
    assert!(
        matches!(result, Err(ProjectionError::StaleProjection(1, 2, 4))),
        "BUG: stale v1 should be rejected in batch"
    );
}

#[test]
fn migration_projection_json_payload_roundtrip_through_fjall() {
    let dir = tempdir().unwrap();
    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let partition = keyspace
        .open_partition("projection_test", fjall::PartitionCreateOptions::default())
        .unwrap();

    let window = projection_compat_window(1, 3).unwrap();

    let payloads_by_version: Vec<(u8, &[u8])> = vec![
        (1, br#"{"version": 1, "field_a": "old"}"#),
        (2, br#"{"version": 2, "field_a": "new", "field_b": 42}"#),
        (
            3,
            br#"{"version": 3, "field_a": "latest", "field_b": 42, "field_c": true}"#,
        ),
    ];

    for (v, payload) in &payloads_by_version {
        let key = format!("projection-v{v}");
        partition.insert(key.as_bytes(), *payload).unwrap();
    }

    for (v, original_payload) in &payloads_by_version {
        let key = format!("projection-v{v}");
        let stored = partition.get(key.as_bytes()).unwrap().unwrap();
        let compat = validate_projection_payload(&stored, &window).unwrap();
        if *v == window_max_supported(&window) {
            assert_eq!(compat, ProjectionCompat::Fresh);
        } else {
            assert!(compat.is_compatible());
        }
        assert_eq!(&*stored, *original_payload);
    }
}

// ========================================================================
// SUITE 4: Partition layout migration
// ========================================================================

#[test]
fn migration_all_partitions_openable_on_fresh_keyspace() {
    let dir = tempdir().unwrap();
    let layout = create_partition_layout(dir.path()).unwrap();
    let result = open_all_partitions(&layout);
    assert!(
        result.is_ok(),
        "BUG: failed to open all partitions on fresh keyspace"
    );
    let partitions = result.unwrap();
    assert_eq!(partitions.len(), ALL_PARTITIONS.len());
}

#[test]
fn migration_partition_names_match_constants() {
    let expected = vec![
        EVENTS_PARTITION,
        INSTANCES_PARTITION,
        TIMERS_PARTITION,
        SNAPSHOTS_PARTITION,
        DEDUPE_PARTITION,
        "dedupe_retention",
        EFFECTS_PARTITION,
        LEASE_PARTITION,
        WORKFLOW_VERSIONS_PARTITION,
        PAYLOAD_BLOBS_PARTITION,
        BLOB_RECORDS_PARTITION,
        BLOB_PACK_INDEX_PARTITION,
    ];
    assert_eq!(ALL_PARTITIONS.len(), expected.len());
    for name in &expected {
        assert!(
            ALL_PARTITIONS.contains(name),
            "BUG: missing partition constant '{name}'"
        );
    }
}

#[test]
fn migration_classified_partitions_have_correct_configs() {
    for name in HOT_PARTITIONS {
        let config = get_partition_config(name);
        assert_eq!(
            config.bloom_filter_bits_per_key, 10,
            "BUG: {name} not classified as hot"
        );
    }
    for name in COLD_PARTITIONS {
        let config = get_partition_config(name);
        assert_eq!(
            config.bloom_filter_bits_per_key, 0,
            "BUG: {name} cold should have 0 bloom bits"
        );
        assert_eq!(
            config.flush_interval_bytes,
            256 * 1024 * 1024,
            "BUG: {name} cold flush interval wrong"
        );
    }
    for name in BLOB_PARTITIONS {
        let config = get_partition_config(name);
        assert_eq!(
            config.flush_interval_bytes,
            1024 * 1024 * 1024,
            "BUG: {name} blob flush interval wrong"
        );
    }
}

#[test]
fn migration_unclassified_partitions_get_default_config() {
    let unclassified: Vec<&&str> = ALL_PARTITIONS
        .iter()
        .filter(|name| {
            !HOT_PARTITIONS.contains(name)
                && !COLD_PARTITIONS.contains(name)
                && !BLOB_PARTITIONS.contains(name)
        })
        .collect();

    for name in &unclassified {
        let config = get_partition_config(name);
        assert_eq!(
            config,
            PartitionConfig::default(),
            "BUG: unclassified partition '{}' should get default config",
            name
        );
    }
}

#[test]
fn migration_adding_new_partition_does_not_corrupt_existing() {
    let dir = tempdir().unwrap();
    let keyspace = fjall::Config::new(dir.path()).open().unwrap();

    let dedupe = FjallDedupeStore::open(&keyspace).unwrap();
    let dk = DedupeKey::parse("migration-new-part-ve-yv21").unwrap();
    dedupe
        .check_and_insert(&dk, &sample_instance_id(), 60_000)
        .unwrap();

    let custom_partition = keyspace
        .open_partition(
            "future_new_partition_v2",
            fjall::PartitionCreateOptions::default(),
        )
        .unwrap();
    custom_partition.insert(b"new-key", b"new-value").unwrap();

    assert!(
        dedupe.contains(&dk).unwrap(),
        "BUG: existing data corrupted by adding new partition"
    );

    let val = custom_partition.get(b"new-key").unwrap().unwrap();
    assert_eq!(&*val, b"new-value");
}

// ========================================================================
// SUITE 5: Data integrity across write-migrate-read cycles
// ========================================================================

#[test]
fn migration_write_close_reopen_read_all_partitions_intact() {
    let dir = tempdir().unwrap();

    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let dedupe = FjallDedupeStore::open(&keyspace).unwrap();
    let lease = FjallLeaseStore::open(&keyspace).unwrap();

    let events_partition = keyspace
        .open_partition(EVENTS_PARTITION, fjall::PartitionCreateOptions::default())
        .unwrap();
    let instances_partition = keyspace
        .open_partition(
            INSTANCES_PARTITION,
            fjall::PartitionCreateOptions::default(),
        )
        .unwrap();

    let iid = sample_instance_id();
    let dk = DedupeKey::parse("migration-cycle-ve-yv21").unwrap();
    let step = sample_step_id();

    dedupe.check_and_insert(&dk, &iid, 60_000).unwrap();
    let lease_result = lease.acquire(&iid, &step, 60_000).unwrap();
    let token = lease_result.token().clone();

    let id = InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    let seq = SequenceNumber::try_from(1u64).unwrap();
    let event_key = encode_event_key(&id, &seq).unwrap();
    events_partition.insert(&event_key, b"event-v1").unwrap();
    instances_partition
        .insert(iid.to_string().as_bytes(), b"instance-running")
        .unwrap();

    drop(events_partition);
    drop(instances_partition);
    drop(dedupe);
    drop(lease);
    drop(keyspace);

    let keyspace2 = fjall::Config::new(dir.path()).open().unwrap();
    let dedupe2 = FjallDedupeStore::open(&keyspace2).unwrap();
    let lease2 = FjallLeaseStore::open(&keyspace2).unwrap();
    let events2 = keyspace2
        .open_partition(EVENTS_PARTITION, fjall::PartitionCreateOptions::default())
        .unwrap();
    let instances2 = keyspace2
        .open_partition(
            INSTANCES_PARTITION,
            fjall::PartitionCreateOptions::default(),
        )
        .unwrap();

    assert!(
        dedupe2.contains(&dk).unwrap(),
        "BUG: dedupe lost after cycle"
    );
    assert!(
        !lease2.check_stale_fence(&iid, &step, &token).unwrap(),
        "BUG: lease lost after cycle"
    );

    let event_val = events2.get(&event_key).unwrap().unwrap();
    assert_eq!(&*event_val, b"event-v1", "BUG: event data corrupted");

    let instance_val = instances2.get(iid.to_string().as_bytes()).unwrap().unwrap();
    assert_eq!(
        &*instance_val, b"instance-running",
        "BUG: instance data corrupted"
    );
}

#[test]
fn migration_binary_dedupe_entry_in_fjall_survives_codec_evolution() {
    let dir = tempdir().unwrap();
    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let partition = keyspace
        .open_partition(DEDUPE_PARTITION, fjall::PartitionCreateOptions::default())
        .unwrap();

    let entry = DedupeEntry::new(
        "migration-fjall-codec-ve-yv21".to_string(),
        "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
        u64::MAX,
    )
    .unwrap();
    let binary = encode_dedupe_entry(&entry).unwrap();

    let key_bytes = entry.dedupe_key().as_bytes().to_vec();
    partition.insert(&key_bytes, &binary).unwrap();

    let store = FjallDedupeStore::open(&keyspace).unwrap();
    let dk = DedupeKey::parse("migration-fjall-codec-ve-yv21").unwrap();
    assert!(store.contains(&dk).unwrap());

    let result = store
        .check_and_insert(&dk, &alternate_instance_id(), 60_000)
        .unwrap();
    match result {
        AdmissionResult::Duplicate { instance_id } => {
            assert_eq!(
                instance_id, "01ARZ3NDEKTSV4RRFFQ69G5FAV",
                "BUG: binary-encoded instance_id corrupted"
            );
        }
        other => panic!("BUG: expected Duplicate, got {:?}", other),
    }
}

#[test]
fn migration_multiple_entries_survive_reopen() {
    let dir = tempdir().unwrap();
    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let store = FjallDedupeStore::open(&keyspace).unwrap();

    let count = 50usize;
    for i in 0..count {
        let key = DedupeKey::parse(&format!("migration-batch-{i}-ve-yv21")).unwrap();
        let iid = InstanceId::from_bytes([i as u8; 16]);
        store.check_and_insert(&key, &iid, 60_000).unwrap();
    }
    drop(store);
    drop(keyspace);

    let keyspace2 = fjall::Config::new(dir.path()).open().unwrap();
    let store2 = FjallDedupeStore::open(&keyspace2).unwrap();

    for i in 0..count {
        let key = DedupeKey::parse(&format!("migration-batch-{i}-ve-yv21")).unwrap();
        assert!(
            store2.contains(&key).unwrap(),
            "BUG: entry {i} lost after reopen"
        );
    }
}

// ========================================================================
// SUITE 6: ProjectionCompat iterator and window edge cases
// ========================================================================

#[test]
fn migration_projection_window_min_equals_max_still_fresh() {
    let window = projection_compat_window(5, 5).unwrap();
    assert_eq!(
        check_projection_compat(5, &window).unwrap(),
        ProjectionCompat::Fresh
    );
    assert_eq!(
        check_projection_compat(4, &window).unwrap(),
        ProjectionCompat::StaleTooOld {
            projection: 4,
            window_min: 5
        }
    );
}

#[test]
fn migration_projection_iterator_accepts_valid_window() {
    let window = projection_compat_window(1, 3).unwrap();
    let records: Vec<Result<ProjectionRecord, &'static str>> = vec![
        Ok(ProjectionRecord::new(3, vec![])),
        Ok(ProjectionRecord::new(2, vec![])),
    ];
    let iter = CompatibleProjectionIterator::new(records.into_iter(), window);
    assert!(iter.is_ok());
}

#[test]
fn migration_projection_window_constructor_rejects_zero_min() {
    let result = projection_compat_window(0, 5);
    assert!(
        matches!(result, Err(ProjectionError::WindowMisconfigured { .. })),
        "BUG: zero min should be rejected"
    );
}

#[test]
fn migration_projection_window_constructor_rejects_max_lt_min() {
    let result = projection_compat_window(5, 3);
    assert!(
        matches!(result, Err(ProjectionError::WindowMisconfigured { .. })),
        "BUG: max < min should be rejected"
    );
}

#[test]
fn migration_projection_error_display_includes_version_info() {
    let err = ProjectionError::StaleProjection(1, 3, 7);
    let msg = err.to_string();
    assert!(msg.contains('1'), "should contain stale version");
    assert!(msg.contains('3'), "should contain window min");
    assert!(msg.contains('7'), "should contain window max");
}

#[test]
fn migration_projection_error_is_stale_predicate() {
    assert!(ProjectionError::StaleProjection(1, 2, 5).is_stale());
    assert!(!ProjectionError::MissingSchemaVersion.is_stale());
    assert!(!ProjectionError::InvalidSchemaVersionType.is_stale());
}

// ========================================================================
// SUITE 7: Partition config consistency and migration safety
// ========================================================================

#[test]
fn migration_partition_config_hot_cold_blob_are_stable() {
    let hot = PartitionConfig::hot();
    let cold = PartitionConfig::cold();
    let blob = PartitionConfig::blob();

    assert!(hot.compaction_enabled);
    assert!(cold.compaction_enabled);
    assert!(blob.compaction_enabled);

    assert_eq!(hot.bloom_filter_bits_per_key, 10);
    assert_eq!(cold.bloom_filter_bits_per_key, 0);
    assert_eq!(blob.bloom_filter_bits_per_key, 0);

    assert!(cold.flush_interval_bytes > hot.flush_interval_bytes);
    assert!(blob.flush_interval_bytes > cold.flush_interval_bytes);
}

#[test]
fn migration_storage_config_default_path_is_deterministic() {
    let config = StorageConfig::default();
    assert_eq!(config.path, "/tmp/veloxide-storage");
    assert!(config.compaction_enabled);
}

#[test]
fn migration_partition_info_new_records_class_correctly() {
    let info = PartitionInfo::new("test_partition", PartitionClass::Hot);
    assert_eq!(info.name, "test_partition");
    assert_eq!(info.class, PartitionClass::Hot);
}

#[test]
fn migration_partition_class_display_roundtrips() {
    assert_eq!(PartitionClass::Hot.to_string(), "hot");
    assert_eq!(PartitionClass::Cold.to_string(), "cold");
    assert_eq!(PartitionClass::Blob.to_string(), "blob");
}

#[test]
fn migration_get_partition_config_default_for_unknown() {
    let config = get_partition_config("nonexistent_future_partition");
    assert_eq!(config, PartitionConfig::default());
}

// ========================================================================
// SUITE 8: Window evolution across migration steps
// ========================================================================

#[test]
fn migration_window_narrowing_makes_previously_compatible_stale() {
    let wide_window = projection_compat_window(1, 10).unwrap();
    assert!(is_projection_compatible(2, &wide_window));

    let narrow_window = projection_compat_window(5, 10).unwrap();
    assert!(
        !is_projection_compatible(2, &narrow_window),
        "BUG: v2 should become stale after window narrows to [5,10]"
    );

    let result = check_projection_compat(2, &narrow_window).unwrap();
    assert_eq!(
        result,
        ProjectionCompat::StaleTooOld {
            projection: 2,
            window_min: 5
        }
    );
}

#[test]
fn migration_window_bump_max_makes_previous_max_need_upcast() {
    let v1_window = projection_compat_window(1, 3).unwrap();
    assert_eq!(
        check_projection_compat(3, &v1_window).unwrap(),
        ProjectionCompat::Fresh
    );

    let v2_window = projection_compat_window(1, 4).unwrap();
    assert_eq!(
        check_projection_compat(3, &v2_window).unwrap(),
        ProjectionCompat::NeedsUpcast { from: 3, to: 4 }
    );
    assert_eq!(
        check_projection_compat(4, &v2_window).unwrap(),
        ProjectionCompat::Fresh
    );
}

#[test]
fn migration_all_stale_variants_are_incompatible() {
    let window = projection_compat_window(3, 7).unwrap();

    let stale_cases = [
        check_projection_compat(0, &window).unwrap(),
        check_projection_compat(1, &window).unwrap(),
        check_projection_compat(2, &window).unwrap(),
        check_projection_compat(10, &window).unwrap(),
        check_projection_compat(255, &window).unwrap(),
    ];

    for case in stale_cases {
        assert!(
            !case.is_compatible(),
            "BUG: stale variant {:?} reported as compatible",
            case
        );
    }
}

// ========================================================================
// SUITE 9: Large-scale data integrity
// ========================================================================

#[test]
fn migration_large_batch_dedupe_entries_survive_codec() {
    let count = 200usize;
    let entries: Vec<DedupeEntry> = (0..count)
        .map(|i| {
            DedupeEntry::new(
                format!("migration-large-{i}-ve-yv21"),
                format!("{i:026}"),
                u64::MAX - i as u64,
            )
            .unwrap()
        })
        .collect();

    for entry in &entries {
        let encoded = encode_dedupe_entry(entry).unwrap();
        let decoded = decode_dedupe_entry(&encoded).unwrap();
        assert_eq!(decoded.dedupe_key(), entry.dedupe_key());
        assert_eq!(decoded.instance_id(), entry.instance_id());
        assert_eq!(decoded.expires_at(), entry.expires_at());
    }
}

#[test]
fn migration_dedupe_entry_at_field_boundaries() {
    let max_key = "x".repeat(1000);
    let max_iid = "y".repeat(1000);
    let entry = DedupeEntry::new(max_key.clone(), max_iid.clone(), u64::MAX).unwrap();

    let encoded = encode_dedupe_entry(&entry).unwrap();
    let decoded = decode_dedupe_entry(&encoded).unwrap();

    assert_eq!(decoded.dedupe_key(), max_key);
    assert_eq!(decoded.instance_id(), max_iid);
    assert_eq!(decoded.expires_at(), u64::MAX);
}
