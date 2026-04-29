//! RED-QUEEN coevolutionary adversarial tests for vo-sdk.
//! bead_id: ve-hthi9
//!
//! Attack surfaces:
//!   - Workflow builder edge cases (disconnect, duplicate connect, mixed kinds)
//!   - Type erasure attacks (phantom type bypass via serde, cross-type connect)
//!   - Serde roundtrip failures (corrupt JSON, mismatched kinds, edge drift)

use vo_sdk::dag::Dag;
use vo_sdk::graph::{EdgeSpec, NodeSpec, WorkflowSpec};
use vo_sdk::node_handle::NodeHandle;
use vo_sdk::Workflow;
use vo_types::NodeKind;
use vo_types::NodeName;

#[test]
fn rq_builder_two_node_chain_no_connect_produces_zero_edges() {
    let mut wf = Workflow::new("chain");
    wf.pure::<i32, i32, _>("a", |x| x).unwrap();
    wf.pure::<i32, i32, _>("b", |x| x).unwrap();
    let spec = wf.build().unwrap();
    assert_eq!(spec.edges.len(), 0);
    assert_eq!(spec.nodes.len(), 2);
}

#[test]
fn rq_builder_duplicate_connect_creates_two_edges() {
    let mut dag = Dag::new();
    let a = dag
        .add_node_with_kind::<i32, i32, _>("a", NodeKind::Pure, |x: i32| x)
        .unwrap();
    let b = dag
        .add_node_with_kind::<i32, i32, _>("b", NodeKind::Pure, |x: i32| x)
        .unwrap();
    dag.connect(&a, &b).unwrap();
    dag.connect(&a, &b).unwrap();
    assert_eq!(dag.edge_count(), 2);
    let spec = dag.build("dup-edge").unwrap();
    assert_eq!(spec.edges.len(), 2);
}

#[test]
fn rq_builder_all_five_node_kinds_accepted() {
    let mut wf = Workflow::new("kitchen-sink");
    wf.pure::<(), (), _>("p", |_: ()| ()).unwrap();
    wf.effect::<(), (), _>("e", |_: ()| ()).unwrap();
    wf.wait::<(), (), _>("w", |_: ()| ()).unwrap();
    wf.signal::<(), (), _>("s", |_: ()| ()).unwrap();
    wf.unsafe_node::<(), (), _>("u", |_: ()| ()).unwrap();
    let spec = wf.build().unwrap();
    assert_eq!(spec.nodes.len(), 5);
    let kinds: Vec<_> = spec.nodes.iter().map(|n| n.kind).collect();
    assert_eq!(
        kinds,
        vec![
            NodeKind::Pure,
            NodeKind::ManagedEffect,
            NodeKind::Wait,
            NodeKind::Signal,
            NodeKind::Unsafe,
        ]
    );
}

#[test]
fn rq_serde_node_handle_erases_phantom_types() {
    let nm = NodeName::parse("x").unwrap();
    let h1: NodeHandle<String, i32> = NodeHandle::new(nm.clone());
    let h2: NodeHandle<bool, ()> = NodeHandle::new(nm);
    let j1 = serde_json::to_string(&h1).unwrap();
    let j2 = serde_json::to_string(&h2).unwrap();
    assert_eq!(j1, j2, "Phantom types must NOT leak into serialized form");
    let from_other: NodeHandle<String, i32> = serde_json::from_str(&j2).unwrap();
    assert_eq!(from_other.name(), "x");
}

#[test]
fn rq_serde_roundtrip_spec_preserves_structure() {
    let mut wf = Workflow::new("roundtrip");
    let a = wf
        .pure::<i32, String, _>("a", |_: i32| String::new())
        .unwrap();
    let b = wf.effect::<String, bool, _>("b", |_: String| true).unwrap();
    wf.connect(&a, &b).unwrap();
    let spec = wf.build().unwrap();
    let json = serde_json::to_vec(&spec).unwrap();
    let back: WorkflowSpec = serde_json::from_slice(&json).unwrap();
    assert_eq!(spec, back);
}

#[test]
fn rq_serde_corrupt_trailing_comma_rejected() {
    let bad = r#"{"workflow_name":"w","nodes":[{"name":"n","kind":"pure"},],"edges":[]}"#;
    assert!(serde_json::from_str::<WorkflowSpec>(bad).is_err());
}

#[test]
fn rq_serde_corrupt_missing_brace_rejected() {
    let bad = r#"{"workflow_name":"w","nodes":[{"name":"n","kind":"pure"}],"edges":[]"#;
    assert!(serde_json::from_str::<WorkflowSpec>(bad).is_err());
}

#[test]
fn rq_serde_edge_missing_node_rejected() {
    let j = r#"{"workflow_name":"w","nodes":[{"name":"a","kind":"pure"}],"edges":[{"from":"ghost","to":"a"}]}"#;
    let result = serde_json::from_str::<WorkflowSpec>(j);
    assert!(
        result.is_err(),
        "serde rejects edges to nonexistent nodes: {:?}",
        result
    );
}

#[test]
fn rq_serde_nodespec_roundtrip_preserves_kind() {
    for kind in [
        NodeKind::Pure,
        NodeKind::ManagedEffect,
        NodeKind::Wait,
        NodeKind::Signal,
        NodeKind::Unsafe,
    ] {
        let ns = NodeSpec {
            name: NodeName::parse("n").unwrap(),
            kind,
        };
        let j = serde_json::to_string(&ns).unwrap();
        let back: NodeSpec = serde_json::from_str(&j).unwrap();
        assert_eq!(ns.kind, back.kind);
    }
}

#[test]
fn rq_serde_edgespec_roundtrip() {
    let es = EdgeSpec {
        from: NodeName::parse("a").unwrap(),
        to: NodeName::parse("b").unwrap(),
    };
    let j = serde_json::to_string(&es).unwrap();
    let back: EdgeSpec = serde_json::from_str(&j).unwrap();
    assert_eq!(es, back);
}

#[test]
fn rq_io_read_empty_input_rejected() {
    let mut empty = &b""[..];
    let mut r = false;
    assert!(vo_sdk::io::read_input_inner_with_state(&mut empty, &mut r).is_err());
}

#[test]
fn rq_io_read_invalid_utf8_rejected() {
    let mut bad = &b"\xff\xfe"[..];
    let mut r = false;
    assert!(vo_sdk::io::read_input_inner_with_state(&mut bad, &mut r).is_err());
}

#[test]
fn rq_io_read_missing_key_field_rejected() {
    let mut data = br#"{"data":42}"#.as_slice();
    let mut r = false;
    assert!(vo_sdk::io::read_input_inner_with_state(&mut data, &mut r).is_err());
}

#[test]
fn rq_io_double_read_blocked() {
    let mut data = br#"{"idempotency_key":"k","data":1}"#.as_slice();
    let mut r = false;
    assert!(vo_sdk::io::read_input_inner_with_state(&mut data, &mut r).is_ok());
    let mut data2 = br#"{"idempotency_key":"k2","data":2}"#.as_slice();
    assert!(vo_sdk::io::read_input_inner_with_state(&mut data2, &mut r).is_err());
}
