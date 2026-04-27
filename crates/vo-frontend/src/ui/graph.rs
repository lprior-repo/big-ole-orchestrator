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
pub use vo_types::GuaranteeClass;
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
    #[must_use]
    pub fn new(id: NodeId, name: String, kind: NodeKind) -> Self {
        let category = node_kind_to_category(kind);
        let icon = category_to_icon(category);
        Self {
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
        }
    }

    /// Create a node from a workflow node variant.
    #[must_use]
    pub fn from_workflow_node(name: String, workflow_node: WorkflowNode, x: f64, y: f64) -> Self {
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
        Self {
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
        }
    }

    /// Update the node kind and recalculate the category.
    pub fn set_kind(&mut self, kind: NodeKind) {
        self.kind = kind;
        self.category = node_kind_to_category(kind);
        self.icon = category_to_icon(self.category);
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub nodes: Vec<Node>,
    pub connections: Vec<Connection>,
    pub name: String,
    pub guarantee_class: GuaranteeClass,
}

impl Workflow {
    /// Create a new empty workflow with the given guarantee class.
    #[must_use]
    pub fn new(name: String, guarantee_class: GuaranteeClass) -> Self {
        Self {
            nodes: Vec::new(),
            connections: Vec::new(),
            name,
            guarantee_class,
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
        let node = Node::new(NodeId::new(), "test".to_string(), NodeKind::Pure);
        assert_eq!(node.category, NodeCategory::Flow);
        assert_eq!(node.kind, NodeKind::Pure);
        assert_eq!(node.icon, "zap");
    }

    #[test]
    fn node_set_kind_updates_category() {
        let mut node = Node::new(NodeId::new(), "test".to_string(), NodeKind::Pure);
        assert_eq!(node.category, NodeCategory::Flow);

        node.set_kind(NodeKind::ManagedEffect);
        assert_eq!(node.category, NodeCategory::Durable);
        assert_eq!(node.icon, "database");
    }

    #[test]
    fn workflow_add_and_remove_node() {
        let mut workflow = Workflow::new("test".to_string(), GuaranteeClass::BestEffort);
        let node = Node::new(NodeId::new(), "test".to_string(), NodeKind::Pure);
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
        let mut workflow = Workflow::new("test".to_string(), GuaranteeClass::BestEffort);
        let node1 = Node::new(NodeId::new(), "test1".to_string(), NodeKind::Pure);
        let node2 = Node::new(NodeId::new(), "test2".to_string(), NodeKind::Wait);
        let node1_id = node1.id.0.clone();
        let node2_id = node2.id.0.clone();

        workflow.add_node(node1);
        workflow.add_node(node2);

        let nodes = workflow.nodes_by_id();
        assert_eq!(nodes.len(), 2);
        assert!(nodes.contains_key(&node1_id));
        assert!(nodes.contains_key(&node2_id));
    }

    #[test]
    fn guarantee_class_badge_class_returns_distinct_css() {
        let exact = GuaranteeClass::ExactOnce.badge_class();
        let atleast = GuaranteeClass::AtLeastOnce.badge_class();
        let best = GuaranteeClass::BestEffort.badge_class();

        assert!(exact.contains("emerald"), "exact-once should use emerald");
        assert!(atleast.contains("amber"), "at-least-once should use amber");
        assert!(best.contains("red"), "best-effort should use red");
    }

    #[test]
    fn guarantee_class_icon_returns_shield_names() {
        assert_eq!(GuaranteeClass::ExactOnce.icon(), "shield-check");
        assert_eq!(GuaranteeClass::AtLeastOnce.icon(), "shield-alert");
        assert_eq!(GuaranteeClass::BestEffort.icon(), "shield-off");
    }

    #[test]
    fn workflow_stores_guarantee_class() {
        let wf = Workflow::new("test".to_string(), GuaranteeClass::ExactOnce);
        assert_eq!(wf.guarantee_class, GuaranteeClass::ExactOnce);
    }

    #[test]
    fn node_id_parse_accepts_valid_26_char_string() {
        let id = NodeId::parse("01ARYZ6S41TSV4RRFFQ69G5FAV");
        assert!(id.is_some());
        assert_eq!(id.unwrap().0, "01ARYZ6S41TSV4RRFFQ69G5FAV");
    }

    #[test]
    fn node_id_parse_rejects_empty_string() {
        assert_eq!(NodeId::parse(""), None);
    }

    #[test]
    fn node_id_parse_rejects_short_string() {
        assert_eq!(NodeId::parse("01ARYZ6S41TSV4RRFFQ69G5FA"), None);
    }

    #[test]
    fn node_id_parse_rejects_long_string() {
        assert_eq!(NodeId::parse("01ARYZ6S41TSV4RRFFQ69G5FAVG"), None);
    }

    #[test]
    fn node_id_display_shows_inner_value() {
        let id = NodeId::parse("01ARYZ6S41TSV4RRFFQ69G5FAV").unwrap();
        assert_eq!(format!("{}", id), "01ARYZ6S41TSV4RRFFQ69G5FAV");
    }

    #[test]
    fn port_name_from_str() {
        let port: PortName = "input".into();
        assert_eq!(port.0, "input");
    }

    #[test]
    fn port_name_display() {
        let port = PortName::from("output");
        assert_eq!(format!("{}", port), "output");
    }

    #[test]
    fn connection_clone_is_independent() {
        let conn = Connection {
            id: Uuid::new_v4(),
            source: NodeId::new(),
            target: NodeId::new(),
            source_port: PortName::from("out"),
            target_port: PortName::from("in"),
        };
        let cloned = conn.clone();
        assert_eq!(conn.id, cloned.id);
        assert_eq!(conn.source, cloned.source);
        assert_eq!(conn.target, cloned.target);
    }

    #[test]
    fn run_record_creation() {
        let record = RunRecord {
            id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            results: HashMap::new(),
            success: true,
        };
        assert!(record.success);
        assert!(record.results.is_empty());
    }

    #[test]
    fn validation_result_with_no_issues_is_valid() {
        let result = ValidationResult::new(vec![]);
        assert!(result.is_valid());
        assert_eq!(result.error_count(), 0);
        assert_eq!(result.warning_count(), 0);
    }

    #[test]
    fn validation_result_error_count() {
        let issues = vec![
            ValidationIssue {
                node_id: None,
                severity: ValidationSeverity::Error,
                message: "error 1".to_string(),
            },
            ValidationIssue {
                node_id: None,
                severity: ValidationSeverity::Warning,
                message: "warning 1".to_string(),
            },
            ValidationIssue {
                node_id: None,
                severity: ValidationSeverity::Error,
                message: "error 2".to_string(),
            },
        ];
        let result = ValidationResult::new(issues);
        assert!(!result.is_valid());
        assert_eq!(result.error_count(), 2);
        assert_eq!(result.warning_count(), 1);
    }

    #[test]
    fn validation_result_warning_count() {
        let issues = vec![
            ValidationIssue {
                node_id: None,
                severity: ValidationSeverity::Warning,
                message: "warning 1".to_string(),
            },
            ValidationIssue {
                node_id: None,
                severity: ValidationSeverity::Warning,
                message: "warning 2".to_string(),
            },
        ];
        let result = ValidationResult::new(issues);
        assert!(!result.is_valid());
        assert_eq!(result.warning_count(), 2);
    }

    #[test]
    fn execution_state_status_badge_class() {
        use super::ExecutionState;
        assert_eq!(
            ExecutionState::Idle.status_badge_class(),
            "bg-slate-100 text-slate-700 border-slate-200"
        );
        assert_eq!(
            ExecutionState::Running.status_badge_class(),
            "bg-blue-100 text-blue-700 border-blue-200"
        );
        assert_eq!(
            ExecutionState::Completed.status_badge_class(),
            "bg-green-100 text-green-700 border-green-200"
        );
        assert_eq!(
            ExecutionState::Failed.status_badge_class(),
            "bg-red-100 text-red-700 border-red-200"
        );
        assert_eq!(
            ExecutionState::Skipped.status_badge_class(),
            "bg-slate-100 text-slate-500 border-slate-200"
        );
    }

    #[test]
    fn execution_state_label() {
        use super::ExecutionState;
        assert_eq!(ExecutionState::Idle.label(), "pending");
        assert_eq!(ExecutionState::Queued.label(), "pending");
        assert_eq!(ExecutionState::Running.label(), "running");
        assert_eq!(ExecutionState::Completed.label(), "completed");
        assert_eq!(ExecutionState::Failed.label(), "failed");
        assert_eq!(ExecutionState::Skipped.label(), "skipped");
    }

    #[test]
    fn workflow_node_from_str() {
        use workflow_node::WorkflowNode;
        assert!(matches!(
            WorkflowNode::from_str("run"),
            Ok(WorkflowNode::Run(_))
        ));
        assert!(matches!(
            WorkflowNode::from_str("parallel"),
            Ok(WorkflowNode::Parallel(_))
        ));
        assert!(WorkflowNode::from_str("unknown").is_err());
    }

    #[test]
    fn workflow_node_default() {
        use workflow_node::WorkflowNode;
        assert!(matches!(WorkflowNode::default(), WorkflowNode::Run(_)));
    }

    #[test]
    fn node_apply_config_update_merges_values() {
        let mut node = Node::new(NodeId::new(), "test".to_string(), NodeKind::Pure);
        node.apply_config_update(&serde_json::json!({"key1": "value1"}));
        assert_eq!(node.config["key1"], "value1");

        node.apply_config_update(&serde_json::json!({"key2": "value2"}));
        assert_eq!(node.config["key1"], "value1");
        assert_eq!(node.config["key2"], "value2");

        node.apply_config_update(&serde_json::json!({"key1": "updated"}));
        assert_eq!(node.config["key1"], "updated");
    }

    #[test]
    fn node_apply_config_update_with_empty_object_is_noop() {
        let mut node = Node::new(NodeId::new(), "test".to_string(), NodeKind::Pure);
        node.apply_config_update(&serde_json::json!({"key": "value"}));
        let before = node.config.clone();
        node.apply_config_update(&serde_json::json!({}));
        assert_eq!(node.config, before);
    }

    #[test]
    fn node_apply_config_update_with_non_object_ignores() {
        let mut node = Node::new(NodeId::new(), "test".to_string(), NodeKind::Pure);
        let before = node.config.clone();
        node.apply_config_update(&serde_json::json!("not an object"));
        assert_eq!(node.config, before);
    }

    #[test]
    fn node_category_display() {
        assert_eq!(format!("{}", NodeCategory::Entry), "entry");
        assert_eq!(format!("{}", NodeCategory::Durable), "durable");
        assert_eq!(format!("{}", NodeCategory::State), "state");
        assert_eq!(format!("{}", NodeCategory::Flow), "flow");
        assert_eq!(format!("{}", NodeCategory::Timing), "timing");
        assert_eq!(format!("{}", NodeCategory::Signal), "signal");
    }

    #[test]
    fn workflow_remove_node_returns_true_when_existed() {
        let mut workflow = Workflow::new("test".to_string(), GuaranteeClass::BestEffort);
        let node = Node::new(NodeId::new(), "test".to_string(), NodeKind::Pure);
        let node_id = node.id.clone();
        workflow.add_node(node);
        assert_eq!(workflow.nodes.len(), 1);
        workflow.remove_node(node_id);
        assert_eq!(workflow.nodes.len(), 0);
    }

    #[test]
    fn workflow_get_node_mut() {
        let mut workflow = Workflow::new("test".to_string(), GuaranteeClass::BestEffort);
        let node = Node::new(NodeId::new(), "original".to_string(), NodeKind::Pure);
        let node_id = node.id.clone();
        workflow.add_node(node);

        if let Some(n) = workflow.get_node_mut(node_id.clone()) {
            n.name = "modified".to_string();
        }
        assert_eq!(workflow.get_node(node_id).unwrap().name, "modified");
    }

    #[test]
    fn workflow_multiple_nodes_with_same_kind() {
        let mut workflow = Workflow::new("test".to_string(), GuaranteeClass::BestEffort);
        for i in 0..5 {
            let node = Node::new(NodeId::new(), format!("node_{}", i), NodeKind::Pure);
            workflow.add_node(node);
        }
        assert_eq!(workflow.nodes.len(), 5);
    }
}
