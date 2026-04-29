//! BDD Tests for GDPR Compliant Purging (ADR-025)
//!
//! These tests follow the Given-When-Then BDD format to verify:
//! - Scenario 1: Given redaction policy, When applied, Then sensitive fields replaced with redaction tokens
//! - Scenario 2: Given full purge request, When executed, Then no PII remains in event log
//! - Scenario 3: Given partial redaction, When event replayed, Then non-redacted fields preserved
//!
//! Per ADR-025 §1: Dual-representation privacy model with canonical (encrypted) and
//! operator projection (redacted).

#![allow(clippy::unwrap_used)]

use vo_types::{apply_redaction, RedactionKind, RedactionPolicy, RedactionRule};

const PII_SSN: &str = "123-45-6789";
const PII_EMAIL: &str = "alice@example.com";
const PII_CREDIT_CARD: &str = "4111-1111-1111-1111";
const PII_PHONE: &str = "+1-555-123-4567";
const PII_ADDRESS: &str = "123 Main Street, Anytown, USA";
const NON_PII_NAME: &str = "Alice Smith";
const NON_PII_SALARY: i64 = 75000;

fn gdpr_redaction_policy() -> RedactionPolicy {
    RedactionPolicy::new(
        "gdpr_subject".to_string(),
        vec![
            RedactionRule::new(vec!["subject".into(), "ssn".into()], RedactionKind::Remove),
            RedactionRule::new(
                vec!["subject".into(), "email".into()],
                RedactionKind::ReplaceWith("[EMAIL_REDACTED]".into()),
            ),
            RedactionRule::new(
                vec!["subject".into(), "credit_card".into()],
                RedactionKind::ReplaceWith("[CC_REDACTED]".into()),
            ),
            RedactionRule::new(vec!["subject".into(), "phone".into()], RedactionKind::Hash),
            RedactionRule::new(
                vec!["subject".into(), "address".into()],
                RedactionKind::Remove,
            ),
        ],
    )
}

fn full_purge_policy() -> RedactionPolicy {
    RedactionPolicy::new(
        "full_purge".to_string(),
        vec![
            RedactionRule::new(vec!["subject".into(), "ssn".into()], RedactionKind::Remove),
            RedactionRule::new(
                vec!["subject".into(), "email".into()],
                RedactionKind::Remove,
            ),
            RedactionRule::new(
                vec!["subject".into(), "credit_card".into()],
                RedactionKind::Remove,
            ),
            RedactionRule::new(
                vec!["subject".into(), "phone".into()],
                RedactionKind::Remove,
            ),
            RedactionRule::new(
                vec!["subject".into(), "address".into()],
                RedactionKind::Remove,
            ),
            RedactionRule::new(vec!["subject".into(), "name".into()], RedactionKind::Remove),
        ],
    )
}

fn partial_redaction_policy() -> RedactionPolicy {
    RedactionPolicy::new(
        "partial_redaction".to_string(),
        vec![
            RedactionRule::new(vec!["subject".into(), "ssn".into()], RedactionKind::Remove),
            RedactionRule::new(
                vec!["subject".into(), "email".into()],
                RedactionKind::ReplaceWith("[EMAIL_REDACTED]".into()),
            ),
        ],
    )
}

// ========================================================================
// SCENARIO 1: Given redaction policy, When applied, Then sensitive fields
// replaced with redaction tokens
// ========================================================================

#[test]
fn bdd_given_redaction_policy_when_applied_then_sensitive_fields_replaced_with_tokens() {
    let canonical = serde_json::json!({
        "subject": {
            "name": NON_PII_NAME,
            "ssn": PII_SSN,
            "email": PII_EMAIL,
            "credit_card": PII_CREDIT_CARD,
            "phone": PII_PHONE,
            "address": PII_ADDRESS,
            "salary": NON_PII_SALARY
        }
    });

    let policy = gdpr_redaction_policy();
    let (operator_view, redacted_paths) = apply_redaction(&canonical, &policy.redaction_rules);

    // THEN: Sensitive fields are replaced with redaction tokens
    assert_eq!(
        operator_view["subject"]["name"], NON_PII_NAME,
        "Non-sensitive name must be preserved"
    );
    assert_eq!(
        operator_view["subject"]["salary"], NON_PII_SALARY,
        "Non-sensitive salary must be preserved"
    );

    // SSN removed (null)
    assert_eq!(
        operator_view["subject"]["ssn"],
        serde_json::Value::Null,
        "SSN must be removed (null)"
    );
    assert!(
        !serde_json::to_string(&operator_view)
            .unwrap()
            .contains(PII_SSN),
        "SSN must not appear in serialized output"
    );

    // Email replaced with token
    assert_eq!(
        operator_view["subject"]["email"], "[EMAIL_REDACTED]",
        "Email must be replaced with token"
    );
    assert!(
        !serde_json::to_string(&operator_view)
            .unwrap()
            .contains(PII_EMAIL),
        "Original email must not appear"
    );

    // Credit card replaced with token
    assert_eq!(
        operator_view["subject"]["credit_card"], "[CC_REDACTED]",
        "Credit card must be replaced with token"
    );
    assert!(
        !serde_json::to_string(&operator_view)
            .unwrap()
            .contains(PII_CREDIT_CARD),
        "Original credit card must not appear"
    );

    // Phone hashed (starts with HASH prefix)
    let phone_str = operator_view["subject"]["phone"].as_str().unwrap();
    assert!(
        phone_str.starts_with("HASH"),
        "Phone must be hashed, got: {}",
        phone_str
    );
    assert!(
        !serde_json::to_string(&operator_view)
            .unwrap()
            .contains(PII_PHONE),
        "Original phone must not appear"
    );

    // Address removed
    assert_eq!(
        operator_view["subject"]["address"],
        serde_json::Value::Null,
        "Address must be removed"
    );

    // THEN: redacted_paths tracks all redactions
    assert_eq!(
        redacted_paths.len(),
        5,
        "All 5 sensitive fields must be tracked"
    );
    assert!(
        redacted_paths.contains(&vec!["subject".into(), "ssn".into()]),
        "SSN redaction must be tracked"
    );
    assert!(
        redacted_paths.contains(&vec!["subject".into(), "email".into()]),
        "Email redaction must be tracked"
    );
    assert!(
        redacted_paths.contains(&vec!["subject".into(), "credit_card".into()]),
        "Credit card redaction must be tracked"
    );
    assert!(
        redacted_paths.contains(&vec!["subject".into(), "phone".into()]),
        "Phone redaction must be tracked"
    );
    assert!(
        redacted_paths.contains(&vec!["subject".into(), "address".into()]),
        "Address redaction must be tracked"
    );
}

#[test]
fn bdd_given_redaction_policy_with_replace_with_kind_when_applied_then_replacement_token_used() {
    let canonical = serde_json::json!({
        "data": {
            "api_key": "sk-secret-12345",
            "public_field": "visible"
        }
    });

    let rules = vec![RedactionRule::new(
        vec!["data".into(), "api_key".into()],
        RedactionKind::ReplaceWith("[API_KEY_REDACTED]".into()),
    )];

    let (operator_view, _) = apply_redaction(&canonical, &rules);

    assert_eq!(
        operator_view["data"]["api_key"], "[API_KEY_REDACTED]",
        "api_key must be replaced with specified token"
    );
    assert_eq!(
        operator_view["data"]["public_field"], "visible",
        "public_field must be preserved"
    );
    assert!(
        !serde_json::to_string(&operator_view)
            .unwrap()
            .contains("sk-secret-12345"),
        "Original api_key must not appear"
    );
}

#[test]
fn bdd_given_redaction_policy_with_hash_kind_when_applied_then_field_hashed() {
    let canonical = serde_json::json!({
        "user": {
            "identifier": "unique-user-12345"
        }
    });

    let rules = vec![RedactionRule::new(
        vec!["user".into(), "identifier".into()],
        RedactionKind::Hash,
    )];

    let (operator_view, redacted_paths) = apply_redaction(&canonical, &rules);

    let hashed_value = operator_view["user"]["identifier"].as_str().unwrap();
    assert!(
        hashed_value.starts_with("HASH"),
        "Value must be hashed, got: {}",
        hashed_value
    );
    assert!(
        !serde_json::to_string(&operator_view)
            .unwrap()
            .contains("unique-user-12345"),
        "Original identifier must not appear"
    );
    assert_eq!(redacted_paths.len(), 1);
}

#[test]
fn bdd_given_redaction_policy_with_remove_kind_when_applied_then_field_absent() {
    let canonical = serde_json::json!({
        "record": {
            "temporary_data": "should be gone",
            "permanent_data": "keep this"
        }
    });

    let rules = vec![RedactionRule::new(
        vec!["record".into(), "temporary_data".into()],
        RedactionKind::Remove,
    )];

    let (operator_view, _) = apply_redaction(&canonical, &rules);

    assert_eq!(
        operator_view["record"]["temporary_data"],
        serde_json::Value::Null,
        "temporary_data must be null (removed)"
    );
    assert_eq!(
        operator_view["record"]["permanent_data"], "keep this",
        "permanent_data must be preserved"
    );
}

#[test]
fn bdd_given_redaction_policy_with_replace_with_type_when_applied_then_type_name_used() {
    let canonical = serde_json::json!({
        "user": {
            "age": 42,
            "name": "Bob"
        }
    });

    let rules = vec![RedactionRule::new(
        vec!["user".into(), "age".into()],
        RedactionKind::ReplaceWithType,
    )];

    let (operator_view, _) = apply_redaction(&canonical, &rules);

    let type_str = operator_view["user"]["age"].as_str().unwrap();
    assert!(
        type_str.contains("Value"),
        "Type name should indicate serde_json::Value type, got: {}",
        type_str
    );
    assert_eq!(
        operator_view["user"]["name"], "Bob",
        "name must be preserved"
    );
}

// ========================================================================
// SCENARIO 2: Given full purge request, When executed, Then no PII remains
// in event log
// ========================================================================

#[test]
fn bdd_given_full_purge_request_when_executed_then_no_pii_remains_in_event_log() {
    let event_log = serde_json::json!({
        "subject": {
            "name": NON_PII_NAME,
            "ssn": PII_SSN,
            "email": PII_EMAIL,
            "credit_card": PII_CREDIT_CARD,
            "phone": PII_PHONE,
            "address": PII_ADDRESS
        }
    });

    let purge_policy = full_purge_policy();
    let (purged_log, _) = apply_redaction(&event_log, &purge_policy.redaction_rules);

    let serialized = serde_json::to_string(&purged_log).unwrap();

    // THEN: No PII remains in the purged event log
    assert!(
        !serialized.contains(PII_SSN),
        "SSN must not remain in purged event log"
    );
    assert!(
        !serialized.contains(PII_EMAIL),
        "Email must not remain in purged event log"
    );
    assert!(
        !serialized.contains(PII_CREDIT_CARD),
        "Credit card must not remain in purged event log"
    );
    assert!(
        !serialized.contains(PII_PHONE),
        "Phone must not remain in purged event log"
    );
    assert!(
        !serialized.contains(PII_ADDRESS),
        "Address must not remain in purged event log"
    );
    // Non-PII fields are preserved
    assert_eq!(
        purged_log["subject"]["name"],
        serde_json::Value::Null,
        "Name should be null (removed by purge)"
    );
}

#[test]
fn bdd_given_full_purge_with_multiple_pii_types_when_executed_then_all_pii_eliminated() {
    let canonical = serde_json::json!({
        "subject": {
            "id": "sub_12345",
            "profile": {
                "name": NON_PII_NAME,
                "ssn": PII_SSN,
                "email": PII_EMAIL,
                "phone": PII_PHONE,
                "address": PII_ADDRESS,
                "drivers_license": "DL1234567",
                "tax_id": "XX-XXXXXXX"
            },
            "financials": {
                "salary": NON_PII_SALARY,
                "bank_account": "1234567890",
                "credit_card": PII_CREDIT_CARD
            }
        }
    });

    let full_purge_rules = vec![
        RedactionRule::new(
            vec!["subject".into(), "profile".into(), "ssn".into()],
            RedactionKind::Remove,
        ),
        RedactionRule::new(
            vec!["subject".into(), "profile".into(), "email".into()],
            RedactionKind::Remove,
        ),
        RedactionRule::new(
            vec!["subject".into(), "profile".into(), "phone".into()],
            RedactionKind::Remove,
        ),
        RedactionRule::new(
            vec!["subject".into(), "profile".into(), "address".into()],
            RedactionKind::Remove,
        ),
        RedactionRule::new(
            vec!["subject".into(), "profile".into(), "drivers_license".into()],
            RedactionKind::Remove,
        ),
        RedactionRule::new(
            vec!["subject".into(), "profile".into(), "tax_id".into()],
            RedactionKind::Remove,
        ),
        RedactionRule::new(
            vec!["subject".into(), "financials".into(), "bank_account".into()],
            RedactionKind::Remove,
        ),
        RedactionRule::new(
            vec!["subject".into(), "financials".into(), "credit_card".into()],
            RedactionKind::Remove,
        ),
    ];

    let (purged_view, _) = apply_redaction(&canonical, &full_purge_rules);
    let serialized = serde_json::to_string(&purged_view).unwrap();

    // THEN: All PII types eliminated
    assert!(!serialized.contains(PII_SSN), "SSN must be eliminated");
    assert!(!serialized.contains(PII_EMAIL), "Email must be eliminated");
    assert!(!serialized.contains(PII_PHONE), "Phone must be eliminated");
    assert!(
        !serialized.contains(PII_ADDRESS),
        "Address must be eliminated"
    );
    assert!(
        !serialized.contains("DL1234567"),
        "Driver license must be eliminated"
    );
    assert!(
        !serialized.contains("XX-XXXXXXX"),
        "Tax ID must be eliminated"
    );
    assert!(
        !serialized.contains("1234567890"),
        "Bank account must be eliminated"
    );
    assert!(
        !serialized.contains(PII_CREDIT_CARD),
        "Credit card must be eliminated"
    );

    // Non-PII preserved
    assert_eq!(
        purged_view["subject"]["id"], "sub_12345",
        "Subject ID must be preserved"
    );
    assert_eq!(
        purged_view["subject"]["profile"]["name"], NON_PII_NAME,
        "Name must be preserved"
    );
    assert_eq!(
        purged_view["subject"]["financials"]["salary"], NON_PII_SALARY,
        "Salary must be preserved"
    );
}

#[test]
fn bdd_given_purge_request_on_nested_structure_when_executed_then_deep_pii_removed() {
    let nested_event = serde_json::json!({
        "workflow": {
            "id": "wf_001",
            "type": "customer_onboarding",
            "steps": [
                {
                    "step": 1,
                    "action": "collect_pii",
                    "data": {
                        "customer_name": NON_PII_NAME,
                        "ssn": PII_SSN,
                        "email": PII_EMAIL
                    }
                },
                {
                    "step": 2,
                    "action": "verify_identity",
                    "data": {
                        "verification_code": "123456",
                        "ssn_last_four": "6789"
                    }
                }
            ]
        }
    });

    let purge_rules = vec![
        RedactionRule::new(
            vec![
                "workflow".into(),
                "steps".into(),
                "data".into(),
                "ssn".into(),
            ],
            RedactionKind::Remove,
        ),
        RedactionRule::new(
            vec![
                "workflow".into(),
                "steps".into(),
                "data".into(),
                "email".into(),
            ],
            RedactionKind::Remove,
        ),
        RedactionRule::new(
            vec![
                "workflow".into(),
                "steps".into(),
                "data".into(),
                "verification_code".into(),
            ],
            RedactionKind::Remove,
        ),
        RedactionRule::new(
            vec![
                "workflow".into(),
                "steps".into(),
                "data".into(),
                "ssn_last_four".into(),
            ],
            RedactionKind::Remove,
        ),
    ];

    let (purged, _) = apply_redaction(&nested_event, &purge_rules);
    let serialized = serde_json::to_string(&purged).unwrap();

    // THEN: Deep nested PII removed
    assert!(!serialized.contains(PII_SSN), "Nested SSN must be removed");
    assert!(
        !serialized.contains(PII_EMAIL),
        "Nested email must be removed"
    );
    assert!(
        !serialized.contains("123456"),
        "Verification code must be removed"
    );
    assert!(
        !serialized.contains("6789"),
        "SSN last four must be removed"
    );

    // Non-PII preserved
    assert_eq!(
        purged["workflow"]["id"], "wf_001",
        "Workflow ID must be preserved"
    );
    assert_eq!(
        purged["workflow"]["steps"][0]["data"]["customer_name"], NON_PII_NAME,
        "Customer name must be preserved"
    );
}

// ========================================================================
// SCENARIO 3: Given partial redaction, When event replayed, Then non-redacted
// fields preserved
// ========================================================================

#[test]
fn bdd_given_partial_redaction_when_event_replayed_then_non_redacted_fields_preserved() {
    let canonical = serde_json::json!({
        "subject": {
            "id": "sub_001",
            "name": NON_PII_NAME,
            "email": PII_EMAIL,
            "ssn": PII_SSN,
            "account_status": "active",
            "risk_score": 25
        }
    });

    let policy = partial_redaction_policy();
    let (operator_view, redacted_paths) = apply_redaction(&canonical, &policy.redaction_rules);

    // WHEN: Event is replayed (operator_view is used)

    // THEN: Non-redacted fields are preserved
    assert_eq!(
        operator_view["subject"]["id"], "sub_001",
        "Subject ID must be preserved after replay"
    );
    assert_eq!(
        operator_view["subject"]["name"], NON_PII_NAME,
        "Name must be preserved after replay"
    );
    assert_eq!(
        operator_view["subject"]["account_status"], "active",
        "Account status must be preserved after replay"
    );
    assert_eq!(
        operator_view["subject"]["risk_score"], 25,
        "Risk score must be preserved after replay"
    );

    // THEN: Redacted fields are properly handled
    assert_eq!(
        operator_view["subject"]["ssn"],
        serde_json::Value::Null,
        "SSN must be redacted after replay"
    );
    assert_eq!(
        operator_view["subject"]["email"], "[EMAIL_REDACTED]",
        "Email must be redacted with token after replay"
    );

    // THEN: redacted_paths correctly identifies what was redacted
    assert_eq!(redacted_paths.len(), 2);
    assert!(redacted_paths.contains(&vec!["subject".into(), "ssn".into()]));
    assert!(redacted_paths.contains(&vec!["subject".into(), "email".into()]));
}

#[test]
fn bdd_given_partial_redaction_on_array_elements_when_replayed_then_non_redacted_in_each_element_preserved(
) {
    let canonical = serde_json::json!({
        "transactions": [
            {"tx_id": "tx_001", "amount": 100, "description": "Purchase", "card_last_four": "1234"},
            {"tx_id": "tx_002", "amount": 250, "description": "Refund", "card_last_four": "5678"},
            {"tx_id": "tx_003", "amount": 75, "description": "Purchase", "card_last_four": "9012"}
        ]
    });

    let rules = vec![RedactionRule::new(
        vec!["transactions".into(), "card_last_four".into()],
        RedactionKind::ReplaceWith("[CARD_REDACTED]".into()),
    )];

    let (operator_view, _) = apply_redaction(&canonical, &rules);

    // THEN: Each array element has non-redacted fields preserved
    for i in 0..3 {
        assert_eq!(
            operator_view["transactions"][i]["tx_id"],
            format!("tx_00{}", i + 1),
            "tx_id {} must be preserved",
            i + 1
        );
        assert_eq!(
            operator_view["transactions"][i]["amount"],
            [100, 250, 75][i],
            "amount {} must be preserved",
            i + 1
        );
        assert_eq!(
            operator_view["transactions"][i]["description"],
            ["Purchase", "Refund", "Purchase"][i],
            "description {} must be preserved",
            i + 1
        );
        assert_eq!(
            operator_view["transactions"][i]["card_last_four"],
            "[CARD_REDACTED]",
            "card_last_four {} must be redacted",
            i + 1
        );
    }

    let serialized = serde_json::to_string(&operator_view).unwrap();
    assert!(!serialized.contains("1234"), "Card 1234 must not appear");
    assert!(!serialized.contains("5678"), "Card 5678 must not appear");
    assert!(!serialized.contains("9012"), "Card 9012 must not appear");
}

#[test]
fn bdd_given_partial_redaction_preserves_structure_when_multiple_replays() {
    let original = serde_json::json!({
        "session": {
            "session_id": "sess_abc123",
            "user": {
                "user_id": "usr_001",
                "email": PII_EMAIL,
                "name": NON_PII_NAME,
                "auth_method": "oauth2"
            },
            "events": [
                {"event_id": "e1", "action": "login", "ip": "192.168.1.1"},
                {"event_id": "e2", "action": "view_page", "page": "/dashboard"},
                {"event_id": "e3", "action": "logout", "ip": "192.168.1.1"}
            ]
        }
    });

    let rules = vec![RedactionRule::new(
        vec!["session".into(), "user".into(), "email".into()],
        RedactionKind::Remove,
    )];

    // WHEN: Replayed multiple times
    let (view1, _) = apply_redaction(&original, &rules);
    let (view2, _) = apply_redaction(&view1, &rules);

    // THEN: Structure preserved after multiple replays
    assert_eq!(
        view2["session"]["session_id"], "sess_abc123",
        "Session ID must be preserved"
    );
    assert_eq!(
        view2["session"]["user"]["user_id"], "usr_001",
        "User ID must be preserved"
    );
    assert_eq!(
        view2["session"]["user"]["name"], NON_PII_NAME,
        "Name must be preserved"
    );
    assert_eq!(
        view2["session"]["user"]["auth_method"], "oauth2",
        "Auth method must be preserved"
    );
    assert_eq!(
        view2["session"]["events"].as_array().unwrap().len(),
        3,
        "All events must be preserved"
    );

    // Email consistently absent
    assert!(
        !serde_json::to_string(&view1).unwrap().contains(PII_EMAIL),
        "Email must be absent after first replay"
    );
    assert!(
        !serde_json::to_string(&view2).unwrap().contains(PII_EMAIL),
        "Email must be absent after second replay"
    );
}

#[test]
fn bdd_given_mixed_redaction_kinds_when_replayed_then_each_kind_preserves_correctly() {
    let canonical = serde_json::json!({
        "record": {
            "public_id": "rec_public",
            "secret_key": "sk_abc123",
            "username": "alice",
            "password_hash": "HASHED_PASSWORD",
            "ssn": PII_SSN,
            "email": PII_EMAIL,
            "created_at": "2024-01-01",
            "ip_address": "10.0.0.1"
        }
    });

    let rules = vec![
        RedactionRule::new(
            vec!["record".into(), "secret_key".into()],
            RedactionKind::Remove,
        ),
        RedactionRule::new(
            vec!["record".into(), "password_hash".into()],
            RedactionKind::Hash,
        ),
        RedactionRule::new(vec!["record".into(), "ssn".into()], RedactionKind::Remove),
        RedactionRule::new(
            vec!["record".into(), "email".into()],
            RedactionKind::ReplaceWith("[EMAIL_REDACTED]".into()),
        ),
        RedactionRule::new(
            vec!["record".into(), "ip_address".into()],
            RedactionKind::ReplaceWith("[IP_REDACTED]".into()),
        ),
    ];

    let (result, _) = apply_redaction(&canonical, &rules);

    // THEN: Each redaction kind works correctly
    assert_eq!(
        result["record"]["public_id"], "rec_public",
        "public_id must be preserved"
    );
    assert_eq!(
        result["record"]["username"], "alice",
        "username must be preserved"
    );
    assert_eq!(
        result["record"]["created_at"], "2024-01-01",
        "created_at must be preserved"
    );

    // Remove: field becomes null
    assert_eq!(
        result["record"]["secret_key"],
        serde_json::Value::Null,
        "secret_key must be removed"
    );
    assert_eq!(
        result["record"]["ssn"],
        serde_json::Value::Null,
        "ssn must be removed"
    );

    // Hash: field becomes hashed
    let pwd_hash = result["record"]["password_hash"].as_str().unwrap();
    assert!(pwd_hash.starts_with("HASH"), "password_hash must be hashed");
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains("HASHED_PASSWORD"),
        "Original password hash must not appear"
    );

    // ReplaceWith: field becomes token
    assert_eq!(
        result["record"]["email"], "[EMAIL_REDACTED]",
        "email must be replaced"
    );
    assert_eq!(
        result["record"]["ip_address"], "[IP_REDACTED]",
        "ip_address must be replaced"
    );

    let serialized = serde_json::to_string(&result).unwrap();
    assert!(!serialized.contains(PII_SSN), "SSN must not appear");
    assert!(!serialized.contains(PII_EMAIL), "Email must not appear");
    assert!(
        !serialized.contains("sk_abc123"),
        "Secret key must not appear"
    );
    assert!(
        !serialized.contains("10.0.0.1"),
        "IP address must not appear"
    );
}

// ========================================================================
// EDGE CASES: Partial redaction with empty/missing fields
// ========================================================================

#[test]
fn bdd_given_partial_redaction_with_null_fields_when_replayed_then_null_handled_gracefully() {
    let canonical = serde_json::json!({
        "subject": {
            "name": NON_PII_NAME,
            "email": null,
            "ssn": PII_SSN
        }
    });

    let rules = vec![
        RedactionRule::new(
            vec!["subject".into(), "email".into()],
            RedactionKind::Remove,
        ),
        RedactionRule::new(vec!["subject".into(), "ssn".into()], RedactionKind::Remove),
    ];

    let (result, _) = apply_redaction(&canonical, &rules);

    assert_eq!(
        result["subject"]["name"], NON_PII_NAME,
        "Name must be preserved even when other fields are null"
    );
    assert_eq!(
        result["subject"]["email"],
        serde_json::Value::Null,
        "Null email becomes null (already absent)"
    );
    assert_eq!(
        result["subject"]["ssn"],
        serde_json::Value::Null,
        "SSN must be removed"
    );
}

#[test]
fn bdd_given_partial_redaction_with_array_of_objects_when_replayed_then_each_object_preserved_correctly(
) {
    let canonical = serde_json::json!({
        "users": [
            {"id": 1, "name": "User One", "email": "user1@example.com", "ssn": "111-11-1111"},
            {"id": 2, "name": "User Two", "email": "user2@example.com", "ssn": "222-22-2222"},
            {"id": 3, "name": "User Three", "email": "user3@example.com", "ssn": "333-33-3333"}
        ]
    });

    let rules = vec![
        RedactionRule::new(vec!["users".into(), "email".into()], RedactionKind::Remove),
        RedactionRule::new(vec!["users".into(), "ssn".into()], RedactionKind::Remove),
    ];

    let (result, _) = apply_redaction(&canonical, &rules);
    let serialized = serde_json::to_string(&result).unwrap();

    // THEN: Each user's non-PII data preserved
    for i in 0..3 {
        assert_eq!(
            result["users"][i]["id"],
            i as i64 + 1,
            "User {} id must be preserved",
            i + 1
        );
        assert_eq!(
            result["users"][i]["name"],
            format!("User {}", ["One", "Two", "Three"][i]),
            "User {} name must be preserved",
            i + 1
        );
    }

    // THEN: All PII removed
    assert!(
        !serialized.contains("@example.com"),
        "Emails must not appear"
    );
    assert!(!serialized.contains("111-11-1111"), "SSN 1 must not appear");
    assert!(!serialized.contains("222-22-2222"), "SSN 2 must not appear");
    assert!(!serialized.contains("333-33-3333"), "SSN 3 must not appear");
}
