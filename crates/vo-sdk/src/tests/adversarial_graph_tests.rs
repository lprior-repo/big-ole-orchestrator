//! Adversarial tests for vo-sdk (bead ve-z32z).
//!
//! DIMENSION: parse_graph_args edge cases.

use crate::graph::{parse_graph_args, GraphArgs, GraphArgsError};

#[test]
fn parse_graph_args_rejects_args_after_graph_in_middle() {
    let args = vec![
        "bin".to_string(),
        "other".to_string(),
        "--graph".to_string(),
    ];
    let result = parse_graph_args(&args);
    assert_eq!(result, Ok(GraphArgs));
}

#[test]
fn parse_graph_args_rejects_second_graph_flag() {
    let args = vec![
        "bin".to_string(),
        "--graph".to_string(),
        "--graph".to_string(),
    ];
    let result = parse_graph_args(&args);
    assert!(matches!(
        result,
        Err(GraphArgsError::UnrecognizedArgument { .. })
    ));
}

#[test]
fn parse_graph_args_accepts_graph_as_first_arg() {
    let args = vec!["bin".to_string(), "--graph".to_string()];
    let result = parse_graph_args(&args);
    assert_eq!(result, Ok(GraphArgs));
}

#[test]
fn parse_graph_args_rejects_empty_arg_after_graph() {
    let args = vec!["bin".to_string(), "--graph".to_string(), "".to_string()];
    let result = parse_graph_args(&args);
    assert!(matches!(
        result,
        Err(GraphArgsError::UnrecognizedArgument { .. })
    ));
}

#[test]
fn graph_args_error_no_graph_flag_display() {
    let err = GraphArgsError::NoGraphFlag;
    let msg = err.to_string();
    assert!(msg.contains("no --graph flag found"), "display: {}", msg);
}

#[test]
fn graph_args_error_unrecognized_argument_display() {
    let err = GraphArgsError::UnrecognizedArgument {
        arg: "extra".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("extra"), "display: {}", msg);
    assert!(msg.contains("unrecognized"), "display: {}", msg);
}

#[test]
fn graph_args_is_copy_and_clone() {
    let args = vec!["bin".to_string(), "--graph".to_string()];
    let ga = parse_graph_args(&args).unwrap();
    let copied = ga;
    let cloned = ga.clone();
    assert_eq!(ga, copied);
    assert_eq!(ga, cloned);
}