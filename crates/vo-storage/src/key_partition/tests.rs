//! Unit tests for key_partition module (DekStore trait, encoding, DekEntry).

use super::*;
use vo_types::{
    CryptoAlgorithm, DekId, InstanceId, KeyMetadata, WrappedDek,
};

fn sample_instance_id() -> InstanceId {
    InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap()
}

fn sample_dek_id() -> DekId {
    DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap()
}

fn sample_kek() -> [u8; 32] {
    [0x42u8; 32]
}

fn sample_wrapped_dek() -> WrappedDek {
    WrappedDek::new({
        let mut v = vec![0xDE, 0xAD, 0xBE, 0xEF];
        v.extend_from_slice(&[0x00u8; 56]);
        v
    })
}

fn sample_metadata() -> KeyMetadata {
    KeyMetadata::new(sample_instance_id(), CryptoAlgorithm::Aes256Gcm)
}

// ---------------------------------------------------------------------------
// DekEntry tests
// ---------------------------------------------------------------------------

#[test]
fn dek_entry_new_succeeds() {
    let entry = DekEntry::new(
        sample_dek_id(),
        sample_instance_id(),
        sample_wrapped_dek(),
        sample_metadata(),
    );
    assert!(entry.is_ok());
}

#[test]
fn dek_entry_accessors() {
    let entry = DekEntry::new(
        sample_dek_id(),
        sample_instance_id(),
        sample_wrapped_dek(),
        sample_metadata(),
    )
    .unwrap();

    assert_eq!(entry.dek_id(), &sample_dek_id());
    assert_eq!(entry.instance_id(), &sample_instance_id());
    assert_eq!(entry.status(), DekStatus::Active);
}

#[test]
fn dek_entry_retire_changes_status() {
    let mut entry = DekEntry::new(
        sample_dek_id(),
        sample_instance_id(),
        sample_wrapped_dek(),
        sample_metadata(),
    )
    .unwrap();

    assert_eq!(entry.status(), DekStatus::Active);
    entry.retire();
    assert_eq!(entry.status(), DekStatus::Retired);
}

#[test]
fn dek_entry_serialization_roundtrip() {
    let entry = DekEntry::new(
        sample_dek_id(),
        sample_instance_id(),
        sample_wrapped_dek(),
        sample_metadata(),
    )
    .unwrap();

    let json = serde_json::to_string(&entry).unwrap();
    let decoded: DekEntry = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded.dek_id(), entry.dek_id());
    assert_eq!(decoded.instance_id(), entry.instance_id());
    assert_eq!(decoded.status(), entry.status());
}

#[test]
fn dek_entry_retired_serialization_roundtrip() {
    let mut entry = DekEntry::new(
        sample_dek_id(),
        sample_instance_id(),
        sample_wrapped_dek(),
        sample_metadata(),
    )
    .unwrap();
    entry.retire();

    let json = serde_json::to_string(&entry).unwrap();
    let decoded: DekEntry = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded.status(), DekStatus::Retired);
}

// ---------------------------------------------------------------------------
// DekStatus tests
// ---------------------------------------------------------------------------

#[test]
fn dek_status_debug() {
    assert_eq!(format!("{:?}", DekStatus::Active), "Active");
    assert_eq!(format!("{:?}", DekStatus::Retired), "Retired");
}

#[test]
fn dek_status_equality() {
    assert_eq!(DekStatus::Active, DekStatus::Active);
    assert_ne!(DekStatus::Active, DekStatus::Retired);
}

#[test]
fn dek_status_clone_copy() {
    let status = DekStatus::Retired;
    let copied = status;
    assert_eq!(status, copied);
}

// ---------------------------------------------------------------------------
// DekStoreError tests
// ---------------------------------------------------------------------------

#[test]
fn dek_store_error_display() {
    let errors = vec![
        DekStoreError::DekNotFound {
            instance_id: "inst-123".to_string(),
        },
        DekStoreError::DekRetired {
            dek_id: "dek-456".to_string(),
        },
        DekStoreError::DekAlreadyExists {
            instance_id: "inst-789".to_string(),
        },
        DekStoreError::Storage {
            reason: "disk full".to_string(),
        },
        DekStoreError::Codec {
            reason: "bad json".to_string(),
        },
        DekStoreError::InvalidArgument,
        DekStoreError::KeyStoreUnavailable,
    ];

    for err in &errors {
        let msg = format!("{err}");
        assert!(!msg.is_empty());
    }
}

#[test]
fn dek_store_error_debug() {
    let err = DekStoreError::DekNotFound {
        instance_id: "test".to_string(),
    };
    let debug = format!("{err:?}");
    assert!(debug.contains("DekNotFound"));
}

#[test]
fn dek_store_error_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<DekStoreError>();
}

#[test]
fn dek_store_error_implements_std_error() {
    let err = DekStoreError::Storage {
        reason: "test".to_string(),
    };
    let _: &dyn std::error::Error = &err;
}

#[test]
fn dek_store_error_equality() {
    let e1 = DekStoreError::InvalidArgument;
    let e2 = DekStoreError::InvalidArgument;
    assert_eq!(e1, e2);

    let e3 = DekStoreError::KeyStoreUnavailable;
    assert_ne!(e1, e3);
}

// ---------------------------------------------------------------------------
// Key encoding tests
// ---------------------------------------------------------------------------

#[test]
fn encode_decode_instance_key_roundtrip() {
    let instance_id = sample_instance_id();
    let encoded = encode_instance_key(&instance_id);
    let decoded = decode_instance_key(&encoded).unwrap();
    assert_eq!(decoded, instance_id);
}

#[test]
fn encode_instance_key_produces_valid_utf8() {
    let encoded = encode_instance_key(&sample_instance_id());
    assert!(std::str::from_utf8(&encoded).is_ok());
}

#[test]
fn decode_instance_key_rejects_invalid_utf8() {
    let bad_bytes = vec![0xFF, 0xFE, 0xFD];
    let result = decode_instance_key(&bad_bytes);
    assert!(matches!(result, Err(DekStoreError::Codec { .. })));
}

#[test]
fn decode_instance_key_rejects_invalid_instance_id() {
    let bad_str = b"not-a-valid-ulid";
    let result = decode_instance_key(bad_str);
    assert!(matches!(result, Err(DekStoreError::Codec { .. })));
}

#[test]
fn encode_decode_dek_entry_roundtrip() {
    let entry = DekEntry::new(
        sample_dek_id(),
        sample_instance_id(),
        sample_wrapped_dek(),
        sample_metadata(),
    )
    .unwrap();

    let encoded = encode_dek_entry(&entry);
    let decoded = decode_dek_entry(&encoded).unwrap();

    assert_eq!(decoded.dek_id(), entry.dek_id());
    assert_eq!(decoded.instance_id(), entry.instance_id());
    assert_eq!(decoded.status(), entry.status());
}

#[test]
fn decode_dek_entry_rejects_invalid_json() {
    let bad_json = b"not json at all";
    let result = decode_dek_entry(bad_json);
    assert!(matches!(result, Err(DekStoreError::Codec { .. })));
}

#[test]
fn decode_dek_entry_rejects_wrong_structure() {
    let bad_json = b"{}";
    let result = decode_dek_entry(bad_json);
    assert!(matches!(result, Err(DekStoreError::Codec { .. })));
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

#[test]
fn dek_partition_name() {
    assert_eq!(DEK_PARTITION, "dek_store");
}
