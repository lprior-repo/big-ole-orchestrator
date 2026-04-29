#![allow(clippy::redundant_pattern_matching)]
use std::path::PathBuf;
use vo_cli::commands::rebuild::{RebuildReport, RebuildStatus};
use vo_cli::{
    format_report, format_report_json, BinaryFormat, CategoryReport, CheckCategory, CheckResult,
    DoctorReport, Severity,
};

#[test]
fn format_report_healthy_project_output() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/healthy"),
        categories: vec![CategoryReport {
            category: CheckCategory::Workspace,
            checks: vec![CheckResult {
                check: "vo-dir",
                severity: Severity::Info,
                message: ".vo/ exists".into(),
            }],
        }],
    };
    let (stdout, stderr) = format_report(&report);
    assert!(stdout.contains("Doctor Report"));
    assert!(stdout.contains("/healthy"));
    assert!(stdout.contains("workspace"));
    assert!(stdout.contains("All checks passed"));
    assert!(stderr.is_empty());
}

#[test]
fn format_report_errors_go_to_stderr() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/broken"),
        categories: vec![CategoryReport {
            category: CheckCategory::ConfigValidation,
            checks: vec![CheckResult {
                check: "config-exists",
                severity: Severity::Error,
                message: "config.toml missing".into(),
            }],
        }],
    };
    let (stdout, stderr) = format_report(&report);
    assert!(stdout.contains("Doctor Report"));
    assert!(stderr.contains("config-exists"));
    assert!(stderr.contains("error(s)"));
}

#[test]
fn format_report_warnings_go_to_stderr() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/warned"),
        categories: vec![CategoryReport {
            category: CheckCategory::StorageIntegrity,
            checks: vec![CheckResult {
                check: "storage-dir",
                severity: Severity::Warn,
                message: "storage directory missing".into(),
            }],
        }],
    };
    let (stdout, stderr) = format_report(&report);
    assert!(stdout.contains("Doctor Report"));
    assert!(stderr.contains("storage-dir"));
    assert!(stderr.contains("warning(s)"));
}

#[test]
fn format_report_mixed_severity() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/mixed"),
        categories: vec![CategoryReport {
            category: CheckCategory::Workspace,
            checks: vec![
                CheckResult {
                    check: "ok1",
                    severity: Severity::Info,
                    message: "all good".into(),
                },
                CheckResult {
                    check: "warn1",
                    severity: Severity::Warn,
                    message: "watch out".into(),
                },
                CheckResult {
                    check: "err1",
                    severity: Severity::Error,
                    message: "broken".into(),
                },
            ],
        }],
    };
    let (stdout, stderr) = format_report(&report);
    assert!(stdout.contains("ok1"));
    assert!(stderr.contains("warn1"));
    assert!(stderr.contains("err1"));
    assert!(stderr.contains("1 error(s)"));
    assert!(stderr.contains("1 warning(s)"));
}

#[test]
fn format_report_empty_categories() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/empty"),
        categories: vec![CategoryReport {
            category: CheckCategory::SubprocessLiveness,
            checks: vec![],
        }],
    };
    let (stdout, stderr) = format_report(&report);
    assert!(stdout.contains("no checks"));
    assert!(stdout.contains("All checks passed"));
    assert!(stderr.is_empty());
}

#[test]
fn format_report_json_structure() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/json-test"),
        categories: vec![
            CategoryReport {
                category: CheckCategory::Workspace,
                checks: vec![CheckResult {
                    check: "test-check",
                    severity: Severity::Info,
                    message: "all good".into(),
                }],
            },
            CategoryReport {
                category: CheckCategory::LockState,
                checks: vec![CheckResult {
                    check: "lock-check",
                    severity: Severity::Error,
                    message: "hash mismatch".into(),
                }],
            },
        ],
    };
    let json_str = format_report_json(&report);
    let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(json["project_dir"].as_str(), Some("/json-test"));
    assert_eq!(json["healthy"].as_bool(), Some(false));
    assert_eq!(json["error_count"].as_u64(), Some(1));
    assert_eq!(json["warn_count"].as_u64(), Some(0));
    let cats = json["categories"].as_array().unwrap();
    assert_eq!(cats.len(), 2);
    assert_eq!(cats[0]["category"].as_str(), Some("workspace"));
    assert_eq!(cats[0]["healthy"].as_bool(), Some(true));
    assert_eq!(cats[1]["category"].as_str(), Some("lock-state"));
    assert_eq!(cats[1]["healthy"].as_bool(), Some(false));
}

#[test]
fn format_report_json_severity_serialization() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/sev-test"),
        categories: vec![CategoryReport {
            category: CheckCategory::ConfigValidation,
            checks: vec![
                CheckResult {
                    check: "i",
                    severity: Severity::Info,
                    message: "info".into(),
                },
                CheckResult {
                    check: "w",
                    severity: Severity::Warn,
                    message: "warn".into(),
                },
                CheckResult {
                    check: "e",
                    severity: Severity::Error,
                    message: "error".into(),
                },
            ],
        }],
    };
    let json_str = format_report_json(&report);
    let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    let checks = json["categories"][0]["checks"].as_array().unwrap();
    assert_eq!(checks[0]["severity"].as_str(), Some("info"));
    assert_eq!(checks[1]["severity"].as_str(), Some("warn"));
    assert_eq!(checks[2]["severity"].as_str(), Some("error"));
}

#[test]
fn rebuild_format_progress_started() {
    let report = RebuildReport {
        projection_id: Some("p1".into()),
        rebuild_id: Some("r1".into()),
        status: RebuildStatus::Started { from_sequence: 100 },
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("started"));
    assert!(output.contains("100"));
}

#[test]
fn rebuild_format_progress_failed() {
    let report = RebuildReport {
        projection_id: Some("p1".into()),
        rebuild_id: Some("r1".into()),
        status: RebuildStatus::Failed {
            reason: "disk full".into(),
        },
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("failed"));
    assert!(output.contains("disk full"));
}

#[test]
fn rebuild_format_progress_noop() {
    let report = RebuildReport {
        projection_id: Some("p1".into()),
        rebuild_id: Some("r1".into()),
        status: RebuildStatus::NoOp {
            reason: "already up to date".into(),
        },
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("skipped"));
    assert!(output.contains("already up to date"));
}

#[test]
fn rebuild_format_progress_listed_empty() {
    let report = RebuildReport {
        projection_id: None,
        rebuild_id: None,
        status: RebuildStatus::Listed(vec![]),
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("Registered projections"));
}

#[test]
fn rebuild_format_progress_listed_with_items() {
    let report = RebuildReport {
        projection_id: None,
        rebuild_id: None,
        status: RebuildStatus::Listed(vec!["proj-a".into(), "proj-b".into()]),
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("proj-a"));
    assert!(output.contains("proj-b"));
}
