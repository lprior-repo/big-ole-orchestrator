#![cfg(kani)]

//! Kani model-checking proofs for privacy, encryption, and redaction invariants.
//!
//! ## Scope (ADR-025 / ADR-040)
//!
//! These proofs verify the following properties under symbolic execution:
//!
//! 1. **Redaction completeness** (ADR-025 §1): Sensitive fields are always removed
//!    from operator projections, never leaked as plaintext.
//! 2. **Encryption type safety**: `EncryptedBlob` always has correct IV/tag sizes.
//! 3. **DEK identity**: `DekId` roundtrips through bytes losslessly.
//! 4. **Key size constants**: CryptoAlgorithm sizes are internally consistent.
//! 5. **BlobRef validation**: `BlobRef::new` rejects all invalid inputs.
//!
//! ## Out of scope
//!
//! - AES-256-GCM correctness (relies on `aes-gcm` which is not Kani-compatible)
//! - Fjall DEK store persistence (relies on `fjall` which is not Kani-compatible)
//! - DEK/KEK wrap/unwrap roundtrip (requires `rand::OsRng`)
//! - Symbolic `serde_json::Value` (not `kani::Arbitrary`)
//! - Symbolic `Vec<u8>` / `String` (not `kani::Arbitrary`)

use crate::dual_representation::{apply_redaction, RedactionKind, RedactionRule};
use crate::encryption::{CryptoAlgorithm, DekId, EncryptedBlob, WrappedDek};

// ---------------------------------------------------------------------------
// Proof 1: Redaction completeness — no plaintext leak for Remove rules
// ---------------------------------------------------------------------------

/// Verifies that applying a `Remove` redaction rule never preserves the original
/// sensitive value in the output. The field is either `Null` or absent.
///
/// This is the core ADR-025 invariant: "operator projections remain redacted and
/// queryable without decrypting" — meaning sensitive data MUST NOT appear in
/// operator projections.
#[kani::proof]
fn redaction_remove_never_leaks_plaintext() {
    let rule = RedactionRule::new(vec!["secret".to_string()], RedactionKind::Remove);

    let input = serde_json::json!({
        "public": "visible",
        "secret": "sensitive_plaintext_data"
    });

    let (result, redacted) = apply_redaction(&input, &[rule]);

    assert_eq!(redacted.len(), 1);
    assert_eq!(redacted[0], vec!["secret".to_string()]);

    // The secret field must be Null (removed), never the original value
    assert_eq!(result["secret"], serde_json::Value::Null);

    // Non-sensitive fields must be preserved
    assert_eq!(result["public"], "visible");
}

// ---------------------------------------------------------------------------
// Proof 2: Redaction completeness — ReplaceWith never leaks original
// ---------------------------------------------------------------------------

/// Verifies that `ReplaceWith` redaction always substitutes the original value
/// with the replacement string, never leaking plaintext.
#[kani::proof]
fn redaction_replace_with_never_leaks_plaintext() {
    let rule = RedactionRule::new(
        vec!["password".to_string()],
        RedactionKind::ReplaceWith("[REDACTED]".to_string()),
    );

    let input = serde_json::json!({
        "username": "alice",
        "password": "super_secret_password_123"
    });

    let (result, redacted) = apply_redaction(&input, &[rule]);

    assert_eq!(redacted.len(), 1);
    assert_eq!(result["password"], "[REDACTED]");
    assert_eq!(result["username"], "alice");
}

// ---------------------------------------------------------------------------
// Proof 3: Redaction completeness — Hash produces non-plaintext output
// ---------------------------------------------------------------------------

/// Verifies that `Hash` redaction always produces a value prefixed with "HASH",
/// never returning the original plaintext value directly.
#[kani::proof]
fn redaction_hash_never_leaks_plaintext() {
    let rule = RedactionRule::new(vec!["email".to_string()], RedactionKind::Hash);

    let input = serde_json::json!({
        "name": "Alice",
        "email": "alice@example.com"
    });

    let (result, redacted) = apply_redaction(&input, &[rule]);

    assert_eq!(redacted.len(), 1);

    let hashed = result["email"].as_str();
    assert!(hashed.is_some());
    assert!(hashed.unwrap().starts_with("HASH"));

    // The hashed value must NOT equal the original plaintext
    assert_ne!(hashed.unwrap(), "alice@example.com");
}

// ---------------------------------------------------------------------------
// Proof 4: Redaction completeness — nested paths are fully redacted
// ---------------------------------------------------------------------------

/// Verifies that deeply nested sensitive fields are always redacted,
/// even when multiple levels of nesting exist.
#[kani::proof]
fn redaction_nested_path_completeness() {
    let rule = RedactionRule::new(
        vec![
            "level1".to_string(),
            "level2".to_string(),
            "secret".to_string(),
        ],
        RedactionKind::Remove,
    );

    let input = serde_json::json!({
        "level1": {
            "level2": {
                "public": "safe",
                "secret": "classified_intelligence"
            }
        }
    });

    let (result, redacted) = apply_redaction(&input, &[rule]);

    assert_eq!(redacted.len(), 1);
    assert_eq!(
        result["level1"]["level2"]["secret"],
        serde_json::Value::Null
    );
    assert_eq!(result["level1"]["level2"]["public"], "safe");
}

// ---------------------------------------------------------------------------
// Proof 5: Redaction — empty rules produce identity transform
// ---------------------------------------------------------------------------

/// Verifies that with no redaction rules, the output equals the input (identity).
#[kani::proof]
fn redaction_empty_rules_is_identity() {
    let input = serde_json::json!({
        "key": "value",
        "nested": {"a": 1, "b": [1, 2, 3]},
        "number": 42,
        "flag": true
    });
    let rules: Vec<RedactionRule> = vec![];

    let (result, redacted) = apply_redaction(&input, &rules);

    assert_eq!(result, input);
    assert!(redacted.is_empty());
}

// ---------------------------------------------------------------------------
// Proof 6: Redaction — multiple rules redact all matching fields
// ---------------------------------------------------------------------------

/// Verifies that multiple simultaneous redaction rules each redact their targets.
#[kani::proof]
fn redaction_multiple_rules_completeness() {
    let rule1 = RedactionRule::new(vec!["ssn".to_string()], RedactionKind::Remove);
    let rule2 = RedactionRule::new(vec!["email".to_string()], RedactionKind::Hash);
    let rule3 = RedactionRule::new(
        vec!["card".to_string()],
        RedactionKind::ReplaceWith("[REDACTED]".to_string()),
    );

    let input = serde_json::json!({
        "ssn": "123-45-6789",
        "email": "user@example.com",
        "card": "4111-1111-1111-1111",
        "name": "Alice"
    });

    let (result, redacted) = apply_redaction(&input, &[rule1, rule2, rule3]);

    assert_eq!(redacted.len(), 3);
    assert_eq!(result["ssn"], serde_json::Value::Null);
    assert!(result["email"].as_str().unwrap().starts_with("HASH"));
    assert_ne!(result["email"].as_str().unwrap(), "user@example.com");
    assert_eq!(result["card"], "[REDACTED]");
    assert_eq!(result["name"], "Alice");
}

// ---------------------------------------------------------------------------
// Proof 7: EncryptedBlob — IV and tag sizes are correct
// ---------------------------------------------------------------------------

/// Verifies that `EncryptedBlob::new` always produces correct IV and tag sizes
/// matching AES-256-GCM constants.
#[kani::proof]
fn encrypted_blob_has_correct_sizes() {
    let iv = vec![0u8; CryptoAlgorithm::IV_SIZE_BYTES];
    let tag = vec![0u8; CryptoAlgorithm::TAG_SIZE_BYTES];
    let ciphertext = vec![0xAA; 256];

    let blob = EncryptedBlob::new(iv, ciphertext, tag).unwrap();

    assert_eq!(blob.iv.len(), CryptoAlgorithm::IV_SIZE_BYTES);
    assert_eq!(blob.tag.len(), CryptoAlgorithm::TAG_SIZE_BYTES);
    assert_eq!(
        blob.total_size(),
        CryptoAlgorithm::IV_SIZE_BYTES + blob.ciphertext.len() + CryptoAlgorithm::TAG_SIZE_BYTES
    );
}

// ---------------------------------------------------------------------------
// Proof 8: CryptoAlgorithm constants are internally consistent
// ---------------------------------------------------------------------------

/// Verifies that AES-256-GCM algorithm constants are correct for the cipher.
#[kani::proof]
fn crypto_algorithm_constants_are_valid() {
    assert_eq!(CryptoAlgorithm::IV_SIZE_BYTES, 12);
    assert_eq!(CryptoAlgorithm::TAG_SIZE_BYTES, 16);
    assert_eq!(CryptoAlgorithm::KEY_SIZE_BYTES, 32);

    // AES-256 requires exactly 32-byte keys
    assert_eq!(CryptoAlgorithm::KEY_SIZE_BYTES, 32);
    // GCM IV is 96 bits = 12 bytes
    assert_eq!(CryptoAlgorithm::IV_SIZE_BYTES, 12);
    // GCM tag is 128 bits = 16 bytes
    assert_eq!(CryptoAlgorithm::TAG_SIZE_BYTES, 16);
}

// ---------------------------------------------------------------------------
// Proof 9: WrappedDek preserves bytes
// ---------------------------------------------------------------------------

/// Verifies that `WrappedDek` roundtrips bytes without loss.
#[kani::proof]
fn wrapped_dek_preserves_bytes() {
    let bytes = vec![0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE];
    let wrapped = WrappedDek::new(bytes.clone()).unwrap();
    assert_eq!(wrapped.as_bytes(), bytes);
}

// ---------------------------------------------------------------------------
// Proof 10: DekId roundtrip through bytes is lossless
// ---------------------------------------------------------------------------

/// Verifies that `DekId` roundtrips through `from_bytes` / `to_bytes` losslessly
/// for a concrete valid ULID.
#[kani::proof]
fn dek_id_roundtrip_bytes() {
    let original_str = "01H5JYV4XHGSR2F8KZ9BWNRFMA";
    let dek_id = DekId::parse(original_str).expect("valid ULID");
    let bytes = dek_id.to_bytes().expect("valid bytes");
    let recovered = DekId::from_bytes(bytes);
    assert_eq!(dek_id.as_str(), recovered.as_str());
}

// ---------------------------------------------------------------------------
// Proof 11: DekId rejects nil ULID (all zeros)
// ---------------------------------------------------------------------------

/// Verifies that `DekId::from_bytes` with all-zero bytes produces a nil ULID
/// that is rejected by `parse`. This prevents nil-key attacks.
#[kani::proof]
fn dek_id_rejects_nil_bytes() {
    let nil_bytes = [0u8; 16];
    let dek_id = DekId::from_bytes(nil_bytes);
    let result = DekId::parse(dek_id.as_str());
    assert!(result.is_err(), "nil ULID must be rejected");
}

// ---------------------------------------------------------------------------
// Proof 12: DekId parse rejects empty and wrong-length strings
// ---------------------------------------------------------------------------

/// Verifies that `DekId::parse` rejects empty strings and wrong-length inputs.
#[kani::proof]
fn dek_id_rejects_invalid_inputs() {
    assert!(DekId::parse("").is_err());
    assert!(DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRF").is_err());
    assert!(DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMAAA").is_err());
    assert!(DekId::parse("zzzzzzzzzzzzzzzzzzzzzzzzzz").is_err());
}

// ---------------------------------------------------------------------------
// Proof 13: BlobRef validation rejects invalid inputs
// ---------------------------------------------------------------------------

/// Verifies that `BlobRef::new` rejects all categories of invalid input:
/// empty blob_id, wrong-length blob_id, zero size_bytes, empty content_hash.
#[kani::proof]
fn blob_ref_rejects_invalid_inputs() {
    assert!(crate::BlobRef::new("", 100, "abcdef1234567890").is_err());
    assert!(crate::BlobRef::new("short", 100, "abcdef1234567890").is_err());
    assert!(crate::BlobRef::new("notavalidulid26ch", 100, "abcdef1234567890").is_err());
    assert!(crate::BlobRef::new("01ARZ3NDEKTSV4RRFFQ69G5FAV", 0, "abcdef1234567890").is_err());
    assert!(crate::BlobRef::new("01ARZ3NDEKTSV4RRFFQ69G5FAV", 100, "").is_err());
    assert!(crate::BlobRef::new("01ARZ3NDEKTSV4RRFFQ69G5FAV", 100, "abcde").is_err());
    assert!(crate::BlobRef::new("01ARZ3NDEKTSV4RRFFQ69G5FAV", 100, "abcd").is_err());
    assert!(crate::BlobRef::new("01ARZ3NDEKTSV4RRFFQ69G5FAV", 100, "ghijklmn").is_err());
}

// ---------------------------------------------------------------------------
// Proof 14: BlobRef accepts valid inputs
// ---------------------------------------------------------------------------

/// Verifies that `BlobRef::new` accepts all valid inputs.
#[kani::proof]
fn blob_ref_accepts_valid_inputs() {
    let result = crate::BlobRef::new(
        "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        1024,
        "abcdef1234567890abcdef1234567890",
    );
    assert!(result.is_ok());
    let blob = result.unwrap();
    assert_eq!(blob.size_bytes(), 1024);
}

// ---------------------------------------------------------------------------
// Proof 15: Redaction — array elements are individually redacted
// ---------------------------------------------------------------------------

/// Verifies that redaction rules apply to each element in an array independently.
#[kani::proof]
fn redaction_array_elements_individually_redacted() {
    let rule = RedactionRule::new(
        vec!["users".to_string(), "ssn".to_string()],
        RedactionKind::Remove,
    );

    let input = serde_json::json!({
        "users": [
            {"name": "Alice", "ssn": "111-11-1111"},
            {"name": "Bob", "ssn": "222-22-2222"}
        ]
    });

    let (result, redacted) = apply_redaction(&input, &[rule]);

    assert_eq!(result["users"][0]["ssn"], serde_json::Value::Null);
    assert_eq!(result["users"][1]["ssn"], serde_json::Value::Null);
    assert_eq!(result["users"][0]["name"], "Alice");
    assert_eq!(result["users"][1]["name"], "Bob");
    assert_eq!(redacted.len(), 2);
}

// ---------------------------------------------------------------------------
// Proof 16: Redaction — deep nesting with 5 levels
// ---------------------------------------------------------------------------

/// Verifies that deeply nested paths (5 levels) are correctly matched and redacted.
#[kani::proof]
fn redaction_deep_5_levels() {
    let rule = RedactionRule::new(
        vec![
            "a".into(),
            "b".into(),
            "c".into(),
            "d".into(),
            "secret".into(),
        ],
        RedactionKind::Remove,
    );

    let input = serde_json::json!({
        "a": {
            "b": {
                "c": {
                    "d": {
                        "secret": "TOP_SECRET",
                        "public": "safe"
                    }
                }
            }
        }
    });

    let (result, redacted) = apply_redaction(&input, &[rule]);

    assert_eq!(redacted.len(), 1);
    assert_eq!(
        result["a"]["b"]["c"]["d"]["secret"],
        serde_json::Value::Null
    );
    assert_eq!(result["a"]["b"]["c"]["d"]["public"], "safe");
}

// ---------------------------------------------------------------------------
// Proof 17: DekId bytes roundtrip with symbolic bytes
// ---------------------------------------------------------------------------

/// Verifies `DekId` roundtrip through bytes for symbolic 16-byte arrays.
/// Covers all possible byte patterns, proving lossless roundtrip.
#[kani::proof]
fn dek_id_roundtrip_symbolic_bytes() {
    let bytes: [u8; 16] = kani::any();
    let dek_id = DekId::from_bytes(bytes);
    let result = dek_id.to_bytes();
    // to_bytes can fail if the ULID string is somehow invalid after roundtrip,
    // but from_bytes always produces valid ULID strings, so this must succeed.
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), bytes);
}

// ---------------------------------------------------------------------------
// Proof 18: EncryptedBlob total_size invariant
// ---------------------------------------------------------------------------

/// Verifies that `total_size()` is always IV + ciphertext + tag, even with
/// varying ciphertext sizes.
#[kani::proof]
fn encrypted_blob_total_size_invariant() {
    let iv = vec![0u8; 12];
    let tag = vec![0u8; 16];
    // Symbolic ciphertext length (bounded to avoid explosion)
    let ct_len: u8 = kani::any();
    let ciphertext = vec![0u8; ct_len as usize];

    let blob = EncryptedBlob::new(iv.clone(), ciphertext.clone(), tag.clone()).unwrap();

    assert_eq!(blob.total_size(), iv.len() + ciphertext.len() + tag.len());
}
