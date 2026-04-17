//! Adversarial tests for UI badge guarantee accuracy (ADR-007/031).
//!
//! These tests verify that the UI badge system correctly maps NodeKind
//! variants to NodeCategory categories, and that badges update correctly
//! when node types change during workflow modification.

<<<<<<< HEAD
use super::graph::{node_kind_to_category, Node, NodeCategory, NodeId, Workflow};
=======
use crate::ui::graph::{node_kind_to_category, Node, NodeCategory, NodeId, Workflow};
>>>>>>> origin/vo-worker-tests
use vo_types::NodeKind;

// ============================================================================
// Adversarial Tests: Badge Shows Wrong Guarantee Level
// ============================================================================

#[test]
fn given_pure_node_kind_when_displaying_badge_then_shows_flow_category() {
    let node = Node::new(NodeId::new(), "test".to_string(), NodeKind::Pure);
    assert_eq!(node.category, NodeCategory::Flow);
}

#[test]
fn given_managed_effect_kind_when_displaying_badge_then_shows_durable_category() {
    let node = Node::new(NodeId::new(), "test".to_string(), NodeKind::ManagedEffect);
    assert_eq!(node.category, NodeCategory::Durable);
}

#[test]
fn given_wait_kind_when_displaying_badge_then_shows_timing_category() {
    let node = Node::new(NodeId::new(), "test".to_string(), NodeKind::Wait);
    assert_eq!(node.category, NodeCategory::Timing);
}

#[test]
fn given_signal_kind_when_displaying_badge_then_shows_signal_category() {
    let node = Node::new(NodeId::new(), "test".to_string(), NodeKind::Signal);
    assert_eq!(node.category, NodeCategory::Signal);
}

#[test]
fn given_unsafe_kind_when_displaying_badge_then_shows_flow_category() {
    let node = Node::new(NodeId::new(), "test".to_string(), NodeKind::Unsafe);
    assert_eq!(node.category, NodeCategory::Flow);
}

// ============================================================================
// Adversarial Tests: Badge Doesn't Update on Node Type Change
// ============================================================================

#[test]
fn given_node_when_kind_changes_from_pure_to_managed_effect_then_category_updates() {
    let mut node = Node::new(NodeId::new(), "test".to_string(), NodeKind::Pure);
    assert_eq!(node.category, NodeCategory::Flow);

    node.set_kind(NodeKind::ManagedEffect);
    assert_eq!(node.category, NodeCategory::Durable);
}

#[test]
fn given_node_when_kind_changes_from_wait_to_signal_then_category_updates() {
    let mut node = Node::new(NodeId::new(), "test".to_string(), NodeKind::Wait);
    assert_eq!(node.category, NodeCategory::Timing);

    node.set_kind(NodeKind::Signal);
    assert_eq!(node.category, NodeCategory::Signal);
}

#[test]
fn given_node_when_kind_changes_from_managed_effect_to_unsafe_then_category_updates() {
    let mut node = Node::new(NodeId::new(), "test".to_string(), NodeKind::ManagedEffect);
    assert_eq!(node.category, NodeCategory::Durable);

    node.set_kind(NodeKind::Unsafe);
    assert_eq!(node.category, NodeCategory::Flow);
}

// ============================================================================
// Adversarial Tests: Race Condition During Workflow Modification
// ============================================================================

#[test]
fn given_sequential_kind_changes_then_each_category_matches_current_kind() {
    let mut node = Node::new(NodeId::new(), "test".to_string(), NodeKind::Pure);

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
fn given_all_node_kinds_when_converted_to_category_then_all_map_correctly() {
    for kind in NodeKind::all_variants() {
        let category = node_kind_to_category(*kind);
        // Verify that every NodeKind maps to a valid NodeCategory
        match category {
            NodeCategory::Entry
            | NodeCategory::Durable
            | NodeCategory::State
            | NodeCategory::Flow
            | NodeCategory::Timing
            | NodeCategory::Signal => {}
        }
    }
}

#[test]
fn given_entry_and_state_categories_when_node_kind_converted_then_not_mapped() {
    // Entry and State categories are special - they don't come from NodeKind
    // They represent workflow structure rather than execution guarantees
    assert_ne!(node_kind_to_category(NodeKind::Pure), NodeCategory::Entry);
    assert_ne!(node_kind_to_category(NodeKind::Pure), NodeCategory::State);
}

// ============================================================================
// Integration Tests: Workflow-Level Badge Consistency
// ============================================================================

#[test]
fn given_workflow_with_multiple_nodes_when_displaying_all_badges_then_all_categories_valid() {
    let mut workflow = Workflow::default();

    for (kind, _expected_category) in [
        (NodeKind::Pure, NodeCategory::Flow),
        (NodeKind::ManagedEffect, NodeCategory::Durable),
        (NodeKind::Wait, NodeCategory::Timing),
        (NodeKind::Signal, NodeCategory::Signal),
        (NodeKind::Unsafe, NodeCategory::Flow),
    ] {
        let node = Node::new(NodeId::new(), "test".to_string(), kind);
        workflow.add_node(node);
    }

    for node in &workflow.nodes {
        let calculated = node_kind_to_category(node.kind);
        assert_eq!(
            node.category, calculated,
            "category mismatch for node {:?}",
            node.name
        );
    }
}
