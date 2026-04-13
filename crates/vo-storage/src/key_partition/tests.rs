use super::*;
use vo_types::{CryptoAlgorithm, InstanceId, KeyMetadata, WrappedDek};

fn sample_instance_id() -> InstanceId {
    InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap()
}

fn sample_dek_id() -> vo_types::DekId {
    vo_types::DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap()
}

fn sample_wrapped_dek() -> WrappedDek {
    WrappedDek::new(vec![0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE])
}

fn sample_metadata() -> KeyMetadata {
    KeyMetadata::new(sample_instance_id(), CryptoAlgorithm::Aes256Gcm)
}

fn sample_dek_entry() -> DekEntry {
    DekEntry::new(
        sample_dek_id(),
        sample_instance_id(),
        sample_wrapped_dek(),
        sample_metadata(),
    )
    .expect("valid entry")
}

#[test]
fn dek_entry_constructs_with_valid_inputs() {
    let entry = sample_dek_entry();
    assert_eq!(entry.dek_id().as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
    assert_eq!(entry.instance_id().as_str(), "01ARZ3NDEKTSV4RRFFQ69G5FAV");
    assert_eq!(entry.status(), DekStatus::Active);
}

#[test]
fn dek_entry_retire_changes_status() {
    let mut entry = sample_dek_entry();
    assert_eq!(entry.status(), DekStatus::Active);
    entry.retire();
    assert_eq!(entry.status(), DekStatus::Retired);
}

#[test]
fn fjall_dek_store_rotation_clears_stale_index() {
    use super::fjall_dek_store::FjallDekStore;
    let dir = tempfile::tempdir().unwrap();
    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let store = FjallDekStore::open(&keyspace).unwrap();
    let kek = [0x42u8; 32];
    let instance = sample_instance_id();

    // Generate initial DEK
    let old_id = store.generate_and_store_dek(&instance, &kek).unwrap();

    // Rotate should succeed (not fail with DekAlreadyExists)
    let new_id = store.rotate_dek(&instance, &kek).unwrap();
    assert_ne!(old_id, new_id);

    // New DEK should be retrievable
    let retrieved = store.retrieve_dek(&instance, &kek).unwrap();
    assert_eq!(retrieved.len(), 32);
}

#[test]
fn dek_store_error_display_dek_not_found() {
    let err = DekStoreError::DekNotFound { instance_id: "inst-123".to_string() };
    let msg = format!("{err}");
    assert!(msg.contains("DEK not found"));
    assert!(msg.contains("inst-123"));
}

#[test]
fn dek_store_error_display_dek_retired() {
    let err = DekStoreError::DekRetired { dek_id: "dek-456".to_string() };
    let msg = format!("{err}");
    assert!(msg.contains("retired"));
    assert!(msg.contains("crypto-shredded"));
}

#[test]
fn dek_store_error_display_dek_already_exists() {
    let err = DekStoreError::DekAlreadyExists { instance_id: "inst-789".to_string() };
    assert!(format!("{err}").contains("already exists"));
}

#[test]
fn dek_store_error_display_storage() {
    let err = DekStoreError::Storage { reason: "disk full".to_string() };
    assert!(format!("{err}").contains("disk full"));
}

#[test]
fn dek_store_error_display_codec() {
    let err = DekStoreError::Codec { reason: "bad json".to_string() };
    assert!(format!("{err}").contains("bad json"));
}

#[test]
fn dek_store_error_display_invalid_argument() {
    assert!(format!("{}", DekStoreError::InvalidArgument).contains("invalid"));
}

#[test]
fn dek_store_error_display_key_store_unavailable() {
    assert!(format!("{}", DekStoreError::KeyStoreUnavailable).contains("inaccessible"));
}

#[test]
fn dek_store_error_implements_error_trait() {
    let err: Box<dyn std::error::Error> = Box::new(DekStoreError::DekNotFound {
        instance_id: "x".to_string(),
    });
    let _ = format!("{err}");
}

#[test]
fn encode_decode_instance_key_roundtrip() {
    let id = sample_instance_id();
    let encoded = encode_instance_key(&id);
    let decoded = decode_instance_key(&encoded).expect("decode should succeed");
    assert_eq!(id, decoded);
}

#[test]
fn encode_instance_key_produces_utf8() {
    let id = sample_instance_id();
    let encoded = encode_instance_key(&id);
    assert!(std::str::from_utf8(&encoded).is_ok());
}

#[test]
fn decode_instance_key_rejects_invalid_utf8() {
    let result = decode_instance_key(&vec![0xFF, 0xFE]);
    assert!(matches!(result, Err(DekStoreError::Codec { .. })));
}

#[test]
fn decode_instance_key_rejects_invalid_ulid() {
    let result = decode_instance_key(b"not-a-valid-ulid");
    assert!(matches!(result, Err(DekStoreError::Codec { .. })));
}

#[test]
fn encode_decode_dek_entry_roundtrip() {
    let entry = sample_dek_entry();
    let encoded = encode_dek_entry(&entry);
    let decoded = decode_dek_entry(&encoded).expect("decode should succeed");
    assert_eq!(entry.dek_id(), decoded.dek_id());
    assert_eq!(entry.instance_id(), decoded.instance_id());
    assert_eq!(entry.status(), decoded.status());
}

#[test]
fn encode_decode_dek_entry_roundtrip_retired() {
    let mut entry = sample_dek_entry();
    entry.retire();
    let encoded = encode_dek_entry(&entry);
    let decoded = decode_dek_entry(&encoded).expect("decode should succeed");
    assert_eq!(decoded.status(), DekStatus::Retired);
}

#[test]
fn decode_dek_entry_rejects_invalid_json() {
    let result = decode_dek_entry(b"not valid json");
    assert!(matches!(result, Err(DekStoreError::Codec { .. })));
}

#[test]
fn decode_dek_entry_rejects_empty_structure() {
    let result = decode_dek_entry(b"{}");
    assert!(matches!(result, Err(DekStoreError::Codec { .. })));
}

#[test]
fn dek_entry_serde_roundtrips() {
    let entry = sample_dek_entry();
    let json = serde_json::to_string(&entry).expect("serialize");
    let recovered: DekEntry = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(entry.dek_id(), recovered.dek_id());
    assert_eq!(entry.status(), recovered.status());
}

#[test]
fn dek_entry_retired_serde_roundtrips() {
    let mut entry = sample_dek_entry();
    entry.retire();
    let json = serde_json::to_string(&entry).expect("serialize");
    let recovered: DekEntry = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(recovered.status(), DekStatus::Retired);
}

#[test]
fn dek_status_equality() {
    assert_eq!(DekStatus::Active, DekStatus::Active);
    assert_ne!(DekStatus::Active, DekStatus::Retired);
}

#[test]
fn dek_status_serde_roundtrips() {
    for status in [DekStatus::Active, DekStatus::Retired] {
        let json = serde_json::to_string(&status).expect("serialize");
        let recovered: DekStatus = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(status, recovered);
    }
}

#[test]
fn dek_entry_retire_is_idempotent() {
    let mut entry = sample_dek_entry();
    entry.retire();
    entry.retire();
    assert_eq!(entry.status(), DekStatus::Retired);
}

#[test]
fn dek_entry_accessors_return_correct_values() {
    let entry = sample_dek_entry();
    assert_eq!(entry.dek_id().as_str(), sample_dek_id().as_str());
    assert_eq!(entry.instance_id().as_str(), sample_instance_id().as_str());
    assert_eq!(entry.wrapped_dek().as_bytes(), &[0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE]);
    assert_eq!(entry.metadata().algorithm, CryptoAlgorithm::Aes256Gcm);
}

#[test]
fn dek_partition_constant() {
    assert_eq!(DEK_PARTITION, "dek_store");
}

#[test]
fn invariant_dek_entry_only_contains_wrapped_dek() {
    let entry = sample_dek_entry();
    let json = serde_json::to_value(&entry).expect("serialize");
    assert!(json.get("wrapped_dek").is_some());
    assert!(json.get("raw_key").is_none());
    assert!(json.get("dek").is_none());
    assert!(json.get("key_bytes").is_none());
}

#[test]
fn invariant_retired_dek_cannot_be_used() {
    let mut entry = sample_dek_entry();
    entry.retire();
    assert_eq!(entry.status(), DekStatus::Retired);
}
