//! Section 1: Fjall Persistence — put/get/delete on raw keyspaces

use crate::{make_envelope, open_partition, encode_event_seq};

#[test]
fn fjall_put_then_get_returns_stored_value() {
    let (_dir, _db, ks) = open_partition("test_put_get");

    ks.insert(b"key-1", b"value-1").expect("insert");
    let val = ks.get(b"key-1").expect("get").expect("value exists");
    assert_eq!(val.as_ref(), b"value-1");
}

#[test]
fn fjall_get_missing_key_returns_none() {
    let (_dir, _db, ks) = open_partition("test_missing");

    let val = ks.get(b"no-such-key").expect("get");
    assert!(val.is_none());
}

#[test]
fn fjall_delete_removes_key() {
    let (_dir, _db, ks) = open_partition("test_delete");

    ks.insert(b"key-del", b"val-del").expect("insert");
    assert!(ks.get(b"key-del").expect("get").is_some());

    ks.remove(b"key-del").expect("remove");
    assert!(ks.get(b"key-del").expect("get").is_none());
}

#[test]
fn fjall_overwrite_replaces_value() {
    let (_dir, _db, ks) = open_partition("test_overwrite");

    ks.insert(b"k", b"v1").expect("insert 1");
    ks.insert(b"k", b"v2").expect("insert 2");

    let val = ks.get(b"k").expect("get").expect("value");
    assert_eq!(val.as_ref(), b"v2");
}

#[test]
fn fjall_persists_across_multiple_inserts() {
    let (_dir, _db, ks) = open_partition("test_multi");

    for i in 0..100u32 {
        let key = format!("key-{i}");
        let val = format!("val-{i}");
        ks.insert(key.as_bytes(), val.as_bytes()).expect("insert");
    }

    for i in 0..100u32 {
        let key = format!("key-{i}");
        let val = format!("val-{i}");
        let stored = ks.get(key.as_bytes()).expect("get").expect("value");
        assert_eq!(stored.as_ref(), val.as_bytes(), "mismatch at {i}");
    }
}

#[test]
fn fjall_binary_key_values_roundtrip() {
    let (_dir, _db, ks) = open_partition("test_binary");

    let key: [u8; 24] = [0xFF; 24];
    let value: [u8; 128] = {
        let mut v = [0u8; 128];
        v.iter_mut()
            .enumerate()
            .for_each(|(i, b)| *b = (i as u8).wrapping_mul(7));
        v
    };

    ks.insert(key, value).expect("insert");
    let retrieved = ks.get(&key).expect("get").expect("value");
    assert_eq!(retrieved.as_ref(), &value);
}
