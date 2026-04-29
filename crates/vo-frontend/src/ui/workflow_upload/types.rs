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
