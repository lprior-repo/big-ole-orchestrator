#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

use crate::graph::{Connection, NodeId, Workflow};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowEvent {
    ActivityCompleted(NodeId),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SimDagState {
    pub completed: HashSet<NodeId>,
    pub event_log: Vec<WorkflowEvent>,
}

impl SimDagState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimError {
    NodeNotFound,
    AlreadyCompleted,
    NotReady,
}

impl std::fmt::Display for SimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NodeNotFound => write!(f, "node not found"),
            Self::AlreadyCompleted => write!(f, "node already completed"),
            Self::NotReady => write!(f, "node not ready (dependencies unmet)"),
        }
    }
}

impl std::error::Error for SimError {}

#[must_use]
pub fn readiness_check(workflow: &Workflow, completed: &HashSet<NodeId>) -> HashSet<NodeId> {
    workflow
        .nodes
        .iter()
        .filter(|node| !completed.contains(&node.id))
        .filter(|node| are_dependencies_met(workflow, &node.id, completed))
        .map(|node| node.id)
        .collect()
}

fn are_dependencies_met(
    workflow: &Workflow,
    node_id: &NodeId,
    completed: &HashSet<NodeId>,
) -> bool {
    let has_incoming: bool = workflow
        .connections
        .iter()
        .any(|conn| conn.target == *node_id);
    if !has_incoming {
        return true;
    }
    workflow
        .connections
        .iter()
        .filter(|conn| conn.target == *node_id)
        .all(|conn| completed.contains(&conn.source))
}

#[must_use]
pub fn is_dag_complete(workflow: &Workflow, completed: &HashSet<NodeId>) -> bool {
    workflow
        .nodes
        .iter()
        .all(|node| completed.contains(&node.id))
}

pub fn complete_node(
    state: &mut SimDagState,
    workflow: &Workflow,
    node_id: NodeId,
) -> Result<(), SimError> {
    let node_exists = workflow.nodes.iter().any(|n| n.id == node_id);
    if !node_exists {
        return Err(SimError::NodeNotFound);
    }
    if state.completed.contains(&node_id) {
        return Err(SimError::AlreadyCompleted);
    }
    let ready_nodes = readiness_check(workflow, &state.completed);
    if !ready_nodes.contains(&node_id) {
        return Err(SimError::NotReady);
    }
    state.completed.insert(node_id);
    state
        .event_log
        .push(WorkflowEvent::ActivityCompleted(node_id));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Node, NodeCategory, PortName, WorkflowNode};

    fn create_test_workflow_with_chain() -> (Workflow, NodeId, NodeId, NodeId) {
        let mut workflow = Workflow::new();
        let node_a = workflow.add_node("run", 0.0, 0.0);
        let node_b = workflow.add_node("run", 100.0, 0.0);
        let node_c = workflow.add_node("run", 200.0, 0.0);
        let main = PortName::from("main");
        let _ = workflow.add_connection(node_a, node_b, &main, &main);
        let _ = workflow.add_connection(node_b, node_c, &main, &main);
        (workflow, node_a, node_b, node_c)
    }

    fn create_test_workflow_with_parallel() -> (Workflow, NodeId, NodeId, NodeId) {
        let mut workflow = Workflow::new();
        let node_r = workflow.add_node("run", 0.0, 0.0);
        let node_a = workflow.add_node("run", 100.0, -50.0);
        let node_b = workflow.add_node("run", 100.0, 50.0);
        let main = PortName::from("main");
        let _ = workflow.add_connection(node_r, node_a, &main, &main);
        let _ = workflow.add_connection(node_r, node_b, &main, &main);
        (workflow, node_r, node_a, node_b)
    }

    #[test]
    fn readiness_check_returns_root_nodes_when_nothing_completed() {
        let (workflow, node_a, _, _) = create_test_workflow_with_chain();
        let completed = HashSet::new();
        let ready = readiness_check(&workflow, &completed);
        assert!(ready.contains(&node_a));
        assert_eq!(ready.len(), 1);
    }

    #[test]
    fn readiness_check_returns_multiple_ready_nodes() {
        let (workflow, node_r, node_a, node_b) = create_test_workflow_with_parallel();
        let completed = HashSet::new();
        let ready = readiness_check(&workflow, &completed);
        assert!(ready.contains(&node_r));
        assert_eq!(ready.len(), 1);
        drop(node_a);
        drop(node_b);
    }

    #[test]
    fn complete_node_appends_activity_completed_to_event_log() {
        let (workflow, node_a, _, _) = create_test_workflow_with_chain();
        let mut state = SimDagState::new();
        let result = complete_node(&mut state, &workflow, node_a);
        assert!(result.is_ok());
        assert_eq!(state.event_log.len(), 1);
        assert!(matches!(
            state.event_log.last(),
            Some(WorkflowEvent::ActivityCompleted(id)) if *id == node_a
        ));
    }

    #[test]
    fn complete_node_adds_node_to_completed() {
        let (workflow, node_a, _, _) = create_test_workflow_with_chain();
        let mut state = SimDagState::new();
        let result = complete_node(&mut state, &workflow, node_a);
        assert!(result.is_ok());
        assert!(state.completed.contains(&node_a));
    }

    #[test]
    fn is_dag_complete_returns_true_when_all_nodes_completed() {
        let (workflow, node_a, node_b, node_c) = create_test_workflow_with_chain();
        let mut completed = HashSet::new();
        completed.insert(node_a);
        completed.insert(node_b);
        completed.insert(node_c);
        assert!(is_dag_complete(&workflow, &completed));
    }

    #[test]
    fn complete_node_returns_node_not_found_for_invalid_id() {
        let (workflow, node_a, _, _) = create_test_workflow_with_chain();
        let mut state = SimDagState::new();
        let fake_id = NodeId::new();
        let result = complete_node(&mut state, &workflow, fake_id);
        assert!(matches!(result, Err(SimError::NodeNotFound)));
        drop(node_a);
    }

    #[test]
    fn complete_node_returns_already_completed_for_duplicate() {
        let (workflow, node_a, _, _) = create_test_workflow_with_chain();
        let mut state = SimDagState::new();
        let _ = complete_node(&mut state, &workflow, node_a);
        let result = complete_node(&mut state, &workflow, node_a);
        assert!(matches!(result, Err(SimError::AlreadyCompleted)));
    }

    #[test]
    fn complete_node_returns_not_ready_when_dependencies_unmet() {
        let (workflow, _, node_b, _) = create_test_workflow_with_chain();
        let mut state = SimDagState::new();
        let result = complete_node(&mut state, &workflow, node_b);
        assert!(matches!(result, Err(SimError::NotReady)));
    }

    #[test]
    fn readiness_check_with_single_node_workflow() {
        let mut workflow = Workflow::new();
        let node_a = workflow.add_node("run", 0.0, 0.0);
        let completed = HashSet::new();
        let ready = readiness_check(&workflow, &completed);
        assert!(ready.contains(&node_a));
        assert_eq!(ready.len(), 1);
    }

    #[test]
    fn is_dag_complete_returns_false_when_partial_completion() {
        let (workflow, node_a, node_b, node_c) = create_test_workflow_with_chain();
        let mut completed = HashSet::new();
        completed.insert(node_a);
        completed.insert(node_b);
        assert!(!is_dag_complete(&workflow, &completed));
        drop(node_c);
    }

    #[test]
    fn empty_workflow_is_immediately_complete() {
        let workflow = Workflow::new();
        let completed = HashSet::new();
        assert!(is_dag_complete(&workflow, &completed));
    }

    #[test]
    fn readiness_check_allows_multiple_ready_nodes_after_root_complete() {
        let (workflow, node_r, node_a, node_b) = create_test_workflow_with_parallel();
        let mut completed = HashSet::new();
        completed.insert(node_r);
        let ready = readiness_check(&workflow, &completed);
        assert!(ready.contains(&node_a));
        assert!(ready.contains(&node_b));
        assert_eq!(ready.len(), 2);
        drop(node_r);
    }

    #[test]
    fn completed_intersection_with_ready_is_empty() {
        let (workflow, node_a, _, _) = create_test_workflow_with_chain();
        let mut state = SimDagState::new();
        let _ = complete_node(&mut state, &workflow, node_a);
        let ready = readiness_check(&workflow, &state.completed);
        let intersection: HashSet<_> = state.completed.intersection(&ready).collect();
        assert!(intersection.is_empty());
    }

    #[test]
    fn completed_always_subset_of_workflow_nodes() {
        let (workflow, node_a, node_b, _) = create_test_workflow_with_chain();
        let mut state = SimDagState::new();
        let _ = complete_node(&mut state, &workflow, node_a);
        let _ = complete_node(&mut state, &workflow, node_b);
        let workflow_ids: HashSet<NodeId> = workflow.nodes.iter().map(|n| n.id).collect();
        assert!(state.completed.is_subset(&workflow_ids));
    }
}
