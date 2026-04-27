//! TDD-RED: Privacy/Encryption/Blob Publication Tests (ADR-025/040)
//!
//! BEAD: ve-flb8q | PARENT: ve-6e68
//!
//! CONTRACT: These tests define the required behavior for:
//! - Dual representation correctness (ADR-025 §1-2)
//! - Redaction completeness (ADR-025 §1)
//! - Blob publication ordering (ADR-040 §2)
//! - DEK/KEK lifecycle validation (ADR-025 §2-3, ADR-040)
//!
//! ALL TESTS MUST FAIL — implementation does not yet satisfy contracts.
//! Next bead (ve-xlfki, IMPL phase) will make these pass.

#![allow(clippy::unwrap_used)]

use vo_types::{apply_redaction, RedactionKind, RedactionRule};

// ========================================================================
// GROUP 1: Remove Semantics (ADR-025 §1)
//
// ADR-025 §1: "Field is omitted entirely from the operator projection.
// The field key and value are not present in the result."
//
// BUG: Current implementation sets value to Null but keeps the key.
// Fix: When is_remove is true, skip the insert entirely.
// ALL TESTS IN THIS GROUP FAIL until the bug is fixed.
// ========================================================================

#[test]
fn tdd_red_001_remove_omits_key_from_flat_object() {
    // ADR-025: Remove means key AND value absent from result
    let canonical = serde_json::json!({
        "user": {
            "name": "Alice",
            "ssn": "123-45-6789"
        }
    });

    let rules = vec![RedactionRule::new(
        vec!["user".into(), "ssn".into()],
        RedactionKind::Remove,
    )];

    let (projection, _) = apply_redaction(&canonical, &rules);

    // Per ADR-025: "ssn" key should be ABSENT, not present with null
    assert!(
        projection["user"].get("ssn").is_none(),
        "Remove should omit key entirely, but found: {:?}",
        projection["user"].get("ssn")
    );
}

#[test]
fn tdd_red_002_remove_omits_key_from_deeply_nested_object() {
    let canonical = serde_json::json!({
        "level1": {
            "level2": {
                "level3": {
                    "secret": "classified"
                }
            }
        }
    });

    let rules = vec![RedactionRule::new(
        vec![
            "level1".into(),
            "level2".into(),
            "level3".into(),
            "secret".into(),
        ],
        RedactionKind::Remove,
    )];

    let (projection, _) = apply_redaction(&canonical, &rules);

    assert!(
        projection["level1"]["level2"]["level3"]
            .get("secret")
            .is_none(),
        "Remove should omit deeply nested key entirely"
    );
    assert!(projection["level1"]["level2"]["level3"].is_object());
}

#[test]
fn tdd_red_003_remove_omits_key_from_array_elements() {
    let canonical = serde_json::json!({
        "users": [
            {"name": "Alice", "ssn": "111-22-3333"},
            {"name": "Bob", "ssn": "444-55-6666"}
        ]
    });

    let rules = vec![RedactionRule::new(
        vec!["users".into(), "ssn".into()],
        RedactionKind::Remove,
    )];

    let (projection, _) = apply_redaction(&canonical, &rules);

    assert!(
        projection["users"][0].get("ssn").is_none(),
        "Remove should omit key from first array element"
    );
    assert!(
        projection["users"][1].get("ssn").is_none(),
        "Remove should omit key from second array element"
    );
    assert_eq!(projection["users"][0]["name"], "Alice");
    assert_eq!(projection["users"][1]["name"], "Bob");
}

#[test]
fn tdd_red_004_remove_omits_boolean_field_key() {
    let canonical = serde_json::json!({
        "user": {"name": "Alice", "is_admin": true}
    });

    let rules = vec![RedactionRule::new(
        vec!["user".into(), "is_admin".into()],
        RedactionKind::Remove,
    )];

    let (projection, _) = apply_redaction(&canonical, &rules);

    assert!(
        projection["user"].get("is_admin").is_none(),
        "Remove should omit boolean field key"
    );
    assert_eq!(projection["user"]["name"], "Alice");
}

#[test]
fn tdd_red_005_remove_omits_numeric_field_key() {
    let canonical = serde_json::json!({
        "employee": {"name": "Bob", "salary": 75000}
    });

    let rules = vec![RedactionRule::new(
        vec!["employee".into(), "salary".into()],
        RedactionKind::Remove,
    )];

    let (projection, _) = apply_redaction(&canonical, &rules);

    assert!(
        projection["employee"].get("salary").is_none(),
        "Remove should omit numeric field key"
    );
    assert_eq!(projection["employee"]["name"], "Bob");
}

#[test]
fn tdd_red_006_remove_omits_null_valued_field_key() {
    let canonical = serde_json::json!({
        "record": {"field": null, "other": "safe"}
    });

    let rules = vec![RedactionRule::new(
        vec!["record".into(), "field".into()],
        RedactionKind::Remove,
    )];

    let (projection, _) = apply_redaction(&canonical, &rules);

    assert!(
        projection["record"].get("field").is_none(),
        "Remove should omit key even when original value is null"
    );
    assert_eq!(projection["record"]["other"], "safe");
}

#[test]
fn tdd_red_007_remove_omits_array_valued_field_key() {
    let canonical = serde_json::json!({
        "document": {"title": "Report", "tags": ["confidential", "internal"]}
    });

    let rules = vec![RedactionRule::new(
        vec!["document".into(), "tags".into()],
        RedactionKind::Remove,
    )];

    let (projection, _) = apply_redaction(&canonical, &rules);

    assert!(
        projection["document"].get("tags").is_none(),
        "Remove should omit array-valued field key"
    );
    assert_eq!(projection["document"]["title"], "Report");
}

// ========================================================================
// GROUP 2: Non-Redacted Value Preservation (ADR-025 §1)
//
// BUG: Condition `was_redacted || new_val != Null` drops non-redacted nulls.
// ========================================================================

#[test]
fn tdd_red_008_non_redacted_null_values_preserved() {
    let canonical = serde_json::json!({
        "user": {
            "name": "Alice",
            "middle_name": null,
            "email": "alice@example.com"
        }
    });

    let rules = vec![RedactionRule::new(
        vec!["user".into(), "email".into()],
        RedactionKind::Remove,
    )];

    let (projection, _) = apply_redaction(&canonical, &rules);

    assert!(
        projection["user"].get("middle_name").is_some(),
        "Non-redacted null field 'middle_name' must be preserved"
    );
    assert_eq!(
        projection["user"]["middle_name"],
        serde_json::Value::Null,
        "Non-redacted null field must retain null value"
    );
    assert_eq!(projection["user"]["name"], "Alice");
}

// ========================================================================
// GROUP 3: Redaction Completeness — Deduplication (ADR-025 §1)
//
// BUG: Array element matches push the same field_path multiple times.
// ========================================================================

#[test]
fn tdd_red_009_redacted_fields_list_has_no_duplicates() {
    let canonical = serde_json::json!({
        "users": [
            {"name": "Alice", "ssn": "111"},
            {"name": "Bob", "ssn": "222"},
            {"name": "Carol", "ssn": "333"}
        ]
    });

    let rules = vec![RedactionRule::new(
        vec!["users".into(), "ssn".into()],
        RedactionKind::Remove,
    )];

    let (_, redacted) = apply_redaction(&canonical, &rules);

    assert_eq!(
        redacted.len(),
        1,
        "Redacted fields should be deduplicated: expected 1 unique path, got {} entries: {:?}",
        redacted.len(),
        redacted
    );
    assert_eq!(redacted[0], vec!["users", "ssn"]);
}

#[test]
fn tdd_red_010_redacted_fields_count_equals_unique_rule_matches() {
    let canonical = serde_json::json!({
        "data": {
            "items": [
                {"id": 1, "secret": "a"},
                {"id": 2, "secret": "b"}
            ],
            "key": "sensitive"
        }
    });

    let rules = vec![
        RedactionRule::new(
            vec!["data".into(), "items".into(), "secret".into()],
            RedactionKind::Remove,
        ),
        RedactionRule::new(vec!["data".into(), "key".into()], RedactionKind::Remove),
    ];

    let (_, redacted) = apply_redaction(&canonical, &rules);

    // Two unique rule paths; "secret" matches 2 array elements but counts once
    assert_eq!(
        redacted.len(),
        2,
        "Expected 2 redacted entries (one per unique rule path), got {}: {:?}",
        redacted.len(),
        redacted
    );
}

// ========================================================================
// GROUP 4: Dual Representation Invariants (ADR-025 §1-2)
//
// After Remove redaction, no trace of the field should remain —
// not the key name, not the value, not a null placeholder.
// ========================================================================

#[test]
fn tdd_red_011_removed_field_not_in_serialized_projection() {
    let canonical = serde_json::json!({
        "patient": {
            "name": "John",
            "ssn": "123-45-6789",
            "diagnosis": "Flu"
        }
    });

    let rules = vec![RedactionRule::new(
        vec!["patient".into(), "ssn".into()],
        RedactionKind::Remove,
    )];

    let (projection, _) = apply_redaction(&canonical, &rules);
    let serialized = serde_json::to_string(&projection).unwrap();

    assert!(
        !serialized.contains("ssn"),
        "Removed field name 'ssn' must not appear in serialized projection: {}",
        serialized
    );
    assert!(
        !serialized.contains("6789"),
        "Removed field value must not leak in serialized projection: {}",
        serialized
    );
    assert!(
        serialized.contains("John"),
        "Non-redacted 'name' should be present"
    );
    assert!(
        serialized.contains("Flu"),
        "Non-redacted 'diagnosis' should be present"
    );
}

#[test]
fn tdd_red_012_multiple_removed_fields_absent_from_projection() {
    let canonical = serde_json::json!({
        "user": {
            "name": "Alice",
            "ssn": "123-45-6789",
            "password": "hunter2",
            "email": "alice@example.com"
        }
    });

    let rules = vec![
        RedactionRule::new(vec!["user".into(), "ssn".into()], RedactionKind::Remove),
        RedactionRule::new(
            vec!["user".into(), "password".into()],
            RedactionKind::Remove,
        ),
        RedactionRule::new(vec!["user".into(), "email".into()], RedactionKind::Remove),
    ];

    let (projection, _) = apply_redaction(&canonical, &rules);
    let serialized = serde_json::to_string(&projection).unwrap();

    assert!(!serialized.contains("ssn"), "Removed 'ssn' leaked");
    assert!(
        !serialized.contains("password"),
        "Removed 'password' leaked"
    );
    assert!(!serialized.contains("email"), "Removed 'email' leaked");
    assert!(
        !serialized.contains("hunter2"),
        "Removed password value leaked"
    );
    assert!(
        !serialized.contains("example.com"),
        "Removed email domain leaked"
    );
    assert!(
        serialized.contains("Alice"),
        "Non-redacted 'name' should be present"
    );

    let obj = projection["user"].as_object().unwrap();
    assert_eq!(
        obj.keys().collect::<Vec<_>>(),
        vec!["name"],
        "Projection should only contain non-redacted keys, got: {:?}",
        obj.keys().collect::<Vec<_>>()
    );
}

#[test]
fn tdd_red_013_removed_nested_object_absent_from_projection() {
    let canonical = serde_json::json!({
        "config": {
            "public": {"theme": "dark", "language": "en"},
            "private": {"api_key": "sk-12345", "webhook_secret": "whs-67890"}
        }
    });

    let rules = vec![RedactionRule::new(
        vec!["config".into(), "private".into()],
        RedactionKind::Remove,
    )];

    let (projection, _) = apply_redaction(&canonical, &rules);

    assert_eq!(projection["config"]["public"]["theme"], "dark");
    assert_eq!(projection["config"]["public"]["language"], "en");
    assert!(
        projection["config"].get("private").is_none(),
        "Removed 'private' object should be absent from projection"
    );

    let serialized = serde_json::to_string(&projection).unwrap();
    assert!(!serialized.contains("sk-12345"), "API key leaked");
    assert!(!serialized.contains("whs-67890"), "Webhook secret leaked");
    assert!(
        !serialized.contains("private"),
        "Removed key name 'private' leaked"
    );
}

// ========================================================================
// GROUP 5: DEK/KEK Lifecycle Validation (ADR-025 §2-3, ADR-040)
//
// ADR-025 §2: Per-instance DEK wrapped by engine-managed KEK via AES-256-GCM.
// Wrapping overhead: IV(12) + Tag(16) = 28 bytes minimum.
// Full wrapped DEK: IV(12) + ciphertext(32) + tag(16) = 60 bytes.
//
// WrappedDek must enforce minimum size to catch truncation/corruption.
// Current: WrappedDek::new accepts any Vec<u8> without validation.
// ========================================================================

use vo_types::{CryptoAlgorithm, WrappedDek};

#[test]
fn tdd_red_014_wrapped_dek_minimum_size_is_iv_plus_tag() {
    let min_wrapped_size = CryptoAlgorithm::IV_SIZE_BYTES + CryptoAlgorithm::TAG_SIZE_BYTES; // 28

    let result = WrappedDek::new(vec![0u8; min_wrapped_size - 1]);
    assert!(
        result.is_err(),
        "I-DEK-1: WrappedDek must reject < 60 bytes"
    );
}

#[test]
fn tdd_red_015_wrapped_dek_expected_size_for_aes256_key() {
    let expected_size = CryptoAlgorithm::IV_SIZE_BYTES
        + CryptoAlgorithm::KEY_SIZE_BYTES
        + CryptoAlgorithm::TAG_SIZE_BYTES; // 60

    let properly_wrapped =
        WrappedDek::new(vec![0u8; expected_size]).expect("valid size should be accepted");
    assert_eq!(
        properly_wrapped.as_bytes().len(),
        expected_size,
        "I-DEK-2: Properly wrapped AES-256 DEK should be exactly {} bytes",
        expected_size
    );

    let result = WrappedDek::new(vec![0u8; expected_size - 1]);
    assert!(
        result.is_err(),
        "I-DEK-2: WrappedDek < {} bytes indicates corruption",
        expected_size
    );
}

#[test]
fn tdd_red_016_empty_wrapped_dek_is_structurally_invalid() {
    let result = WrappedDek::new(vec![]);
    assert!(
        result.is_err(),
        "I-DEK-1: Empty WrappedDek is structurally invalid (no IV, ciphertext, or tag)"
    );
}

#[test]
fn tdd_red_017_wrapped_dek_size_consistent_with_aes256gcm_algorithm() {
    let iv_tag_overhead = CryptoAlgorithm::IV_SIZE_BYTES + CryptoAlgorithm::TAG_SIZE_BYTES; // 28
    let key_size = CryptoAlgorithm::KEY_SIZE_BYTES; // 32
    let min_for_key = iv_tag_overhead + key_size; // 60

    // Only IV+tag overhead, no room for encrypted key material
    let result = WrappedDek::new(vec![0u8; iv_tag_overhead]);
    assert!(
        result.is_err(),
        "I-DEK-2: {} bytes is only IV+tag overhead, no room for encrypted key material",
        iv_tag_overhead
    );
}
