use super::helpers::*;
use crate::ui::edges::graph_types::NodeId;
use crate::ui::edges::layout::find_parallel_branches;
use uuid::Uuid;

#[test]
fn given_source_with_two_targets_when_find_parallel_then_returns_one_group() {
    let source_id = NodeId::new();
    let target_a_id = NodeId::new();
    let target_b_id = NodeId::new();

    let source = build_parallel_node(source_id, 100.0, 100.0);
    let target_a = build_node(target_a_id, 300.0, 100.0);
    let target_b = build_node(target_b_id, 300.0, 200.0);

    let nodes = vec![source.clone(), target_a.clone(), target_b.clone()];

    let conn_a = build_connection(Uuid::new_v4(), source_id, target_a_id);
    let conn_b = build_connection(Uuid::new_v4(), source_id, target_b_id);
    let connections = vec![conn_a, conn_b];

    let groups = find_parallel_branches(&nodes, &connections);

    assert_eq!(groups.len(), 1);
    let group = &groups[0];

    assert_eq!(group.parallel_node_id, source_id);
    assert_eq!(group.branch_node_ids.len(), 2);
    // Target nodes are sorted by ID lexicographically
    let mut sorted_ids = [target_a_id, target_b_id];
    sorted_ids.sort_by_key(|left| left.0);
    assert_eq!(group.branch_node_ids[0], sorted_ids[0]);
    assert_eq!(group.branch_node_ids[1], sorted_ids[1]);
    assert_eq!(group.bounding_box.x, 292.0);
    assert_eq!(group.bounding_box.y, 92.0);
    assert_eq!(group.bounding_box.width, 236.0);
    assert_eq!(group.bounding_box.height, 184.0);
}

#[test]
fn given_source_with_three_targets_when_find_parallel_then_returns_one_group() {
    let source_id = NodeId::new();
    let target_a_id = NodeId::new();
    let target_b_id = NodeId::new();
    let target_c_id = NodeId::new();

    let source = build_parallel_node(source_id, 100.0, 100.0);
    let target_a = build_node(target_a_id, 300.0, 100.0);
    let target_b = build_node(target_b_id, 300.0, 200.0);
    let target_c = build_node(target_c_id, 300.0, 300.0);

    let nodes = vec![source, target_a, target_b, target_c];

    let conn_a = build_connection(Uuid::new_v4(), source_id, target_a_id);
    let conn_b = build_connection(Uuid::new_v4(), source_id, target_b_id);
    let conn_c = build_connection(Uuid::new_v4(), source_id, target_c_id);
    let connections = vec![conn_a, conn_b, conn_c];

    let groups = find_parallel_branches(&nodes, &connections);

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].branch_node_ids.len(), 3);
}

#[test]
fn given_source_with_many_targets_when_find_parallel_then_returns_one_group() {
    let source_id = NodeId::new();
    let mut target_ids = vec![];
    let mut nodes = vec![];
    let mut connections = vec![];

    for i in 0..5 {
        let target_id = NodeId::new();
        target_ids.push(target_id);
        nodes.push(build_node(target_id, 300.0, 100.0 + (i as f32) * 100.0));
        connections.push(build_connection(Uuid::new_v4(), source_id, target_id));
    }

    let source = build_parallel_node(source_id, 100.0, 100.0);
    nodes.insert(0, source);

    let groups = find_parallel_branches(&nodes, &connections);

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].branch_node_ids.len(), 5);
}

#[test]
fn given_single_connection_when_find_parallel_then_returns_empty_vec() {
    let source_id = NodeId::new();
    let target_id = NodeId::new();

    let source = build_node(source_id, 100.0, 100.0);
    let target = build_node(target_id, 300.0, 100.0);

    let nodes = vec![source, target];

    let connection = build_connection(Uuid::new_v4(), source_id, target_id);
    let connections = vec![connection];

    let groups = find_parallel_branches(&nodes, &connections);

    assert!(groups.is_empty());
}

#[test]
fn given_empty_connections_when_find_parallel_then_returns_empty_vec() {
    let nodes = vec![];
    let connections = vec![];

    let groups = find_parallel_branches(&nodes, &connections);

    assert!(groups.is_empty());
}

#[test]
fn given_empty_nodes_when_find_parallel_then_returns_empty_vec() {
    let nodes = vec![];
    let source_id = NodeId::new();
    let target_id = NodeId::new();

    let connection = build_connection(Uuid::new_v4(), source_id, target_id);
    let connections = vec![connection];

    let groups = find_parallel_branches(&nodes, &connections);

    assert!(groups.is_empty());
}

#[test]
fn given_many_non_parallel_sources_when_find_parallel_then_returns_empty_vec() {
    let mut nodes = vec![];
    let mut connections = vec![];

    for i in 0..10 {
        let source_id = NodeId::new();
        let target_id = NodeId::new();

        let source = build_node(source_id, 100.0, (i as f32) * 200.0);
        let target = build_node(target_id, 300.0, (i as f32) * 200.0);

        nodes.push(source);
        nodes.push(target);

        connections.push(build_connection(Uuid::new_v4(), source_id, target_id));
    }

    let groups = find_parallel_branches(&nodes, &connections);

    assert!(groups.is_empty());
}

#[test]
fn given_duplicate_connections_when_find_parallel_then_treats_as_single_connection() {
    let source_id = NodeId::new();
    let target_id = NodeId::new();

    let source = build_node(source_id, 100.0, 100.0);
    let target = build_node(target_id, 300.0, 100.0);

    let nodes = vec![source, target];

    // Two connections from same source to same target
    let conn_a = build_connection(Uuid::new_v4(), source_id, target_id);
    let conn_b = build_connection(Uuid::new_v4(), source_id, target_id);
    let connections = vec![conn_a, conn_b];

    let groups = find_parallel_branches(&nodes, &connections);

    assert!(groups.is_empty());
}

#[test]
fn given_mixed_parallel_and_non_parallel_when_find_parallel_then_only_parallel_returned() {
    let source_a_id = NodeId::new();
    let source_b_id = NodeId::new();
    let target_a1_id = NodeId::new();
    let target_a2_id = NodeId::new();
    let target_b1_id = NodeId::new();

    let source_a = build_parallel_node(source_a_id, 100.0, 100.0);
    let source_b = build_node(source_b_id, 100.0, 300.0);
    let target_a1 = build_node(target_a1_id, 300.0, 100.0);
    let target_a2 = build_node(target_a2_id, 300.0, 200.0);
    let target_b1 = build_node(target_b1_id, 300.0, 300.0);

    let nodes = vec![source_a, source_b, target_a1, target_a2, target_b1];

    let conn_a1 = build_connection(Uuid::new_v4(), source_a_id, target_a1_id);
    let conn_a2 = build_connection(Uuid::new_v4(), source_a_id, target_a2_id);
    let conn_b1 = build_connection(Uuid::new_v4(), source_b_id, target_b1_id);
    let connections = vec![conn_a1, conn_a2, conn_b1];

    let groups = find_parallel_branches(&nodes, &connections);

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].branch_node_ids.len(), 2);
}

// ==================== Explicit Parallel Source Gating Tests ====================

#[test]
fn given_non_parallel_node_with_two_targets_when_find_parallel_then_returns_empty() {
    // Even with >=2 outgoing edges, non-Parallel nodes should NOT create parallel groups
    let source_id = NodeId::new();
    let target_a_id = NodeId::new();
    let target_b_id = NodeId::new();

    let source = build_node(source_id, 100.0, 100.0); // Not a Parallel node
    let target_a = build_node(target_a_id, 300.0, 100.0);
    let target_b = build_node(target_b_id, 300.0, 200.0);

    let nodes = vec![source.clone(), target_a.clone(), target_b.clone()];

    let conn_a = build_connection(Uuid::new_v4(), source_id, target_a_id);
    let conn_b = build_connection(Uuid::new_v4(), source_id, target_b_id);
    let connections = vec![conn_a, conn_b];

    let groups = find_parallel_branches(&nodes, &connections);

    // Should be empty because source is not WorkflowNode::Parallel
    assert!(groups.is_empty());
}

#[test]
fn given_parallel_node_with_two_targets_when_find_parallel_then_returns_one_group() {
    let source_id = NodeId::new();
    let target_a_id = NodeId::new();
    let target_b_id = NodeId::new();

    let source = build_parallel_node(source_id, 100.0, 100.0); // Explicit Parallel node
    let target_a = build_node(target_a_id, 300.0, 100.0);
    let target_b = build_node(target_b_id, 300.0, 200.0);

    let nodes = vec![source.clone(), target_a.clone(), target_b.clone()];

    let conn_a = build_connection(Uuid::new_v4(), source_id, target_a_id);
    let conn_b = build_connection(Uuid::new_v4(), source_id, target_b_id);
    let connections = vec![conn_a, conn_b];

    let groups = find_parallel_branches(&nodes, &connections);

    assert_eq!(groups.len(), 1);
    let group = &groups[0];

    assert_eq!(group.parallel_node_id, source_id);
    assert_eq!(group.branch_node_ids.len(), 2);
}

#[test]
fn given_multiple_parallel_nodes_when_find_parallel_then_returns_groups_for_each() {
    let source_a_id = NodeId::new();
    let source_b_id = NodeId::new();
    let target_a1_id = NodeId::new();
    let target_a2_id = NodeId::new();
    let target_b1_id = NodeId::new();
    let target_b2_id = NodeId::new();

    let source_a = build_parallel_node(source_a_id, 100.0, 100.0);
    let source_b = build_parallel_node(source_b_id, 100.0, 300.0);
    let target_a1 = build_node(target_a1_id, 300.0, 100.0);
    let target_a2 = build_node(target_a2_id, 300.0, 200.0);
    let target_b1 = build_node(target_b1_id, 300.0, 300.0);
    let target_b2 = build_node(target_b2_id, 300.0, 400.0);

    let nodes = vec![
        source_a, source_b, target_a1, target_a2, target_b1, target_b2,
    ];

    let conn_a1 = build_connection(Uuid::new_v4(), source_a_id, target_a1_id);
    let conn_a2 = build_connection(Uuid::new_v4(), source_a_id, target_a2_id);
    let conn_b1 = build_connection(Uuid::new_v4(), source_b_id, target_b1_id);
    let conn_b2 = build_connection(Uuid::new_v4(), source_b_id, target_b2_id);
    let connections = vec![conn_a1, conn_a2, conn_b1, conn_b2];

    let groups = find_parallel_branches(&nodes, &connections);

    assert_eq!(groups.len(), 2);
}
