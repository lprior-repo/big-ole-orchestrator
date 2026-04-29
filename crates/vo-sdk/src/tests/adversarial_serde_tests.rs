//! Adversarial tests for vo-sdk (bead ve-z32z).
//!
//! DIMENSION: WorkflowSpec serde integrity.

use serde_json::{json, Value};

use crate::graph::{DedupeScope, EdgeSpec, GraphArgs, NodeSpec, WorkflowSpec};
use vo_types::{NodeKind, NodeName, WorkflowName};

#[test]
fn workflow_spec_json_uses_snake_case() {
    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("test").unwrap(),
        nodes: vec![NodeSpec {
            name: NodeName::parse("a").unwrap(),
            kind: NodeKind::Pure,
            retry_policy: None,
            signal_scope: None,
        }],
        edges: vec![],
        dedupe_scope: DedupeScope::default(),
    };
    let bytes = spec.to_json_bytes();
    let json_str = String::from_utf8(bytes).unwrap();

    assert!(json_str.contains("workflow_name"), "should use snake_case");
    assert!(
        !json_str.contains("workflowName"),
        "should not use camelCase"
    );
}

#[test]
fn workflow_spec_large_graph_roundtrip() {
    let nodes: Vec<NodeSpec> = (0..50)
        .map(|i| NodeSpec {
            name: NodeName::parse(&format!("node{}", i)).unwrap(),
            kind: NodeKind::Pure,
        })
        .collect();

    let mut edges = Vec::new();
    for i in 0..49 {
        edges.push(EdgeSpec {
            from: NodeName::parse(&format!("node{}", i)).unwrap(),
            to: NodeName::parse(&format!("node{}", i + 1)).unwrap(),
        });
    }
    for i in 0..25 {
        edges.push(EdgeSpec {
            from: NodeName::parse(&format!("node{}", i)).unwrap(),
            to: NodeName::parse(&format!("node{}", i + 25)).unwrap(),
        });
    }

    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("large_graph").unwrap(),
        nodes: nodes.clone(),
        edges: edges.clone(),
        dedupe_scope: DedupeScope::default(),
    };

    let json = serde_json::to_string(&spec).unwrap();
    let restored: WorkflowSpec = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.nodes.len(), 50);
    assert_eq!(restored.edges.len(), 74);
    assert_eq!(restored, spec);
}

#[test]
fn workflow_spec_to_json_bytes_never_panics() {
    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("empty").unwrap(),
        nodes: vec![],
        edges: vec![],
        dedupe_scope: DedupeScope::default(),
    };
    let bytes = spec.to_json_bytes();
    assert!(!bytes.is_empty(), "should produce non-empty JSON");
    let _: Value = serde_json::from_slice(&bytes).expect("should be valid JSON");
}
