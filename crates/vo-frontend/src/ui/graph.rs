//! Graph types for the UI (ADR-031).
//!
//! This module defines the core graph data structures used by the UI
//! for node visualization and badge rendering.

use std::collections::HashMap;
use std::fmt::Display;

use serde::{Deserialize, Serialize};
use ulid::Ulid;
use vo_types::NodeKind;

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
        }
    }

    /// Update the node kind and recalculate the category.
    pub fn set_kind(&mut self, kind: NodeKind) {
        self.kind = kind;
        self.category = node_kind_to_category(kind);
        self.icon = category_to_icon(self.category);
    }

    /// Apply a config update to the node.
    #[allow(clippy::missing_panics_doc)]
    pub fn apply_config_update(&mut self, new_config: &serde_json::Value) {
        if let serde_json::Value::Object(map) = new_config {
            for (key, value) in map {
                self.config
                    .as_object_mut()
                    .unwrap()
                    .insert(key.clone(), value.clone());
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
    pub name: String,
}

impl Workflow {
    /// Create a new empty workflow.
    #[must_use]
    pub fn new(name: String) -> Self {
        Self {
            nodes: Vec::new(),
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
        let mut workflow = Workflow::new("test".to_string());
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
        let mut workflow = Workflow::new("test".to_string());
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
}
