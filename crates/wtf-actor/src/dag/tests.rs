//! Tests for DAG paradigm.

use super::*;
use bytes::Bytes;
use std::collections::HashMap;
use wtf_common::WorkflowEvent;

fn linear_dag() -> HashMap<NodeId, DagNode> {
    let mut nodes = HashMap::new();
    nodes.insert(
        NodeId::new("A"),
        DagNode {
            activity_type: "task_a".into(),
            predecessors: vec![],
        },
    );
    nodes.insert(
        NodeId::new("B"),
        DagNode {
            activity_type: "task_b".into(),
            predecessors: vec![NodeId::new("A")],
        },
    );
    nodes.insert(
        NodeId::new("C"),
        DagNode {
            activity_type: "task_c".into(),
            predecessors: vec![NodeId::new("B")],
        },
    );
    nodes
}

fn parallel_dag() -> HashMap<NodeId, DagNode> {
    let mut nodes = HashMap::new();
    nodes.insert(
        NodeId::new("A"),
        DagNode {
            activity_type: "task_a".into(),
            predecessors: vec![],
        },
    );
    nodes.insert(
        NodeId::new("B"),
        DagNode {
            activity_type: "task_b".into(),
            predecessors: vec![],
        },
    );
    nodes.insert(
        NodeId::new("C"),
        DagNode {
            activity_type: "task_c".into(),
            predecessors: vec![NodeId::new("A"), NodeId::new("B")],
        },
    );
    nodes
}

fn completed_event(id: &str) -> WorkflowEvent {
    WorkflowEvent::ActivityCompleted {
        activity_id: id.into(),
        result: Bytes::from_static(b"ok"),
        duration_ms: 10,
    }
}

fn dispatched_event(id: &str) -> WorkflowEvent {
    WorkflowEvent::ActivityDispatched {
        activity_id: id.into(),
        activity_type: "task".into(),
        payload: Bytes::new(),
        retry_policy: wtf_common::RetryPolicy::default(),
        attempt: 1,
    }
}

fn failed_event(id: &str, exhausted: bool) -> WorkflowEvent {
    WorkflowEvent::ActivityFailed {
        activity_id: id.into(),
        error: "boom".into(),
        retries_exhausted: exhausted,
    }
}

#[test]
fn root_nodes_ready_on_empty_state() {
    let state = DagActorState::new(linear_dag());
    let ready = ready_nodes(&state);
    assert_eq!(ready, vec![NodeId::new("A")]);
}

#[test]
fn parallel_roots_both_ready() {
    let state = DagActorState::new(parallel_dag());
    let ready = ready_nodes(&state);
    assert_eq!(ready, vec![NodeId::new("A"), NodeId::new("B")]);
}

#[test]
fn in_flight_node_not_ready() {
    let state = DagActorState::new(linear_dag());
    let event = dispatched_event("A");
    let (s1, _) = apply_event(&state, &event, 1).expect("apply");
    let ready = ready_nodes(&s1);
    assert!(ready.is_empty());
}

#[test]
fn completed_unblocks_successor() {
    let state = DagActorState::new(linear_dag());
    let (s1, _) = apply_event(&state, &dispatched_event("A"), 1).expect("dispatch A");
    let (s2, _) = apply_event(&s1, &completed_event("A"), 2).expect("complete A");
    let ready = ready_nodes(&s2);
    assert_eq!(ready, vec![NodeId::new("B")]);
}

#[test]
fn duplicate_seq_returns_already_applied() {
    let state = DagActorState::new(linear_dag());
    let (s1, _) = apply_event(&state, &completed_event("A"), 1).expect("first");
    let (_, result) = apply_event(&s1, &completed_event("A"), 1).expect("duplicate");
    assert!(matches!(result, DagApplyResult::AlreadyApplied));
}

#[test]
fn activity_failed_exhausted_adds_to_failed() {
    let state = DagActorState::new(linear_dag());
    let (s1, _) = apply_event(&state, &dispatched_event("A"), 1).expect("dispatch");
    let (s2, result) = apply_event(&s1, &failed_event("A", true), 2).expect("fail exhausted");
    assert!(matches!(result, DagApplyResult::ActivityFailed { .. }));
    assert!(s2.failed.contains(&NodeId::new("A")));
}

#[test]
fn is_succeeded_when_all_complete() {
    let state = DagActorState::new(linear_dag());
    let (s1, _) = apply_event(&state, &completed_event("A"), 1).expect("A");
    let (s2, _) = apply_event(&s1, &completed_event("B"), 2).expect("B");
    let (s3, _) = apply_event(&s2, &completed_event("C"), 3).expect("C");
    assert!(is_succeeded(&s3));
}

// ---------------------------------------------------------------------------
// parse_dag_graph tests (wtf-bx19)
// TODO: Uncomment when dag/parse.rs module is implemented by bead wtf-bx19.
// These tests reference super::parse::{parse_dag_graph, DagParseError} which
// does not exist yet.
// ---------------------------------------------------------------------------

#[test]
fn parse_parallel_roots() {
    let json = r#"{"nodes":[
        {"id":"A","activity_type":"t1","predecessors":[]},
        {"id":"B","activity_type":"t2","predecessors":[]},
        {"id":"C","activity_type":"t3","predecessors":["A","B"]}
    ]}"#;
    let map = parse_dag_graph(json).expect("parse");
    assert_eq!(map.len(), 3);
    assert_eq!(
        map[&NodeId::new("C")].predecessors,
        vec![NodeId::new("A"), NodeId::new("B")]
    );
}

#[test]
fn parse_empty_nodes_yields_empty_map() {
    let json = r#"{"nodes":[]}"#;
    let map = parse_dag_graph(json).expect("parse");
    assert!(map.is_empty());
}

#[test]
fn parse_single_root_node() {
    let json = r#"{"nodes":[
        {"id":"solo","activity_type":"only_task","predecessors":[]}
    ]}"#;
    let map = parse_dag_graph(json).expect("parse");
    assert_eq!(map.len(), 1);
    assert_eq!(&map[&NodeId::new("solo")].activity_type, "only_task");
}

#[test]
fn parse_invalid_json() {
    let result = parse_dag_graph("not json at all");
    assert!(matches!(result, Err(DagParseError::InvalidJson(_))));
}

#[test]
fn parse_missing_nodes_field() {
    let result = parse_dag_graph(r#"{"edges":[]}"#);
    assert!(matches!(result, Err(DagParseError::MissingNodesField)));
}

#[test]
fn parse_nodes_not_array() {
    let result = parse_dag_graph(r#"{"nodes":"oops"}"#);
    assert!(matches!(result, Err(DagParseError::NodesNotArray)));
}

#[test]
fn parse_duplicate_node_id() {
    let json = r#"{"nodes":[
        {"id":"A","activity_type":"t1","predecessors":[]},
        {"id":"A","activity_type":"t2","predecessors":[]}
    ]}"#;
    let result = parse_dag_graph(json);
    assert!(matches!(result, Err(DagParseError::DuplicateNodeId(id)) if id == "A"));
}

#[test]
fn parse_unknown_predecessor() {
    let json = r#"{"nodes":[
        {"id":"B","activity_type":"t1","predecessors":["NONEXISTENT"]}
    ]}"#;
    let result = parse_dag_graph(json);
    assert!(matches!(
        result,
        Err(DagParseError::UnknownPredecessor { .. })
    ));
}

#[test]
fn parse_cycle_detected() {
    let json = r#"{"nodes":[
        {"id":"A","activity_type":"t1","predecessors":["C"]},
        {"id":"B","activity_type":"t2","predecessors":["A"]},
        {"id":"C","activity_type":"t3","predecessors":["B"]}
    ]}"#;
    let result = parse_dag_graph(json);
    assert!(matches!(result, Err(DagParseError::CycleDetected(_))));
}

#[test]
fn parse_self_loop_detected() {
    let json = r#"{"nodes":[
        {"id":"A","activity_type":"t1","predecessors":["A"]}
    ]}"#;
    let result = parse_dag_graph(json);
    assert!(matches!(result, Err(DagParseError::CycleDetected(_))));
}

#[test]
fn parse_missing_activity_type_field() {
    let json = r#"{"nodes":[{"id":"A","predecessors":[]}]}"#;
    let result = parse_dag_graph(json);
    assert!(matches!(
        result,
        Err(DagParseError::MissingNodeField {
            field: "activity_type",
            ..
        })
    ));
}

#[test]
fn parse_missing_id_field() {
    let json = r#"{"nodes":[{"activity_type":"t1","predecessors":[]}]}"#;
    let result = parse_dag_graph(json);
    assert!(matches!(
        result,
        Err(DagParseError::MissingNodeField { field: "id", .. })
    ));
}

#[test]
fn parse_missing_predecessors_field() {
    let json = r#"{"nodes":[{"id":"A","activity_type":"t1"}]}"#;
    let result = parse_dag_graph(json);
    assert!(matches!(
        result,
        Err(DagParseError::MissingNodeField {
            field: "predecessors",
            ..
        })
    ));
}

#[test]
fn parse_diamond_dag() {
    let json = r#"{"nodes":[
        {"id":"A","activity_type":"start","predecessors":[]},
        {"id":"B","activity_type":"left","predecessors":["A"]},
        {"id":"C","activity_type":"right","predecessors":["A"]},
        {"id":"D","activity_type":"end","predecessors":["B","C"]}
    ]}"#;
    let map = parse_dag_graph(json).expect("parse");
    assert_eq!(map.len(), 4);
    assert_eq!(
        map[&NodeId::new("D")].predecessors,
        vec![NodeId::new("B"), NodeId::new("C")]
    );
}

#[test]
fn parse_preserves_activity_type() {
    let json = r#"{"nodes":[
        {"id":"X","activity_type":"http_fetch","predecessors":[]}
    ]}"#;
    let map = parse_dag_graph(json).expect("parse");
    assert_eq!(&map[&NodeId::new("X")].activity_type, "http_fetch");
}

// ---------------------------------------------------------------------------
// Additional parse_dag_graph tests (wtf-bx19)
// ---------------------------------------------------------------------------

#[test]
fn parse_node_not_an_object() {
    let json = r#"{"nodes":["not an object"]}"#;
    let result = parse_dag_graph(json);
    assert!(matches!(
        result,
        Err(DagParseError::MissingNodeField {
            field: "(object)",
            index: 0
        })
    ));
}

#[test]
fn parse_node_id_is_number() {
    let json = r#"{"nodes":[{"id":123,"activity_type":"t","predecessors":[]}]}"#;
    let result = parse_dag_graph(json);
    assert!(matches!(
        result,
        Err(DagParseError::MissingNodeField {
            field: "id",
            index: 0
        })
    ));
}

#[test]
fn parse_node_activity_type_is_number() {
    let json = r#"{"nodes":[{"id":"A","activity_type":999,"predecessors":[]}]}"#;
    let result = parse_dag_graph(json);
    assert!(matches!(
        result,
        Err(DagParseError::MissingNodeField {
            field: "activity_type",
            index: 0
        })
    ));
}

#[test]
fn parse_predecessors_is_object() {
    let json = r#"{"nodes":[{"id":"A","activity_type":"t","predecessors":{"bad":true}}]}"#;
    let result = parse_dag_graph(json);
    assert!(matches!(
        result,
        Err(DagParseError::MissingNodeField {
            field: "predecessors",
            index: 0
        })
    ));
}

#[test]
fn parse_predecessor_element_not_string() {
    let json = r#"{"nodes":[
        {"id":"A","activity_type":"t","predecessors":[]},
        {"id":"B","activity_type":"t2","predecessors":[123]}
    ]}"#;
    let result = parse_dag_graph(json).expect("parse should succeed for valid json");
    assert_eq!(result[&NodeId::new("B")].predecessors, vec![]);
}

#[test]
fn parse_complex_dag_five_levels() {
    let json = r#"{"nodes":[
        {"id":"L1","activity_type":"level1","predecessors":[]},
        {"id":"L2a","activity_type":"level2a","predecessors":["L1"]},
        {"id":"L2b","activity_type":"level2b","predecessors":["L1"]},
        {"id":"L3a","activity_type":"level3a","predecessors":["L2a"]},
        {"id":"L3b","activity_type":"level3b","predecessors":["L2a","L2b"]},
        {"id":"L4","activity_type":"level4","predecessors":["L3a","L3b"]},
        {"id":"L5","activity_type":"level5","predecessors":["L4"]}
    ]}"#;
    let map = parse_dag_graph(json).expect("parse");
    assert_eq!(map.len(), 7);
    assert_eq!(
        map[&NodeId::new("L3b")].predecessors,
        vec![NodeId::new("L2a"), NodeId::new("L2b")]
    );
    assert_eq!(
        map[&NodeId::new("L5")].predecessors,
        vec![NodeId::new("L4")]
    );
}

#[test]
fn parse_many_parallel_roots() {
    let json = r#"{"nodes":[
        {"id":"R1","activity_type":"r1","predecessors":[]},
        {"id":"R2","activity_type":"r2","predecessors":[]},
        {"id":"R3","activity_type":"r3","predecessors":[]},
        {"id":"R4","activity_type":"r4","predecessors":[]},
        {"id":"R5","activity_type":"r5","predecessors":[]},
        {"id":"M","activity_type":"merge","predecessors":["R1","R2","R3","R4","R5"]}
    ]}"#;
    let map = parse_dag_graph(json).expect("parse");
    assert_eq!(map.len(), 6);
    assert_eq!(
        map[&NodeId::new("M")].predecessors,
        vec![
            NodeId::new("R1"),
            NodeId::new("R2"),
            NodeId::new("R3"),
            NodeId::new("R4"),
            NodeId::new("R5")
        ]
    );
}

#[test]
fn parse_empty_string_json() {
    let result = parse_dag_graph("");
    assert!(matches!(result, Err(DagParseError::InvalidJson(_))));
}

#[test]
fn parse_truncated_json() {
    let result = parse_dag_graph(r#"{"nodes":[{"id":"A""#);
    assert!(matches!(result, Err(DagParseError::InvalidJson(_))));
}

// ---------------------------------------------------------------------------
// is_terminal tests
// ---------------------------------------------------------------------------

#[test]
fn is_terminal_on_empty_graph() {
    let state = DagActorState::new(HashMap::new());
    assert!(is_terminal(&state));
}

#[test]
fn is_terminal_when_succeeded() {
    let mut nodes = HashMap::new();
    nodes.insert(
        NodeId::new("A"),
        DagNode {
            activity_type: "t".into(),
            predecessors: vec![],
        },
    );
    let mut state = DagActorState::new(nodes);
    state
        .completed
        .insert(NodeId::new("A"), Bytes::from_static(b"ok"));
    assert!(is_terminal(&state));
    assert!(is_succeeded(&state));
}

#[test]
fn is_terminal_when_failed() {
    let mut nodes = HashMap::new();
    nodes.insert(
        NodeId::new("A"),
        DagNode {
            activity_type: "t".into(),
            predecessors: vec![],
        },
    );
    let state = DagActorState::new(nodes);
    assert!(!is_terminal(&state));
}

// ---------------------------------------------------------------------------
// is_failed tests
// ---------------------------------------------------------------------------

#[test]
fn is_failed_empty_graph() {
    let state = DagActorState::new(HashMap::new());
    assert!(!is_failed(&state));
}

#[test]
fn is_failed_no_failures() {
    let state = DagActorState::new(linear_dag());
    assert!(!is_failed(&state));
}

#[test]
fn is_failed_with_failed_node() {
    let mut state = DagActorState::new(linear_dag());
    state.failed.insert(NodeId::new("A"));
    assert!(is_failed(&state));
}

// ---------------------------------------------------------------------------
// is_succeeded tests (additional)
// ---------------------------------------------------------------------------

#[test]
fn is_succeeded_empty_graph() {
    let state = DagActorState::new(HashMap::new());
    assert!(is_succeeded(&state));
}

#[test]
fn is_succeeded_not_all_complete() {
    let state = DagActorState::new(linear_dag());
    assert!(!is_succeeded(&state));
}

// ---------------------------------------------------------------------------
// ready_nodes tests (additional boundary conditions)
// ---------------------------------------------------------------------------

#[test]
fn ready_nodes_all_in_flight() {
    let state = DagActorState::new(linear_dag());
    let mut s1 = state.clone();
    s1.in_flight.insert(NodeId::new("A"));
    assert!(ready_nodes(&s1).is_empty());
}

#[test]
fn ready_nodes_some_completed() {
    let mut state = DagActorState::new(linear_dag());
    state
        .completed
        .insert(NodeId::new("A"), Bytes::from_static(b"ok"));
    let ready = ready_nodes(&state);
    assert_eq!(ready, vec![NodeId::new("B")]);
}

#[test]
fn ready_nodes_some_failed() {
    let mut state = DagActorState::new(linear_dag());
    state.failed.insert(NodeId::new("A"));
    assert!(ready_nodes(&state).is_empty());
}

#[test]
fn ready_nodes_complex_parallel() {
    let json = r#"{"nodes":[
        {"id":"A","activity_type":"a","predecessors":[]},
        {"id":"B","activity_type":"b","predecessors":[]},
        {"id":"C","activity_type":"c","predecessors":[]},
        {"id":"D","activity_type":"d","predecessors":["A"]},
        {"id":"E","activity_type":"e","predecessors":["A","B"]},
        {"id":"F","activity_type":"f","predecessors":["C"]}
    ]}"#;
    let map = parse_dag_graph(json).expect("parse");
    let state = DagActorState::new(map);
    let ready = ready_nodes(&state);
    assert_eq!(
        ready,
        vec![NodeId::new("A"), NodeId::new("B"), NodeId::new("C")]
    );
}

// ---------------------------------------------------------------------------
// apply_event tests (additional error variants)
// ---------------------------------------------------------------------------

#[test]
fn apply_event_completed_unknown_node_returns_error() {
    let state = DagActorState::new(linear_dag());
    let event = WorkflowEvent::ActivityCompleted {
        activity_id: "UNKNOWN".into(),
        result: Bytes::from_static(b"ok"),
        duration_ms: 10,
    };
    let result = apply_event(&state, &event, 1);
    assert!(matches!(result, Err(DagApplyError::UnknownNode(_))));
}

#[test]
fn apply_event_snapshot_resets_counter() {
    let mut state = DagActorState::new(linear_dag());
    state.events_since_snapshot = 5;
    let event = WorkflowEvent::SnapshotTaken {
        seq: 1,
        checksum: 0,
    };
    let (next, _) = apply_event(&state, &event, 1).expect("apply");
    assert_eq!(next.events_since_snapshot, 0);
}

#[test]
fn apply_event_out_of_order_seq_already_applied() {
    let state = DagActorState::new(linear_dag());
    let event = completed_event("A");
    let (s1, result1) = apply_event(&state, &event, 5).expect("first apply at seq 5");
    assert!(matches!(result1, DagApplyResult::ActivityCompleted { .. }));
    let (s2, result2) = apply_event(&s1, &event, 5).expect("second apply at same seq 5");
    assert!(matches!(result2, DagApplyResult::AlreadyApplied));
    assert!(s2.applied_seq.contains(&5));
}
