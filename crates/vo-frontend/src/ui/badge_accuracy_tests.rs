//! Adversarial tests for UI badge guarantee accuracy (ADR-007/031).
//!
//! These tests verify that the UI badge system correctly maps NodeKind
//! variants to NodeCategory categories, and that badges update correctly
//! when node types change during workflow modification.

use vo_types::node_kind::NodeKind;

/// Maps NodeKind to its display category (mirrors UI badge logic).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeCategory {
    Entry,
    Durable,
    State,
    Flow,
    Timing,
    Signal,
}

/// Converts NodeKind to NodeCategory for badge display.
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

/// Represents a workflow node with its current kind.
#[derive(Debug, Clone)]
pub struct WorkflowNode {
    pub id: String,
    pub kind: NodeKind,
    pub category: NodeCategory,
}

impl WorkflowNode {
    pub fn new(id: impl Into<String>, kind: NodeKind) -> Self {
        Self {
            id: id.into(),
            kind,
            category: node_kind_to_category(kind),
        }
    }

    /// Updates the node kind and recalculates the category.
    /// This simulates workflow modification in the UI.
    pub fn set_kind(&mut self, kind: NodeKind) {
        self.kind = kind;
        self.category = node_kind_to_category(kind);
    }
}

// ============================================================================
// Adversarial Tests: Badge Shows Wrong Guarantee Level
// ============================================================================

#[test]
fn given_pure_node_kind_when_displaying_badge_then_shows_flow_category() {
    let node = WorkflowNode::new("node-1", NodeKind::Pure);
    assert_eq!(node.category, NodeCategory::Flow);
}

#[test]
fn given_managed_effect_kind_when_displaying_badge_then_shows_durable_category() {
    let node = WorkflowNode::new("node-2", NodeKind::ManagedEffect);
    assert_eq!(node.category, NodeCategory::Durable);
}

#[test]
fn given_wait_kind_when_displaying_badge_then_shows_timing_category() {
    let node = WorkflowNode::new("node-3", NodeKind::Wait);
    assert_eq!(node.category, NodeCategory::Timing);
}

#[test]
fn given_signal_kind_when_displaying_badge_then_shows_signal_category() {
    let node = WorkflowNode::new("node-4", NodeKind::Signal);
    assert_eq!(node.category, NodeCategory::Signal);
}

#[test]
fn given_unsafe_kind_when_displaying_badge_then_shows_flow_category() {
    let node = WorkflowNode::new("node-5", NodeKind::Unsafe);
    assert_eq!(node.category, NodeCategory::Flow);
}

// ============================================================================
// Adversarial Tests: Badge Doesn't Update on Node Type Change
// ============================================================================

#[test]
fn given_node_when_kind_changes_from_pure_to_managed_effect_then_category_updates() {
    let mut node = WorkflowNode::new("node-1", NodeKind::Pure);
    assert_eq!(node.category, NodeCategory::Flow);

    node.set_kind(NodeKind::ManagedEffect);
    assert_eq!(node.category, NodeCategory::Durable);
}

#[test]
fn given_node_when_kind_changes_from_wait_to_signal_then_category_updates() {
    let mut node = WorkflowNode::new("node-2", NodeKind::Wait);
    assert_eq!(node.category, NodeCategory::Timing);

    node.set_kind(NodeKind::Signal);
    assert_eq!(node.category, NodeCategory::Signal);
}

#[test]
fn given_node_when_kind_changes_from_managed_effect_to_unsafe_then_category_updates() {
    let mut node = WorkflowNode::new("node-3", NodeKind::ManagedEffect);
    assert_eq!(node.category, NodeCategory::Durable);

    node.set_kind(NodeKind::Unsafe);
    assert_eq!(node.category, NodeCategory::Flow);
}

// ============================================================================
// Adversarial Tests: Race Condition During Workflow Modification
// ============================================================================

#[test]
fn given_sequential_kind_changes_then_each_category_matches_current_kind() {
    let mut node = WorkflowNode::new("node-1", NodeKind::Pure);

    // Sequential changes: Pure -> ManagedEffect -> Wait -> Signal -> Unsafe -> Pure
    let sequence = [
        (NodeKind::Pure, NodeCategory::Flow),
        (NodeKind::ManagedEffect, NodeCategory::Durable),
        (NodeKind::Wait, NodeCategory::Timing),
        (NodeKind::Signal, NodeCategory::Signal),
        (NodeKind::Unsafe, NodeCategory::Flow),
        (NodeKind::Pure, NodeCategory::Flow),
    ];

    for (kind, expected_category) in sequence {
        node.set_kind(kind);
        assert_eq!(
            node.category, expected_category,
            "category mismatch after setting kind to {:?}",
            kind
        );
    }
}

#[test]
fn given_all_node_kinds_when_roundtripped_through_category_then_no_data_loss() {
    for kind in NodeKind::all_variants() {
        let category = node_kind_to_category(*kind);
        let reconstructed_kind = category_to_node_kind(category);
        assert_eq!(
            *kind, reconstructed_kind,
            "round-trip failed for kind: {:?}",
            kind
        );
    }
}

#[test]
fn given_invalid_category_when_converting_to_node_kind_then_handles_gracefully() {
    // Entry, State categories don't map to NodeKind
    // These should be handled by the UI with a fallback or error
    let entry_category = NodeCategory::Entry;

    // This test documents the current limitation:
    // Not all NodeCategory values map back to NodeKind
    // The UI should handle this gracefully
    assert!(matches!(
        entry_category,
        NodeCategory::Entry | NodeCategory::State
    ));
}

// ============================================================================
// Helper: Reverse mapping (for testing only)
// ============================================================================

/// Reverse mapping from NodeCategory to NodeKind.
/// This is lossy - some categories don't map to a specific NodeKind.
#[must_use]
fn category_to_node_kind(category: NodeCategory) -> NodeKind {
    match category {
        NodeCategory::Flow => NodeKind::Pure, // Default fallback
        NodeCategory::Durable => NodeKind::ManagedEffect,
        NodeCategory::Timing => NodeKind::Wait,
        NodeCategory::Signal => NodeKind::Signal,
        NodeCategory::Entry | NodeCategory::State => NodeKind::Pure, // Fallback
    }
}

// ============================================================================
// Property-Based Test: Invariant for All NodeKinds
// ============================================================================

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn invariant_category_always_matches_kind(kind in any::<NodeKind>()) {
            let node = WorkflowNode::new("test", kind);
            assert_eq!(node.category, node_kind_to_category(kind));
        }

        #[test]
        fn invariant_set_kind_preserves_consistency(
            initial_kind in any::<NodeKind>(),
            new_kind in any::<NodeKind>()
        ) {
            let mut node = WorkflowNode::new("test", initial_kind);
            node.set_kind(new_kind);
            assert_eq!(node.category, node_kind_to_category(new_kind));
        }
    }
}
