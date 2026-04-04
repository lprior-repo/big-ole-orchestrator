//! Tests for graph_args module (--graph CLI argument handling per ADR-004, ADR-009).

use crate::graph_args::{parse_graph_args, GraphArgs, GraphArgsError, NodeSpec, EdgeSpec, NodeKind};
use vo_types::{WorkflowName, NodeName};

#[test]
fn parse_graph_args_returns_err_when_no_graph_flag() {
    let args: Vec<String> = vec!["my_binary".to_string()];
    let result = parse_graph_args(&args);
    assert!(result.is_err(), "should return Err when --graph is absent");
}

#[test]
fn parse_graph_args_returns_ok_when_graph_flag_present() {
    let args: Vec<String> = vec!["my_binary".to_string(), "--graph".to_string()];
    let result = parse_graph_args(&args);
    assert_eq!(result, Ok(GraphArgs), "should return Ok(GraphArgs) when --graph is present");
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
    let spec = crate::graph_args::GraphWorkflowSpec {
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
    let restored: crate::graph_args::GraphWorkflowSpec =
        serde_json::from_str(&json).expect("deserialize");
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
