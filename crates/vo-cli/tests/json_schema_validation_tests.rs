//! QA: vo-cli JSON output schema validation (ve-30w)
//!
//! Validates that:
//! 1. history --json outputs a valid WorkflowHistoryResponse JSON object
//! 2. Each entry has required fields (sequence, timestamp_ms, event_type)
//! 3. JSON output envelope schema (type, command, exit_code, version, data/error)
//! 4. All JSON output is valid, parseable, and structurally correct

use std::io::Write;

use vo_cli::commands::workflow_history::{
    WorkflowHistoryConfig, WorkflowHistoryEntry, WorkflowHistoryResponse,
};

// ---------------------------------------------------------------------------
// JSON envelope construction (mirrors json_output.rs privately)
// These construct the same envelope shapes that the CLI produces.
// ---------------------------------------------------------------------------

fn build_success_envelope(command: &str, data: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "type": "success",
        "command": command,
        "exit_code": 0,
        "version": env!("CARGO_PKG_VERSION"),
        "commit": option_env!("GIT_HASH").unwrap_or("unknown"),
        "data": data,
    })
}

fn build_error_envelope(
    command: &str,
    exit_code: i32,
    kind: &str,
    message: &str,
) -> serde_json::Value {
    serde_json::json!({
        "type": "error",
        "command": command,
        "exit_code": exit_code,
        "version": env!("CARGO_PKG_VERSION"),
        "commit": option_env!("GIT_HASH").unwrap_or("unknown"),
        "error": {
            "kind": kind,
            "message": message,
        },
    })
}

// ---------------------------------------------------------------------------
// Success envelope schema validation
// ---------------------------------------------------------------------------

#[test]
fn success_envelope_has_all_required_fields() {
    let payload = build_success_envelope("history", serde_json::json!({"entries": []}));
    let json = serde_json::to_string(&payload).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");

    for field in &["type", "command", "exit_code", "version", "commit", "data"] {
        assert!(
            parsed.get(*field).is_some(),
            "success envelope missing '{field}' field"
        );
    }
}

#[test]
fn success_envelope_type_is_string_success() {
    let payload = build_success_envelope("check", serde_json::json!({}));
    assert_eq!(payload["type"].as_str(), Some("success"));
}

#[test]
fn success_envelope_command_matches_input() {
    let payload = build_success_envelope("status", serde_json::json!({}));
    assert_eq!(payload["command"].as_str(), Some("status"));
}

#[test]
fn success_envelope_exit_code_is_zero() {
    let payload = build_success_envelope("gc", serde_json::json!({}));
    assert_eq!(payload["exit_code"].as_i64(), Some(0));
}

#[test]
fn success_envelope_version_is_string() {
    let payload = build_success_envelope("init", serde_json::json!({}));
    assert!(payload["version"].is_string());
    assert!(!payload["version"].as_str().unwrap().is_empty());
}

#[test]
fn success_envelope_commit_is_string() {
    let payload = build_success_envelope("purge", serde_json::json!({}));
    assert!(payload["commit"].is_string());
}

#[test]
fn success_envelope_data_is_object() {
    let payload = build_success_envelope(
        "history",
        serde_json::json!({"entries": [{"sequence": 1}]}),
    );
    assert!(payload["data"].is_object());
    assert!(payload["data"]["entries"].is_array());
}

#[test]
fn success_envelope_has_no_error_field() {
    let payload = build_success_envelope("purge", serde_json::json!({}));
    assert!(
        payload.get("error").is_none(),
        "success envelope must not contain 'error'"
    );
}

// ---------------------------------------------------------------------------
// Error envelope schema validation
// ---------------------------------------------------------------------------

#[test]
fn error_envelope_has_all_required_fields() {
    let payload = build_error_envelope("check", 1, "check_error", "File not found");
    let json = serde_json::to_string(&payload).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");

    for field in &["type", "command", "exit_code", "version", "commit", "error"] {
        assert!(
            parsed.get(*field).is_some(),
            "error envelope missing '{field}' field"
        );
    }
}

#[test]
fn error_envelope_type_is_string_error() {
    let payload = build_error_envelope("status", 1, "connection_error", "timeout");
    assert_eq!(payload["type"].as_str(), Some("error"));
}

#[test]
fn error_envelope_exit_code_is_nonzero() {
    let payload = build_error_envelope("history", 1, "not_found", "not found");
    assert_ne!(payload["exit_code"].as_i64(), Some(0));
}

#[test]
fn error_envelope_error_has_kind_and_message() {
    let payload = build_error_envelope("gc", 1, "dispatch_error", "connection refused");
    let error = &payload["error"];

    assert!(error.is_object(), "'error' must be a JSON object");
    assert!(error.get("kind").is_some(), "'error.kind' missing");
    assert!(error.get("message").is_some(), "'error.message' missing");
    assert_eq!(error["kind"].as_str(), Some("dispatch_error"));
    assert_eq!(error["message"].as_str(), Some("connection refused"));
}

#[test]
fn error_envelope_has_no_data_field() {
    let payload = build_error_envelope("init", 1, "init_error", "dir exists");
    assert!(
        payload.get("data").is_none(),
        "error envelope must not contain 'data'"
    );
}

// ---------------------------------------------------------------------------
// WorkflowHistoryResponse schema (history --json output)
// ---------------------------------------------------------------------------

#[test]
fn history_response_has_required_top_level_fields() {
    let response = WorkflowHistoryResponse {
        instance_id: "ns/01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
        entries: vec![],
        redacted_fields: None,
    };

    let json = serde_json::to_string(&response).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");

    assert!(parsed.get("instance_id").is_some(), "missing instance_id");
    assert!(parsed.get("entries").is_some(), "missing entries");
}

#[test]
fn history_response_instance_id_is_string() {
    let response = WorkflowHistoryResponse {
        instance_id: "ns/01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
        entries: vec![],
        redacted_fields: None,
    };

    let json = serde_json::to_string(&response).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");

    assert!(parsed["instance_id"].is_string());
    assert_eq!(
        parsed["instance_id"].as_str(),
        Some("ns/01ARZ3NDEKTSV4RRFFQ69G5FAV")
    );
}

#[test]
fn history_response_entries_is_array() {
    let response = WorkflowHistoryResponse {
        instance_id: "test".to_string(),
        entries: vec![],
        redacted_fields: None,
    };

    let json = serde_json::to_string(&response).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");

    assert!(parsed["entries"].is_array());
    assert_eq!(parsed["entries"].as_array().unwrap().len(), 0);
}

// ---------------------------------------------------------------------------
// WorkflowHistoryEntry required fields validation
// ---------------------------------------------------------------------------

#[test]
fn history_entry_has_required_fields_with_correct_types() {
    let entry = WorkflowHistoryEntry {
        sequence: 1,
        timestamp_ms: 1700000000000,
        event_type: "workflow_started".to_string(),
        step_id: None,
        error: None,
        output: None,
    };

    let json = serde_json::to_string(&entry).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");

    assert!(parsed.get("sequence").is_some(), "missing sequence");
    assert!(parsed.get("timestamp_ms").is_some(), "missing timestamp_ms");
    assert!(parsed.get("event_type").is_some(), "missing event_type");

    assert!(parsed["sequence"].is_number(), "sequence must be a number");
    assert!(
        parsed["timestamp_ms"].is_number(),
        "timestamp_ms must be a number"
    );
    assert!(
        parsed["event_type"].is_string(),
        "event_type must be a string"
    );
}

#[test]
fn history_entry_sequence_is_u64() {
    let entry = WorkflowHistoryEntry {
        sequence: u64::MAX,
        timestamp_ms: 0,
        event_type: "test".to_string(),
        step_id: None,
        error: None,
        output: None,
    };

    let json = serde_json::to_string(&entry).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");

    assert_eq!(parsed["sequence"].as_u64(), Some(u64::MAX));
}

#[test]
fn history_entry_timestamp_ms_is_u64() {
    let entry = WorkflowHistoryEntry {
        sequence: 0,
        timestamp_ms: 1700000000000,
        event_type: "test".to_string(),
        step_id: None,
        error: None,
        output: None,
    };

    let json = serde_json::to_string(&entry).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");

    assert_eq!(parsed["timestamp_ms"].as_u64(), Some(1700000000000));
}

#[test]
fn history_entry_optional_fields_present_when_some() {
    let entry = WorkflowHistoryEntry {
        sequence: 1,
        timestamp_ms: 1000,
        event_type: "step_completed".to_string(),
        step_id: Some("step-1".to_string()),
        error: Some("timeout exceeded".to_string()),
        output: Some(serde_json::json!({"result": "ok"})),
    };

    let json = serde_json::to_string(&entry).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");

    assert_eq!(parsed["step_id"].as_str(), Some("step-1"));
    assert_eq!(parsed["error"].as_str(), Some("timeout exceeded"));
    assert!(parsed["output"].is_object());
    assert_eq!(parsed["output"]["result"].as_str(), Some("ok"));
}

#[test]
fn history_entry_optional_fields_absent_when_none() {
    let entry = WorkflowHistoryEntry {
        sequence: 1,
        timestamp_ms: 1000,
        event_type: "workflow_started".to_string(),
        step_id: None,
        error: None,
        output: None,
    };

    let json = serde_json::to_string(&entry).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");

    assert!(parsed.get("step_id").is_none(), "step_id should be absent");
    assert!(parsed.get("error").is_none(), "error should be absent");
    assert!(parsed.get("output").is_none(), "output should be absent");
}

#[test]
fn history_response_with_multiple_entries_validates_each() {
    let entries = vec![
        WorkflowHistoryEntry {
            sequence: 1,
            timestamp_ms: 1700000000000,
            event_type: "workflow_started".to_string(),
            step_id: None,
            error: None,
            output: None,
        },
        WorkflowHistoryEntry {
            sequence: 2,
            timestamp_ms: 1700000001000,
            event_type: "step_completed".to_string(),
            step_id: Some("fetch-data".to_string()),
            error: None,
            output: Some(serde_json::json!({"count": 42})),
        },
        WorkflowHistoryEntry {
            sequence: 3,
            timestamp_ms: 1700000002000,
            event_type: "step_failed".to_string(),
            step_id: Some("transform".to_string()),
            error: Some("division by zero".to_string()),
            output: None,
        },
    ];

    let response = WorkflowHistoryResponse {
        instance_id: "ns/01ARZ".to_string(),
        entries,
        redacted_fields: Some(vec![vec!["output".to_string(), "secrets".to_string()]]),
    };

    let json = serde_json::to_string(&response).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");

    let entries_arr = parsed["entries"].as_array().expect("entries array");
    assert_eq!(entries_arr.len(), 3);

    // Entry 1: minimal
    assert_eq!(entries_arr[0]["sequence"].as_u64(), Some(1));
    assert_eq!(
        entries_arr[0]["event_type"].as_str(),
        Some("workflow_started")
    );
    assert!(entries_arr[0].get("step_id").is_none());

    // Entry 2: with output
    assert_eq!(entries_arr[1]["sequence"].as_u64(), Some(2));
    assert_eq!(entries_arr[1]["step_id"].as_str(), Some("fetch-data"));
    assert_eq!(entries_arr[1]["output"]["count"].as_u64(), Some(42));

    // Entry 3: with error
    assert_eq!(entries_arr[2]["sequence"].as_u64(), Some(3));
    assert_eq!(entries_arr[2]["error"].as_str(), Some("division by zero"));
    assert!(entries_arr[2].get("output").is_none());

    // redacted_fields present
    assert!(parsed["redacted_fields"].is_array());
}

#[test]
fn history_response_redacted_fields_absent_when_none() {
    let response = WorkflowHistoryResponse {
        instance_id: "test".to_string(),
        entries: vec![],
        redacted_fields: None,
    };

    let json = serde_json::to_string(&response).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");

    assert!(
        parsed.get("redacted_fields").is_none(),
        "redacted_fields should be absent when None"
    );
}

// ---------------------------------------------------------------------------
// Serde roundtrip: ensures JSON is parseable and lossless
// ---------------------------------------------------------------------------

#[test]
fn history_response_roundtrip() {
    let response = WorkflowHistoryResponse {
        instance_id: "ns/01ARZ3NDEK".to_string(),
        entries: vec![
            WorkflowHistoryEntry {
                sequence: 1,
                timestamp_ms: 1700000000000,
                event_type: "workflow_started".to_string(),
                step_id: None,
                error: None,
                output: None,
            },
            WorkflowHistoryEntry {
                sequence: 2,
                timestamp_ms: 1700000001000,
                event_type: "step_completed".to_string(),
                step_id: Some("node-a".to_string()),
                error: None,
                output: Some(serde_json::json!({"processed": 100})),
            },
        ],
        redacted_fields: None,
    };

    let json = serde_json::to_string(&response).expect("serialize");
    let back: WorkflowHistoryResponse =
        serde_json::from_str(&json).expect("deserialize roundtrip");

    assert_eq!(back.instance_id, "ns/01ARZ3NDEK");
    assert_eq!(back.entries.len(), 2);
    assert_eq!(back.entries[0].event_type, "workflow_started");
    assert_eq!(back.entries[1].step_id, Some("node-a".to_string()));
    assert_eq!(
        back.entries[1].output,
        Some(serde_json::json!({"processed": 100}))
    );
}

#[test]
fn history_entry_roundtrip_preserves_all_fields() {
    let entry = WorkflowHistoryEntry {
        sequence: 42,
        timestamp_ms: 1700000000000,
        event_type: "step_completed".to_string(),
        step_id: Some("transform".to_string()),
        error: Some("partial failure".to_string()),
        output: Some(serde_json::json!({"items": [1, 2, 3]})),
    };

    let json = serde_json::to_string(&entry).expect("serialize");
    let back: WorkflowHistoryEntry = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(back.sequence, 42);
    assert_eq!(back.timestamp_ms, 1700000000000);
    assert_eq!(back.event_type, "step_completed");
    assert_eq!(back.step_id, Some("transform".to_string()));
    assert_eq!(back.error, Some("partial failure".to_string()));
    assert_eq!(back.output, Some(serde_json::json!({"items": [1, 2, 3]})));
}

#[test]
fn envelope_success_roundtrip_via_json() {
    let payload = build_success_envelope(
        "history",
        serde_json::json!({"instance_id": "ns/abc", "entries": []}),
    );
    let json = serde_json::to_string(&payload).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");

    assert_eq!(parsed["type"], "success");
    assert_eq!(parsed["data"]["instance_id"], "ns/abc");
}

#[test]
fn envelope_error_roundtrip_via_json() {
    let payload = build_error_envelope("check", 1, "check_error", "bad magic");
    let json = serde_json::to_string(&payload).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");

    assert_eq!(parsed["type"], "error");
    assert_eq!(parsed["exit_code"], 1);
    assert_eq!(parsed["error"]["kind"], "check_error");
    assert_eq!(parsed["error"]["message"], "bad magic");
}

// ---------------------------------------------------------------------------
// Edge cases and special characters
// ---------------------------------------------------------------------------

#[test]
fn envelope_handles_unicode_in_data() {
    let payload = build_success_envelope(
        "history",
        serde_json::json!({"message": "こんにちは世界"}),
    );
    let json = serde_json::to_string(&payload).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(parsed["data"]["message"].as_str(), Some("こんにちは世界"));
}

#[test]
fn envelope_handles_special_chars_in_error_message() {
    let payload = build_error_envelope(
        "check",
        1,
        "check_error",
        "path: /tmp/test (1).bin\nnewline",
    );
    let json = serde_json::to_string(&payload).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert!(parsed["error"]["message"]
        .as_str()
        .unwrap()
        .contains("path: /tmp/test"));
}

#[test]
fn history_entry_with_nested_json_output() {
    let entry = WorkflowHistoryEntry {
        sequence: 1,
        timestamp_ms: 0,
        event_type: "test".to_string(),
        step_id: None,
        error: None,
        output: Some(serde_json::json!({
            "nested": {
                "deep": {
                    "value": 42,
                    "tags": ["a", "b", "c"]
                }
            }
        })),
    };

    let json = serde_json::to_string(&entry).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");

    assert_eq!(
        parsed["output"]["nested"]["deep"]["value"].as_u64(),
        Some(42)
    );
    assert_eq!(
        parsed["output"]["nested"]["deep"]["tags"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
}

// ---------------------------------------------------------------------------
// WorkflowHistoryConfig defaults
// ---------------------------------------------------------------------------

#[test]
fn workflow_history_config_defaults() {
    let config = WorkflowHistoryConfig::default();
    assert_eq!(config.instance_id, "");
    assert_eq!(config.engine_url, "http://localhost:3000");
    assert!(!config.json);
}

// ---------------------------------------------------------------------------
// check command: no --json flag (human-readable output only)
// ---------------------------------------------------------------------------

#[test]
fn check_command_rejects_json_flag() {
    let args: Vec<std::ffi::OsString> = vec![
        "vo".into(),
        "check".into(),
        "/bin/ls".into(),
        "--json".into(),
    ];
    let result = vo_cli::interpret_cli_from(args);
    assert!(
        result.is_err(),
        "check command should not accept --json flag"
    );
}

#[test]
fn check_binary_validation_output_is_human_readable() {
    let mut f = tempfile::Builder::new()
        .suffix(".elf")
        .tempfile()
        .expect("create temp file");
    f.write_all(b"\x7FELF\x02\x01\x01\x00")
        .expect("write ELF header");
    f.flush().expect("flush");

    let result = vo_cli::validate_binary_header(f.path()).expect("validate");
    let display = result.display_name();
    assert!(!display.starts_with('{'));
    assert!(!display.contains("\"type\""));
}

// ---------------------------------------------------------------------------
// CLI parsing: history --json flag propagation
// ---------------------------------------------------------------------------

#[test]
fn history_cli_json_flag_propagates_to_command() {
    let args: Vec<std::ffi::OsString> = vec![
        "vo".into(),
        "history".into(),
        "ns/01ARZ3NDEK".into(),
        "--json".into(),
    ];
    let cli = vo_cli::interpret_cli_from(args).unwrap();
    match cli.command {
        vo_cli::Command::History { json, .. } => assert!(json, "--json should set json=true"),
        other => panic!("expected History, got {other:?}"),
    }
}

#[test]
fn history_cli_without_json_defaults_false() {
    let args: Vec<std::ffi::OsString> = vec![
        "vo".into(),
        "history".into(),
        "ns/01ARZ3NDEK".into(),
    ];
    let cli = vo_cli::interpret_cli_from(args).unwrap();
    match cli.command {
        vo_cli::Command::History { json, .. } => assert!(!json),
        other => panic!("expected History, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// error_kind mapping (test through CliError -> kind string)
// ---------------------------------------------------------------------------

#[test]
fn error_kind_clap_maps_to_invalid_usage() {
    let err =
        vo_cli::CliError::Clap(clap::Error::new(clap::error::ErrorKind::InvalidValue));
    let kind = vo_cli::map_error_to_exit_code(&err);
    assert_eq!(kind, 2, "Clap errors should map to exit code 2");
}

#[test]
fn error_kind_check_maps_to_exit_1() {
    let err = vo_cli::CliError::Check(vo_cli::CheckError::FileNotFound {
        path: std::path::PathBuf::from("/x"),
    });
    assert_eq!(vo_cli::map_error_to_exit_code(&err), 1);
}

#[test]
fn error_kind_dispatch_maps_to_exit_1() {
    let err = vo_cli::CliError::Dispatch("test".to_string());
    assert_eq!(vo_cli::map_error_to_exit_code(&err), 1);
}

#[test]
fn error_kind_invalid_numeric_maps_to_exit_2() {
    let err = vo_cli::CliError::InvalidNumeric("bad".to_string());
    assert_eq!(vo_cli::map_error_to_exit_code(&err), 2);
}
