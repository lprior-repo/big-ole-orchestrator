//! Canonical Encryption BDD Tests (ADR-025 §2)
//!
//! Tests for encrypted canonical state per ADR-025:
//! - EncryptedBlob structure validation
//! - DEK/KEK lifecycle correctness
//! - Encrypted payload decryption semantics
//!
//! Required proof command: cargo test -p vo-types moon_gate_canonical_encryption

#![allow(clippy::unwrap_used)]

use vo_types::{
    CryptoAlgorithm, DekId, EncryptedBlob, KeyMetadata,
    InstanceId,
};

fn instance_id() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID")
}

// ========================================================================
// DIMENSION: EncryptedBlob Structure (ADR-025 §2)
// ADR-025: "Canonical payload blobs are encrypted with a per-instance DEK"
// ========================================================================

#[test]
fn given_encrypted_blob_when_valid_structure_then_creation_succeeds() {
    // GIVEN: Valid AES-256-GCM encrypted blob components
    let iv = vec![0u8; CryptoAlgorithm::IV_SIZE_BYTES];
    let ciphertext = vec![1u8; 32]; // AES-256 key size
    let tag = vec![2u8; CryptoAlgorithm::TAG_SIZE_BYTES];

    // WHEN: Creating EncryptedBlob with valid components
    let blob = EncryptedBlob::new(iv.clone(), ciphertext.clone(), tag.clone());

    // THEN: Creation succeeds
    assert!(blob.is_ok(), "Valid encrypted blob must be created");
    let blob = blob.unwrap();

    // THEN: Components are preserved
    assert_eq!(blob.iv.len(), CryptoAlgorithm::IV_SIZE_BYTES);
    assert_eq!(blob.ciphertext.len(), 32);
    assert_eq!(blob.tag.len(), CryptoAlgorithm::TAG_SIZE_BYTES);

    // THEN: Total size is correct
    assert_eq!(
        blob.total_size(),
        CryptoAlgorithm::IV_SIZE_BYTES + 32 + CryptoAlgorithm::TAG_SIZE_BYTES
    );
}

#[test]
fn given_encrypted_blob_with_invalid_iv_length_then_rejected() {
    // GIVEN: Invalid IV length (not 12 bytes for AES-256-GCM)
    let invalid_iv = vec![0u8; 8]; // Wrong size
    let ciphertext = vec![1u8; 32];
    let tag = vec![2u8; CryptoAlgorithm::TAG_SIZE_BYTES];

    // WHEN: Creating EncryptedBlob with invalid IV
    let result = EncryptedBlob::new(invalid_iv, ciphertext, tag);

    // THEN: Creation fails
    assert!(result.is_err(), "Invalid IV length must be rejected");
}

#[test]
fn given_encrypted_blob_with_invalid_tag_length_then_rejected() {
    // GIVEN: Invalid tag length
    let iv = vec![0u8; CryptoAlgorithm::IV_SIZE_BYTES];
    let ciphertext = vec![1u8; 32];
    let invalid_tag = vec![2u8; 8]; // Wrong size

    // WHEN: Creating EncryptedBlob with invalid tag
    let result = EncryptedBlob::new(iv, ciphertext, invalid_tag);

    // THEN: Creation fails
    assert!(result.is_err(), "Invalid tag length must be rejected");
}

// ========================================================================
// DIMENSION: DEK ID Validation (ADR-025 §2)
// ========================================================================

#[test]
fn given_dek_id_with_valid_ulid_then_parsed() {
    // GIVEN: Valid ULID-formatted DEK ID
    let valid_ulid = "01H5JYV4XHGSR2F8KZ9BWNRFMA";

    // WHEN: Parsing DEK ID
    let dek_id = DekId::parse(valid_ulid);

    // THEN: Parsing succeeds
    assert!(dek_id.is_ok(), "Valid ULID must parse as DEK ID");
    assert_eq!(dek_id.unwrap().as_str(), valid_ulid);
}

#[test]
fn given_dek_id_with_invalid_ulid_then_rejected() {
    // GIVEN: Invalid ULID
    let invalid_ulid = "not-a-ulid";

    // WHEN: Parsing DEK ID
    let result = DekId::parse(invalid_ulid);

    // THEN: Parsing fails
    assert!(result.is_err(), "Invalid ULID must be rejected");
}

#[test]
fn given_dek_id_with_nil_ulid_then_rejected() {
    // GIVEN: Nil ULID (all zeros)
    let nil_ulid = "00000000000000000000000000";

    // WHEN: Parsing DEK ID
    let result = DekId::parse(nil_ulid);

    // THEN: Parsing fails (nil is not permitted)
    assert!(result.is_err(), "Nil ULID must be rejected for DEK ID");
}

#[test]
fn given_dek_id_roundtrip_bytes() {
    // GIVEN: Valid DEK ID
    let original = DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

    // WHEN: Converting to bytes and back
    let bytes = original.to_bytes().expect("valid bytes");
    let recovered = DekId::from_bytes(bytes);

    // THEN: DEK ID is preserved
    assert_eq!(original.as_str(), recovered.as_str());
}

// ========================================================================
// DIMENSION: Key Metadata (ADR-025 §2)
// ========================================================================

#[test]
fn given_key_metadata_when_created_then_contains_required_fields() {
    // GIVEN: Instance ID and algorithm
    let instance = instance_id();
    let algorithm = CryptoAlgorithm::Aes256Gcm;

    // WHEN: Creating KeyMetadata
    let metadata = KeyMetadata::new(instance.clone(), algorithm);

    // THEN: All fields are populated
    assert_eq!(metadata.instance_id, instance);
    assert_eq!(metadata.algorithm, CryptoAlgorithm::Aes256Gcm);
    assert!(metadata.created_at_ms > 0, "Created timestamp must be set");
}

#[test]
fn given_key_metadata_when_serialized_then_deserialized_preserves_fields() {
    // GIVEN: KeyMetadata
    let instance = instance_id();
    let metadata = KeyMetadata::new(instance, CryptoAlgorithm::Aes256Gcm);

    // WHEN: Serializing to JSON and back
    let json = serde_json::to_string(&metadata).expect("serialize");
    let recovered: KeyMetadata = serde_json::from_str(&json).expect("deserialize");

    // THEN: All fields are preserved
    assert_eq!(metadata.instance_id, recovered.instance_id);
    assert_eq!(metadata.algorithm, recovered.algorithm);
    assert_eq!(metadata.created_at_ms, recovered.created_at_ms);
}

// ========================================================================
// DIMENSION: Crypto Algorithm Constants (ADR-025 §2)
// ========================================================================

#[test]
fn given_crypto_algorithm_when_checking_constants_then_values_correct() {
    // THEN: AES-256-GCM constants are correct
    assert_eq!(CryptoAlgorithm::IV_SIZE_BYTES, 12, "AES-256-GCM IV is 12 bytes");
    assert_eq!(CryptoAlgorithm::TAG_SIZE_BYTES, 16, "AES-256-GCM tag is 16 bytes");
    assert_eq!(CryptoAlgorithm::KEY_SIZE_BYTES, 32, "AES-256-GCM key is 32 bytes");
}

#[test]
fn given_wrapped_dek_with_insufficient_length_then_rejected() {
    // GIVEN: Wrapped DEK that is too short
    // Per ADR-025 §2: minimum is IV(12) + DEK(32) + tag(16) = 60 bytes
    let short_wrapped = vec![0u8; 59];

    // WHEN: Creating WrappedDek
    let result = vo_types::WrappedDek::new(short_wrapped);

    // THEN: Creation fails
    assert!(result.is_err(), "Wrapped DEK < 60 bytes must be rejected");
}

#[test]
fn given_wrapped_dek_with_valid_length_then_accepted() {
    // GIVEN: Valid Wrapped DEK (60 bytes minimum)
    let valid_wrapped = vec![0u8; 60];

    // WHEN: Creating WrappedDek
    let result = vo_types::WrappedDek::new(valid_wrapped);

    // THEN: Creation succeeds
    assert!(result.is_ok(), "Valid Wrapped DEK must be accepted");
}

// ========================================================================
// DIMENSION: EncryptedBlob Display Format
// ========================================================================

#[test]
fn given_encrypted_blob_when_displayed_then_shows_component_sizes() {
    // GIVEN: EncryptedBlob
    let blob = EncryptedBlob::new(
        vec![0u8; 12],
        vec![1u8; 32],
        vec![2u8; 16],
    ).unwrap();

    // WHEN: Converting to string
    let display = format!("{}", blob);

    // THEN: Display contains size information
    assert!(display.contains("12"), "Display should show IV size");
    assert!(display.contains("32"), "Display should show ciphertext size");
    assert!(display.contains("16"), "Display should show tag size");
}
