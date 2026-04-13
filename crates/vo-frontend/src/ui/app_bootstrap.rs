//! Application bootstrap for the vo-frontend.
//!
//! Provides default workflows and initial state setup.

use vo_types::node_kind::NodeKind;

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
}
