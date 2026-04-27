use std::io::Write;
use std::path::Path;

use tempfile::NamedTempFile;
use vo_cli::cli::{interpret_cli_from, Command};
use vo_cli::commands::check::{
    detect_elf_architecture, validate_binary_header, validate_workflow_spec, BinaryFormat, ElfMachine,
    CheckError,
};
use vo_cli::commands::history::{
    undo_command, redo_command, UndoResult, RedoResult,
};
use vo_cli::commands::workflow_history::WorkflowHistoryEntry;

fn write_json(json: &[u8]) -> NamedTempFile {
    let mut f = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("create temp file");
    f.write_all(json).expect("write json");
    f
}

#[test]
fn history_cli_parses_json_flag() {
    let args: Vec<std::ffi::OsString> = vec![
        "vo".into(),
        "history".into(),
        "ns/01ARZ3NDEKTSV4RRFFQ69G5FAV".into(),
        "--json".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::History { json, .. } => assert!(json),
        other => panic!("expected History command with json=true, got: {other:?}"),
    }
}

#[test]
fn history_cli_json_flag_defaults_to_false() {
    let args: Vec<std::ffi::OsString> = vec![
        "vo".into(),
        "history".into(),
        "ns/01ARZ3NDEKTSV4RRFFQ69G5FAV".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::History { json, .. } => assert!(!json),
        other => panic!("expected History command with json=false, got: {other:?}"),
    }
}

#[test]
fn history_cli_parses_with_custom_engine_url() {
    let args: Vec<std::ffi::OsString> = vec![
        "vo".into(),
        "history".into(),
        "ns/01ARZ3NDEK".into(),
        "--engine-url".into(),
        "http://localhost:9000".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::History {
            engine_url, json, ..
        } => {
            assert_eq!(engine_url, "http://localhost:9000");
            assert!(!json);
        }
        other => panic!("expected History command, got: {other:?}"),
    }
}

#[test]
fn history_cli_canonical_flag() {
    let args: Vec<std::ffi::OsString> = vec![
        "vo".into(),
        "history".into(),
        "ns/01ARZ3NDEK".into(),
        "--canonical".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::History { canonical, .. } => assert!(canonical),
        other => panic!("expected History command with canonical=true, got: {other:?}"),
    }
}

#[test]
fn history_cli_json_and_canonical_together() {
    let args: Vec<std::ffi::OsString> = vec![
        "vo".into(),
        "history".into(),
        "ns/01ARZ3NDEK".into(),
        "--json".into(),
        "--canonical".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::History {
            json, canonical, ..
        } => {
            assert!(json);
            assert!(canonical);
        }
        other => panic!("expected History command, got: {other:?}"),
    }
}

#[test]
fn history_cli_instance_id_parsing() {
    let args: Vec<std::ffi::OsString> = vec![
        "vo".into(),
        "history".into(),
        "ns/01ARZ3NDEKTSV4RRFFQ69G5FAV".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::History { instance_id, .. } => {
            assert_eq!(instance_id, "ns/01ARZ3NDEKTSV4RRFFQ69G5FAV");
        }
        other => panic!("expected History command, got: {other:?}"),
    }
}

#[test]
fn check_cli_binary_mode_by_default() {
    let args: Vec<std::ffi::OsString> = vec!["vo".into(), "check".into(), "/bin/ls".into()];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::Check { path, workflow } => {
            assert_eq!(path, Path::new("/bin/ls"));
            assert!(!workflow, "Default should be binary mode");
        }
        other => panic!("expected Check command, got: {other:?}"),
    }
}

#[test]
fn check_cli_workflow_mode_with_flag() {
    let args: Vec<std::ffi::OsString> = vec![
        "vo".into(),
        "check".into(),
        "workflow.json".into(),
        "--workflow".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::Check { path, workflow } => {
            assert_eq!(path, Path::new("workflow.json"));
            assert!(workflow, "Should be workflow mode");
        }
        other => panic!("expected Check command, got: {other:?}"),
    }
}

#[test]
fn check_cli_workflow_flag_requires_path() {
    let args: Vec<std::ffi::OsString> = vec!["vo".into(), "check".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err());
}

#[test]
fn validate_binary_header_elf_format() {
    let mut elf_file = tempfile::Builder::new()
        .suffix(".elf")
        .tempfile()
        .expect("create temp file");
    elf_file
        .write_all(b"\x7FELF\x02\x01\x01\x00")
        .expect("write ELF header");
    elf_file.flush().expect("flush");

    let result = validate_binary_header(elf_file.path()).expect("validate ELF");
    assert_eq!(result, BinaryFormat::Elf);
}

#[test]
fn validate_binary_header_macho_64_le() {
    let mut f = tempfile::Builder::new()
        .suffix(".macho")
        .tempfile()
        .expect("create temp file");
    f.write_all(b"\xCF\xFA\xED\xFE")
        .expect("write Mach-O magic");
    f.flush().expect("flush");

    let result = validate_binary_header(f.path()).expect("validate Mach-O");
    assert_eq!(result, BinaryFormat::MachO64LittleEndian);
}

#[test]
fn validate_binary_header_macho_64_be() {
    let mut f = tempfile::Builder::new()
        .suffix(".macho")
        .tempfile()
        .expect("create temp file");
    f.write_all(b"\xFE\xED\xFA\xCF")
        .expect("write Mach-O magic");
    f.flush().expect("flush");

    let result = validate_binary_header(f.path()).expect("validate Mach-O");
    assert_eq!(result, BinaryFormat::MachO64BigEndian);
}

#[test]
fn validate_binary_header_macho_32_le() {
    let mut f = tempfile::Builder::new()
        .suffix(".macho")
        .tempfile()
        .expect("create temp file");
    f.write_all(b"\xCE\xFA\xED\xFE")
        .expect("write Mach-O magic");
    f.flush().expect("flush");

    let result = validate_binary_header(f.path()).expect("validate Mach-O");
    assert_eq!(result, BinaryFormat::MachO32LittleEndian);
}

#[test]
fn validate_binary_header_macho_32_be() {
    let mut f = tempfile::Builder::new()
        .suffix(".macho")
        .tempfile()
        .expect("create temp file");
    f.write_all(b"\xFE\xED\xFA\xCE")
        .expect("write Mach-O magic");
    f.flush().expect("flush");

    let result = validate_binary_header(f.path()).expect("validate Mach-O");
    assert_eq!(result, BinaryFormat::MachO32BigEndian);
}

#[test]
fn validate_binary_header_invalid_magic() {
    let mut f = tempfile::Builder::new()
        .suffix(".bin")
        .tempfile()
        .expect("create temp file");
    f.write_all(b"ABCD")
        .expect("write invalid magic");
    f.flush().expect("flush");

    let result = validate_binary_header(f.path());
    assert!(result.is_err());
    match result.unwrap_err() {
        CheckError::InvalidMagic { .. } => {}
        other => panic!("expected InvalidMagic error, got: {other}"),
    }
}

#[test]
fn validate_binary_header_file_too_small() {
    let mut f = tempfile::Builder::new()
        .suffix(".bin")
        .tempfile()
        .expect("create temp file");
    f.write_all(b"ABC")
        .expect("write only 3 bytes");
    f.flush().expect("flush");

    let result = validate_binary_header(f.path());
    assert!(result.is_err());
    match result.unwrap_err() {
        CheckError::FileTooSmall { .. } => {}
        other => panic!("expected FileTooSmall error, got: {other}"),
    }
}

#[test]
fn validate_binary_header_nonexistent_file() {
    let result = validate_binary_header(Path::new("/nonexistent/file"));
    assert!(result.is_err());
    match result.unwrap_err() {
        CheckError::FileNotFound { .. } => {}
        other => panic!("expected FileNotFound error, got: {other}"),
    }
}

#[test]
fn detect_elf_architecture_valid_elf_returns_some() {
    let mut f = tempfile::Builder::new()
        .suffix(".elf")
        .tempfile()
        .expect("create temp file");
    f.write_all(b"\x7FELF\x02\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x3E\x00")
        .expect("write ELF header");
    f.flush().expect("flush");

    let result = detect_elf_architecture(f.path()).expect("detect architecture");
    assert!(result.is_some());
}

#[test]
fn detect_elf_architecture_non_elf_returns_error() {
    let mut f = tempfile::Builder::new()
        .suffix(".bin")
        .tempfile()
        .expect("create temp file");
    f.write_all(b"NOT_ELF")
        .expect("write non-ELF");
    f.flush().expect("flush");

    let result = detect_elf_architecture(f.path());
    assert!(result.is_err());
    match result.unwrap_err() {
        CheckError::InvalidMagic { .. } => {}
        other => panic!("expected InvalidMagic error, got: {other}"),
    }
}

#[test]
fn workflow_spec_valid_single_node() {
    let json = r#"{
        "workflow_name": "single-step",
        "nodes": [
            { "node_name": "start", "retry_policy": { "max_attempts": 3, "backoff_ms": 100, "backoff_multiplier": 2.0, "max_backoff_ms": 60000 } }
        ],
        "edges": []
    }"#;
    let f = write_json(json.as_bytes());
    let result = validate_workflow_spec(f.path());
    assert!(result.is_ok(), "Valid workflow should pass: {:?}", result);
}

#[test]
fn workflow_spec_dependency_chain() {
    let json = r#"{
        "workflow_name": "dependency-chain",
        "nodes": [
            { "node_name": "fetch", "retry_policy": { "max_attempts": 1, "backoff_ms": 50, "backoff_multiplier": 1.0, "max_backoff_ms": 100 } },
            { "node_name": "validate", "retry_policy": { "max_attempts": 2, "backoff_ms": 100, "backoff_multiplier": 1.5, "max_backoff_ms": 500 } },
            { "node_name": "store", "retry_policy": { "max_attempts": 3, "backoff_ms": 200, "backoff_multiplier": 2.0, "max_backoff_ms": 1000 } }
        ],
        "edges": [
            { "source_node": "fetch", "target_node": "validate", "condition": "Always" },
            { "source_node": "validate", "target_node": "store", "condition": "OnSuccess" }
        ]
    }"#;
    let f = write_json(json.as_bytes());
    let result = validate_workflow_spec(f.path());
    assert!(result.is_ok(), "Dependency chain should be valid: {:?}", result);
}

#[test]
fn workflow_spec_output_format_json() {
    let json = r#"{
        "workflow_name": "output-test",
        "nodes": [
            { "node_name": "start", "retry_policy": { "max_attempts": 1, "backoff_ms": 50, "backoff_multiplier": 1.0, "max_backoff_ms": 100 } }
        ],
        "edges": []
    }"#;
    let f = write_json(json.as_bytes());
    let result = validate_workflow_spec(f.path()).expect("parse workflow");
    let def = result;

    let serialized = serde_json::to_string_pretty(&def).expect("serialize WorkflowDefinition");
    assert!(serialized.contains("workflow_name"));
    assert!(serialized.contains("nodes"));
    assert!(serialized.contains("edges"));
}

#[test]
fn workflow_spec_output_parseable_by_jq() {
    let json = r#"{
        "workflow_name": "jq-test",
        "nodes": [
            { "node_name": "a", "retry_policy": { "max_attempts": 1, "backoff_ms": 50, "backoff_multiplier": 1.0, "max_backoff_ms": 100 } },
            { "node_name": "b", "retry_policy": { "max_attempts": 1, "backoff_ms": 50, "backoff_multiplier": 1.0, "max_backoff_ms": 100 } }
        ],
        "edges": [
            { "source_node": "a", "target_node": "b", "condition": "Always" }
        ]
    }"#;
    let f = write_json(json.as_bytes());
    let result = validate_workflow_spec(f.path()).expect("parse workflow");
    let def = result;

    let json_output = serde_json::to_string(&def).expect("serialize to JSON");
    let parsed: serde_json::Value = serde_json::from_str(&json_output).expect("parse JSON");

    assert!(parsed.get("workflow_name").is_some());
    assert!(parsed.get("nodes").is_some());
    assert!(parsed.get("edges").is_some());

    let nodes = parsed.get("nodes").unwrap();
    assert!(nodes.is_array());
    assert_eq!(nodes.as_array().unwrap().len(), 2);
}

#[test]
fn workflow_spec_check_command_output_format() {
    let json = r#"{
        "workflow_name": "output-format",
        "nodes": [
            { "node_name": "test", "retry_policy": { "max_attempts": 1, "backoff_ms": 50, "backoff_multiplier": 1.0, "max_backoff_ms": 100 } }
        ],
        "edges": []
    }"#;
    let f = write_json(json.as_bytes());
    let result = validate_workflow_spec(f.path()).expect("parse workflow");
    let def = result;

    let json_str = serde_json::to_string(&def).expect("serialize WorkflowDefinition");

    let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("parse for jq");
    let keys: Vec<&str> = parsed
        .as_object()
        .unwrap()
        .keys()
        .map(|s| s.as_str())
        .collect();
    assert!(keys.contains(&"workflow_name"));
    assert!(keys.contains(&"nodes"));
    assert!(keys.contains(&"edges"));
}

#[test]
fn workflow_spec_with_conditional_dependencies() {
    let json = r#"{
        "workflow_name": "conditional-deps",
        "nodes": [
            { "node_name": "decide", "retry_policy": { "max_attempts": 1, "backoff_ms": 50, "backoff_multiplier": 1.0, "max_backoff_ms": 100 } },
            { "node_name": "path_a", "retry_policy": { "max_attempts": 1, "backoff_ms": 50, "backoff_multiplier": 1.0, "max_backoff_ms": 100 } },
            { "node_name": "path_b", "retry_policy": { "max_attempts": 1, "backoff_ms": 50, "backoff_multiplier": 1.0, "max_backoff_ms": 100 } },
            { "node_name": "merge", "retry_policy": { "max_attempts": 1, "backoff_ms": 50, "backoff_multiplier": 1.0, "max_backoff_ms": 100 } }
        ],
        "edges": [
            { "source_node": "decide", "target_node": "path_a", "condition": "OnSuccess" },
            { "source_node": "decide", "target_node": "path_b", "condition": "OnFailure" },
            { "source_node": "path_a", "target_node": "merge", "condition": "Always" },
            { "source_node": "path_b", "target_node": "merge", "condition": "Always" }
        ]
    }"#;
    let f = write_json(json.as_bytes());
    let result = validate_workflow_spec(f.path());
    assert!(result.is_ok(), "Conditional dependencies should be valid: {:?}", result);
}

#[test]
fn workflow_spec_json_field_naming_consistency() {
    let json = r#"{
        "workflow_name": "field-test",
        "nodes": [
            { "node_name": "test", "retry_policy": { "max_attempts": 1, "backoff_ms": 50, "backoff_multiplier": 1.0, "max_backoff_ms": 100 } }
        ],
        "edges": []
    }"#;
    let f = write_json(json.as_bytes());
    let result = validate_workflow_spec(f.path()).expect("parse workflow");
    let def = result;

    let json_str = serde_json::to_string(&def).expect("serialize WorkflowDefinition");
    let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("parse JSON");

    fn check_snake_case(val: &serde_json::Value, path: &str) {
        if let Some(obj) = val.as_object() {
            for (key, v) in obj {
                assert!(
                    key.chars().all(|c| c.is_lowercase() || c == '_'),
                    "Field {path}.{key} should be snake_case"
                );
                check_snake_case(v, &format!("{path}.{key}"));
            }
        } else if let Some(arr) = val.as_array() {
            for (i, item) in arr.iter().enumerate() {
                check_snake_case(item, &format!("{path}[{i}]"));
            }
        }
    }

    check_snake_case(&parsed, "$");
}

#[test]
fn workflow_history_entry_skips_none_fields_in_output() {
    let entry = WorkflowHistoryEntry {
        sequence: 1,
        timestamp_ms: 1700000000000,
        event_type: "test".to_string(),
        step_id: None,
        error: None,
        output: None,
    };

    let json = serde_json::to_string(&entry).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse JSON");

    assert!(parsed.get("step_id").is_none(), "None fields should be skipped");
    assert!(parsed.get("error").is_none(), "None fields should be skipped");
    assert!(parsed.get("output").is_none(), "None fields should be skipped");
}

#[test]
fn workflow_history_entry_has_all_required_fields_when_present() {
    let entry = WorkflowHistoryEntry {
        sequence: 42,
        timestamp_ms: 1700000000000,
        event_type: "step_completed".to_string(),
        step_id: Some("step-1".to_string()),
        error: None,
        output: Some(serde_json::json!({"result": "ok"})),
    };

    let json = serde_json::to_string(&entry).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse JSON");

    assert_eq!(parsed["sequence"], 42);
    assert_eq!(parsed["timestamp_ms"], 1700000000000i64);
    assert_eq!(parsed["event_type"], "step_completed");
    assert_eq!(parsed["step_id"], "step-1");
    assert!(parsed.get("error").is_none());
    assert!(parsed.get("output").is_some());
}

#[test]
fn elf_machine_display_name() {
    assert_eq!(ElfMachine::X86_64.display_name(), "x86_64");
    assert_eq!(ElfMachine::AArch64.display_name(), "AArch64");
    assert_eq!(ElfMachine::Arm.display_name(), "ARM");
    assert_eq!(ElfMachine::X86.display_name(), "x86");
    assert!(ElfMachine::Unknown(999).display_name().contains("999"));
}

#[test]
fn binary_format_display_name() {
    assert_eq!(
        BinaryFormat::Elf.display_name(),
        "valid ELF binary"
    );
    assert_eq!(
        BinaryFormat::MachO64LittleEndian.display_name(),
        "valid Mach-O 64-bit binary"
    );
    assert_eq!(
        BinaryFormat::MachO64BigEndian.display_name(),
        "valid Mach-O 64-bit binary"
    );
    assert_eq!(
        BinaryFormat::MachO32LittleEndian.display_name(),
        "valid Mach-O 32-bit binary"
    );
    assert_eq!(
        BinaryFormat::MachO32BigEndian.display_name(),
        "valid Mach-O 32-bit binary"
    );
}

#[test]
fn check_error_display_includes_path() {
    let err = CheckError::FileNotFound {
        path: Path::new("/test/path").to_path_buf(),
    };
    let msg = err.to_string();
    assert!(msg.contains("/test/path"));
    assert!(msg.contains("not found") || msg.contains("FileNotFound"));
}

#[test]
fn check_error_eq_for_same_path() {
    let err1 = CheckError::FileNotFound {
        path: Path::new("/a").to_path_buf(),
    };
    let err2 = CheckError::FileNotFound {
        path: Path::new("/a").to_path_buf(),
    };
    assert_eq!(err1, err2);
}

#[test]
fn check_error_neq_for_different_path() {
    let err1 = CheckError::FileNotFound {
        path: Path::new("/a").to_path_buf(),
    };
    let err2 = CheckError::FileNotFound {
        path: Path::new("/b").to_path_buf(),
    };
    assert_ne!(err1, err2);
}

#[test]
fn check_error_invalid_magic_display() {
    let err = CheckError::InvalidMagic {
        path: Path::new("/test").to_path_buf(),
        magic: [0xDE, 0xAD, 0xBE, 0xEF],
    };
    let msg = err.to_string();
    assert!(msg.contains("/test"));
    assert!(msg.contains("invalid") || msg.contains("magic"));
}

#[test]
fn check_error_not_regular_file() {
    let err = CheckError::NotRegularFile {
        path: Path::new("/dev/null").to_path_buf(),
    };
    let msg = err.to_string();
    assert!(msg.contains("/dev/null"));
}

#[test]
fn check_error_permission_denied() {
    let err = CheckError::PermissionDenied {
        path: Path::new("/root-only").to_path_buf(),
    };
    let msg = err.to_string();
    assert!(msg.contains("/root-only"));
    assert!(msg.contains("permission") || msg.contains("denied"));
}

#[test]
fn undo_result_serialization() {
    let success = UndoResult {
        success: true,
        message: "Undo successful".to_string(),
    };
    let fail = UndoResult {
        success: false,
        message: "Nothing to undo".to_string(),
    };

    let success_json = serde_json::to_string(&success).expect("serialize");
    let fail_json = serde_json::to_string(&fail).expect("serialize");

    assert!(success_json.contains("\"success\":true"));
    assert!(fail_json.contains("\"success\":false"));
    assert!(fail_json.contains("Nothing to undo"));
}

#[test]
fn redo_result_serialization() {
    let success = RedoResult {
        success: true,
        message: "Redo successful".to_string(),
    };
    let fail = RedoResult {
        success: false,
        message: "Nothing to redo".to_string(),
    };

    let success_json = serde_json::to_string(&success).expect("serialize");
    let fail_json = serde_json::to_string(&fail).expect("serialize");

    assert!(success_json.contains("\"success\":true"));
    assert!(fail_json.contains("\"success\":false"));
    assert!(fail_json.contains("Nothing to redo"));
}

#[test]
fn binary_check_result_is_human_readable() {
    let mut f = tempfile::Builder::new()
        .suffix(".elf")
        .tempfile()
        .expect("create temp file");
    f.write_all(b"\x7FELF\x02\x01\x01\x00")
        .expect("write ELF header");
    f.flush().expect("flush");

    let result = validate_binary_header(f.path()).expect("validate ELF");
    let display_name = result.display_name();
    assert!(!display_name.is_empty());
    assert!(display_name.contains("ELF") || display_name.contains("binary"));
}

#[test]
fn history_undo_when_empty_fails_gracefully() {
    use vo_types::CommandHistory;
    let mut history = CommandHistory::new();
    let result = undo_command(&mut history);

    assert!(!result.success);
    assert!(result.message.contains("Nothing to undo") || result.message.contains("nothing"));
}

#[test]
fn history_redo_when_empty_fails_gracefully() {
    use vo_types::CommandHistory;
    let mut history = CommandHistory::new();
    let result = redo_command(&mut history);

    assert!(!result.success);
    assert!(result.message.contains("Nothing to redo") || result.message.contains("nothing"));
}