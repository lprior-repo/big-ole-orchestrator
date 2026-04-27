//! Section 5: Partition Layout

use vo_storage::partitions::{create_partition_layout, open_all_partitions, ALL_PARTITIONS, BLOB_PARTITIONS, COLD_PARTITIONS, HOT_PARTITIONS};

#[test]
fn create_partition_layout_opens_fjall_database() {
    let dir = tempfile::tempdir().expect("tempdir");
    let layout = create_partition_layout(dir.path()).expect("layout");
    assert!(dir.path().exists());
    let _db = layout.db();
}

#[test]
fn open_all_partitions_opens_every_defined_partition() {
    let dir = tempfile::tempdir().expect("tempdir");
    let layout = create_partition_layout(dir.path()).expect("layout");

    let partitions = open_all_partitions(&layout).expect("open all");
    assert_eq!(partitions.len(), ALL_PARTITIONS.len());

    let names: Vec<&str> = partitions.iter().map(|(n, _)| *n).collect();
    for expected in ALL_PARTITIONS {
        assert!(names.contains(expected), "missing partition: {expected}");
    }
}

#[test]
fn partition_class_counts_match_constants() {
    let hot = HOT_PARTITIONS.len();
    let cold = COLD_PARTITIONS.len();
    let blob = BLOB_PARTITIONS.len();
    assert_eq!(hot + cold + blob + 1, ALL_PARTITIONS.len());
}

#[test]
fn storage_engine_opens_with_all_stores() {
    let dir = tempfile::tempdir().expect("tempdir");
    let engine = vo_storage::partitions::StorageEngine::open(dir.path()).expect("engine open");
    let _db = engine.db();
}
