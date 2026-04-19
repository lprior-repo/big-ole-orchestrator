//! Black Hat Adversarial Tests: AI Redaction Completeness (ADR-008/025)
//!
//! Adversarial review verifying zero PII leaks through the dual-representation
//! privacy model. Each test targets a specific bypass vector.
//!
//! NOTE: These are AUDIT tests, not TDD-driven feature tests. Every test
//! documents an existing vulnerability by asserting current (vulnerable) behavior.
//! Tests pass because the vulnerability exists; a fix would require changing
//! the redaction engine and updating the assertion.
//!
//! Vulnerability categories tested:
//! BH-01: PII in unlisted field paths (missed field = full leak)
//! BH-02: Encoding bypass (base64/unicode/HTML entities in values)
//! BH-03: Key-as-PII (sensitive data in JSON object keys, not values)
//! BH-04: Hash algorithm mismatch (ADR-025 says SHA-256, code uses DefaultHasher/SipHash)
//! BH-05: ReplaceWithType information leakage
//! BH-06: Remove inconsistency (objects omit key, arrays leave null)
//! BH-07: Default-deny failure (empty rules = zero redaction)
//! BH-08: Array index path fragility (reordering breaks positional rules)
//! BH-09: Canonical view isolation verification
//! BH-10: OperatorProjection metadata leakage

use serde_json::json;
use vo_types::{apply_redaction, RedactionKind, RedactionRule, OperatorProjection};

// ────────────────────────────────────────────────────────────────────
// BH-01: PII injection in every field type (unlisted paths leak verbatim)
// ────────────────────────────────────────────────────────────────────

#[test]
fn bh01_pii_leaks_through_unlisted_string_field() {
    let value = json!({
        "ssn": "123-45-6789",
        "secret_answer": "My first pet was Fluffy"
    });
    let rules = vec![RedactionRule::new(
        vec!["ssn".into()],
        RedactionKind::Remove,
    )];

    let (result, _) = apply_redaction(&value, &rules);
    let serialized = serde_json::to_string(&result).unwrap();

    assert!(
        !serialized.contains("123-45-6789"),
        "SSN leaked in output: {serialized}"
    );
    // FINDING: secret_answer leaks — policy gap, not code bug.
    assert!(
        serialized.contains("Fluffy"),
        "Expected unlisted field to pass through (policy gap, not code bug)"
    );
}

#[test]
fn bh01_pii_in_nested_unlisted_field() {
    let value = json!({
        "user": {
            "profile": {
                "medical_history": "Patient has condition X",
                "allergies": ["penicillin", "latex"]
            }
        }
    });
    let rules = vec![RedactionRule::new(
        vec!["user".into(), "profile".into(), "allergies".into()],
        RedactionKind::Remove,
    )];

    let (result, _) = apply_redaction(&value, &rules);
    let serialized = serde_json::to_string(&result).unwrap();

    assert!(
        !serialized.contains("penicillin"),
        "Allergy data leaked: {serialized}"
    );
    assert!(
        serialized.contains("condition X"),
        "Expected unlisted nested field to pass through"
    );
}

#[test]
fn bh01_pii_in_array_elements_beyond_policy_depth() {
    let value = json!({
        "orders": [
            {"id": 1, "items": [{"name": "Widget", "credit_card": "4111-1111-1111-1111"}]},
            {"id": 2, "items": [{"name": "Gadget", "credit_card": "5500-0000-0000-0004"}]}
        ]
    });
    let rules = vec![RedactionRule::new(
        vec!["orders".into()],
        RedactionKind::ReplaceWith("[REDACTED]".into()),
    )];

    let (result, _) = apply_redaction(&value, &rules);
    let serialized = serde_json::to_string(&result).unwrap();

    assert!(!serialized.contains("4111"), "CC leaked despite array-level redaction: {serialized}");
    assert!(!serialized.contains("5500"), "CC leaked despite array-level redaction: {serialized}");
}

#[test]
fn bh01_numeric_pii_passes_through_unredacted() {
    let value = json!({
        "employee": {"name": "Alice", "salary": 185000, "credit_score": 742}
    });
    let rules = vec![RedactionRule::new(
        vec!["employee".into(), "name".into()],
        RedactionKind::Remove,
    )];

    let (result, _) = apply_redaction(&value, &rules);
    let serialized = serde_json::to_string(&result).unwrap();

    assert!(serialized.contains("185000"), "Expected numeric PII to pass through when unlisted");
}

#[test]
fn bh01_boolean_pii_passes_through_unredacted() {
    let value = json!({
        "applicant": {"has_disability": true, "criminal_record": false, "is_veteran": true}
    });
    let rules: Vec<RedactionRule> = vec![];

    let (result, _) = apply_redaction(&value, &rules);
    let serialized = serde_json::to_string(&result).unwrap();

    assert!(serialized.contains("has_disability"), "Boolean PII field names leaked through empty policy");
}

// ────────────────────────────────────────────────────────────────────
// BH-02: Redaction bypass via encoding tricks
// ────────────────────────────────────────────────────────────────────

#[test]
fn bh02_base64_encoded_pii_bypasses_field_redaction() {
    let value = json!({
        "ssn": "123-45-6789",
        "notes": "U1NOOiAxMjMtNDUtNjc4OQ==" // base64("SSN: 123-45-6789")
    });
    let rules = vec![RedactionRule::new(
        vec!["ssn".into()],
        RedactionKind::Remove,
    )];

    let (result, _) = apply_redaction(&value, &rules);
    let serialized = serde_json::to_string(&result).unwrap();

    assert!(!serialized.contains("123-45-6789"));
    // FINDING: base64-encoded PII in non-redacted field bypasses content-agnostic redaction
    assert!(
        serialized.contains("U1NOOiAxMjMtNDUtNjc4OQ=="),
        "Base64-encoded PII in non-redacted field bypasses content-agnostic redaction"
    );
}

#[test]
fn bh02_unicode_homoglyph_pii_bypass() {
    let value = json!({
        "ssn": "123-45-6789",
        "\u{ff33}\u{ff33}n": "123-45-6789" // fullwidth 's' (U+FF33)
    });
    let rules = vec![RedactionRule::new(
        vec!["ssn".into()],
        RedactionKind::Remove,
    )];

    let (result, _) = apply_redaction(&value, &rules);
    let serialized = serde_json::to_string(&result).unwrap();

    assert!(!result["ssn"].is_string());
    assert!(
        serialized.contains("123-45-6789"),
        "Unicode homoglyph field bypassed redaction"
    );
}

#[test]
fn bh02_pii_duplicated_across_fields_bypasses_single_field_redaction() {
    let value = json!({
        "ssn": "123-45-6789",
        "description": "User's SSN is \"123-45-6789\" — handle with care"
    });
    let rules = vec![RedactionRule::new(
        vec!["ssn".into()],
        RedactionKind::Remove,
    )];

    let (result, _) = apply_redaction(&value, &rules);
    let serialized = serde_json::to_string(&result).unwrap();

    assert!(!result["ssn"].is_string());
    // FINDING: PII duplicated in non-redacted field leaks
    assert!(
        serialized.contains("123-45-6789"),
        "PII duplicated in non-redacted field bypasses redaction"
    );
}

// ────────────────────────────────────────────────────────────────────
// BH-03: Key-as-PII (sensitive data in JSON object keys)
// ────────────────────────────────────────────────────────────────────

#[test]
fn bh03_pii_in_object_keys_is_never_redacted() {
    let value = json!({
        "user_123-45-6789": {"name": "Alice"},
        "email_alice@example.com": "active"
    });
    let rules = vec![RedactionRule::new(
        vec!["ssn".into()],
        RedactionKind::Remove,
    )];

    let (result, _) = apply_redaction(&value, &rules);
    let serialized = serde_json::to_string(&result).unwrap();

    // FINDING: PII in keys is invisible to the redaction engine
    assert!(
        serialized.contains("123-45-6789"),
        "PII in object key leaked through redaction"
    );
    assert!(
        serialized.contains("alice@example.com"),
        "Email PII in object key leaked through redaction"
    );
}

#[test]
fn bh03_pii_in_dynamic_keys_within_arrays() {
    let value = json!({
        "metadata": [
            {"SSN-123-45-6789": "verified"},
            {"DOB-1990-01-15": "confirmed"}
        ]
    });
    let rules: Vec<RedactionRule> = vec![];

    let (result, _) = apply_redaction(&value, &rules);
    let serialized = serde_json::to_string(&result).unwrap();

    assert!(
        serialized.contains("123-45-6789"),
        "PII in dynamic array-element key leaked"
    );
    assert!(
        serialized.contains("1990-01-15"),
        "Date of birth PII in array-element key leaked"
    );
}

// ────────────────────────────────────────────────────────────────────
// BH-04: Hash algorithm mismatch (ADR-025 specifies SHA-256, code uses SipHash)
// ────────────────────────────────────────────────────────────────────

#[test]
fn bh04_hash_uses_siphash_not_sha256_as_specified_in_adr() {
    // ADR-025 says: "Field is hashed with SHA-256 (preserves uniqueness for correlation)"
    // Code uses std::hash::DefaultHasher (SipHash-1-3), NOT SHA-256.
    // SipHash is NOT cryptographically secure — it's a hash-DoS protection hash.

    let kind = RedactionKind::Hash;
    let value = json!("alice@example.com");

    let result = kind.redact_value("email", &value);
    let hash_str = result.as_str().unwrap();

    assert!(hash_str.starts_with("HASH"));

    // SipHash produces u64 (16 hex chars), SHA-256 would be 64
    let hash_part = &hash_str[4..];
    assert_eq!(
        hash_part.len(),
        16,
        "FINDING: Hash is {} hex chars (SipHash/u64), not 64 hex chars (SHA-256). \
         ADR-025 specifies SHA-256 for PII pseudonymization.",
        hash_part.len()
    );
}

#[test]
fn bh04_siphash_insufficient_collision_resistance_for_pii() {
    // SipHash-1-3 has 64-bit output (2^64 space). Birthday attack: ~2^32 collisions.
    // SHA-256 has 2^256 space — exponentially more collision-resistant.

    let kind = RedactionKind::Hash;
    let value = json!("test-pii-value");

    let result = kind.redact_value("field", &value);
    let hash_str = result.as_str().unwrap();
    let hash_bits = (&hash_str[4..]).len() * 4;

    assert_eq!(
        hash_bits, 64,
        "FINDING: Hash output is only {hash_bits} bits. \
         GDPR pseudonymization typically requires >=128 bits (SHA-256 at 256 bits)."
    );
}

// ────────────────────────────────────────────────────────────────────
// BH-05: ReplaceWithType information leakage
// ────────────────────────────────────────────────────────────────────

#[test]
fn bh05_replace_with_type_leaks_rust_type_names() {
    let value = json!({
        "password": "super_secret_123",
        "api_key": "sk-abc123",
        "token_count": 42,
        "is_admin": true
    });

    let rules = vec![
        RedactionRule::new(vec!["password".into()], RedactionKind::ReplaceWithType),
        RedactionRule::new(vec!["api_key".into()], RedactionKind::ReplaceWithType),
        RedactionRule::new(vec!["token_count".into()], RedactionKind::ReplaceWithType),
        RedactionRule::new(vec!["is_admin".into()], RedactionKind::ReplaceWithType),
    ];

    let (result, _) = apply_redaction(&value, &rules);

    let password_type = result["password"].as_str().unwrap();
    let count_type = result["token_count"].as_str().unwrap();
    // FINDING: ALL values get the same type name "serde_json::value::Value"
    // because type_name_of_val receives &serde_json::Value, not the inner type.
    // ReplaceWithType is fundamentally broken — no type discrimination possible.
    assert_eq!(password_type, "serde_json::value::Value",
        "ReplaceWithType produces serde_json::Value for strings");
    assert_eq!(count_type, "serde_json::value::Value",
        "ReplaceWithType produces serde_json::Value for numbers — no type discrimination");
}

#[test]
fn bh05_type_names_enable_schema_inference() {
    let value = json!({
        "id": "uuid-1234",
        "amount": 99.99,
        "active": true,
        "metadata": {"key": "val"}
    });

    let rules = vec![
        RedactionRule::new(vec!["id".into()], RedactionKind::ReplaceWithType),
        RedactionRule::new(vec!["amount".into()], RedactionKind::ReplaceWithType),
        RedactionRule::new(vec!["active".into()], RedactionKind::ReplaceWithType),
        RedactionRule::new(vec!["metadata".into()], RedactionKind::ReplaceWithType),
    ];

    let (result, _) = apply_redaction(&value, &rules);

    let id_type = result["id"].as_str().unwrap();
    let active_type = result["active"].as_str().unwrap();
    // FINDING: All values produce identical type — schema inference impossible.
    // But implementation detail (serde_json) IS leaked to the adversary.
    assert_eq!(id_type, "serde_json::value::Value",
        "ReplaceWithType always returns serde_json::value::Value");
    assert_eq!(id_type, active_type,
        "FINDING: No type discrimination — all JSON values look the same");
}

// ────────────────────────────────────────────────────────────────────
// BH-06: Remove inconsistency between objects and arrays
// ────────────────────────────────────────────────────────────────────

#[test]
fn bh06_remove_omits_key_from_object_but_leaves_null_in_array() {
    // Remove omits the key from objects entirely (key + value both gone)
    let obj_value = json!({"secret": "hidden", "public": "visible"});
    let rules = vec![RedactionRule::new(
        vec!["secret".into()],
        RedactionKind::Remove,
    )];
    let (obj_result, _) = apply_redaction(&obj_value, &rules);

    let obj = obj_result.as_object().unwrap();
    assert!(!obj.contains_key("secret"),
        "Remove should omit key from object");
    assert_eq!(obj.len(), 1);

    // FINDING: Redaction rules NEVER apply to primitive array elements!
    // The recursive function only checks rules in the Object branch.
    // Primitive array elements (strings, numbers, bools) are returned as-is.
    let arr_value = json!({"items": ["secret1", "secret2", "public"]});
    let rules = vec![RedactionRule::new(
        vec!["items".into(), "1".into()],
        RedactionKind::Remove,
    )];
    let (arr_result, _) = apply_redaction(&arr_value, &rules);

    let items = arr_result["items"].as_array().unwrap();
    assert_eq!(items[1], "secret2",
        "FINDING: Primitive array elements are NEVER redacted — rule ignored");
    assert_eq!(items[0], "secret1");
    assert_eq!(items[2], "public");
    assert_eq!(items.len(), 3);
}

#[test]
fn bh06_array_null_gaps_reveal_redaction_pattern() {
    let value = json!({
        "records": [
            {"ssn": "111-11-1111", "name": "Alice"},
            {"ssn": "222-22-2222", "name": "Bob"},
            {"ssn": "333-33-3333", "name": "Carol"}
        ]
    });
    let rules = vec![RedactionRule::new(
        vec!["records".into(), "ssn".into()],
        RedactionKind::Remove,
    )];

    let (result, _) = apply_redaction(&value, &rules);
    let records = result["records"].as_array().unwrap();

    // FINDING: BUG! Path ["records", "ssn"] does NOT match inside array elements.
    // The actual path becomes ["records", "0", "ssn"], ["records", "1", "ssn"], etc.
    // because the array index is pushed to current_path before descending.
    // This means field-path redaction is COMPLETELY BROKEN for nested objects
    // inside arrays — the most common pattern for lists of records!
    for (i, record) in records.iter().enumerate() {
        let expected = ["111-11-1111", "222-22-2222", "333-33-3333"]; assert_eq!(record["ssn"], json!(expected[i]),
            "FINDING: SSN NOT redacted — array index in path breaks rule matching");
        assert!(record.get("name").is_some());
    }

    // NOTE: The existing unit test apply_redaction_handles_arrays_recursively
    // also fails with this same bug. This is a critical vulnerability:
    // any workflow that stores PII in arrays of objects (the most common
    // pattern) has NO redaction coverage.
}

// ────────────────────────────────────────────────────────────────────
// BH-07: Default-deny failure (empty rules = full PII exposure)
// ────────────────────────────────────────────────────────────────────

#[test]
fn bh07_empty_redaction_rules_exposes_all_pii() {
    let value = json!({
        "ssn": "123-45-6789",
        "credit_card": "4111-1111-1111-1111",
        "password": "hunter2",
        "api_key": "sk-live-abc123",
        "medical_notes": "Patient has chronic condition",
        "salary": 250000
    });
    let rules: Vec<RedactionRule> = vec![];

    let (result, _) = apply_redaction(&value, &rules);
    let serialized = serde_json::to_string(&result).unwrap();

    // FINDING: no default-deny safety net — EVERYTHING leaks
    assert!(serialized.contains("123-45-6789"), "SSN leaked with empty rules");
    assert!(serialized.contains("4111-1111-1111-1111"), "CC leaked with empty rules");
    assert!(serialized.contains("hunter2"), "Password leaked with empty rules");
    assert!(serialized.contains("sk-live-abc123"), "API key leaked with empty rules");
    assert!(serialized.contains("chronic condition"), "Medical data leaked with empty rules");
    assert!(serialized.contains("250000"), "Salary leaked with empty rules");
}

#[test]
fn bh07_non_matching_rules_same_as_empty() {
    let value = json!({
        "ssn": "123-45-6789",
        "name": "Alice"
    });
    let rules = vec![RedactionRule::new(
        vec!["nonexistent".into(), "field".into()],
        RedactionKind::Remove,
    )];

    let (result, _) = apply_redaction(&value, &rules);
    let serialized = serde_json::to_string(&result).unwrap();

    assert!(
        serialized.contains("123-45-6789"),
        "PII leaked when no rules match any actual field path"
    );
}

// ────────────────────────────────────────────────────────────────────
// BH-08: Array index path fragility
// ────────────────────────────────────────────────────────────────────

#[test]
fn bh08_array_reordering_breaks_positional_redaction_rules() {
    let value = json!({
        "users": [
            {"name": "Alice", "ssn": "111-11-1111"},
            {"name": "Bob", "ssn": "222-22-2222"}
        ]
    });

    let rules = vec![RedactionRule::new(
        vec!["users".into(), "0".into(), "ssn".into()],
        RedactionKind::Remove,
    )];

    let (result, _) = apply_redaction(&value, &rules);

    assert_eq!(result["users"][0]["ssn"], json!(null));
    // FINDING: Index 1 SSN is NOT redacted — positional rules are fragile
    assert_eq!(
        result["users"][1]["ssn"], "222-22-2222",
        "FINDING: Positional redaction only covers the specified index"
    );
}

#[test]
fn bh08_wildcard_redaction_not_supported() {
    let value = json!({
        "users": [
            {"ssn": "111"}, {"ssn": "222"}, {"ssn": "333"},
            {"ssn": "444"}, {"ssn": "555"}
        ]
    });

    let rules = vec![RedactionRule::new(
        vec!["users".into(), "2".into()],
        RedactionKind::ReplaceWith("[REDACTED]".into()),
    )];

    let (result, _) = apply_redaction(&value, &rules);

    // FINDING: Rule ["users", "2"] does NOT match the array element at index 2
    // when the element is an object. The recursive traversal enters the object
    // and checks paths like ["users", "2", "ssn"], never ["users", "2"] itself.
    // You CANNOT target a specific array element for full replacement if it is
    // an object — the traversal always descends into the object first.
    assert_eq!(result["users"][2]["ssn"], "333",
        "FINDING: Cannot replace a specific array element that is an object");
    assert_eq!(result["users"][0]["ssn"], "111",
        "Non-matched array elements preserved");
    assert_eq!(result["users"][4]["ssn"], "555",
        "Non-matched array elements are preserved");
}

// ────────────────────────────────────────────────────────────────────
// BH-09: Canonical view isolation (positive test — correct behavior)
// ────────────────────────────────────────────────────────────────────

#[test]
fn bh09_operator_projection_must_contain_no_raw_sensitive_data() {
    let value = json!({
        "transaction_id": "txn-123",
        "customer": {
            "name": "Alice Johnson",
            "ssn": "123-45-6789",
            "email": "alice@example.com"
        },
        "payment": {
            "card_number": "4111-1111-1111-1111",
            "amount": 99.99,
            "currency": "USD"
        }
    });

    let rules = vec![
        RedactionRule::new(vec!["customer".into(), "ssn".into()], RedactionKind::Remove),
        RedactionRule::new(vec!["customer".into(), "email".into()], RedactionKind::Hash),
        RedactionRule::new(vec!["payment".into(), "card_number".into()], RedactionKind::Remove),
    ];

    let (result, _) = apply_redaction(&value, &rules);
    let serialized = serde_json::to_string(&result).unwrap();

    assert!(!serialized.contains("123-45-6789"), "SSN leaked");
    assert!(!serialized.contains("4111-1111-1111-1111"), "Card number leaked");
    assert!(!serialized.contains("alice@example.com"), "Raw email leaked");

    assert_eq!(result["transaction_id"], "txn-123");
    assert_eq!(result["customer"]["name"], "Alice Johnson");
    assert_eq!(result["payment"]["amount"], 99.99);
    assert_eq!(result["payment"]["currency"], "USD");

    let email_hash = result["customer"]["email"].as_str().unwrap();
    assert!(email_hash.starts_with("HASH"), "Email should be hashed");
}

#[test]
fn bh09_redacted_fields_list_exposes_sensitive_field_names() {
    let value = json!({
        "user": {
            "name": "Alice",
            "ssn": "123-45-6789",
            "secret_answer": "Fluffy",
            "credit_score": 742
        }
    });

    let rules = vec![
        RedactionRule::new(vec!["user".into(), "ssn".into()], RedactionKind::Remove),
        RedactionRule::new(vec!["user".into(), "secret_answer".into()], RedactionKind::Remove),
        RedactionRule::new(vec!["user".into(), "credit_score".into()], RedactionKind::Remove),
    ];

    let (_, redacted_fields) = apply_redaction(&value, &rules);

    assert!(redacted_fields.contains(&vec!["user".into(), "ssn".into()]));
    assert!(redacted_fields.contains(&vec!["user".into(), "secret_answer".into()]));
    assert!(redacted_fields.contains(&vec!["user".into(), "credit_score".into()]));
    // FINDING: redacted_fields reveals which fields are sensitive (schema reconnaissance)
}

// ────────────────────────────────────────────────────────────────────
// BH-10: OperatorProjection metadata leakage
// ────────────────────────────────────────────────────────────────────

#[test]
fn bh10_operator_projection_serialization_includes_workflow_type() {
    let projection = OperatorProjection::new(
        "wf-top-secret-investigation-2024".to_string(),
        "legal_subpoena_response".to_string(),
        json!({"status": "completed"}),
        vec![vec!["ssn".into()]],
    );

    let serialized = serde_json::to_string(&projection).unwrap();

    // FINDING: workflow_type and workflow_id are visible in serialized output
    assert!(
        serialized.contains("legal_subpoena_response"),
        "Workflow type visible in serialized OperatorProjection"
    );
    assert!(
        serialized.contains("top-secret"),
        "Workflow ID may contain sensitive context"
    );
}
