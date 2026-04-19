use std::io::Write;

use tempfile::NamedTempFile;

/// Helper: write JSON bytes to a temp file and return its path.
fn write_json(json: &[u8]) -> NamedTempFile {
    let mut f = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("create temp file");
    f.write_all(json).expect("write json");
    f
}

// ---------------------------------------------------------------------------
// Valid workflow specs
// ---------------------------------------------------------------------------

#[test]
fn valid_single_node_workflow_passes() {
    let json = r#"{
        "workflow_name": "single-step",
        "nodes": [
            { "node_name": "start", "retry_policy": { "max_attempts": 3, "backoff_ms": 100, "backoff_multiplier": 2.0, "max_backoff_ms": 60000 } }
        ],
        "edges": []
    }"#;
    let f = write_json(json.as_bytes());
    let result = vo_cli::commands::check::validate_workflow_spec(f.path());
    assert!(result.is_ok());
}

#[test]
fn valid_two_node_linear_workflow_passes() {
    let json = r#"{
        "workflow_name": "linear-pipeline",
        "nodes": [
            { "node_name": "fetch", "retry_policy": { "max_attempts": 1, "backoff_ms": 50, "backoff_multiplier": 1.0, "max_backoff_ms": 100 } },
            { "node_name": "transform", "retry_policy": { "max_attempts": 3, "backoff_ms": 200, "backoff_multiplier": 2.0, "max_backoff_ms": 10000 } }
        ],
        "edges": [
            { "source_node": "fetch", "target_node": "transform", "condition": "Always" }
        ]
    }"#;
    let f = write_json(json.as_bytes());
    let result = vo_cli::commands::check::validate_workflow_spec(f.path());
    assert!(result.is_ok());
}

#[test]
fn valid_diamond_dag_passes() {
    let json = r#"{
        "workflow_name": "diamond",
        "nodes": [
            { "node_name": "a", "retry_policy": { "max_attempts": 1, "backoff_ms": 100, "backoff_multiplier": 1.0, "max_backoff_ms": 200 } },
            { "node_name": "b", "retry_policy": { "max_attempts": 1, "backoff_ms": 100, "backoff_multiplier": 1.0, "max_backoff_ms": 200 } },
            { "node_name": "c", "retry_policy": { "max_attempts": 1, "backoff_ms": 100, "backoff_multiplier": 1.0, "max_backoff_ms": 200 } },
            { "node_name": "d", "retry_policy": { "max_attempts": 1, "backoff_ms": 100, "backoff_multiplier": 1.0, "max_backoff_ms": 200 } }
        ],
        "edges": [
            { "source_node": "a", "target_node": "b", "condition": "Always" },
            { "source_node": "a", "target_node": "c", "condition": "Always" },
            { "source_node": "b", "target_node": "d", "condition": "Always" },
            { "source_node": "c", "target_node": "d", "condition": "Always" }
        ]
    }"#;
    let f = write_json(json.as_bytes());
    assert!(vo_cli::commands::check::validate_workflow_spec(f.path()).is_ok());
}

#[test]
fn valid_workflow_with_conditional_edges_passes() {
    let json = r#"{
        "workflow_name": "conditional",
        "nodes": [
            { "node_name": "decide", "retry_policy": { "max_attempts": 1, "backoff_ms": 50, "backoff_multiplier": 1.0, "max_backoff_ms": 100 } },
            { "node_name": "on_success", "retry_policy": { "max_attempts": 1, "backoff_ms": 50, "backoff_multiplier": 1.0, "max_backoff_ms": 100 } },
            { "node_name": "on_failure", "retry_policy": { "max_attempts": 1, "backoff_ms": 50, "backoff_multiplier": 1.0, "max_backoff_ms": 100 } }
        ],
        "edges": [
            { "source_node": "decide", "target_node": "on_success", "condition": "OnSuccess" },
            { "source_node": "decide", "target_node": "on_failure", "condition": "OnFailure" }
        ]
    }"#;
    let f = write_json(json.as_bytes());
    assert!(vo_cli::commands::check::validate_workflow_spec(f.path()).is_ok());
}

// ---------------------------------------------------------------------------
// Invalid workflow specs — structural errors
// ---------------------------------------------------------------------------

#[test]
fn empty_nodes_list_fails() {
    let json = r#"{
        "workflow_name": "empty",
        "nodes": [],
        "edges": []
    }"#;
    let f = write_json(json.as_bytes());
    let err = vo_cli::commands::check::validate_workflow_spec(f.path()).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("at least one node"),
        "expected 'at least one node' error, got: {msg}"
    );
}

#[test]
fn cycle_detected_fails() {
    let json = r#"{
        "workflow_name": "cyclic",
        "nodes": [
            { "node_name": "a", "retry_policy": { "max_attempts": 1, "backoff_ms": 50, "backoff_multiplier": 1.0, "max_backoff_ms": 100 } },
            { "node_name": "b", "retry_policy": { "max_attempts": 1, "backoff_ms": 50, "backoff_multiplier": 1.0, "max_backoff_ms": 100 } },
            { "node_name": "c", "retry_policy": { "max_attempts": 1, "backoff_ms": 50, "backoff_multiplier": 1.0, "max_backoff_ms": 100 } }
        ],
        "edges": [
            { "source_node": "a", "target_node": "b", "condition": "Always" },
            { "source_node": "b", "target_node": "c", "condition": "Always" },
            { "source_node": "c", "target_node": "a", "condition": "Always" }
        ]
    }"#;
    let f = write_json(json.as_bytes());
    let err = vo_cli::commands::check::validate_workflow_spec(f.path()).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("cycle"), "expected cycle error, got: {msg}");
}

#[test]
fn self_loop_detected_fails() {
    let json = r#"{
        "workflow_name": "self-loop",
        "nodes": [
            { "node_name": "loop", "retry_policy": { "max_attempts": 1, "backoff_ms": 50, "backoff_multiplier": 1.0, "max_backoff_ms": 100 } }
        ],
        "edges": [
            { "source_node": "loop", "target_node": "loop", "condition": "Always" }
        ]
    }"#;
    let f = write_json(json.as_bytes());
    let err = vo_cli::commands::check::validate_workflow_spec(f.path()).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("cycle"),
        "expected cycle error for self-loop, got: {msg}"
    );
}

#[test]
fn unknown_edge_target_fails() {
    let json = r#"{
        "workflow_name": "bad-edge",
        "nodes": [
            { "node_name": "start", "retry_policy": { "max_attempts": 1, "backoff_ms": 50, "backoff_multiplier": 1.0, "max_backoff_ms": 100 } }
        ],
        "edges": [
            { "source_node": "start", "target_node": "nonexistent", "condition": "Always" }
        ]
    }"#;
    let f = write_json(json.as_bytes());
    let err = vo_cli::commands::check::validate_workflow_spec(f.path()).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unknown"),
        "expected unknown node error, got: {msg}"
    );
}

#[test]
fn invalid_retry_policy_zero_attempts_fails() {
    let json = r#"{
        "workflow_name": "bad-retry",
        "nodes": [
            { "node_name": "start", "retry_policy": { "max_attempts": 0, "backoff_ms": 50, "backoff_multiplier": 1.0, "max_backoff_ms": 100 } }
        ],
        "edges": []
    }"#;
    let f = write_json(json.as_bytes());
    let err = vo_cli::commands::check::validate_workflow_spec(f.path()).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("retry policy"),
        "expected retry policy error, got: {msg}"
    );
}

#[test]
fn invalid_retry_policy_zero_multiplier_fails() {
    let json = r#"{
        "workflow_name": "bad-mult",
        "nodes": [
            { "node_name": "start", "retry_policy": { "max_attempts": 3, "backoff_ms": 50, "backoff_multiplier": 0.0, "max_backoff_ms": 100 } }
        ],
        "edges": []
    }"#;
    let f = write_json(json.as_bytes());
    let err = vo_cli::commands::check::validate_workflow_spec(f.path()).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("retry policy"),
        "expected retry policy error, got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// File I/O errors
// ---------------------------------------------------------------------------

#[test]
fn missing_workflow_file_fails() {
    let result = vo_cli::commands::check::validate_workflow_spec(std::path::Path::new(
        "/nonexistent/path/workflow.json",
    ));
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("file not found") || msg.contains("not found"),
        "expected file not found error, got: {msg}"
    );
}

#[test]
fn invalid_json_fails() {
    let mut f = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("create temp file");
    f.write_all(b"this is not json {{{{").expect("write");
    let err = vo_cli::commands::check::validate_workflow_spec(f.path()).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("deserialization") || msg.contains("invalid"),
        "expected deserialization error, got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// CLI integration: --workflow flag parsing
// ---------------------------------------------------------------------------

#[test]
fn cli_check_workflow_flag_parses() {
    use std::ffi::OsString;
    let args: Vec<OsString> = vec![
        "vo".into(),
        "check".into(),
        "--workflow".into(),
        "workflow.json".into(),
    ];
    let cli = vo_cli::cli::interpret_cli_from(args).unwrap();
    match cli.command {
        vo_cli::cli::Command::Check { path, workflow } => {
            assert_eq!(path, std::path::PathBuf::from("workflow.json"));
            assert!(workflow);
        }
        other => panic!("expected Check command, got: {other:?}"),
    }
}

#[test]
fn cli_check_without_workflow_flag_defaults_to_binary_mode() {
    use std::ffi::OsString;
    let args: Vec<OsString> = vec!["vo".into(), "check".into(), "/bin/ls".into()];
    let cli = vo_cli::cli::interpret_cli_from(args).unwrap();
    match cli.command {
        vo_cli::cli::Command::Check { path, workflow } => {
            assert_eq!(path, std::path::PathBuf::from("/bin/ls"));
            assert!(!workflow);
        }
        other => panic!("expected Check command, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// validate_workflow_spec returns WorkflowDefinition details on success
// ---------------------------------------------------------------------------

#[test]
fn valid_spec_returns_workflow_name_and_node_count() {
    let json = r#"{
        "workflow_name": "test-wf",
        "nodes": [
            { "node_name": "a", "retry_policy": { "max_attempts": 1, "backoff_ms": 100, "backoff_multiplier": 1.0, "max_backoff_ms": 200 } },
            { "node_name": "b", "retry_policy": { "max_attempts": 1, "backoff_ms": 100, "backoff_multiplier": 1.0, "max_backoff_ms": 200 } }
        ],
        "edges": [
            { "source_node": "a", "target_node": "b", "condition": "Always" }
        ]
    }"#;
    let f = write_json(json.as_bytes());
    let result = vo_cli::commands::check::validate_workflow_spec(f.path());
    assert!(result.is_ok());
    let def = result.unwrap();
    assert_eq!(def.workflow_name.as_str(), "test-wf");
    assert_eq!(def.nodes.as_slice().len(), 2);
}
