//! WorkflowSnapshot tests: construction and checksum behavior.
//!
//! Behaviors: B-004, B-005

#[test]
fn workflow_snapshot_captures_complete_graph_state() {
    let nodes = vec![make_node("node-a"), make_node("node-b")];
    let edges = vec![Edge {
        source_node: NodeName::parse("node-a").unwrap(),
        target_node: NodeName::parse("node-b").unwrap(),
        condition: EdgeCondition::Always,
    }];
    let snapshot = make_snapshot("test-workflow", nodes, edges);

    assert_eq!(snapshot.nodes.len(), 2);
    assert_eq!(snapshot.edges.len(), 1);
    assert_ne!(
        snapshot.checksum, 0,
        "checksum should be non-zero for non-empty graph"
    );
}

#[test]
fn workflow_snapshot_checksum_is_deterministic() {
    let nodes = vec![make_node("a")];
    let edges = vec![];

    let snapshot1 = make_snapshot("workflow".into(), nodes.clone(), edges.clone());
    let snapshot2 = make_snapshot("workflow".into(), nodes, edges);

    assert_eq!(
        snapshot1.checksum, snapshot2.checksum,
        "identical graphs must have identical checksums"
    );
}

#[test]
fn workflow_snapshot_checksum_detects_difference() {
    let nodes1 = vec![make_node("a")];
    let nodes2 = vec![make_node("b")];

    let snapshot1 = make_snapshot("workflow".into(), nodes1, vec![]);
    let snapshot2 = make_snapshot("workflow".into(), nodes2, vec![]);

    assert_ne!(
        snapshot1.checksum, snapshot2.checksum,
        "different graphs must have different checksums"
    );
}
