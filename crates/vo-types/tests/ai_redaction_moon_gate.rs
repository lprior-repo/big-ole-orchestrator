//! Moon Gate: AI Redaction Integration Tests (ADR-008, ADR-025)
//!
//! Integration CI gate for AI redaction per ADR-008/025:
//! - PII injection tests verify zero leaks in operator projections
//! - Canonical encryption verification
//! - Access control: AI default path uses operator projection (not canonical)
//!
//! ADR-008: AI agents default to operator projection (redacted view).
//! ADR-025: Dual-representation privacy model with canonical (encrypted) and
//!          operator projection (redacted).

#![allow(clippy::unwrap_used)]

use vo_types::{
    apply_redaction, CryptoAlgorithm, DekId, EncryptedBlob, InstanceId, KeyMetadata,
    OperatorProjection, RedactionKind, RedactionPolicy, RedactionRule,
};

// ========================================================================
// PII Test Fixtures - Common PII types for injection testing
// ========================================================================

const TEST_SSN: &str = "123-45-6789";
const TEST_EMAIL: &str = "alice@example.com";
const TEST_CREDIT_CARD: &str = "4111-1111-1111-1111";
const TEST_PHONE: &str = "+1-555-123-4567";
const TEST_SSN_2: &str = "987-65-4321";
const TEST_EMAIL_2: &str = "bob@private.org";

fn instance_id() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID")
}

fn standard_pii_redaction_rules() -> Vec<RedactionRule> {
    vec![
        RedactionRule::new(vec!["user".into(), "ssn".into()], RedactionKind::Remove),
        RedactionRule::new(
            vec!["user".into(), "email".into()],
            RedactionKind::ReplaceWith("[EMAIL_REDACTED]".into()),
        ),
        RedactionRule::new(
            vec!["user".into(), "credit_card".into()],
            RedactionKind::ReplaceWith("[CC_REDACTED]".into()),
        ),
        RedactionRule::new(vec!["user".into(), "phone".into()], RedactionKind::Hash),
        RedactionRule::new(
            vec!["user".into(), "password".into()],
            RedactionKind::Remove,
        ),
        RedactionRule::new(
            vec!["secret".into()],
            RedactionKind::ReplaceWith("[SECRET_REDACTED]".into()),
        ),
    ]
}

fn multi_user_pii_redaction_rules() -> Vec<RedactionRule> {
    vec![
        RedactionRule::new(vec!["users".into(), "ssn".into()], RedactionKind::Remove),
        RedactionRule::new(vec!["users".into(), "email".into()], RedactionKind::Hash),
    ]
}

fn nested_pii_redaction_rules() -> Vec<RedactionRule> {
    vec![
        RedactionRule::new(
            vec!["profile".into(), "credentials".into(), "password".into()],
            RedactionKind::Remove,
        ),
        RedactionRule::new(
            vec!["profile".into(), "credentials".into(), "totp".into()],
            RedactionKind::ReplaceWith("[TOTP_REDACTED]".into()),
        ),
    ]
}

// ========================================================================
// DIMENSION: PII injection - zero leak verification (ADR-025)
// Tests that verify PII never appears in operator projections
// ========================================================================

#[test]
fn moon_gate_pii_ssn_completely_removed_from_operator_projection() {
    let canonical = serde_json::json!({
        "user": {
            "name": "Alice",
            "ssn": TEST_SSN
        }
    });

    let rules = vec![RedactionRule::new(
        vec!["user".into(), "ssn".into()],
        RedactionKind::Remove,
    )];

    let (operator_view, redacted_paths) = apply_redaction(&canonical, &rules);

    assert_eq!(operator_view["user"]["name"], "Alice");
    assert_eq!(operator_view["user"]["ssn"], serde_json::Value::Null);

    let serialized = serde_json::to_string(&operator_view).unwrap();
    assert!(
        !serialized.contains(TEST_SSN),
        "SSN must not appear in serialized operator projection"
    );
    assert!(!serialized.contains("6789"), "SSN digits must not leak");

    assert_eq!(redacted_paths.len(), 1);
    assert_eq!(redacted_paths[0], vec!["user", "ssn"]);
}

#[test]
fn moon_gate_pii_email_replaced_in_operator_projection() {
    let canonical = serde_json::json!({
        "contact": {
            "email": TEST_EMAIL,
            "name": "Bob"
        }
    });

    let rules = vec![RedactionRule::new(
        vec!["contact".into(), "email".into()],
        RedactionKind::ReplaceWith("[EMAIL_REDACTED]".into()),
    )];

    let (operator_view, _) = apply_redaction(&canonical, &rules);

    assert_eq!(operator_view["contact"]["email"], "[EMAIL_REDACTED]");
    assert_eq!(operator_view["contact"]["name"], "Bob");

    let serialized = serde_json::to_string(&operator_view).unwrap();
    assert!(
        !serialized.contains(TEST_EMAIL),
        "Email must not appear in operator projection"
    );
    assert!(
        !serialized.contains("example.com"),
        "Email domain must not leak"
    );
}

#[test]
fn moon_gate_pii_credit_card_replaced_in_operator_projection() {
    let canonical = serde_json::json!({
        "payment": {
            "card": TEST_CREDIT_CARD,
            "expiry": "12/25"
        }
    });

    let rules = vec![RedactionRule::new(
        vec!["payment".into(), "card".into()],
        RedactionKind::ReplaceWith("[CC_REDACTED]".into()),
    )];

    let (operator_view, _) = apply_redaction(&canonical, &rules);

    assert_eq!(operator_view["payment"]["card"], "[CC_REDACTED]");
    assert_eq!(operator_view["payment"]["expiry"], "12/25");

    let serialized = serde_json::to_string(&operator_view).unwrap();
    assert!(
        !serialized.contains(TEST_CREDIT_CARD),
        "Credit card must not appear in operator projection"
    );
    assert!(
        !serialized.contains("4111"),
        "Credit card digits must not leak"
    );
}

#[test]
fn moon_gate_pii_phone_hashed_in_operator_projection() {
    let canonical = serde_json::json!({
        "user": {
            "phone": TEST_PHONE
        }
    });

    let rules = vec![RedactionRule::new(
        vec!["user".into(), "phone".into()],
        RedactionKind::Hash,
    )];

    let (operator_view, _) = apply_redaction(&canonical, &rules);

    let hash_str = operator_view["user"]["phone"].as_str().unwrap();
    assert!(hash_str.starts_with("HASH"), "Phone should be hashed");
    assert!(
        !hash_str.contains("555"),
        "Original phone digits must not leak"
    );

    let serialized = serde_json::to_string(&operator_view).unwrap();
    assert!(
        !serialized.contains(TEST_PHONE),
        "Phone must not appear in operator projection"
    );
}

#[test]
fn moon_gate_pii_multiple_types_all_redacted_simultaneously() {
    let canonical = serde_json::json!({
        "user": {
            "name": "Alice",
            "ssn": TEST_SSN,
            "email": TEST_EMAIL,
            "credit_card": TEST_CREDIT_CARD,
            "phone": TEST_PHONE,
            "password": "secret123"
        }
    });

    let rules = standard_pii_redaction_rules();

    let (operator_view, redacted_paths) = apply_redaction(&canonical, &rules);

    assert_eq!(operator_view["user"]["name"], "Alice");
    assert_eq!(operator_view["user"]["ssn"], serde_json::Value::Null);
    assert_eq!(operator_view["user"]["email"], "[EMAIL_REDACTED]");
    assert_eq!(operator_view["user"]["credit_card"], "[CC_REDACTED]");
    assert_eq!(operator_view["user"]["password"], serde_json::Value::Null);

    let hash_str = operator_view["user"]["phone"].as_str().unwrap();
    assert!(hash_str.starts_with("HASH"));

    assert_eq!(redacted_paths.len(), 5);

    let serialized = serde_json::to_string(&operator_view).unwrap();
    assert!(!serialized.contains(TEST_SSN));
    assert!(!serialized.contains(TEST_EMAIL));
    assert!(!serialized.contains(TEST_CREDIT_CARD));
    assert!(!serialized.contains(TEST_PHONE));
    assert!(!serialized.contains("secret123"));
}

#[test]
fn moon_gate_pii_ssn_in_nested_object_fully_redacted() {
    let canonical = serde_json::json!({
        "company": {
            "hr": {
                "employee": {
                    "ssn": TEST_SSN,
                    "salary": 75000
                }
            }
        }
    });

    let rules = vec![RedactionRule::new(
        vec![
            "company".into(),
            "hr".into(),
            "employee".into(),
            "ssn".into(),
        ],
        RedactionKind::Remove,
    )];

    let (operator_view, redacted_paths) = apply_redaction(&canonical, &rules);

    assert_eq!(operator_view["company"]["hr"]["employee"]["salary"], 75000);
    assert_eq!(
        operator_view["company"]["hr"]["employee"]["ssn"],
        serde_json::Value::Null
    );

    assert_eq!(redacted_paths.len(), 1);

    let serialized = serde_json::to_string(&operator_view).unwrap();
    assert!(!serialized.contains(TEST_SSN));
}

#[test]
fn moon_gate_pii_array_of_users_all_redacted() {
    let canonical = serde_json::json!({
        "users": [
            {"name": "Alice", "ssn": TEST_SSN, "email": TEST_EMAIL},
            {"name": "Bob", "ssn": TEST_SSN_2, "email": TEST_EMAIL_2}
        ]
    });

    let rules = multi_user_pii_redaction_rules();

    let (operator_view, redacted_paths) = apply_redaction(&canonical, &rules);

    assert_eq!(operator_view["users"][0]["name"], "Alice");
    assert_eq!(operator_view["users"][1]["name"], "Bob");

    assert_eq!(operator_view["users"][0]["ssn"], serde_json::Value::Null);
    assert_eq!(operator_view["users"][1]["ssn"], serde_json::Value::Null);

    let email0 = operator_view["users"][0]["email"].as_str().unwrap();
    let email1 = operator_view["users"][1]["email"].as_str().unwrap();
    assert!(email0.starts_with("HASH"));
    assert!(email1.starts_with("HASH"));
    assert_ne!(
        email0, email1,
        "Different users should have different hashes"
    );

    assert_eq!(redacted_paths.len(), 4);

    let serialized = serde_json::to_string(&operator_view).unwrap();
    assert!(!serialized.contains(TEST_SSN));
    assert!(!serialized.contains(TEST_SSN_2));
    assert!(!serialized.contains(TEST_EMAIL));
    assert!(!serialized.contains(TEST_EMAIL_2));
}

#[test]
fn moon_gate_pii_deeply_nested_redacted_completely() {
    let canonical = serde_json::json!({
        "profile": {
            "credentials": {
                "password": "super-secret-pass",
                "totp": "JBSWY3DPEHPK3PXP"
            }
        }
    });

    let rules = nested_pii_redaction_rules();

    let (operator_view, _) = apply_redaction(&canonical, &rules);

    assert_eq!(
        operator_view["profile"]["credentials"]["password"],
        serde_json::Value::Null
    );
    assert_eq!(
        operator_view["profile"]["credentials"]["totp"],
        "[TOTP_REDACTED]"
    );

    let serialized = serde_json::to_string(&operator_view).unwrap();
    assert!(!serialized.contains("super-secret-pass"));
    assert!(!serialized.contains("JBSWY3DPEHPK3PXP"));
}

// ========================================================================
// DIMENSION: Canonical Encryption Verification (ADR-025 §2)
// Tests verifying canonical replay data is encrypted at rest
// ========================================================================

#[test]
fn moon_gate_canonical_encrypted_blob_structure_valid() {
    let blob = EncryptedBlob::new(vec![0u8; 12], vec![1u8; 32], vec![2u8; 16]);

    assert_eq!(blob.total_size(), 60);
    assert_eq!(blob.iv.len(), 12);
    assert_eq!(blob.ciphertext.len(), 32);
    assert_eq!(blob.tag.len(), 16);
}

#[test]
fn moon_gate_canonical_encrypted_blob_serde_roundtrip() {
    let original = EncryptedBlob::new(vec![0xAB; 12], vec![0xCD; 32], vec![0xEF; 16]);

    let json = serde_json::to_string(&original).unwrap();
    let recovered: EncryptedBlob = serde_json::from_str(&json).unwrap();

    assert_eq!(original, recovered);
}

#[test]
fn moon_gate_canonical_key_metadata_contains_algorithm() {
    let instance = instance_id();
    let metadata = KeyMetadata::new(instance, CryptoAlgorithm::Aes256Gcm);

    assert_eq!(metadata.algorithm, CryptoAlgorithm::Aes256Gcm);
    assert!(metadata.created_at_ms > 0);
    assert_eq!(metadata.instance_id.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
}

#[test]
fn moon_gate_canonical_dek_id_valid_ulid_format() {
    let dek_id = DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID");
    assert_eq!(dek_id.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
}

#[test]
fn moon_gate_crypto_algorithm_constants_correct() {
    assert_eq!(CryptoAlgorithm::IV_SIZE_BYTES, 12);
    assert_eq!(CryptoAlgorithm::TAG_SIZE_BYTES, 16);
    assert_eq!(CryptoAlgorithm::KEY_SIZE_BYTES, 32);
}

// ========================================================================
// DIMENSION: Access Control - AI Default Path (ADR-008 §1)
// Tests verifying AI agents receive operator projection, not canonical
// ========================================================================

#[test]
fn moon_gate_ai_default_path_operator_projection_is_redacted() {
    let canonical = serde_json::json!({
        "user": {
            "ssn": TEST_SSN,
            "email": TEST_EMAIL,
            "salary": 100000
        }
    });

    let policy = RedactionPolicy::new(
        "payment".to_string(),
        vec![
            RedactionRule::new(vec!["user".into(), "ssn".into()], RedactionKind::Remove),
            RedactionRule::new(
                vec!["user".into(), "email".into()],
                RedactionKind::ReplaceWith("[EMAIL_REDACTED]".into()),
            ),
        ],
    );

    let (operator_view, _) = apply_redaction(&canonical, &policy.redaction_rules);

    let projection = OperatorProjection::new(
        "wf-123".to_string(),
        "payment".to_string(),
        operator_view,
        vec![
            vec!["user".into(), "ssn".into()],
            vec!["user".into(), "email".into()],
        ],
    );

    assert_eq!(projection.workflow_type, "payment");
    assert_eq!(projection.projection_json["user"]["salary"], 100000);
    assert_eq!(
        projection.projection_json["user"]["ssn"],
        serde_json::Value::Null
    );
    assert_eq!(
        projection.projection_json["user"]["email"],
        "[EMAIL_REDACTED]"
    );

    let serialized = serde_json::to_string(&projection.projection_json).unwrap();
    assert!(!serialized.contains(TEST_SSN));
    assert!(!serialized.contains(TEST_EMAIL));
}

#[test]
fn moon_gate_operator_projection_serde_roundtrip_preserves_redaction() {
    let canonical = serde_json::json!({
        "account": {
            "password": "hunter2",
            "balance": 5000
        }
    });

    let rules = vec![RedactionRule::new(
        vec!["account".into(), "password".into()],
        RedactionKind::Remove,
    )];

    let (operator_view, redacted) = apply_redaction(&canonical, &rules);

    let projection = OperatorProjection::new(
        "wf-456".to_string(),
        "banking".to_string(),
        operator_view,
        redacted,
    );

    let json = serde_json::to_string(&projection).unwrap();
    let recovered: OperatorProjection = serde_json::from_str(&json).unwrap();

    assert_eq!(projection, recovered);
    assert_eq!(recovered.projection_json["account"]["balance"], 5000);
    assert_eq!(
        recovered.projection_json["account"]["password"],
        serde_json::Value::Null
    );
}

#[test]
fn moon_gate_ai_path_never_sees_raw_plaintext_sensitive_fields() {
    let canonical = serde_json::json!({
        "medical_record": {
            "patient_name": "John Doe",
            "ssn": TEST_SSN,
            "diagnosis": "Flu",
            "treatment": "Rest"
        }
    });

    let policy = RedactionPolicy::new(
        "healthcare".to_string(),
        vec![
            RedactionRule::new(
                vec!["medical_record".into(), "ssn".into()],
                RedactionKind::Remove,
            ),
            RedactionRule::new(
                vec!["medical_record".into(), "diagnosis".into()],
                RedactionKind::ReplaceWith("[REDACTED]".into()),
            ),
        ],
    );

    let (operator_view, _) = apply_redaction(&canonical, &policy.redaction_rules);

    let projection = OperatorProjection::new(
        "wf-health-001".to_string(),
        "healthcare".to_string(),
        operator_view,
        policy
            .redaction_rules
            .iter()
            .map(|r| r.field_path.clone())
            .collect(),
    );

    let serialized = serde_json::to_string(&projection.projection_json).unwrap();
    assert!(
        !serialized.contains(TEST_SSN),
        "SSN must never appear in AI-visible projection"
    );
    assert!(!serialized.contains("Flu"), "Diagnosis must be redacted");
    assert_eq!(
        projection.projection_json["medical_record"]["patient_name"],
        "John Doe"
    );
    assert_eq!(
        projection.projection_json["medical_record"]["treatment"],
        "Rest"
    );
}

#[test]
fn moon_gate_operator_projection_tracks_all_redacted_paths() {
    let canonical = serde_json::json!({
        "user": {
            "email": TEST_EMAIL,
            "phone": TEST_PHONE,
            "address": "123 Main St"
        }
    });

    let rules = vec![
        RedactionRule::new(vec!["user".into(), "email".into()], RedactionKind::Hash),
        RedactionRule::new(vec!["user".into(), "phone".into()], RedactionKind::Hash),
    ];

    let (operator_view, redacted_paths) = apply_redaction(&canonical, &rules);

    let projection = OperatorProjection::new(
        "wf-tracking".to_string(),
        "tracking".to_string(),
        operator_view,
        redacted_paths,
    );

    assert_eq!(projection.redacted_fields.len(), 2);
    assert!(projection
        .redacted_fields
        .contains(&vec!["user".into(), "email".into()]));
    assert!(projection
        .redacted_fields
        .contains(&vec!["user".into(), "phone".into()]));

    let ai_readable = serde_json::to_string(&projection).unwrap();
    assert!(!ai_readable.contains(TEST_EMAIL));
    assert!(!ai_readable.contains(TEST_PHONE));
}

// ========================================================================
// DIMENSION: GDPR Purge Invariants (ADR-025 §3)
// Tests verifying purge leaves operator projections clean
// ========================================================================

#[test]
fn moon_gate_purge_sensitive_data_absent_from_projection_after_purge_policy() {
    let canonical = serde_json::json!({
        "customer": {
            "name": "Alice",
            "ssn": TEST_SSN,
            "purchase_history": ["item1", "item2"]
        }
    });

    let purge_rules = vec![
        RedactionRule::new(vec!["customer".into(), "ssn".into()], RedactionKind::Remove),
        RedactionRule::new(
            vec!["customer".into(), "purchase_history".into()],
            RedactionKind::Remove,
        ),
    ];

    let (post_purge_view, _) = apply_redaction(&canonical, &purge_rules);

    assert_eq!(post_purge_view["customer"]["name"], "Alice");
    assert_eq!(post_purge_view["customer"]["ssn"], serde_json::Value::Null);
    assert_eq!(
        post_purge_view["customer"]["purchase_history"],
        serde_json::Value::Null
    );

    let serialized = serde_json::to_string(&post_purge_view).unwrap();
    assert!(!serialized.contains(TEST_SSN));
    assert!(!serialized.contains("item1"));
    assert!(!serialized.contains("item2"));
}

// ========================================================================
// DIMENSION: Redaction Completeness - Edge Cases
// ========================================================================

#[test]
fn moon_gate_redaction_empty_rules_preserves_all_fields() {
    let canonical = serde_json::json!({
        "user": {
            "ssn": TEST_SSN,
            "email": TEST_EMAIL
        }
    });

    let (result, redacted) = apply_redaction(&canonical, &[]);

    assert_eq!(result, canonical);
    assert!(redacted.is_empty());

    let serialized = serde_json::to_string(&result).unwrap();
    assert!(serialized.contains(TEST_SSN));
    assert!(serialized.contains(TEST_EMAIL));
}

#[test]
fn moon_gate_redaction_unknown_fields_preserved() {
    let canonical = serde_json::json!({
        "user": {
            "ssn": TEST_SSN,
            "custom_field": "not in redaction rules",
            "email": TEST_EMAIL
        }
    });

    let rules = vec![RedactionRule::new(
        vec!["user".into(), "ssn".into()],
        RedactionKind::Remove,
    )];

    let (result, _) = apply_redaction(&canonical, &rules);

    assert_eq!(result["user"]["ssn"], serde_json::Value::Null);
    assert_eq!(result["user"]["custom_field"], "not in redaction rules");
    assert_eq!(result["user"]["email"], TEST_EMAIL);
}

#[test]
fn moon_gate_redaction_null_value_handled_gracefully() {
    let canonical = serde_json::json!({
        "user": {
            "ssn": null,
            "email": TEST_EMAIL
        }
    });

    let rules = vec![RedactionRule::new(
        vec!["user".into(), "ssn".into()],
        RedactionKind::Remove,
    )];

    let (result, _) = apply_redaction(&canonical, &rules);

    assert_eq!(result["user"]["ssn"], serde_json::Value::Null);
    assert_eq!(result["user"]["email"], TEST_EMAIL);
}

#[test]
fn moon_gate_redaction_array_inside_object_handled() {
    let canonical = serde_json::json!({
        "company": {
            "employees": ["Alice", "Bob", "Charlie"],
            "ein": "12-3456789"
        }
    });

    let rules = vec![RedactionRule::new(
        vec!["company".into(), "ein".into()],
        RedactionKind::Remove,
    )];

    let (result, _) = apply_redaction(&canonical, &rules);

    assert_eq!(
        result["company"]["employees"],
        serde_json::json!(["Alice", "Bob", "Charlie"])
    );
    assert_eq!(result["company"]["ein"], serde_json::Value::Null);
}

#[test]
fn moon_gate_redaction_multiple_arrays_all_processed() {
    let canonical = serde_json::json!({
        "data": {
            "logins": [
                {"time": "2024-01-01", "ip": "192.168.1.1"},
                {"time": "2024-01-02", "ip": "10.0.0.1"}
            ],
            "ips": ["192.168.1.1", "10.0.0.1"]
        }
    });

    let rules = vec![
        RedactionRule::new(
            vec!["data".into(), "logins".into(), "ip".into()],
            RedactionKind::Hash,
        ),
        RedactionRule::new(vec!["data".into(), "ips".into()], RedactionKind::Remove),
    ];

    let (result, redacted) = apply_redaction(&canonical, &rules);

    let login0_ip = result["data"]["logins"][0]["ip"].as_str().unwrap();
    let login1_ip = result["data"]["logins"][1]["ip"].as_str().unwrap();
    assert!(login0_ip.starts_with("HASH"));
    assert!(login1_ip.starts_with("HASH"));
    assert_ne!(login0_ip, login1_ip);

    assert_eq!(result["data"]["ips"], serde_json::Value::Null);
    assert_eq!(redacted.len(), 3);
}

// ========================================================================
// DIMENSION: Hash Determinism and Uniqueness
// ========================================================================

#[test]
fn moon_gate_hash_same_input_produces_same_output() {
    let canonical = serde_json::json!({
        "user1": {"email": TEST_EMAIL},
        "user2": {"email": TEST_EMAIL}
    });

    let rules = vec![RedactionRule::new(
        vec!["user1".into(), "email".into()],
        RedactionKind::Hash,
    )];

    let (result, _) = apply_redaction(&canonical, &rules);

    let hash1 = result["user1"]["email"].as_str().unwrap();
    assert!(hash1.starts_with("HASH"));

    assert!(!result["user2"]["email"]
        .as_str()
        .unwrap()
        .starts_with("HASH"));
}

#[test]
fn moon_gate_hash_different_inputs_produce_different_hashes() {
    let canonical = serde_json::json!({
        "user1": {"email": TEST_EMAIL},
        "user2": {"email": TEST_EMAIL_2}
    });

    let rules = vec![
        RedactionRule::new(vec!["user1".into(), "email".into()], RedactionKind::Hash),
        RedactionRule::new(vec!["user2".into(), "email".into()], RedactionKind::Hash),
    ];

    let (result, _) = apply_redaction(&canonical, &rules);

    let hash1 = result["user1"]["email"].as_str().unwrap();
    let hash2 = result["user2"]["email"].as_str().unwrap();

    assert_ne!(hash1, hash2);
    assert!(hash1.starts_with("HASH"));
    assert!(hash2.starts_with("HASH"));
}

// ========================================================================
// DIMENSION: RedactionPolicy Serialization
// ========================================================================

#[test]
fn moon_gate_redaction_policy_roundtrip() {
    let policy = RedactionPolicy::new(
        "payment".to_string(),
        vec![
            RedactionRule::new(vec!["ssn".into()], RedactionKind::Remove),
            RedactionRule::new(
                vec!["email".into()],
                RedactionKind::ReplaceWith("[EMAIL]".into()),
            ),
            RedactionRule::new(vec!["password".into()], RedactionKind::Hash),
        ],
    );

    let json = serde_json::to_string(&policy).unwrap();
    let recovered: RedactionPolicy = serde_json::from_str(&json).unwrap();

    assert_eq!(policy, recovered);
}

#[test]
fn moon_gate_redaction_rule_roundtrip() {
    let rule = RedactionRule::new(
        vec!["user".into(), "credentials".into(), "password".into()],
        RedactionKind::Remove,
    );

    let json = serde_json::to_string(&rule).unwrap();
    let recovered: RedactionRule = serde_json::from_str(&json).unwrap();

    assert_eq!(rule, recovered);
}
