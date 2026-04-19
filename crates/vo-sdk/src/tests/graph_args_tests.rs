//! Tests for graph_args module (--graph CLI argument handling per ADR-004, ADR-009).

use crate::graph::{
    parse_graph_args, EdgeSpec, GraphArgs, GraphArgsError, NodeKind, NodeSpec, WorkflowSpec,
};
use vo_types::{NodeName, WorkflowName};

#[test]
fn parse_graph_args_returns_err_when_no_graph_flag() {
    let args: Vec<String> = vec!["my_binary".to_string()];
    assert_eq!(parse_graph_args(&args), Err(GraphArgsError::NoGraphFlag));
}

#[test]
fn parse_graph_args_returns_ok_when_graph_flag_present() {
    let args: Vec<String> = vec!["my_binary".to_string(), "--graph".to_string()];
    let result = parse_graph_args(&args);
    assert_eq!(
        result,
        Ok(GraphArgs),
        "should return Ok(GraphArgs) when --graph is present"
    );
}

#[test]
fn parse_graph_args_rejects_unknown_positional_args_with_graph() {
    let args: Vec<String> = vec![
        "my_binary".to_string(),
        "--graph".to_string(),
        "unexpected_arg".to_string(),
    ];
    let result = parse_graph_args(&args);
    assert!(
        matches!(result, Err(GraphArgsError::UnrecognizedArgument { .. })),
        "should reject unexpected positional argument alongside --graph"
    );
}

#[test]
fn node_spec_serializes_to_snake_case_json() {
    let node = NodeSpec {
        name: NodeName::parse("validate_cart").expect("valid name"),
        kind: NodeKind::Pure,
    };
    let json = serde_json::to_string(&node).expect("serialize");
    assert!(
        json.contains("\"name\":\"validate_cart\""),
        "JSON should contain name field: {json}"
    );
    assert!(
        json.contains("\"kind\":\"pure\""),
        "JSON should contain snake_case kind field: {json}"
    );
}

#[test]
fn graph_workflow_spec_round_trips_via_serde() {
    let spec = crate::graph::WorkflowSpec {
        workflow_name: WorkflowName::parse("checkout_flow").expect("valid name"),
        nodes: vec![
            NodeSpec {
                name: NodeName::parse("validate").expect("valid"),
                kind: NodeKind::Pure,
            },
            NodeSpec {
                name: NodeName::parse("charge").expect("valid"),
                kind: NodeKind::ManagedEffect,
            },
        ],
        edges: vec![EdgeSpec {
            from: NodeName::parse("validate").expect("valid"),
            to: NodeName::parse("charge").expect("valid"),
        }],
    };
    let json = serde_json::to_string_pretty(&spec).expect("serialize");
    let restored: crate::graph::WorkflowSpec = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, spec, "round-trip should preserve all fields");
}

#[test]
fn graph_args_error_is_std_error() {
    let err = GraphArgsError::UnrecognizedArgument {
        arg: "bogus".to_string(),
    };
    let _: &dyn std::error::Error = &err;
    let msg = err.to_string();
    assert!(
        msg.contains("bogus"),
        "Error::to_string() should contain the invalid arg: {msg}"
    );
}

#[test]
fn graph_args_error_no_graph_flag_display() {
    let err = GraphArgsError::NoGraphFlag;
    let msg = err.to_string();
    assert!(
        msg.contains("--graph") || msg.contains("graph") || msg.contains("no graph"),
        "NoGraphFlag display should mention graph: {msg}"
    );
}

#[test]
fn graph_args_error_no_graph_flag_is_std_error() {
    let err = GraphArgsError::NoGraphFlag;
    let _: &dyn std::error::Error = &err;
}

#[test]
fn parse_graph_args_empty_args_returns_no_graph_flag() {
    let args: Vec<String> = vec!["binary".to_string()];
    assert_eq!(parse_graph_args(&args), Err(GraphArgsError::NoGraphFlag));
}

#[test]
fn parse_graph_args_graph_flag_first_position() {
    let args: Vec<String> = vec!["binary".to_string(), "--graph".to_string()];
    assert_eq!(parse_graph_args(&args), Ok(GraphArgs));
}

#[test]
fn parse_graph_args_graph_flag_with_preceding_flag() {
    let args: Vec<String> = vec![
        "binary".to_string(),
        "--other".to_string(),
        "--graph".to_string(),
    ];
    let result = parse_graph_args(&args);
    assert_eq!(result, Ok(GraphArgs));
}

#[test]
fn edge_spec_serde_round_trip() {
    let edge = EdgeSpec {
        from: NodeName::parse("node-a").expect("valid"),
        to: NodeName::parse("node-b").expect("valid"),
    };
    let json = serde_json::to_string(&edge).expect("serialize");
    let restored: EdgeSpec = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, edge);
}

#[test]
fn edge_spec_serializes_to_snake_case() {
    let edge = EdgeSpec {
        from: NodeName::parse("alpha").expect("valid"),
        to: NodeName::parse("beta").expect("valid"),
    };
    let json = serde_json::to_string(&edge).expect("serialize");
    assert!(
        json.contains("\"from\":\"alpha\""),
        "should contain from: {json}"
    );
    assert!(
        json.contains("\"to\":\"beta\""),
        "should contain to: {json}"
    );
}

#[test]
fn node_spec_all_kinds_serialize() {
    let kinds = vec![
        NodeKind::Pure,
        NodeKind::ManagedEffect,
        NodeKind::Wait,
        NodeKind::Signal,
        NodeKind::Unsafe,
    ];
    for kind in kinds {
        let node = NodeSpec {
            name: NodeName::parse("test-node").expect("valid"),
            kind,
        };
        let json = serde_json::to_string(&node).expect("serialize");
        let restored: NodeSpec = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.kind, kind);
    }
}

#[test]
fn workflow_spec_to_json_bytes_produces_valid_json() {
    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("test-wf").expect("valid"),
        nodes: vec![NodeSpec {
            name: NodeName::parse("step-a").expect("valid"),
            kind: NodeKind::Pure,
        }],
        edges: vec![EdgeSpec {
            from: NodeName::parse("step-a").expect("valid"),
            to: NodeName::parse("step-a").expect("valid"),
        }],
    };
    let bytes = spec.to_json_bytes();
    let parsed: serde_json::Value =
        serde_json::from_slice(&bytes).expect("to_json_bytes should produce valid JSON");
    assert_eq!(parsed["workflow_name"], "test-wf");
    assert!(parsed["nodes"].is_array());
    assert!(parsed["edges"].is_array());
}

#[test]
fn workflow_spec_with_empty_nodes_and_edges() {
    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("empty").expect("valid"),
        nodes: vec![],
        edges: vec![],
    };
    let json = serde_json::to_string(&spec).expect("serialize");
    let restored: WorkflowSpec = serde_json::from_str(&json).expect("deserialize");
    assert!(restored.nodes.is_empty());
    assert!(restored.edges.is_empty());
}

#[test]
fn parse_graph_args_unrecognized_arg_display_shows_arg() {
    let err = GraphArgsError::UnrecognizedArgument {
        arg: "something".to_string(),
    };
    assert!(
        err.to_string().contains("something"),
        "display should contain the arg: {}",
        err
    );
}

#[test]
fn workflow_spec_deserialize_valid_spec_succeeds() {
    let json = r#"{
        "workflow_name": "valid_workflow",
        "nodes": [
            {"name": "a", "kind": "pure"},
            {"name": "b", "kind": "managed_effect"},
            {"name": "c", "kind": "pure"}
        ],
        "edges": [
            {"from": "a", "to": "b"},
            {"from": "b", "to": "c"}
        ]
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(
        result.is_ok(),
        "valid spec should deserialize: {:?}",
        result
    );
    let spec = result.unwrap();
    assert_eq!(spec.workflow_name.as_str(), "valid_workflow");
    assert_eq!(spec.nodes.len(), 3);
    assert_eq!(spec.edges.len(), 2);
}

#[test]
fn workflow_spec_deserialize_rejects_self_cycle() {
    let json = r#"{
        "workflow_name": "self_cycle",
        "nodes": [
            {"name": "a", "kind": "pure"},
            {"name": "b", "kind": "pure"}
        ],
        "edges": [
            {"from": "a", "to": "a"}
        ]
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "self-cycle should be rejected: {:?}",
        result
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("self-loop"),
        "error should mention self-loop: {}",
        err_msg
    );
}

#[test]
fn workflow_spec_deserialize_rejects_mutual_dependency() {
    let json = r#"{
        "workflow_name": "mutual_dep",
        "nodes": [
            {"name": "a", "kind": "pure"},
            {"name": "b", "kind": "pure"}
        ],
        "edges": [
            {"from": "a", "to": "b"},
            {"from": "b", "to": "a"}
        ]
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "mutual dependency should be rejected: {:?}",
        result
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("cycle"),
        "error should mention cycle: {}",
        err_msg
    );
}
