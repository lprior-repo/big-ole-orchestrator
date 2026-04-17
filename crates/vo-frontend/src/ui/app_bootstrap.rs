//! Application bootstrap for the vo-frontend.
//!
//! Provides default workflows and initial state setup.

<<<<<<< HEAD
use vo_types::NodeKind;
=======
use vo_types::node_kind::NodeKind;
>>>>>>> origin/vo-worker-tests

use crate::ui::graph::{Node, NodeCategory, NodeId, Workflow};

pub fn default_workflow() -> Workflow {
    let mut workflow = Workflow::new("default".to_string());

    // Entry node - HTTP Handler
    let entry_node = Node::new(NodeId::new(), "HTTP Handler".to_string(), NodeKind::Pure);
    // We need to manually set category to Entry for the entry node
    let mut entry_node = entry_node;
    entry_node.category = NodeCategory::Entry;
    entry_node.kind = NodeKind::Pure;
    workflow.add_node(entry_node);

    // Durable node
    let durable_node = Node::new(
        NodeId::new(),
        "Durable Step".to_string(),
        NodeKind::ManagedEffect,
    );
    workflow.add_node(durable_node);

    // Flow node (condition)
    let flow_node = Node::new(NodeId::new(), "If / Else".to_string(), NodeKind::Pure);
    workflow.add_node(flow_node);

    workflow
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::graph::NodeCategory;

    #[test]
    fn given_default_workflow_when_created_then_it_contains_expected_starter_nodes() {
        let workflow = default_workflow();

        assert_eq!(workflow.nodes.len(), 3);
        assert_eq!(workflow.nodes[0].category, NodeCategory::Entry);
    }
<<<<<<< HEAD

    #[test]
    fn given_default_workflow_when_created_then_first_node_category_is_entry() {
        let workflow = default_workflow();
        assert_eq!(workflow.nodes[0].category, NodeCategory::Entry);
    }

    #[test]
    fn given_default_workflow_when_created_then_node_kinds_are_correct() {
        let workflow = default_workflow();
        let kinds: Vec<_> = workflow.nodes.iter().map(|n| n.kind).collect();
        assert_eq!(
            kinds,
            vec![NodeKind::Pure, NodeKind::ManagedEffect, NodeKind::Pure]
        );
    }

    #[test]
    fn given_default_workflow_when_created_then_workflow_name_is_default() {
        let workflow = default_workflow();
        assert_eq!(workflow.name, "default");
    }

    #[test]
    fn given_default_workflow_when_created_then_all_nodes_have_valid_ids() {
        let workflow = default_workflow();
        for node in &workflow.nodes {
            let id_str: &str = node.id.as_str();
            assert!(!id_str.is_empty(), "Node ID should not be empty");
            assert_eq!(id_str.len(), 26, "Node ID should be 26-char ULID");
        }
    }
=======
>>>>>>> origin/vo-worker-tests
}
