//! QA tests for vo-storage: Batch writes and prefix scans.
//!
//! All tests use real Fjall instances in temp directories. No mocks.

fn open_partition(name: &str) -> (tempfile::TempDir, fjall::Database, fjall::Keyspace) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = fjall::Database::builder(dir.path())
        .open()
        .expect("fjall open");
    let ks = db
        .keyspace(name, || fjall::KeyspaceCreateOptions::default())
        .expect("partition open");
    (dir, db, ks)
}

fn open_fjall() -> (tempfile::TempDir, fjall::Database) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = fjall::Database::builder(dir.path())
        .open()
        .expect("fjall open");
    (dir, db)
}

// ══════════════════════════════════════════════════════════════════════════════
// Section 8: Batch writes
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn batch_write_commits_multiple_keys_atomically() {
    let (_dir, db) = open_fjall();
    let ks = db
        .keyspace("batch_test", || fjall::KeyspaceCreateOptions::default())
        .expect("partition");

    let mut batch = db.batch();
    batch.insert(&ks, b"key-a", b"val-a");
    batch.insert(&ks, b"key-b", b"val-b");
    batch.insert(&ks, b"key-c", b"val-c");
    batch.commit().expect("commit");

    assert_eq!(
        ks.get(b"key-a").expect("get").expect("a").as_ref(),
        b"val-a"
    );
    assert_eq!(
        ks.get(b"key-b").expect("get").expect("b").as_ref(),
        b"val-b"
    );
    assert_eq!(
        ks.get(b"key-c").expect("get").expect("c").as_ref(),
        b"val-c"
    );
}

#[test]
fn batch_write_with_delete_commits_both() {
    let (_dir, db) = open_fjall();
    let ks = db
        .keyspace("batch_del", || fjall::KeyspaceCreateOptions::default())
        .expect("partition");

    ks.insert(b"old", b"will-be-deleted").expect("insert");
    assert!(ks.get(b"old").expect("get").is_some());

    let mut batch = db.batch();
    batch.remove(&ks, b"old");
    batch.insert(&ks, b"new", b"fresh");
    batch.commit().expect("commit");

    assert!(ks.get(b"old").expect("get").is_none());
    assert_eq!(
        ks.get(b"new").expect("get").expect("new").as_ref(),
        b"fresh"
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// Section 9: Prefix scans
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn prefix_scan_returns_matching_keys() {
    let (_dir, _db, ks) = open_partition("prefix_scan");

    ks.insert(b"usr:1:name", b"Alice").unwrap();
    ks.insert(b"usr:1:email", b"alice@example.com").unwrap();
    ks.insert(b"usr:2:name", b"Bob").unwrap();
    ks.insert(b"other:key", b"val").unwrap();

    let mut results: Vec<Vec<u8>> = Vec::new();
    for item in ks.prefix(b"usr:1:") {
        let (_, v) = item.into_inner().expect("item");
        results.push(v.to_vec());
    }

    assert_eq!(results.len(), 2);
}

#[test]
fn prefix_scan_returns_empty_for_no_matches() {
    let (_dir, _db, ks) = open_partition("prefix_empty");

    ks.insert(b"aaa", b"1").unwrap();
    ks.insert(b"bbb", b"2").unwrap();

    let count = ks.prefix(b"zzz").count();
    assert_eq!(count, 0);
}