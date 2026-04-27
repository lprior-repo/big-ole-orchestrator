use super::helpers::*;
use crate::ui::edges::graph_types::NodeId;
use crate::ui::edges::layout::calculate_parallel_offset;
use uuid::Uuid;

#[test]
fn given_two_targets_when_calculate_offset_then_returns_symmetric_values() {
    let target_a_id = NodeId::new();
    let target_b_id = NodeId::new();

    let target_a = build_node(target_a_id, 300.0, 100.0);
    let target_b = build_node(target_b_id, 300.0, 200.0);

    let targets = vec![target_a.clone(), target_b.clone()];

    let offset_a = calculate_parallel_offset(&target_a_id, &targets, NODE_HEIGHT);
    let offset_b = calculate_parallel_offset(&target_b_id, &targets, NODE_HEIGHT);

    let spacing = NODE_HEIGHT / 2.5;

    let mut sorted_ids = [target_a_id, target_b_id];
    sorted_ids.sort_by_key(|left| left.0);

    let expected_a = if target_a_id == sorted_ids[0] {
        -spacing / 2.0
    } else {
        spacing / 2.0
    };
    let expected_b = -expected_a;

    assert_eq!(offset_a, expected_a);
    assert_eq!(offset_b, expected_b);
}

#[test]
fn given_three_targets_when_calculate_offset_then_returns_centered_values() {
    let target_a_id = NodeId::new();
    let target_b_id = NodeId::new();
    let target_c_id = NodeId::new();

    let target_a = build_node(target_a_id, 300.0, 100.0);
    let target_b = build_node(target_b_id, 300.0, 200.0);
    let target_c = build_node(target_c_id, 300.0, 300.0);

    let targets = vec![target_a, target_b, target_c];

    let offset_a = calculate_parallel_offset(&target_a_id, &targets, NODE_HEIGHT);
    let offset_b = calculate_parallel_offset(&target_b_id, &targets, NODE_HEIGHT);
    let offset_c = calculate_parallel_offset(&target_c_id, &targets, NODE_HEIGHT);

    let spacing = NODE_HEIGHT / 2.5;

    let mut sorted_ids = [target_a_id, target_b_id, target_c_id];
    sorted_ids.sort_by_key(|left| left.0);

    let expected_for = |id: NodeId| {
        if id == sorted_ids[0] {
            -spacing
        } else if id == sorted_ids[1] {
            0.0
        } else {
            spacing
        }
    };

    assert_eq!(offset_a, expected_for(target_a_id));
    assert_eq!(offset_b, expected_for(target_b_id));
    assert_eq!(offset_c, expected_for(target_c_id));
}

#[test]
fn given_four_targets_when_calculate_offset_then_returns_symmetric_values() {
    let target_a_id = NodeId::new();
    let target_b_id = NodeId::new();
    let target_c_id = NodeId::new();
    let target_d_id = NodeId::new();

    let target_a = build_node(target_a_id, 300.0, 100.0);
    let target_b = build_node(target_b_id, 300.0, 200.0);
    let target_c = build_node(target_c_id, 300.0, 300.0);
    let target_d = build_node(target_d_id, 300.0, 400.0);

    let targets = vec![target_a, target_b, target_c, target_d];

    let offset_a = calculate_parallel_offset(&target_a_id, &targets, NODE_HEIGHT);
    let offset_b = calculate_parallel_offset(&target_b_id, &targets, NODE_HEIGHT);
    let offset_c = calculate_parallel_offset(&target_c_id, &targets, NODE_HEIGHT);
    let offset_d = calculate_parallel_offset(&target_d_id, &targets, NODE_HEIGHT);

    let spacing = NODE_HEIGHT / 2.5;

    let mut sorted_ids = [target_a_id, target_b_id, target_c_id, target_d_id];
    sorted_ids.sort_by_key(|left| left.0);

    let expected_for = |id: NodeId| {
        if id == sorted_ids[0] {
            -spacing * 1.5
        } else if id == sorted_ids[1] {
            -spacing / 2.0
        } else if id == sorted_ids[2] {
            spacing / 2.0
        } else {
            spacing * 1.5
        }
    };

    assert_eq!(offset_a, expected_for(target_a_id));
    assert_eq!(offset_b, expected_for(target_b_id));
    assert_eq!(offset_c, expected_for(target_c_id));
    assert_eq!(offset_d, expected_for(target_d_id));
}

#[test]
fn given_single_target_when_calculate_offset_then_returns_zero() {
    let target_id = NodeId::new();
    let target = build_node(target_id, 300.0, 100.0);

    let targets = vec![target];

    let offset = calculate_parallel_offset(&target_id, &targets, NODE_HEIGHT);

    assert_eq!(offset, 0.0);
}

#[test]
fn given_target_id_not_in_targets_when_calculate_offset_then_returns_zero() {
    let target_id = NodeId::new();
    let other_id = NodeId::new();
    let target = build_node(other_id, 300.0, 100.0);

    let targets = vec![target];

    let offset = calculate_parallel_offset(&target_id, &targets, NODE_HEIGHT);

    assert_eq!(offset, 0.0);
}

#[test]
fn given_targets_at_varying_y_positions_when_calculate_offset_then_respects_sorted_order() {
    let target_a_id = NodeId::new();
    let target_b_id = NodeId::new();
    let target_c_id = NodeId::new();

    // Create nodes with y-positions that don't match ID order
    let target_a = build_node(target_a_id, 300.0, 300.0); // y=300, but ID sorts first
    let target_b = build_node(target_b_id, 300.0, 100.0); // y=100, but ID sorts middle
    let target_c = build_node(target_c_id, 300.0, 200.0); // y=200, but ID sorts last

    let targets = vec![target_a, target_b, target_c];

    let offset_a = calculate_parallel_offset(&target_a_id, &targets, NODE_HEIGHT);
    let offset_b = calculate_parallel_offset(&target_b_id, &targets, NODE_HEIGHT);
    let offset_c = calculate_parallel_offset(&target_c_id, &targets, NODE_HEIGHT);

    // Offsets are determined by sorted ID order, not y-position
    let spacing = NODE_HEIGHT / 2.5;
    let mut sorted_ids = [target_a_id, target_b_id, target_c_id];
    sorted_ids.sort_by_key(|left| left.0);

    let expected_for = |id: NodeId| {
        if id == sorted_ids[0] {
            -spacing
        } else if id == sorted_ids[1] {
            0.0
        } else {
            spacing
        }
    };

    assert_eq!(offset_a, expected_for(target_a_id));
    assert_eq!(offset_b, expected_for(target_b_id));
    assert_eq!(offset_c, expected_for(target_c_id));
}
