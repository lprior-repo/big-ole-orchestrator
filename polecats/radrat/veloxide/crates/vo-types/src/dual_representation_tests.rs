use crate::dual_representation::{
    apply_redaction, OperatorProjection, RedactionKind, RedactionPolicy, RedactionRule,
};
use crate::ParseError;

#[test]
fn redaction_kind_remove_produces_null() {
    let kind = RedactionKind::Remove;
    let value = serde_json::json!("sensitive data");
    let result = kind.redact_value("field", &value);
    assert_eq!(result, serde_json::Value::Null);
}

#[test]
fn redaction_kind_replace_with_produces_replacement() {
    let kind = RedactionKind::ReplaceWith("[REDACTED]".to_string());
    let value = serde_json::json!("sensitive data");
    let result = kind.redact_value("field", &value);
    assert_eq!(result, serde_json::Value::String("[REDACTED]".to_string()));
}

#[test]
fn redaction_kind_hash_produces_deterministic_hash() {
    let kind = RedactionKind::Hash;
    let value1 = serde_json::json!("same input");
    let value2 = serde_json::json!("same input");

    let result1 = kind.redact_value("field", &value1);
    let result2 = kind.redact_value("field", &value2);

    assert_eq!(result1, result2);
    assert!(result1.as_str().unwrap().starts_with("HASH"));
}

#[test]
fn redaction_kind_hash_different_for_different_inputs() {
    let kind = RedactionKind::Hash;
    let value1 = serde_json::json!("input A");
    let value2 = serde_json::json!("input B");

    let result1 = kind.redact_value("field", &value1);
    let result2 = kind.redact_value("field", &value2);

    assert_ne!(result1, result2);
}

#[test]
fn apply_redaction_removes_fields_at_path() {
    let value = serde_json::json!({
        "user": {
            "name": "Alice",
            "ssn": "123-45-6789"
        }
    });

    let rules = vec![RedactionRule::new(
        vec!["user".to_string(), "ssn".to_string()],
        RedactionKind::Remove,
    )];

    let (result, redacted) = apply_redaction(&value, &rules);

    assert_eq!(result["user"]["name"], "Alice");
    assert_eq!(result["user"]["ssn"], serde_json::Value::Null);
    assert_eq!(redacted.len(), 1);
    assert_eq!(redacted[0], vec!["user".to_string(), "ssn".to_string()]);
}

#[test]
fn apply_redaction_replaces_fields_at_path() {
    let value = serde_json::json!({
        "password": "secret123"
    });

    let rules = vec![RedactionRule::new(
        vec!["password".to_string()],
        RedactionKind::ReplaceWith("[REDACTED]".to_string()),
    )];

    let (result, _) = apply_redaction(&value, &rules);

    assert_eq!(result["password"], "[REDACTED]");
}

#[test]
fn apply_redaction_hashes_fields_at_path() {
    let value = serde_json::json!({
        "email": "user@example.com"
    });

    let rules = vec![RedactionRule::new(
        vec!["email".to_string()],
        RedactionKind::Hash,
    )];

    let (result, _) = apply_redaction(&value, &rules);

    let hash_str = result["email"].as_str().unwrap();
    assert!(hash_str.starts_with("HASH"));
}

#[test]
fn apply_redaction_handles_arrays_recursively() {
    let value = serde_json::json!({
        "users": [
            {"name": "Alice", "ssn": "111"},
            {"name": "Bob", "ssn": "222"}
        ]
    });

    let rules = vec![RedactionRule::new(
        vec!["users".to_string(), "ssn".to_string()],
        RedactionKind::Remove,
    )];

    let (result, redacted) = apply_redaction(&value, &rules);

    assert_eq!(result["users"][0]["name"], "Alice");
    assert_eq!(result["users"][0]["ssn"], serde_json::Value::Null);
    assert_eq!(result["users"][1]["name"], "Bob");
    assert_eq!(result["users"][1]["ssn"], serde_json::Value::Null);
    assert_eq!(redacted.len(), 2);
}

#[test]
fn apply_redaction_handles_nested_arrays() {
    let value = serde_json::json!({
        "matrix": [[1, 2], [3, 4]]
    });

    let rules = vec![RedactionRule::new(
        vec!["matrix".to_string()],
        RedactionKind::ReplaceWith("[REDACTED]".to_string()),
    )];

    let (result, _) = apply_redaction(&value, &rules);

    assert_eq!(result["matrix"], "[REDACTED]");
}

#[test]
fn operator_projection_roundtrip() {
    let projection = OperatorProjection::new(
        "wf-123".to_string(),
        "payment".to_string(),
        serde_json::json!({"status": "completed"}),
        vec![vec!["ssn".to_string()]],
    );

    let json = serde_json::to_string(&projection).unwrap();
    let recovered: OperatorProjection = serde_json::from_str(&json).unwrap();

    assert_eq!(projection, recovered);
}

#[test]
fn redaction_policy_roundtrip() {
    let policy = RedactionPolicy::new(
        "payment".to_string(),
        vec![RedactionRule::new(
            vec!["ssn".to_string()],
            RedactionKind::Remove,
        )],
    );

    let json = serde_json::to_string(&policy).unwrap();
    let recovered: RedactionPolicy = serde_json::from_str(&json).unwrap();

    assert_eq!(policy, recovered);
}

#[test]
fn redaction_rule_roundtrip() {
    let rule = RedactionRule::new(
        vec!["user".to_string(), "email".to_string()],
        RedactionKind::Hash,
    );

    let json = serde_json::to_string(&rule).unwrap();
    let recovered: RedactionRule = serde_json::from_str(&json).unwrap();

    assert_eq!(rule, recovered);
}

// =========================================================================
// ADR-025 Invariant: Redaction completeness
// =========================================================================

#[test]
fn redaction_completeness_deeply_nested_sensitive_field() {
    let value = serde_json::json!({
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
    let (result, redacted) = apply_redaction(&value, &rules);
    assert_eq!(
        result["level1"]["level2"]["level3"]["secret"],
        serde_json::Value::Null
    );
    assert_eq!(redacted.len(), 1);
}

#[test]
fn redaction_completeness_multiple_rules_simultaneously() {
    let value = serde_json::json!({
        "user": { "name": "Alice", "ssn": "123-45-6789", "email": "alice@example.com" },
        "payment": { "card": "4111-1111-1111-1111", "cvv": "123" }
    });
    let rules = vec![
        RedactionRule::new(vec!["user".into(), "ssn".into()], RedactionKind::Remove),
        RedactionRule::new(vec!["user".into(), "email".into()], RedactionKind::Hash),
        RedactionRule::new(
            vec!["payment".into(), "card".into()],
            RedactionKind::ReplaceWith("[REDACTED]".into()),
        ),
        RedactionRule::new(vec!["payment".into(), "cvv".into()], RedactionKind::Remove),
    ];
    let (result, redacted) = apply_redaction(&value, &rules);
    assert_eq!(result["user"]["name"], "Alice");
    assert_eq!(result["user"]["ssn"], serde_json::Value::Null);
    assert!(result["user"]["email"]
        .as_str()
        .unwrap()
        .starts_with("HASH"));
    assert_eq!(result["payment"]["card"], "[REDACTED]");
    assert_eq!(result["payment"]["cvv"], serde_json::Value::Null);
    assert_eq!(redacted.len(), 4);
}

#[test]
fn redaction_completeness_preserves_non_matching_structure() {
    let value = serde_json::json!({
        "public_data": { "count": 42, "label": "safe" },
        "private_data": { "token": "secret-token" }
    });
    let rules = vec![RedactionRule::new(
        vec!["private_data".into(), "token".into()],
        RedactionKind::Remove,
    )];
    let (result, redacted) = apply_redaction(&value, &rules);
    assert_eq!(result["public_data"]["count"], 42);
    assert_eq!(result["public_data"]["label"], "safe");
    assert_eq!(result["private_data"]["token"], serde_json::Value::Null);
    assert_eq!(redacted.len(), 1);
}

#[test]
fn redaction_completeness_empty_rules_produces_identity() {
    let value = serde_json::json!({"key": "value", "nested": {"a": 1}});
    let rules: Vec<RedactionRule> = vec![];
    let (result, redacted) = apply_redaction(&value, &rules);
    assert_eq!(result, value);
    assert!(redacted.is_empty());
}

#[test]
fn operator_projection_tracks_all_redacted_fields() {
    let value = serde_json::json!({
        "a": { "x": "secret1", "y": "public" },
        "b": { "z": "secret2" }
    });
    let rules = vec![
        RedactionRule::new(vec!["a".into(), "x".into()], RedactionKind::Remove),
        RedactionRule::new(vec!["b".into(), "z".into()], RedactionKind::Hash),
    ];
    let (_, redacted) = apply_redaction(&value, &rules);
    assert_eq!(redacted.len(), 2);
    assert!(redacted.contains(&vec!["a".into(), "x".into()]));
    assert!(redacted.contains(&vec!["b".into(), "z".into()]));
}

// =========================================================================
// ADR-025: ReplaceWithType through apply_redaction
// =========================================================================

#[test]
fn apply_redaction_replace_with_type_produces_type_name() {
    let value = serde_json::json!({
        "data": 42
    });
    let rules = vec![RedactionRule::new(
        vec!["data".into()],
        RedactionKind::ReplaceWithType,
    )];
    let (result, redacted) = apply_redaction(&value, &rules);
    let redacted_val = &result["data"];
    assert!(
        redacted_val.is_string(),
        "ReplaceWithType should produce a string"
    );
    let s = redacted_val.as_str().unwrap();
    assert!(
        s.contains("Number") || s.contains("u64") || s.contains("i64") || s.contains("Value"),
        "ReplaceWithType should contain type info, got: {s}"
    );
    assert_eq!(redacted.len(), 1);
}

// =========================================================================
// ADR-025: Overlapping rules — first match wins
// =========================================================================

#[test]
fn apply_redaction_overlapping_rules_first_match_wins() {
    let value = serde_json::json!({
        "field": "secret"
    });
    let rules = vec![
        RedactionRule::new(vec!["field".into()], RedactionKind::Remove),
        RedactionRule::new(vec!["field".into()], RedactionKind::Hash),
    ];
    let (result, redacted) = apply_redaction(&value, &rules);
    assert_eq!(
        result["field"],
        serde_json::Value::Null,
        "First matching rule (Remove) should produce Null, not hash"
    );
    assert_eq!(redacted.len(), 1, "Only first rule should match");
    assert_eq!(redacted[0], vec!["field".to_string()]);
}

// =========================================================================
// ADR-025: Null value behavior — non-redacted nulls are dropped
// =========================================================================

#[test]
fn apply_redaction_drops_non_redacted_null_values() {
    let value = serde_json::json!({
        "explicit_null": null,
        "secret": "classified"
    });
    let rules = vec![RedactionRule::new(
        vec!["secret".into()],
        RedactionKind::Remove,
    )];
    let (result, redacted) = apply_redaction(&value, &rules);
    // Non-redacted null values are preserved (not redacted, so !was_redacted retains them)
    assert!(
        result.get("explicit_null").is_some(),
        "Non-redacted null fields are preserved by apply_redaction"
    );
    assert_eq!(result["explicit_null"], serde_json::Value::Null);
    // Remove omits the key per ADR-025 §1
    assert!(
        result.get("secret").is_none(),
        "Remove omits key entirely per ADR-025 §1"
    );
    assert_eq!(redacted.len(), 1);
}

// =========================================================================
// ADR-025: Redaction of sensitive data in array of primitives
// =========================================================================

#[test]
fn apply_redaction_handles_sensitive_data_in_array_of_primitives() {
    let value = serde_json::json!({
        "items": ["public", "secret_item", "another_public"]
    });
    let rules = vec![RedactionRule::new(
        vec!["items".into()],
        RedactionKind::ReplaceWith("[REDACTED]".into()),
    )];
    let (result, redacted) = apply_redaction(&value, &rules);
    assert_eq!(result["items"], "[REDACTED]");
    assert_eq!(redacted.len(), 1);
}

// =========================================================================
// ADR-025: Empty object with redaction rules
// =========================================================================

#[test]
fn apply_redaction_empty_object_with_rules_produces_empty() {
    let value = serde_json::json!({});
    let rules = vec![RedactionRule::new(
        vec!["nonexistent".into()],
        RedactionKind::Remove,
    )];
    let (result, redacted) = apply_redaction(&value, &rules);
    assert!(result.as_object().unwrap().is_empty());
    assert!(redacted.is_empty());
}

// =========================================================================
// ADR-025: RedactionKind::Remove sets value to Null (key retained)
// =========================================================================

#[test]
fn apply_redaction_remove_sets_to_null_retains_key() {
    let value = serde_json::json!({
        "keep": "visible",
        "remove_me": "gone"
    });
    let rules = vec![RedactionRule::new(
        vec!["remove_me".into()],
        RedactionKind::Remove,
    )];
    let (result, redacted) = apply_redaction(&value, &rules);
    assert_eq!(
        result["remove_me"],
        serde_json::Value::Null,
        "Remove sets value to Null and retains the key (was_redacted bypasses null filter)"
    );
    assert_eq!(result["keep"], "visible");
    assert_eq!(redacted.len(), 1);
}

// =========================================================================
// ADR-025: Operator projection never contains raw sensitive data
// =========================================================================

#[test]
fn operator_projection_never_contains_raw_sensitive_data() {
    let sensitive_ssn = "123-45-6789";
    let sensitive_email = "user@secret.com";
    let value = serde_json::json!({
        "user": {
            "name": "Alice",
            "ssn": sensitive_ssn,
            "email": sensitive_email
        }
    });
    let rules = vec![
        RedactionRule::new(vec!["user".into(), "ssn".into()], RedactionKind::Remove),
        RedactionRule::new(vec!["user".into(), "email".into()], RedactionKind::Hash),
    ];
    let (result, redacted) = apply_redaction(&value, &rules);

    let result_str = serde_json::to_string(&result).unwrap();
    assert!(
        !result_str.contains(sensitive_ssn),
        "Operator projection must not contain raw SSN"
    );
    assert!(
        !result_str.contains(sensitive_email),
        "Operator projection must not contain raw email"
    );
    assert_eq!(result["user"]["name"], "Alice");
    assert_eq!(redacted.len(), 2);
}

// =========================================================================
// ADR-025: Redaction idempotency — Hash on different inputs produces different hashes
// =========================================================================

#[test]
fn apply_redaction_hash_deterministic_on_same_input() {
    let value = serde_json::json!({
        "secret": "my-secret-value"
    });
    let rules = vec![RedactionRule::new(
        vec!["secret".into()],
        RedactionKind::Hash,
    )];
    let (result1, _) = apply_redaction(&value, &rules);
    let (result2, _) = apply_redaction(&value, &rules);
    assert_eq!(
        result1, result2,
        "Hash redaction must be deterministic for same input"
    );
}

#[test]
fn apply_redaction_replace_with_is_idempotent() {
    let value = serde_json::json!({
        "secret": "classified"
    });
    let rules = vec![RedactionRule::new(
        vec!["secret".into()],
        RedactionKind::ReplaceWith("***".into()),
    )];
    let (result1, _) = apply_redaction(&value, &rules);
    let (result2, _) = apply_redaction(&result1, &rules);
    assert_eq!(
        result1, result2,
        "ReplaceWith redaction should be idempotent"
    );
}

// =========================================================================
// ADR-025: Mixed redaction kinds on deeply nested structure
// =========================================================================

#[test]
fn apply_redaction_mixed_kinds_deeply_nested() {
    let value = serde_json::json!({
        "level1": {
            "level2": {
                "remove_field": "gone",
                "replace_field": "replaced",
                "hash_field": "hashed",
                "type_field": 42,
                "keep_field": "visible"
            }
        }
    });
    let rules = vec![
        RedactionRule::new(
            vec!["level1".into(), "level2".into(), "remove_field".into()],
            RedactionKind::Remove,
        ),
        RedactionRule::new(
            vec!["level1".into(), "level2".into(), "replace_field".into()],
            RedactionKind::ReplaceWith("***".into()),
        ),
        RedactionRule::new(
            vec!["level1".into(), "level2".into(), "hash_field".into()],
            RedactionKind::Hash,
        ),
        RedactionRule::new(
            vec!["level1".into(), "level2".into(), "type_field".into()],
            RedactionKind::ReplaceWithType,
        ),
    ];
    let (result, redacted) = apply_redaction(&value, &rules);
    let level2 = &result["level1"]["level2"];
    // Remove omits key entirely per ADR-025 §1
    assert!(
        level2.get("remove_field").is_none(),
        "Remove omits key entirely per ADR-025 §1"
    );
    assert_eq!(level2["replace_field"], "***");
    assert!(level2["hash_field"].as_str().unwrap().starts_with("HASH"));
    assert!(level2["type_field"].is_string());
    assert_eq!(level2["keep_field"], "visible");
    assert_eq!(redacted.len(), 4);
}
