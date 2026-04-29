use std::path::Path;

use vo_cli::utils::{file_hash, sha256_hex};

#[test]
fn sha256_hex_pads_to_64_chars() {
    let result = sha256_hex("test");
    assert_eq!(result.len(), 64);
}

#[test]
fn sha256_hex_empty_input() {
    let result = sha256_hex("");
    assert_eq!(result.len(), 64);
    assert!(result.chars().all(|c| c == '0'));
}

#[test]
fn file_hash_deterministic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("hash_test");
    fs::write(&path, b"hello world").unwrap();

    let h1 = file_hash(&path).unwrap();
    let h2 = file_hash(&path).unwrap();
    assert_eq!(h1, h2);
    assert_eq!(h1.len(), 64);
}

#[test]
fn file_hash_different_content() {
    let dir = tempfile::tempdir().unwrap();
    let p1 = dir.path().join("a");
    let p2 = dir.path().join("b");
    fs::write(&p1, b"content a").unwrap();
    fs::write(&p2, b"content b").unwrap();

    let h1 = file_hash(&p1).unwrap();
    let h2 = file_hash(&p2).unwrap();
    assert_ne!(h1, h2);
}

#[test]
fn file_hash_nonexistent_file() {
    let result = file_hash(Path::new("/tmp/nonexistent-file-hash-test"));
    assert!(result.is_err());
}
