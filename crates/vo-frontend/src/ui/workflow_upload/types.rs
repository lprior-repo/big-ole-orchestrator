//! Types for workflow definition upload.
//!
//! This module defines the data structures used to represent, validate, and
//! upload workflow definitions in TOML or JSON format.

use serde::{Deserialize, Serialize};
use vo_types::NodeKind;

// ---------------------------------------------------------------------------
// WorkflowDefinition — the user-facing definition format
// ---------------------------------------------------------------------------

/// A workflow definition that can be uploaded via the UI.
///
/// Supports both TOML and JSON serialization. The parser in
/// [`super::file_upload`](crate::ui::workflow_upload::file_upload) handles
/// conversion to the internal `Workflow` representation.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct WorkflowDefinition {
    /// Human-readable name for the workflow.
    pub name: String,
    /// Guarantee class for the entire workflow.
    #[serde(default)]
    pub guarantee_class: GuaranteeClassInput,
    /// Nodes that make up the workflow DAG.
    #[serde(default)]
    pub nodes: Vec<WorkflowNode>,
    /// Edges connecting nodes.
    #[serde(default)]
    pub edges: Vec<WorkflowEdge>,
}

/// Input representation of guarantee class (accepts string from JSON/TOML).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GuaranteeClassInput {
    #[default]
    BestEffort,
    AtLeastOnce,
    ExactOnce,
}

impl From<GuaranteeClassInput> for vo_types::GuaranteeClass {
    fn from(value: GuaranteeClassInput) -> Self {
        match value {
            GuaranteeClassInput::BestEffort => vo_types::GuaranteeClass::BestEffort,
            GuaranteeClassInput::AtLeastOnce => vo_types::GuaranteeClass::AtLeastOnce,
            GuaranteeClassInput::ExactOnce => vo_types::GuaranteeClass::ExactOnce,
        }
    }
}

/// A single node in a workflow definition.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct WorkflowNode {
    /// Unique identifier for the node.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Optional description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Node kind/classification.
    #[serde(default)]
    pub kind: NodeKindInput,
    /// Node configuration as a JSON value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
    /// X position for graph layout.
    #[serde(default)]
    pub x: f64,
    /// Y position for graph layout.
    #[serde(default)]
    pub y: f64,
}

/// Input representation of node kind.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum NodeKindInput {
    #[default]
    Pure,
    ManagedEffect,
    Wait,
    Signal,
    Unsafe,
    Router,
}

impl From<NodeKindInput> for NodeKind {
    fn from(value: NodeKindInput) -> Self {
        match value {
            NodeKindInput::Pure => NodeKind::Pure,
            NodeKindInput::ManagedEffect => NodeKind::ManagedEffect,
            NodeKindInput::Wait => NodeKind::Wait,
            NodeKindInput::Signal => NodeKind::Signal,
            NodeKindInput::Unsafe => NodeKind::Unsafe,
            NodeKindInput::Router => NodeKind::Router,
        }
    }
}

/// A directed edge connecting two workflow nodes.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct WorkflowEdge {
    /// Source node ID.
    pub source: String,
    /// Target node ID.
    pub target: String,
    /// Traversal condition.
    #[serde(default)]
    pub condition: EdgeConditionInput,
}

/// Condition on which an edge is traversed.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeConditionInput {
    #[default]
    Always,
    OnSuccess,
    OnFailure,
}

impl From<EdgeConditionInput> for vo_types::EdgeCondition {
    fn from(value: EdgeConditionInput) -> Self {
        match value {
            EdgeConditionInput::Always => vo_types::EdgeCondition::Always,
            EdgeConditionInput::OnSuccess => vo_types::EdgeCondition::OnSuccess,
            EdgeConditionInput::OnFailure => vo_types::EdgeCondition::OnFailure,
        }
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Severity level for a validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationSeverity {
    Error,
    Warning,
}

/// A single validation issue found in a workflow definition.
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    /// Node ID that caused the issue (if applicable).
    pub node_id: Option<String>,
    /// Severity of the issue.
    pub severity: ValidationSeverity,
    /// Human-readable message.
    pub message: String,
}

impl ValidationIssue {
    #[must_use]
    pub fn error(node_id: Option<impl Into<String>>, message: impl Into<String>) -> Self {
        Self {
            node_id: node_id.map(Into::into),
            severity: ValidationSeverity::Error,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn warning(node_id: Option<impl Into<String>>, message: impl Into<String>) -> Self {
        Self {
            node_id: node_id.map(Into::into),
            severity: ValidationSeverity::Warning,
            message: message.into(),
        }
    }
}

/// Result of validating a workflow definition.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// All issues found (errors + warnings).
    pub issues: Vec<ValidationIssue>,
    /// Whether there are any errors (warnings don't block upload).
    pub has_errors: bool,
}

impl ValidationResult {
    #[must_use]
    pub fn new(issues: Vec<ValidationIssue>) -> Self {
        let has_errors = issues.iter().any(|i| i.severity == ValidationSeverity::Error);
        Self {
            issues,
            has_errors,
        }
    }

    #[must_use]
    pub fn ok() -> Self {
        Self {
            issues: Vec::new(),
            has_errors: false,
        }
    }

    /// Returns true if validation passed (no errors).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.has_errors
    }
}

/// Validate a workflow definition, returning all issues found.
pub fn validate_definition(def: &WorkflowDefinition) -> ValidationResult {
    let mut issues: Vec<ValidationIssue> = Vec::new();

    // Name is required
    if def.name.trim().is_empty() {
        issues.push(ValidationIssue::error(
            None::<&str>,
            "Workflow name is required".to_string(),
        ));
    }

    // Check name length
    if def.name.len() > 256 {
        issues.push(ValidationIssue::error(
            None::<&str>,
            "Workflow name must be 256 characters or fewer".to_string(),
        ));
    }

    // Collect all node IDs for duplicate detection
    let mut node_ids: std::collections::HashSet<String> =
        std::collections::HashSet::with_capacity(def.nodes.len());

    for node in &def.nodes {
        // Node ID is required
        if node.id.trim().is_empty() {
            issues.push(ValidationIssue::error(
                None::<&str>,
                "Node ID is required for all nodes".to_string(),
            ));
        }

        // Check for duplicate node IDs
        if !node_ids.insert(node.id.clone()) {
            issues.push(ValidationIssue::error(
                Some(&node.id),
                format!("Duplicate node ID: {}", node.id),
            ));
        }

        // Node name is required
        if node.name.trim().is_empty() {
            issues.push(ValidationIssue::error(
                Some(&node.id),
                "Node name is required".to_string(),
            ));
        }

        // Check x/y are reasonable (prevent extreme positions)
        if node.x.abs() > 100_000.0 || node.y.abs() > 100_000.0 {
            issues.push(ValidationIssue::warning(
                Some(&node.id),
                "Node position is extremely far from origin".to_string(),
            ));
        }
    }

    // Validate edges reference existing nodes
    for edge in &def.edges {
        if !node_ids.contains(&edge.source) {
            issues.push(ValidationIssue::error(
                Some(&edge.source),
                format!(
                    "Edge references non-existent source node: {}",
                    edge.source
                ),
            ));
        }
        if !node_ids.contains(&edge.target) {
            issues.push(ValidationIssue::error(
                Some(&edge.target),
                format!(
                    "Edge references non-existent target node: {}",
                    edge.target
                ),
            ));
        }
    }

    // Check for cycles (simplified: just flag if no edges and multiple nodes)
    if def.nodes.len() > 1 && def.edges.is_empty() {
        issues.push(ValidationIssue::warning(
            None::<&str>,
            "Workflow has multiple nodes but no edges defined".to_string(),
        ));
    }

    ValidationResult::new(issues)
}

// ---------------------------------------------------------------------------
// Upload state
// ---------------------------------------------------------------------------

/// Current state of the upload process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadState {
    /// Ready to edit/upload.
    Idle,
    /// Validation in progress.
    Validating,
    /// Validation passed, showing preview.
    Preview,
    /// Upload in progress.
    Uploading,
    /// Upload succeeded.
    Uploaded,
    /// Upload failed.
    UploadFailed,
}

impl Default for UploadState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Result of an upload attempt.
#[derive(Debug, Clone)]
pub struct UploadResult {
    pub success: bool,
    pub instance_id: Option<String>,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_workflow_definition() -> WorkflowDefinition {
        WorkflowDefinition {
            name: "TestWorkflow".to_string(),
            guarantee_class: GuaranteeClassInput::BestEffort,
            nodes: vec![WorkflowNode {
                id: "node-1".to_string(),
                name: "First Node".to_string(),
                kind: NodeKindInput::Pure,
                config: None,
                x: 100.0,
                y: 200.0,
            }],
            edges: vec![],
        }
    }

    #[test]
    fn validate_definition_accepts_valid_workflow() {
        let def = valid_workflow_definition();
        let result = validate_definition(&def);
        assert!(result.is_valid());
        assert!(result.issues.is_empty());
    }

    #[test]
    fn validate_definition_rejects_empty_workflow_name() {
        let def = WorkflowDefinition {
            name: "".to_string(),
            ..valid_workflow_definition()
        };
        let result = validate_definition(&def);
        assert!(!result.is_valid());
        assert!(result.has_errors);
        assert!(result.issues.iter().any(|i| i.message.contains("Workflow name is required")));
    }

    #[test]
    fn validate_definition_rejects_whitespace_only_workflow_name() {
        let def = WorkflowDefinition {
            name: "   ".to_string(),
            ..valid_workflow_definition()
        };
        let result = validate_definition(&def);
        assert!(!result.is_valid());
        assert!(result.has_errors);
    }

    #[test]
    fn validate_definition_rejects_workflow_name_over_256_chars() {
        let def = WorkflowDefinition {
            name: "a".repeat(257),
            ..valid_workflow_definition()
        };
        let result = validate_definition(&def);
        assert!(!result.is_valid());
        assert!(result.has_errors);
        assert!(result.issues.iter().any(|i| i.message.contains("256 characters")));
    }

    #[test]
    fn validate_definition_accepts_workflow_name_at_256_chars() {
        let def = WorkflowDefinition {
            name: "a".repeat(256),
            ..valid_workflow_definition()
        };
        let result = validate_definition(&def);
        assert!(result.is_valid());
    }

    #[test]
    fn validate_definition_rejects_empty_node_id() {
        let def = WorkflowDefinition {
            nodes: vec![WorkflowNode {
                id: "".to_string(),
                name: "Test Node".to_string(),
                kind: NodeKindInput::Pure,
                config: None,
                x: 0.0,
                y: 0.0,
            }],
            ..valid_workflow_definition()
        };
        let result = validate_definition(&def);
        assert!(!result.is_valid());
        assert!(result.has_errors);
        assert!(result.issues.iter().any(|i| i.message.contains("Node ID is required")));
    }

    #[test]
    fn validate_definition_rejects_duplicate_node_ids() {
        let def = WorkflowDefinition {
            nodes: vec![
                WorkflowNode {
                    id: "node-1".to_string(),
                    name: "Node 1".to_string(),
                    kind: NodeKindInput::Pure,
                    config: None,
                    x: 0.0,
                    y: 0.0,
                },
                WorkflowNode {
                    id: "node-1".to_string(),
                    name: "Node 2".to_string(),
                    kind: NodeKindInput::Pure,
                    config: None,
                    x: 100.0,
                    y: 100.0,
                },
            ],
            ..valid_workflow_definition()
        };
        let result = validate_definition(&def);
        assert!(!result.is_valid());
        assert!(result.has_errors);
        assert!(result.issues.iter().any(|i| i.message.contains("Duplicate node ID")));
    }

    #[test]
    fn validate_definition_rejects_empty_node_name() {
        let def = WorkflowDefinition {
            nodes: vec![WorkflowNode {
                id: "node-1".to_string(),
                name: "".to_string(),
                kind: NodeKindInput::Pure,
                config: None,
                x: 0.0,
                y: 0.0,
            }],
            ..valid_workflow_definition()
        };
        let result = validate_definition(&def);
        assert!(!result.is_valid());
        assert!(result.has_errors);
        assert!(result.issues.iter().any(|i| i.message.contains("Node name is required")));
    }

    #[test]
    fn validate_definition_warns_on_extreme_node_position() {
        let def = WorkflowDefinition {
            nodes: vec![WorkflowNode {
                id: "node-1".to_string(),
                name: "Test Node".to_string(),
                kind: NodeKindInput::Pure,
                config: None,
                x: 200_000.0,
                y: 200_000.0,
            }],
            ..valid_workflow_definition()
        };
        let result = validate_definition(&def);
        assert!(result.is_valid());
        assert!(!result.has_errors);
        assert!(result.issues.iter().any(|i| {
            i.severity == ValidationSeverity::Warning
            && i.message.contains("extremely far from origin")
        }));
    }

    #[test]
    fn validate_definition_accepts_reasonable_node_position() {
        let def = WorkflowDefinition {
            nodes: vec![WorkflowNode {
                id: "node-1".to_string(),
                name: "Test Node".to_string(),
                kind: NodeKindInput::Pure,
                config: None,
                x: 500.0,
                y: 500.0,
            }],
            ..valid_workflow_definition()
        };
        let result = validate_definition(&def);
        assert!(result.is_valid());
        assert!(result.issues.is_empty());
    }

    #[test]
    fn validate_definition_rejects_edge_with_nonexistent_source() {
        let def = WorkflowDefinition {
            nodes: vec![WorkflowNode {
                id: "node-1".to_string(),
                name: "Test Node".to_string(),
                kind: NodeKindInput::Pure,
                config: None,
                x: 0.0,
                y: 0.0,
            }],
            edges: vec![WorkflowEdge {
                source: "nonexistent".to_string(),
                target: "node-1".to_string(),
                condition: EdgeConditionInput::Always,
            }],
            ..valid_workflow_definition()
        };
        let result = validate_definition(&def);
        assert!(!result.is_valid());
        assert!(result.has_errors);
        assert!(result.issues.iter().any(|i| i.message.contains("non-existent source node")));
    }

    #[test]
    fn validate_definition_rejects_edge_with_nonexistent_target() {
        let def = WorkflowDefinition {
            nodes: vec![WorkflowNode {
                id: "node-1".to_string(),
                name: "Test Node".to_string(),
                kind: NodeKindInput::Pure,
                config: None,
                x: 0.0,
                y: 0.0,
            }],
            edges: vec![WorkflowEdge {
                source: "node-1".to_string(),
                target: "nonexistent".to_string(),
                condition: EdgeConditionInput::Always,
            }],
            ..valid_workflow_definition()
        };
        let result = validate_definition(&def);
        assert!(!result.is_valid());
        assert!(result.has_errors);
        assert!(result.issues.iter().any(|i| i.message.contains("non-existent target node")));
    }

    #[test]
    fn validate_definition_accepts_valid_edge() {
        let def = WorkflowDefinition {
            nodes: vec![
                WorkflowNode {
                    id: "node-1".to_string(),
                    name: "Node 1".to_string(),
                    kind: NodeKindInput::Pure,
                    config: None,
                    x: 0.0,
                    y: 0.0,
                },
                WorkflowNode {
                    id: "node-2".to_string(),
                    name: "Node 2".to_string(),
                    kind: NodeKindInput::Pure,
                    config: None,
                    x: 100.0,
                    y: 100.0,
                },
            ],
            edges: vec![WorkflowEdge {
                source: "node-1".to_string(),
                target: "node-2".to_string(),
                condition: EdgeConditionInput::Always,
            }],
            ..valid_workflow_definition()
        };
        let result = validate_definition(&def);
        assert!(result.is_valid());
    }

    #[test]
    fn validate_definition_warns_when_multiple_nodes_have_no_edges() {
        let def = WorkflowDefinition {
            nodes: vec![
                WorkflowNode {
                    id: "node-1".to_string(),
                    name: "Node 1".to_string(),
                    kind: NodeKindInput::Pure,
                    config: None,
                    x: 0.0,
                    y: 0.0,
                },
                WorkflowNode {
                    id: "node-2".to_string(),
                    name: "Node 2".to_string(),
                    kind: NodeKindInput::Pure,
                    config: None,
                    x: 100.0,
                    y: 100.0,
                },
            ],
            edges: vec![],
            ..valid_workflow_definition()
        };
        let result = validate_definition(&def);
        assert!(result.is_valid());
        assert!(!result.has_errors);
        assert!(result.issues.iter().any(|i| {
            i.severity == ValidationSeverity::Warning
            && i.message.contains("no edges defined")
        }));
    }

    #[test]
    fn validate_definition_allows_single_node_with_no_edges() {
        let def = valid_workflow_definition();
        let result = validate_definition(&def);
        assert!(result.is_valid());
        assert!(!result.issues.iter().any(|i| i.message.contains("no edges defined")));
    }

    #[test]
    fn validation_result_ok_produces_valid_result() {
        let result = ValidationResult::ok();
        assert!(result.is_valid());
        assert!(!result.has_errors);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn validation_result_new_marks_errors_correctly() {
        let issues = vec![
            ValidationIssue::error(None::<&str>, "Error 1".to_string()),
            ValidationIssue::warning(None::<&str>, "Warning 1".to_string()),
        ];
        let result = ValidationResult::new(issues);
        assert!(!result.is_valid());
        assert!(result.has_errors);
        assert_eq!(result.issues.len(), 2);
    }

    #[test]
    fn validation_result_warnings_do_not_block_upload() {
        let issues = vec![
            ValidationIssue::warning(None::<&str>, "Warning 1".to_string()),
            ValidationIssue::warning(None::<&str>, "Warning 2".to_string()),
        ];
        let result = ValidationResult::new(issues);
        assert!(result.is_valid());
        assert!(!result.has_errors);
    }
}
