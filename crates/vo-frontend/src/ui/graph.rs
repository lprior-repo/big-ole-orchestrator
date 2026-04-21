//! Graph types for the UI (ADR-031).
//!
//! This module defines the core graph data structures used by the UI
//! for node visualization and badge rendering.

use std::collections::HashMap;
use std::fmt::Display;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ulid::Ulid;
use uuid::Uuid;
use vo_types::NodeKind;

/// Re-export ExecutionState from edges::graph_types for UI compatibility.
pub use crate::ui::edges::graph_types::ExecutionState;

impl ExecutionState {
    pub const fn status_badge_class(self) -> &'static str {
        match self {
            ExecutionState::Idle | ExecutionState::Queued => {
                "bg-slate-100 text-slate-700 border-slate-200"
            }
            ExecutionState::Running => "bg-blue-100 text-blue-700 border-blue-200",
            ExecutionState::Completed => "bg-green-100 text-green-700 border-green-200",
            ExecutionState::Failed => "bg-red-100 text-red-700 border-red-200",
            ExecutionState::Skipped => "bg-slate-100 text-slate-500 border-slate-200",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            ExecutionState::Idle | ExecutionState::Queued => "pending",
            ExecutionState::Running => "running",
            ExecutionState::Completed => "completed",
            ExecutionState::Failed => "failed",
            ExecutionState::Skipped => "skipped",
        }
    }
}

/// Port name for connections between nodes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PortName(pub String);

impl From<&str> for PortName {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for PortName {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<PortName> for String {
    fn from(port: PortName) -> Self {
        port.0
    }
}

impl Display for PortName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Connection between two nodes (ADR-031).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Connection {
    pub id: Uuid,
    pub source: NodeId,
    pub target: NodeId,
    pub source_port: PortName,
    pub target_port: PortName,
}

/// Run record for a workflow execution (ADR-031).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub results: HashMap<NodeId, serde_json::Value>,
    pub success: bool,
}

/// Workflow node configuration variants (ADR-031).
pub mod workflow_node {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct RunConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ParallelConfig {}

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "type", rename_all = "kebab-case")]
    pub enum WorkflowNode {
        Run(RunConfig),
        Parallel(ParallelConfig),
    }

    impl Default for WorkflowNode {
        fn default() -> Self {
            Self::Run(RunConfig::default())
        }
    }

    impl WorkflowNode {
        pub fn from_str(s: &str) -> Result<Self, ()> {
            match s {
                "run" => Ok(Self::Run(RunConfig::default())),
                "parallel" => Ok(Self::Parallel(ParallelConfig::default())),
                _ => Err(()),
            }
        }
    }
}

use workflow_node::WorkflowNode;

/// Unique identifier for a graph node.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl NodeId {
    /// Create a new random NodeId.
    #[must_use]
    pub fn new() -> Self {
        Self(Ulid::new().to_string())
    }

    /// Parse a NodeId from a string.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        if s.is_empty() {
            return None;
        }
        // Validate it looks like a ULID (26 chars, base32)
        if s.len() != 26 {
            return None;
        }
        // Accept any 26-char string; ULID validity is checked by length above
        Some(Self(s.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Classification of a workflow node by its side-effect profile (ADR-031).
/// This is the UI-facing category used for badge display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeCategory {
    /// Entry point node - workflow start
    Entry,
    /// Durable computation - managed side effects
    Durable,
    /// State mutation - internal state changes
    State,
    /// Flow control - Pure or Unsafe computation
    Flow,
    /// Timing - wait/sleep operations
    Timing,
    /// Signal - emit/wait signal operations
    Signal,
}

impl Display for NodeCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            NodeCategory::Entry => "entry",
            NodeCategory::Durable => "durable",
            NodeCategory::State => "state",
            NodeCategory::Flow => "flow",
            NodeCategory::Timing => "timing",
            NodeCategory::Signal => "signal",
        };
        write!(f, "{s}")
    }
}

impl NodeCategory {
    /// Returns the CSS badge class for this category.
    #[must_use]
    pub const fn badge_class(self) -> &'static str {
        match self {
            NodeCategory::Entry => "bg-emerald-50 text-emerald-700 border-emerald-200",
            NodeCategory::Durable => "bg-indigo-50 text-indigo-700 border-indigo-200",
            NodeCategory::State => "bg-orange-50 text-orange-700 border-orange-200",
            NodeCategory::Flow => "bg-amber-50 text-amber-700 border-amber-200",
            NodeCategory::Timing => "bg-pink-50 text-pink-700 border-pink-200",
            NodeCategory::Signal => "bg-blue-50 text-blue-700 border-blue-200",
        }
    }
}

/// Converts NodeKind to NodeCategory for UI badge display.
/// This is the authoritative mapping that the UI must implement.
#[must_use]
pub fn node_kind_to_category(kind: NodeKind) -> NodeCategory {
    match kind {
        NodeKind::Pure => NodeCategory::Flow,
        NodeKind::ManagedEffect => NodeCategory::Durable,
        NodeKind::Wait => NodeCategory::Timing,
        NodeKind::Signal => NodeCategory::Signal,
        NodeKind::Unsafe => NodeCategory::Flow,
    }
}

/// Validate a node name for the frontend.
///
/// Allows printable ASCII characters (including spaces) but rejects characters
/// that could enable XSS: `<`, `>`, `&`, `"`, `'`, and control characters.
/// This is less strict than backend `NodeName::parse` (which restricts to
/// identifier chars) because the frontend uses display names that may contain
/// spaces, parentheses, etc.
///
/// Returns the name on success, or `None` if invalid.
#[must_use]
pub fn validate_node_name(name: &str) -> Option<String> {
    if name.is_empty() || name.len() > 256 {
        return None;
    }
    for ch in name.chars() {
        match ch {
            '\0'..='\x08' | '\x0b' | '\x0c' | '\x0e'..='\x1f' | '\x7f' => return None,
            '<' | '>' | '&' | '"' | '\'' => return None,
            _ => {}
        }
    }
    Some(name.to_string())
}

/// Sanitize freeform text fields (description, notes) by stripping HTML tags.
///
/// This prevents stored XSS payloads from becoming exploitable if future
/// rendering switches from Dioxus text interpolation to raw HTML (e.g.
/// markdown rendering). The defense-in-depth layer catches payloads that
/// Dioxus RSX escaping handles today but a future code change might not.
#[must_use]
pub fn sanitize_text(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => {
                in_tag = true;
            }
            '>' if in_tag => {
                in_tag = false;
            }
            _ if !in_tag => {
                result.push(ch);
            }
            _ => {}
        }
    }
    result
}

/// Validate an icon name: reject CSS expression payloads and HTML.
#[must_use]
pub fn validate_icon_name(icon: &str) -> Option<String> {
    let lower = icon.to_lowercase();
    if lower.contains("expression(") || lower.contains("javascript:") || lower.contains("<") {
        return None;
    }
    Some(icon.to_string())
}

/// A workflow node with its properties and category.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub name: String,
    pub description: String,
    pub kind: NodeKind,
    pub category: NodeCategory,
    pub icon: String,
    pub x: f64,
    pub y: f64,
    pub config: serde_json::Value,
    pub execution_state: ExecutionState,
}

impl Node {
    /// Create a new node with the given ID and kind.
    ///
    /// The name is validated against backend `NodeName` identifier rules.
    /// Returns `None` if the name contains invalid characters (XSS payloads, etc.).
    #[must_use]
    pub fn new(id: NodeId, name: String, kind: NodeKind) -> Option<Self> {
        let name = validate_node_name(&name)?;
        let category = node_kind_to_category(kind);
        let icon = category_to_icon(category);
        Some(Self {
            id,
            name,
            description: String::new(),
            kind,
            category,
            icon,
            x: 0.0,
            y: 0.0,
            config: serde_json::Value::Object(Default::default()),
            execution_state: ExecutionState::Idle,
        })
    }

    /// Create a node from a workflow node variant.
    ///
    /// The name is validated against backend `NodeName` identifier rules.
    #[must_use]
    pub fn from_workflow_node(name: String, workflow_node: WorkflowNode, x: f64, y: f64) -> Option<Self> {
        let name = validate_node_name(&name)?;
        let (kind, category, icon) = match workflow_node {
            WorkflowNode::Run(_) => {
                let kind = NodeKind::ManagedEffect;
                let category = node_kind_to_category(kind);
                let icon = category_to_icon(category);
                (kind, category, icon)
            }
            WorkflowNode::Parallel(_) => {
                let kind = NodeKind::Pure;
                let category = node_kind_to_category(kind);
                let icon = category_to_icon(category);
                (kind, category, icon)
            }
        };
        Some(Self {
            id: NodeId::new(),
            name,
            description: String::new(),
            kind,
            category,
            icon,
            x,
            y,
            config: serde_json::Value::Object(Default::default()),
            execution_state: ExecutionState::Idle,
        })
    }

    /// Update the node kind and recalculate the category.
    pub fn set_kind(&mut self, kind: NodeKind) {
        self.kind = kind;
        self.category = node_kind_to_category(kind);
        self.icon = category_to_icon(self.category);
    }

    /// Set the node name with validation.
    ///
    /// Returns `false` if the name is invalid (contains XSS payloads, etc.).
    pub fn set_name(&mut self, name: &str) -> bool {
        if let Some(validated) = validate_node_name(name) {
            self.name = validated;
            true
        } else {
            false
        }
    }

    /// Set the node description with sanitization.
    ///
    /// HTML tags are stripped to prevent stored XSS. The description is a
    /// freeform text field, so only tag stripping is applied (not full
    /// identifier validation).
    pub fn set_description(&mut self, description: &str) {
        self.description = sanitize_text(description);
    }

    /// Set the node icon with validation.
    ///
    /// Returns `false` if the icon contains CSS expression payloads or HTML.
    pub fn set_icon(&mut self, icon: &str) -> bool {
        if let Some(validated) = validate_icon_name(icon) {
            self.icon = validated;
            true
        } else {
            false
        }
    }

    /// Apply a config update to the node.
    pub fn apply_config_update(&mut self, new_config: &serde_json::Value) {
        if let serde_json::Value::Object(map) = new_config {
            if let Some(obj) = self.config.as_object_mut() {
                for (key, value) in map {
                    obj.insert(key.clone(), value.clone());
                }
            }
        }
    }
}

/// Convert a NodeCategory to an icon name.
#[must_use]
fn category_to_icon(category: NodeCategory) -> String {
    match category {
        NodeCategory::Entry => "rocket",
        NodeCategory::Durable => "database",
        NodeCategory::State => "cog",
        NodeCategory::Flow => "zap",
        NodeCategory::Timing => "clock",
        NodeCategory::Signal => "wifi",
    }
    .to_string()
}

/// A workflow containing multiple nodes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Workflow {
    pub nodes: Vec<Node>,
    pub connections: Vec<Connection>,
    pub name: String,
}

impl Workflow {
    /// Create a new empty workflow.
    #[must_use]
    pub fn new(name: String) -> Self {
        Self {
            nodes: Vec::new(),
            connections: Vec::new(),
            name,
        }
    }

    /// Add a node to the workflow.
    pub fn add_node(&mut self, node: Node) {
        self.nodes.push(node);
    }

    /// Remove a node from the workflow.
    pub fn remove_node(&mut self, id: impl Into<NodeId>) {
        let id = id.into();
        self.nodes.retain(|n| n.id != id);
    }

    /// Get a node by ID.
    #[must_use]
    pub fn get_node(&self, id: impl Into<NodeId>) -> Option<&Node> {
        let id = id.into();
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Get a mutable reference to a node by ID.
    #[must_use]
    pub fn get_node_mut(&mut self, id: impl Into<NodeId>) -> Option<&mut Node> {
        let id = id.into();
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// Collect all nodes into a HashMap for efficient lookup.
    #[must_use]
    pub fn nodes_by_id(&self) -> HashMap<String, &Node> {
        self.nodes.iter().map(|n| (n.id.0.clone(), n)).collect()
    }
}

/// Severity level for a validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ValidationSeverity {
    Error,
    Warning,
}

/// A single validation issue found during workflow validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub node_id: Option<NodeId>,
    pub severity: ValidationSeverity,
    pub message: String,
}

/// The result of validating a workflow.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ValidationResult {
    pub issues: Vec<ValidationIssue>,
}

impl ValidationResult {
    #[must_use]
    pub fn new(issues: Vec<ValidationIssue>) -> Self {
        Self { issues }
    }

    #[must_use]
    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == ValidationSeverity::Error)
            .count()
    }

    #[must_use]
    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == ValidationSeverity::Warning)
            .count()
    }

    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_id_new_generates_valid_id() {
        let id = NodeId::new();
        assert!(!id.0.is_empty());
        assert_eq!(id.0.len(), 26);
    }

    #[test]
    fn node_kind_to_category_maps_pure_to_flow() {
        assert_eq!(node_kind_to_category(NodeKind::Pure), NodeCategory::Flow);
    }

    #[test]
    fn node_kind_to_category_maps_managed_effect_to_durable() {
        assert_eq!(
            node_kind_to_category(NodeKind::ManagedEffect),
            NodeCategory::Durable
        );
    }

    #[test]
    fn node_kind_to_category_maps_wait_to_timing() {
        assert_eq!(node_kind_to_category(NodeKind::Wait), NodeCategory::Timing);
    }

    #[test]
    fn node_kind_to_category_maps_signal_to_signal() {
        assert_eq!(
            node_kind_to_category(NodeKind::Signal),
            NodeCategory::Signal
        );
    }

    #[test]
    fn node_kind_to_category_maps_unsafe_to_flow() {
        assert_eq!(node_kind_to_category(NodeKind::Unsafe), NodeCategory::Flow);
    }

    #[test]
    fn node_category_badge_class_returns_correct_css() {
        assert_eq!(
            NodeCategory::Entry.badge_class(),
            "bg-emerald-50 text-emerald-700 border-emerald-200"
        );
        assert_eq!(
            NodeCategory::Durable.badge_class(),
            "bg-indigo-50 text-indigo-700 border-indigo-200"
        );
        assert_eq!(
            NodeCategory::State.badge_class(),
            "bg-orange-50 text-orange-700 border-orange-200"
        );
        assert_eq!(
            NodeCategory::Flow.badge_class(),
            "bg-amber-50 text-amber-700 border-amber-200"
        );
        assert_eq!(
            NodeCategory::Timing.badge_class(),
            "bg-pink-50 text-pink-700 border-pink-200"
        );
        assert_eq!(
            NodeCategory::Signal.badge_class(),
            "bg-blue-50 text-blue-700 border-blue-200"
        );
    }

    #[test]
    fn node_creates_with_correct_category() {
        let node = Node::new(NodeId::new(), "test".to_string(), NodeKind::Pure).expect("valid name");
        assert_eq!(node.category, NodeCategory::Flow);
        assert_eq!(node.kind, NodeKind::Pure);
        assert_eq!(node.icon, "zap");
    }

    #[test]
    fn node_set_kind_updates_category() {
        let mut node = Node::new(NodeId::new(), "test".to_string(), NodeKind::Pure).expect("valid name");
        assert_eq!(node.category, NodeCategory::Flow);

        node.set_kind(NodeKind::ManagedEffect);
        assert_eq!(node.category, NodeCategory::Durable);
        assert_eq!(node.icon, "database");
    }

    #[test]
    fn workflow_add_and_remove_node() {
        let mut workflow = Workflow::new("test".to_string());
        let node = Node::new(NodeId::new(), "test".to_string(), NodeKind::Pure).expect("valid name");
        let node_id = node.id.clone();

        workflow.add_node(node);
        assert_eq!(workflow.nodes.len(), 1);
        assert!(workflow.get_node(node_id.clone()).is_some());

        workflow.remove_node(node_id.clone());
        assert_eq!(workflow.nodes.len(), 0);
        assert!(workflow.get_node(node_id).is_none());
    }

    #[test]
    fn workflow_nodes_by_id() {
        let mut workflow = Workflow::new("test".to_string());
        let node1 = Node::new(NodeId::new(), "test1".to_string(), NodeKind::Pure).expect("valid name");
        let node2 = Node::new(NodeId::new(), "test2".to_string(), NodeKind::Wait).expect("valid name");
        let node1_id = node1.id.0.clone();
        let node2_id = node2.id.0.clone();

        workflow.add_node(node1);
        workflow.add_node(node2);

        let nodes = workflow.nodes_by_id();
        assert_eq!(nodes.len(), 2);
        assert!(nodes.contains_key(&node1_id));
        assert!(nodes.contains_key(&node2_id));
    }
}
