use super::*;
use tempfile::TempDir;

fn create_test_cache() -> (MmapCache, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let cache = MmapCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
    (cache, temp_dir)
}

#[test]
fn prefetch_existing_key_returns_ok() {
    let (mut cache, _dir) = create_test_cache();
    cache.insert("key1", b"value").unwrap();
    let result = cache.prefetch("key1");
    assert!(result.is_ok());
}

#[test]
fn prefetch_nonexistent_key_returns_ok_silently() {
    let (cache, _dir) = create_test_cache();
    let result = cache.prefetch("nonexistent");
    assert!(result.is_ok());
}

#[test]
fn prefetch_missing_key_does_not_error() {
    let (cache, _dir) = create_test_cache();
    let result = cache.prefetch("missing");
    assert!(result.is_ok());
}

#[test]
fn read_ahead_multiple_existing_keys_returns_ok() {
    let (mut cache, _dir) = create_test_cache();
    cache.insert("key1", b"value1").unwrap();
    cache.insert("key2", b"value2").unwrap();
    let result = cache.read_ahead(&["key1", "key2"]);
    assert!(result.is_ok());
}

#[test]
fn read_ahead_with_mix_of_existing_and_missing_continues_on_error() {
    let (mut cache, _dir) = create_test_cache();
    cache.insert("key1", b"value1").unwrap();
    let result = cache.read_ahead(&["key1", "missing", "key2"]);
    assert!(result.is_ok());
}

#[test]
fn read_ahead_empty_key_list_returns_ok() {
    let (cache, _dir) = create_test_cache();
    let result = cache.read_ahead(&[]);
    assert!(result.is_ok());
}

#[test]
fn read_ahead_single_key_returns_ok() {
    let (mut cache, _dir) = create_test_cache();
    cache.insert("key1", b"value1").unwrap();
    let result = cache.read_ahead(&["key1"]);
    assert!(result.is_ok());
}

#[test]
fn read_ahead_all_missing_keys_returns_ok() {
    let (cache, _dir) = create_test_cache();
    let result = cache.read_ahead(&["missing1", "missing2"]);
    assert!(result.is_ok());
}

#[test]
fn read_ahead_continues_after_first_error_when_second_key_missing() {
    let (mut cache, _dir) = create_test_cache();
    cache.insert("key1", b"value1").unwrap();
    let result = cache.read_ahead(&["missing", "key1"]);
    assert!(result.is_ok());
}

#[test]
fn read_ahead_continues_after_error_even_when_no_valid_keys() {
    let (cache, _dir) = create_test_cache();
    let result = cache.read_ahead(&["missing1", "missing2", "missing3"]);
    assert!(result.is_ok());
}
