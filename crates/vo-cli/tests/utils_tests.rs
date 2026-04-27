use sha2::Digest;
use std::fs;
use std::path::PathBuf;
use vo_cli::utils::{file_hash, sha256_hex};

#[test]
fn file_hash_produces_sha256_hex() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.bin");
    fs::write(&path, b"hello world").unwrap();
    let hash = file_hash(&path).unwrap();
    assert_eq!(hash.len(), 64);
    let expected = format!("{:x}", sha2::Sha256::digest(b"hello world"));
    assert_eq!(hash, expected);
}

#[test]
fn file_hash_empty_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.bin");
    fs::write(&path, b"").unwrap();
    let hash = file_hash(&path).unwrap();
    assert_eq!(hash.len(), 64);
    let expected = format!("{:x}", sha2::Sha256::digest(b""));
    assert_eq!(hash, expected);
}

#[test]
fn file_hash_large_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large.bin");
    let data = vec![0xAB_u8; 100_000];
    fs::write(&path, &data).unwrap();
    let hash = file_hash(&path).unwrap();
    assert_eq!(hash.len(), 64);
    let expected = format!("{:x}", sha2::Sha256::digest(&data));
    assert_eq!(hash, expected);
}

#[test]
fn file_hash_nonexistent_file_returns_error() {
    let path = PathBuf::from("/nonexistent/file/that/does/not/exist");
    assert!(file_hash(&path).is_err());
}

#[test]
fn file_hash_different_contents_different_hashes() {
    let dir = tempfile::tempdir().unwrap();
    let p1 = dir.path().join("a.bin");
    let p2 = dir.path().join("b.bin");
    fs::write(&p1, b"content A").unwrap();
    fs::write(&p2, b"content B").unwrap();
    assert_ne!(file_hash(&p1).unwrap(), file_hash(&p2).unwrap());
}

#[test]
fn file_hash_same_contents_same_hash() {
    let dir = tempfile::tempdir().unwrap();
    let p1 = dir.path().join("a.bin");
    let p2 = dir.path().join("b.bin");
    fs::write(&p1, b"identical content").unwrap();
    fs::write(&p2, b"identical content").unwrap();
    assert_eq!(file_hash(&p1).unwrap(), file_hash(&p2).unwrap());
}

#[test]
fn file_hash_binary_content() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("binary.bin");
    let data: Vec<u8> = (0..=255).collect();
    fs::write(&path, &data).unwrap();
    let hash = file_hash(&path).unwrap();
    assert_eq!(hash.len(), 64);
}

#[test]
fn sha256_hex_always_64_chars() {
    let result = sha256_hex("short");
    assert_eq!(result.len(), 64);
    assert!(result.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn sha256_hex_is_hex_digit_string() {
    let result = sha256_hex("abc");
    assert_eq!(result.len(), 64);
    assert!(result.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn sha256_hex_empty_input() {
    let result = sha256_hex("");
    assert_eq!(result.len(), 64);
    assert_eq!(result, format!("{:x}", Sha256::digest(b"")));
}

#[test]
fn sha256_hex_long_input_still_64_chars() {
    let result = sha256_hex(&"x".repeat(100));
    assert_eq!(result.len(), 64);
}

#[test]
fn sha256_hex_64_char_input_still_64_chars() {
    let result = sha256_hex(&"a".repeat(64));
    assert_eq!(result.len(), 64);
}
