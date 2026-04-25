//! Section 8: Batch writes

use crate::open_fjall;

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
