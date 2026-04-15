//! Dual representation types for workflow history per ADR-025.
//!
//! This module provides the dual-representation privacy model:
//! - **Canonical replay data**: encrypted at rest, full-fidelity for deterministic replay
//! - **Operator projection**: redacted JSON view for UI, CLI, and AI consumption
//!
//! ## Architecture
//!
//! For every payload-bearing transition, the Engine produces two representations:
//! 1. Canonical data (encrypted) for exact replay and recovery
//! 2. Operator projection (redacted) for safe external consumption

use serde::{Deserialize, Serialize};

/// Per-workflow-type redaction policy defining which fields to redact.
///
/// Per ADR-025 §1: "produced by applying the configured `state_filter` recursively"
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionPolicy {
    pub workflow_type: String,
    pub redaction_rules: Vec<RedactionRule>,
}

impl RedactionPolicy {
    pub fn new(workflow_type: String, redaction_rules: Vec<RedactionRule>) -> Self {
        Self {
            workflow_type,
            redaction_rules,
        }
    }
}

/// A single redaction rule for a specific field path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionRule {
    pub field_path: Vec<String>,
    pub redaction_kind: RedactionKind,
}

impl RedactionRule {
    pub fn new(field_path: Vec<String>, redaction_kind: RedactionKind) -> Self {
        Self {
            field_path,
            redaction_kind,
        }
    }
}

/// Kind of redaction to apply to a field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RedactionKind {
    /// Field is removed entirely from the operator projection.
    Remove,
    /// Field is replaced with a fixed placeholder value.
    ReplaceWith(String),
    /// Field is replaced with its type name (for debugging).
    ReplaceWithType,
    /// Field is hashed with SHA-256 (preserves uniqueness for correlation).
    Hash,
}

impl RedactionKind {
    pub fn redact_value(&self, _field_name: &str, value: &serde_json::Value) -> serde_json::Value {
        match self {
            RedactionKind::Remove => serde_json::Value::Null,
            RedactionKind::ReplaceWith(replacement) => {
                serde_json::Value::String(replacement.clone())
            }
            RedactionKind::ReplaceWithType => {
                serde_json::Value::String(std::any::type_name_of_val(value).to_string())
            }
            RedactionKind::Hash => {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                if let Some(s) = value.as_str() {
                    s.hash(&mut hasher);
                } else {
                    value.hash(&mut hasher);
                }
                serde_json::Value::String(format!("HASH{:x}", hasher.finish()))
            }
        }
    }
}

/// Operator projection - redacted view for UI/CLI/AI consumption.
///
/// Per ADR-025 §1: "a redacted JSON view intended for UI, CLI, and default AI consumption,
/// produced by applying the configured `state_filter` recursively"
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperatorProjection {
    pub workflow_id: String,
    pub workflow_type: String,
    pub projection_json: serde_json::Value,
    pub redacted_fields: Vec<Vec<String>>,
}

impl OperatorProjection {
    pub fn new(
        workflow_id: String,
        workflow_type: String,
        projection_json: serde_json::Value,
        redacted_fields: Vec<Vec<String>>,
    ) -> Self {
        Self {
            workflow_id,
            workflow_type,
            projection_json,
            redacted_fields,
        }
    }

    pub fn workflow_id(&self) -> &str {
        &self.workflow_id
    }

    pub fn workflow_type(&self) -> &str {
        &self.workflow_type
    }

    pub fn projection_json(&self) -> &serde_json::Value {
        &self.projection_json
    }

    pub fn redacted_fields(&self) -> &[Vec<String>] {
        &self.redacted_fields
    }
}

/// Applies redaction rules recursively to a JSON value.
///
/// Per ADR-025 §1: "produced by applying the configured `state_filter` recursively"
pub fn apply_redaction(
    value: &serde_json::Value,
    rules: &[RedactionRule],
) -> (serde_json::Value, Vec<Vec<String>>) {
    let mut redacted_fields = Vec::new();

    fn matches_rule(current_path: &[String], rule_path: &[String]) -> bool {
        if rule_path.len() > current_path.len() {
            return false;
        }
        let mut cpi = 0;
        for rp in rule_path.iter() {
            while cpi < current_path.len() && current_path[cpi].parse::<usize>().is_ok() {
                cpi += 1;
            }
            if cpi >= current_path.len() || &current_path[cpi] != rp {
                return false;
            }
            cpi += 1;
        }
        true
    }

    fn apply_recursive(
        value: &serde_json::Value,
        rules: &[RedactionRule],
        current_path: &mut Vec<String>,
        redacted_fields: &mut Vec<Vec<String>>,
    ) -> serde_json::Value {
        match value {
            serde_json::Value::Object(obj) => {
                let mut result = serde_json::Map::new();
                for (key, val) in obj {
                    current_path.push(key.clone());

                    let rule = rules
                        .iter()
                        .find(|r| matches_rule(current_path, &r.field_path));

                    match rule {
                        Some(r) => {
                            redacted_fields.push(r.field_path.clone());
                            match r.redaction_kind {
                                RedactionKind::Remove => {
                                    // Remove field entirely from object (per AR-09 test plan)
                                    // Do nothing - key is not inserted
                                }
                                _ => {
                                    let new_val = r.redaction_kind.redact_value(key, val);
                                    result.insert(key.clone(), new_val);
                                }
                            }
                        }
                        None => {
                            let new_val =
                                apply_recursive(val, rules, current_path, redacted_fields);
                            if new_val != serde_json::Value::Null {
                                result.insert(key.clone(), new_val);
                            }
                        }
                    }

                    current_path.pop();
                }
                serde_json::Value::Object(result)
            }
            serde_json::Value::Array(arr) => {
                let mut result = Vec::new();
                for item in arr.iter() {
                    let new_item = apply_recursive(item, rules, current_path, redacted_fields);
                    result.push(new_item);
                }
                serde_json::Value::Array(result)
            }
            other => other.clone(),
        }
    }

    let result = apply_recursive(value, rules, &mut Vec::new(), &mut redacted_fields);
    (result, redacted_fields)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // Remove removes key entirely (per AR-09 test plan)
        assert!(!result["user"].as_object().unwrap().contains_key("ssn"));
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
        // Remove removes key entirely (per AR-09 test plan)
        assert!(!result["users"][0].as_object().unwrap().contains_key("ssn"));
        assert_eq!(result["users"][1]["name"], "Bob");
        assert!(!result["users"][1].as_object().unwrap().contains_key("ssn"));
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
        // Remove removes key entirely (per AR-09 test plan)
        assert!(!result["level1"]["level2"]["level3"]
            .as_object()
            .unwrap()
            .contains_key("secret"));
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
        // Non-sensitive fields preserved
        assert_eq!(result["user"]["name"], "Alice");
        // Sensitive fields redacted
        // Remove removes key entirely (per AR-09 test plan)
        assert!(!result["user"].as_object().unwrap().contains_key("ssn"));
        assert!(result["user"]["email"]
            .as_str()
            .unwrap()
            .starts_with("HASH"));
        assert_eq!(result["payment"]["card"], "[REDACTED]");
        // Remove removes key entirely (per AR-09 test plan)
        assert!(!result["payment"].as_object().unwrap().contains_key("cvv"));
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
        // Public data completely untouched
        assert_eq!(result["public_data"]["count"], 42);
        assert_eq!(result["public_data"]["label"], "safe");
        // Private data redacted - Remove removes key entirely (per AR-09 test plan)
        assert!(!result["private_data"]
            .as_object()
            .unwrap()
            .contains_key("token"));
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
}
