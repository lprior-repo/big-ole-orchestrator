//! Section 9: Prefix scans

use crate::open_partition;

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
