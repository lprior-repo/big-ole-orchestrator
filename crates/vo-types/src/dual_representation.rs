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
    /// Field is omitted entirely from the operator projection.
    /// The field key and value are not present in the result.
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

    #[allow(clippy::if_same_then_else)]
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

                    let (new_val, was_redacted, is_remove) = if let Some(r) = rule {
                        redacted_fields.push(r.field_path.clone());
                        let new_val = r.redaction_kind.redact_value(key, val);
                        let is_remove = matches!(r.redaction_kind, RedactionKind::Remove);
                        (new_val, true, is_remove)
                    } else {
                        (
                            apply_recursive(val, rules, current_path, redacted_fields),
                            false,
                            false,
                        )
                    };

                    if is_remove {
                        // Remove: key is omitted entirely per ADR-025 §1
                    } else if !was_redacted || new_val != serde_json::Value::Null {
                        result.insert(key.clone(), new_val);
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
