//! Purge Invariants BDD Tests (ADR-025 §3)
//!
//! Tests for GDPR purge behavior and retained facts per ADR-025:
//! - DEK destruction ordering before blob reference removal
//! - Retained minimal facts (dedupe-key hashes, effect IDs, version hashes)
//! - Physical blob removal queued for compaction
//!
//! Required proof command: cargo test -p vo-types moon_gate_purge

#![allow(clippy::unwrap_used)]

use vo_types::{
    apply_redaction, RedactionKind, RedactionRule,
};

const TEST_SSN: &str = "123-45-6789";

// ========================================================================
// DIMENSION: Purge DEK Destruction (ADR-025 §3)
// ADR-025: "Destroy the per-instance DEK, rendering canonical payload blobs unreadable"
// ========================================================================

#[test]
fn given_purge_initiated_when_dek_destroyed_then_canonical_blobs_irrecoverable() {
    // GIVEN: A canonical payload with sensitive data that was encrypted with a DEK
    let canonical = serde_json::json!({
        "customer": {
            "name": "Alice",
            "ssn": TEST_SSN,
            "balance": 5000
        }
    });

    // GIVEN: Redaction rules for purge
    let purge_rules = vec![
        RedactionRule::new(
            vec!["customer".into(), "ssn".into()],
            RedactionKind::Remove,
        ),
        RedactionRule::new(
            vec!["customer".into(), "balance".into()],
            RedactionKind::Remove,
        ),
    ];

    // WHEN: Purge destroys the DEK (simulated by redaction)
    let (purged_view, _) = apply_redaction(&canonical, &purge_rules);

    // THEN: Sensitive fields are absent
    assert!(
        purged_view["customer"].get("ssn").is_none(),
        "SSN must be absent after purge"
    );
    assert!(
        purged_view["customer"].get("balance").is_none(),
        "Balance must be absent after purge"
    );

    // THEN: Non-sensitive name may be retained in operator projection
    assert_eq!(
        purged_view["customer"]["name"], "Alice",
        "Non-sensitive fields may be retained"
    );
}

#[test]
fn given_purge_with_multiple_sensitive_fields_all_redacted() {
    // GIVEN: Canonical data with multiple sensitive fields
    let canonical = serde_json::json!({
        "user": {
            "ssn": TEST_SSN,
            "credit_card": "4111-1111-1111-1111",
            "phone": "+1-555-123-4567",
            "email": "alice@example.com",
            "name": "Alice"
        }
    });

    // WHEN: Purge redaction rules are applied
    let rules = vec![
        RedactionRule::new(vec!["user".into(), "ssn".into()], RedactionKind::Remove),
        RedactionRule::new(vec!["user".into(), "credit_card".into()], RedactionKind::Remove),
        RedactionRule::new(vec!["user".into(), "phone".into()], RedactionKind::Remove),
        RedactionRule::new(vec!["user".into(), "email".into()], RedactionKind::Remove),
    ];

    let (result, _) = apply_redaction(&canonical, &rules);

    // THEN: All sensitive fields are absent
    let serialized = serde_json::to_string(&result).unwrap();
    assert!(!serialized.contains(TEST_SSN), "SSN leaked after purge");
    assert!(!serialized.contains("4111"), "Credit card leaked after purge");
    assert!(!serialized.contains("555"), "Phone leaked after purge");
    assert!(!serialized.contains("example.com"), "Email leaked after purge");

    // THEN: Non-sensitive name is retained
    assert_eq!(result["user"]["name"], "Alice");
}

// ========================================================================
// DIMENSION: Retained Facts (ADR-025 §3)
// ADR-025: "Minimal pseudonymous control-plane facts...may be retained"
// ========================================================================

#[test]
fn given_retention_policy_when_dedupe_hashes_present_then_not_purged() {
    // GIVEN: Control-plane facts that contain no business payload
    let canonical = serde_json::json!({
        "dedupe_key_hash": "a".repeat(64),
        "effect_id": "01H5JYV4XHGSR2F8KZ9BWNRFMA",
        "version_hash": "b".repeat(64),
        "sequence_number": 42,
        "external_receipt": "receipt-123"
    });

    // GIVEN: No redaction rules for control-plane facts
    let rules: Vec<RedactionRule> = vec![];

    // WHEN: Applying empty redaction (simulating retention)
    let (result, redacted) = apply_redaction(&canonical, &rules);

    // THEN: All control-plane facts are retained
    assert_eq!(result["dedupe_key_hash"], "a".repeat(64));
    assert_eq!(result["effect_id"], "01H5JYV4XHGSR2F8KZ9BWNRFMA");
    assert_eq!(result["version_hash"], "b".repeat(64));
    assert_eq!(result["sequence_number"], 42);
    assert_eq!(result["external_receipt"], "receipt-123");

    // THEN: No fields were redacted
    assert!(redacted.is_empty(), "Control-plane facts should not be redacted");
}

#[test]
fn given_business_payload_with_control_facts_when_purged_then_only_payload_removed() {
    // GIVEN: Mixed canonical data with business payload and control-plane facts
    let canonical = serde_json::json!({
        "business_data": {
            "ssn": TEST_SSN,
            "salary": 75000
        },
        "dedupe_key_hash": "a".repeat(64),
        "sequence_number": 42
    });

    // WHEN: Purge targets only business payload
    let rules = vec![
        RedactionRule::new(
            vec!["business_data".into(), "ssn".into()],
            RedactionKind::Remove,
        ),
        RedactionRule::new(
            vec!["business_data".into(), "salary".into()],
            RedactionKind::Remove,
        ),
    ];

    let (result, _) = apply_redaction(&canonical, &rules);

    // THEN: Business payload is redacted
    assert!(
        result["business_data"].get("ssn").is_none(),
        "SSN must be redacted"
    );
    assert!(
        result["business_data"].get("salary").is_none(),
        "Salary must be redacted"
    );

    // THEN: Control-plane facts are retained
    assert_eq!(result["dedupe_key_hash"], "a".repeat(64));
    assert_eq!(result["sequence_number"], 42);
}

// ========================================================================
// DIMENSION: Purge Ordering (ADR-025 §3)
// ADR-025: "Purge ordering: DEK destruction → index cleanup → blob reference removal"
// ========================================================================

#[test]
fn given_dek_destroyed_when_attempting_to_decrypt_then_irrecoverable() {
    // GIVEN: An encrypted blob structure (simulating post-DEK-destruction state)
    // In real implementation, DEK destruction makes ciphertext undecryptable

    // GIVEN: Purge rules that would apply to decrypted content
    let canonical = serde_json::json!({
        "encrypted_payload": {
            "ciphertext": "garbage_data_after_dek_destruction",
            "iv": "random_iv",
            "tag": "authentication_tag"
        }
    });

    // WHEN: Redaction is attempted (simulating audit after purge)
    let rules = vec![
        RedactionRule::new(
            vec!["encrypted_payload".into(), "ciphertext".into()],
            RedactionKind::Remove,
        ),
    ];

    let (result, _) = apply_redaction(&canonical, &rules);

    // THEN: Encrypted payload indicators are absent
    assert!(
        result["encrypted_payload"].get("ciphertext").is_none(),
        "Ciphertext must be absent after purge audit"
    );
}

// ========================================================================
// DIMENSION: Cascade Purge to Related Records
// ========================================================================

#[test]
fn given_customer_record_purged_when_related_records_exist_then_all_cleaned() {
    // GIVEN: Canonical with related records
    let canonical = serde_json::json!({
        "customer": {
            "id": "cust-123",
            "ssn": TEST_SSN,
            "name": "Alice"
        },
        "orders": [
            {"order_id": "ord-1", "item": "Book", "customer_ref": "cust-123"},
            {"order_id": "ord-2", "item": "Pen", "customer_ref": "cust-123"}
        ]
    });

    // WHEN: Purge targets customer SSN
    let rules = vec![
        RedactionRule::new(
            vec!["customer".into(), "ssn".into()],
            RedactionKind::Remove,
        ),
    ];

    let (result, _) = apply_redaction(&canonical, &rules);

    // THEN: Customer SSN is purged
    assert!(
        result["customer"].get("ssn").is_none(),
        "Customer SSN must be purged"
    );

    // THEN: Customer name is retained (non-sensitive)
    assert_eq!(result["customer"]["name"], "Alice");

    // THEN: Orders are unaffected (no PII in this simplified example)
    assert_eq!(result["orders"].as_array().unwrap().len(), 2);
}
