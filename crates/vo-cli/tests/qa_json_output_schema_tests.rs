use vo_cli::commands::workflow_history::{
    WorkflowHistoryEntry, WorkflowHistoryResponse,
};
use vo_cli::commands::doctor_checks::{
    format_report_json, CategoryReport, CheckCategory, CheckResult, DoctorReport, Severity,
};

#[test]
fn history_response_serializes_to_valid_json() {
    let response = WorkflowHistoryResponse {
        instance_id: "ns/01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
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
                step_id: Some("step-1".to_string()),
                error: None,
                output: Some(serde_json::json!({"result": "ok"})),
            },
        ],
        redacted_fields: None,
    };

    let json = serde_json::to_string_pretty(&response).expect("serialize to JSON");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse JSON");
    assert!(parsed.is_object());
}

#[test]
fn history_response_entries_is_an_array() {
    let response = WorkflowHistoryResponse {
        instance_id: "ns/test".to_string(),
        entries: vec![],
        redacted_fields: None,
    };

    let json = serde_json::to_string(&response).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse JSON");

    assert!(parsed.get("entries").expect("entries field").is_array());
}

#[test]
fn history_entry_has_required_sequence_field() {
    let entry = WorkflowHistoryEntry {
        sequence: 42,
        timestamp_ms: 1700000000000,
        event_type: "test".to_string(),
        step_id: None,
        error: None,
        output: None,
    };

    let json = serde_json::to_string(&entry).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse JSON");

    assert!(parsed.get("sequence").is_some(), "sequence field must be present");
    assert!(parsed["sequence"].is_number(), "sequence must be a number");
}

#[test]
fn history_entry_has_required_timestamp_ms_field() {
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

    assert!(parsed.get("timestamp_ms").is_some(), "timestamp_ms field must be present");
    assert!(parsed["timestamp_ms"].is_number(), "timestamp_ms must be a number");
}

#[test]
fn history_entry_has_required_event_type_field() {
    let entry = WorkflowHistoryEntry {
        sequence: 1,
        timestamp_ms: 1700000000000,
        event_type: "step_started".to_string(),
        step_id: None,
        error: None,
        output: None,
    };

    let json = serde_json::to_string(&entry).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse JSON");

    assert!(parsed.get("event_type").is_some(), "event_type field must be present");
    assert!(parsed["event_type"].is_string(), "event_type must be a string");
}

#[test]
fn history_entry_optional_fields_are_omitted_when_none() {
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

    assert!(parsed.get("step_id").is_none(), "step_id should be omitted when None");
    assert!(parsed.get("error").is_none(), "error should be omitted when None");
    assert!(parsed.get("output").is_none(), "output should be omitted when None");
}

#[test]
fn history_entry_optional_fields_present_when_some() {
    let entry = WorkflowHistoryEntry {
        sequence: 1,
        timestamp_ms: 1700000000000,
        event_type: "step_completed".to_string(),
        step_id: Some("fetch-data".to_string()),
        error: Some("connection refused".to_string()),
        output: Some(serde_json::json!({"count": 42})),
    };

    let json = serde_json::to_string(&entry).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse JSON");

    assert_eq!(parsed["step_id"], "fetch-data");
    assert_eq!(parsed["error"], "connection refused");
    assert_eq!(parsed["output"]["count"], 42);
}

#[test]
fn history_response_roundtrip_json() {
    let original = WorkflowHistoryResponse {
        instance_id: "ns/01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
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
                step_id: Some("init".to_string()),
                error: None,
                output: Some(serde_json::json!({"status": "initialized"})),
            },
        ],
        redacted_fields: Some(vec![["output.token".to_string()]]),
    };

    let json = serde_json::to_string(&original).expect("serialize");
    let reparsed: WorkflowHistoryResponse =
        serde_json::from_str(&json).expect("deserialize");

    assert_eq!(reparsed.instance_id, original.instance_id);
    assert_eq!(reparsed.entries.len(), original.entries.len());
    assert_eq!(reparsed.entries[0].sequence, 1);
    assert_eq!(reparsed.entries[1].step_id.as_deref(), Some("init"));
}

#[test]
fn doctor_report_serializes_to_valid_json() {
    let report = DoctorReport {
        project_dir: std::path::PathBuf::from("/tmp/test-project"),
        categories: vec![
            CategoryReport {
                category: CheckCategory::Workspace,
                checks: vec![
                    CheckResult {
                        check: "vo-dir",
                        severity: Severity::Info,
                        message: "vo directory exists".to_string(),
                    },
                    CheckResult {
                        check: "config",
                        severity: Severity::Warn,
                        message: "config file missing".to_string(),
                    },
                ],
            },
            CategoryReport {
                category: CheckCategory::LockState,
                checks: vec![
                    CheckResult {
                        check: "lock-integrity",
                        severity: Severity::Error,
                        message: "lock file corrupted".to_string(),
                    },
                ],
            },
        ],
    };

    let json = format_report_json(&report);
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse JSON");

    assert!(parsed.is_object());
    assert!(parsed.get("healthy").is_some());
    assert!(parsed.get("error_count").is_some());
    assert!(parsed.get("warn_count").is_some());
    assert!(parsed.get("categories").is_some());
}

#[test]
fn doctor_report_json_has_expected_structure() {
    let report = DoctorReport {
        project_dir: std::path::PathBuf::from("/test"),
        categories: vec![
            CategoryReport {
                category: CheckCategory::Storage,
                checks: vec![
                    CheckResult {
                        check: "wal-files",
                        severity: Severity::Info,
                        message: "WAL files healthy".to_string(),
                    },
                ],
            },
        ],
    };

    let json = format_report_json(&report);
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse JSON");

    let categories = parsed.get("categories").expect("categories").as_array().expect("array");
    assert_eq!(categories.len(), 1);

    let cat = &categories[0];
    assert!(cat.get("category").is_some());
    assert!(cat.get("healthy").is_some());
    assert!(cat.get("checks").is_some());

    let checks = cat.get("checks").unwrap().as_array().unwrap();
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0]["check"], "wal-files");
    assert_eq!(checks[0]["severity"], "info");
}

#[test]
fn doctor_report_json_severity_mapping() {
    let report = DoctorReport {
        project_dir: std::path::PathBuf::from("/test"),
        categories: vec![
            CategoryReport {
                category: CheckCategory::Workspace,
                checks: vec![
                    CheckResult {
                        check: "info-check",
                        severity: Severity::Info,
                        message: "info message".to_string(),
                    },
                    CheckResult {
                        check: "warn-check",
                        severity: Severity::Warn,
                        message: "warn message".to_string(),
                    },
                    CheckResult {
                        check: "error-check",
                        severity: Severity::Error,
                        message: "error message".to_string(),
                    },
                ],
            },
        ],
    };

    let json = format_report_json(&report);
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse JSON");

    let checks = parsed["categories"][0]["checks"]
        .as_array()
        .expect("checks array");

    assert_eq!(checks[0]["severity"], "info");
    assert_eq!(checks[1]["severity"], "warn");
    assert_eq!(checks[2]["severity"], "error");
}

#[test]
fn doctor_report_healthy_when_no_errors() {
    let report = DoctorReport {
        project_dir: std::path::PathBuf::from("/healthy"),
        categories: vec![
            CategoryReport {
                category: CheckCategory::Workspace,
                checks: vec![
                    CheckResult {
                        check: "test",
                        severity: Severity::Info,
                        message: "ok".to_string(),
                    },
                ],
            },
        ],
    };

    let json = format_report_json(&report);
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse JSON");

    assert_eq!(parsed["healthy"], true);
    assert_eq!(parsed["error_count"], 0);
}

#[test]
fn doctor_report_unhealthy_when_errors_present() {
    let report = DoctorReport {
        project_dir: std::path::PathBuf::from("/unhealthy"),
        categories: vec![
            CategoryReport {
                category: CheckCategory::LockState,
                checks: vec![
                    CheckResult {
                        check: "lock",
                        severity: Severity::Error,
                        message: "lock broken".to_string(),
                    },
                ],
            },
        ],
    };

    let json = format_report_json(&report);
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse JSON");

    assert_eq!(parsed["healthy"], false);
    assert_eq!(parsed["error_count"], 1);
}