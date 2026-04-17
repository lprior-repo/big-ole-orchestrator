use super::helpers::*;
use crate::ui::edges::graph_types::NodeId;
use crate::ui::edges::layout::{find_parallel_branches, resolve_edge_anchors_with_parallel};
use crate::ui::parallel_group_overlay::{AggregateStatus, BoundingBox, ParallelGroup};
use uuid::Uuid;

// ==================== resolve_edge_anchors_with_parallel Tests ====================

#[test]
fn given_parallel_groups_when_resolve_anchors_then_offsets_applied_to_targets() {
    let source_id = NodeId::new();
    let target_a_id = NodeId::new();
    let target_b_id = NodeId::new();

    let source = build_node(source_id, 100.0, 100.0);
    let target_a = build_node(target_a_id, 300.0, 100.0);
    let target_b = build_node(target_b_id, 300.0, 200.0);

    let nodes = vec![source, target_a.clone(), target_b.clone()];

    let conn_a = build_connection(Uuid::new_v4(), source_id, target_a_id);
    let conn_b = build_connection(Uuid::new_v4(), source_id, target_b_id);
    let connections = vec![conn_a, conn_b];

    // Create parallel group
    let group = ParallelGroup {
        parallel_node_id: source_id,
        branch_node_ids: vec![target_a_id, target_b_id],
        bounding_box: BoundingBox {
            x: 292.0,
            y: 92.0,
            width: 16.0,
            height: 116.0,
        },
        branch_count: 2,
        aggregate_status: AggregateStatus::Pending,
    };
    let groups = vec![group];

    let anchors = resolve_edge_anchors_with_parallel(&connections, &nodes, &groups);

    let anchor_a = anchors.get(&connections[0].id.to_string()).copied();
    let anchor_b = anchors.get(&connections[1].id.to_string()).copied();

    assert!(anchor_a.is_some());
    assert!(anchor_b.is_some());

    let anchor_a = anchor_a.unwrap();
    let anchor_b = anchor_b.unwrap();

    let spacing = NODE_HEIGHT / 2.5;
    let mut sorted_ids = [target_a_id, target_b_id];
    sorted_ids.sort_by(|left, right| left.0.cmp(&right.0));

    let expected_offset_a = if target_a_id == sorted_ids[0] {
        -spacing / 2.0
    } else {
        spacing / 2.0
    };
    let expected_offset_b = -expected_offset_a;

    assert_eq!(anchor_a.from.x, 320.0); // source.x + NODE_WIDTH
    assert_eq!(anchor_a.from.y, 134.0); // source.y + NODE_HEIGHT / 2
    assert_eq!(anchor_a.to.y, 134.0 + expected_offset_a);

    assert_eq!(anchor_b.from.x, 320.0);
    assert_eq!(anchor_b.from.y, 134.0);
    assert_eq!(anchor_b.to.y, 234.0 + expected_offset_b);
}

#[test]
fn given_non_parallel_edges_when_resolve_anchors_then_no_offsets_applied() {
    let source_id = NodeId::new();
    let target_id = NodeId::new();

    let source = build_node(source_id, 100.0, 100.0);
    let target = build_node(target_id, 300.0, 100.0);

    let nodes = vec![source, target];

    let connection = build_connection(Uuid::new_v4(), source_id, target_id);
    let connections = vec![connection.clone()];

    let groups: Vec<ParallelGroup> = vec![];

    let anchors = resolve_edge_anchors_with_parallel(&connections, &nodes, &groups);

    let anchor = anchors.get(&connection.id.to_string()).copied();

    assert!(anchor.is_some());
    let anchor = anchor.unwrap();

    // No offset applied since no parallel group
    assert_eq!(anchor.to.y, 134.0); // target.y + NODE_HEIGHT / 2
}

#[test]
fn given_mixed_parallel_and_non_parallel_edges_when_resolve_anchors() {
    let source_id = NodeId::new();
    let target_a_id = NodeId::new();
    let target_b_id = NodeId::new();
    let target_c_id = NodeId::new();

    let source = build_node(source_id, 100.0, 100.0);
    let target_a = build_node(target_a_id, 300.0, 100.0);
    let target_b = build_node(target_b_id, 300.0, 200.0);
    let target_c = build_node(target_c_id, 300.0, 300.0);

    let nodes = vec![source, target_a.clone(), target_b.clone(), target_c.clone()];

    let conn_a = build_connection(Uuid::new_v4(), source_id, target_a_id);
    let conn_b = build_connection(Uuid::new_v4(), source_id, target_b_id);
    let conn_c = build_connection(Uuid::new_v4(), source_id, target_c_id);
    let connections = vec![conn_a.clone(), conn_b.clone(), conn_c.clone()];

    // Only target_a and target_b are in parallel group
    let group = ParallelGroup {
        parallel_node_id: source_id,
        branch_node_ids: vec![target_a_id, target_b_id],
        bounding_box: BoundingBox {
            x: 292.0,
            y: 92.0,
            width: 16.0,
            height: 116.0,
        },
        branch_count: 2,
        aggregate_status: AggregateStatus::Pending,
    };
    let groups = vec![group];

    let anchors = resolve_edge_anchors_with_parallel(&connections, &nodes, &groups);

    let anchor_a = anchors.get(&conn_a.id.to_string()).copied();
    let anchor_b = anchors.get(&conn_b.id.to_string()).copied();
    let anchor_c = anchors.get(&conn_c.id.to_string()).copied();

    let spacing = NODE_HEIGHT / 2.5;
    let mut sorted_ids = [target_a_id, target_b_id];
    sorted_ids.sort_by(|left, right| left.0.cmp(&right.0));

    let expected_offset_a = if target_a_id == sorted_ids[0] {
        -spacing / 2.0
    } else {
        spacing / 2.0
    };
    let expected_offset_b = -expected_offset_a;

    // Parallel edges have offsets
    assert_eq!(anchor_a.unwrap().to.y, 134.0 + expected_offset_a);
    assert_eq!(anchor_b.unwrap().to.y, 234.0 + expected_offset_b);

    // Non-parallel edge has no offset
    assert_eq!(anchor_c.unwrap().to.y, 334.0);
}

// ==================== Integration Tests ====================

#[test]
fn given_workflow_with_parallel_branches_when_full_pipeline_then_correct_output() {
    // Complete workflow: nodes + connections -> parallel groups -> edge anchors

    let source_id = NodeId::new();
    let target_a_id = NodeId::new();
    let target_b_id = NodeId::new();

    let source = build_parallel_node(source_id, 100.0, 100.0);
    let target_a = build_node(target_a_id, 300.0, 100.0);
    let target_b = build_node(target_b_id, 300.0, 200.0);

    let nodes = vec![source, target_a, target_b];

    let conn_a = build_connection(Uuid::new_v4(), source_id, target_a_id);
    let conn_b = build_connection(Uuid::new_v4(), source_id, target_b_id);
    let connections = vec![conn_a.clone(), conn_b.clone()];

    // Step 1: Find parallel groups
    let groups = find_parallel_branches(&nodes, &connections);
    assert_eq!(groups.len(), 1);

    // Step 2: Resolve edge anchors with parallel groups
    let anchors = resolve_edge_anchors_with_parallel(&connections, &nodes, &groups);

    // Step 3: Verify anchors exist and have correct structure
    assert_eq!(anchors.len(), 2);

    let anchor_a = anchors.get(&conn_a.id.to_string()).copied().unwrap();
    let anchor_b = anchors.get(&conn_b.id.to_string()).copied().unwrap();

    // Both anchors start from same source point
    assert_eq!(anchor_a.from.x, anchor_b.from.x);
    assert_eq!(anchor_a.from.y, anchor_b.from.y);

    // Anchor to is at target position
    assert_eq!(anchor_a.to.x, 300.0);
    assert_eq!(anchor_b.to.x, 300.0);
}

// ==================== Shared Target Disambiguation Test ====================

#[test]
fn given_shared_target_across_sources_when_resolve_anchors_then_uses_source_target_match() {
    // Scenario: Two different Parallel sources both point to the SAME target
    // The anchor resolution should correctly associate each edge with its source
    let source_a_id = NodeId::new();
    let source_b_id = NodeId::new();
    let shared_target_id = NodeId::new();

    // Both sources must be Parallel nodes for parallel group detection
    let source_a = build_parallel_node(source_a_id, 100.0, 100.0);
    let source_b = build_parallel_node(source_b_id, 100.0, 300.0);
    let shared_target = build_node(shared_target_id, 300.0, 200.0);

    let nodes = vec![source_a.clone(), source_b.clone(), shared_target.clone()];

    let conn_a = build_connection(Uuid::new_v4(), source_a_id, shared_target_id);
    let conn_b = build_connection(Uuid::new_v4(), source_b_id, shared_target_id);
    let connections = vec![conn_a.clone(), conn_b.clone()];

    // Create parallel groups for each source (each has single target)
    let group_a = ParallelGroup {
        parallel_node_id: source_a_id,
        branch_node_ids: vec![shared_target_id],
        bounding_box: BoundingBox {
            x: 292.0,
            y: 192.0,
            width: 236.0,
            height: 84.0,
        },
        branch_count: 1,
        aggregate_status: AggregateStatus::Pending,
    };
    let group_b = ParallelGroup {
        parallel_node_id: source_b_id,
        branch_node_ids: vec![shared_target_id],
        bounding_box: BoundingBox {
            x: 292.0,
            y: 392.0,
            width: 236.0,
            height: 84.0,
        },
        branch_count: 1,
        aggregate_status: AggregateStatus::Pending,
    };
    let groups = vec![group_a, group_b];

    let anchors = resolve_edge_anchors_with_parallel(&connections, &nodes, &groups);

    // Both edges should resolve to the same target position (no offset since single target)
    let anchor_a = anchors.get(&conn_a.id.to_string()).copied();
    let anchor_b = anchors.get(&conn_b.id.to_string()).copied();

    assert!(anchor_a.is_some());
    assert!(anchor_b.is_some());

    let anchor_a = anchor_a.unwrap();
    let anchor_b = anchor_b.unwrap();

    // Both should have the same target y since there's only one target in each group
    assert_eq!(anchor_a.to.y, anchor_b.to.y);
}
