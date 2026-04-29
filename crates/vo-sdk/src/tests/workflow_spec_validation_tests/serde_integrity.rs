//! Section 7: Edge spec and Node spec serde integrity

use crate::{DedupeScope, EdgeSpec, NodeSpec, WorkflowSpec};
use vo_types::{NodeKind, NodeName, WorkflowName};

#[test]
fn node_spec_round_trips_all_kinds() {
    for kind in NodeKind::all_variants() {
        let node = NodeSpec {
            name: NodeName::parse("test-node").expect("valid"),
            kind: *kind,
        };
        let json = serde_json::to_string(&node).expect("serialize");
        let restored: NodeSpec = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.kind, *kind, "round-trip failed for {:?}", kind);
        assert_eq!(restored.name, node.name);
    }
}

#[test]
fn edge_spec_round_trips() {
    let edge = EdgeSpec {
        from: NodeName::parse("source").expect("valid"),
        to: NodeName::parse("target").expect("valid"),
    };
    let json = serde_json::to_string(&edge).expect("serialize");
    let restored: EdgeSpec = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, edge);
}

#[test]
fn node_spec_equality_works() {
    let a = NodeSpec {
        name: NodeName::parse("a").expect("valid"),
        kind: NodeKind::Pure,
    };
    let b = NodeSpec {
        name: NodeName::parse("a").expect("valid"),
        kind: NodeKind::Pure,
    };
    let c = NodeSpec {
        name: NodeName::parse("a").expect("valid"),
        kind: NodeKind::ManagedEffect,
    };
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn edge_spec_equality_works() {
    let a = EdgeSpec {
        from: NodeName::parse("x").expect("valid"),
        to: NodeName::parse("y").expect("valid"),
    };
    let b = EdgeSpec {
        from: NodeName::parse("x").expect("valid"),
        to: NodeName::parse("y").expect("valid"),
    };
    let c = EdgeSpec {
        from: NodeName::parse("y").expect("valid"),
        to: NodeName::parse("x").expect("valid"),
    };
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn to_json_bytes_produces_deterministic_output() {
    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("clone-test").expect("valid"),
        nodes: vec![NodeSpec {
            name: NodeName::parse("a").expect("valid"),
            kind: NodeKind::Pure,
            retry_policy: None,
            signal_scope: None,
        }],
        edges: vec![EdgeSpec {
            from: NodeName::parse("a").expect("valid"),
            to: NodeName::parse("a").expect("valid"),
        }],
        dedupe_scope: DedupeScope::default(),
    };
    let bytes1 = spec.to_json_bytes();
    let bytes2 = spec.to_json_bytes();
    assert_eq!(bytes1, bytes2, "to_json_bytes should be deterministic");
}

#[test]
fn workflow_spec_clone_is_equal() {
    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("clone-test").expect("valid"),
        nodes: vec![NodeSpec {
            name: NodeName::parse("a").expect("valid"),
            kind: NodeKind::Pure,
        }],
        edges: vec![EdgeSpec {
            from: NodeName::parse("a").expect("valid"),
            to: NodeName::parse("a").expect("valid"),
        }],
    };
    let cloned = spec.clone();
    assert_eq!(spec, cloned);
}

#[test]
fn workflow_spec_debug_format_includes_fields() {
    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("debug-test").expect("valid"),
        nodes: vec![],
        edges: vec![],
        dedupe_scope: DedupeScope::default(),
    };
    let debug = format!("{:?}", spec);
    assert!(
        debug.contains("debug-test"),
        "debug format should contain workflow name"
    );
}
