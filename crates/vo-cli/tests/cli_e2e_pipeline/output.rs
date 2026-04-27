use std::path::PathBuf;

use vo_cli::commands::doctor::{
    format_report, format_report_json, CategoryReport, CheckCategory, CheckResult, DoctorReport,
    Severity,
};
use vo_cli::commands::rebuild::{RebuildReport, RebuildStatus};

#[test]
fn format_report_empty_categories() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp/empty"),
        categories: vec![],
    };
    let (stdout, stderr) = format_report(&report);
    assert!(stdout.contains("Doctor Report"));
    assert!(stdout.contains("All checks passed"));
    assert!(stderr.is_empty());
}

#[test]
fn format_report_with_errors_and_warnings() {
    let mut cat = CategoryReport::new(CheckCategory::Workspace);
    cat.push("check-a", Severity::Info, "all good".into());
    cat.push("check-b", Severity::Warn, "watch out".into());
    cat.push("check-c", Severity::Error, "broken".into());

    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp/test"),
        categories: vec![cat],
    };
    let (stdout, stderr) = format_report(&report);
    assert!(stdout.contains("all good"));
    assert!(stderr.contains("watch out"));
    assert!(stderr.contains("broken"));
    assert!(stderr.contains("error(s)"));
    assert!(stderr.contains("warning(s)"));
}

#[test]
fn format_report_json_structure() {
    let mut cat = CategoryReport::new(CheckCategory::LockState);
    cat.push("lock", Severity::Info, "valid".into());

    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp/json-test"),
        categories: vec![cat],
    };
    let json = format_report_json(&report);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["project_dir"].as_str(), Some("/tmp/json-test"));
    assert!(parsed["healthy"].as_bool().unwrap());
    assert_eq!(parsed["error_count"].as_u64().unwrap(), 0);
    assert_eq!(parsed["categories"].as_array().unwrap().len(), 1);

    let cat_val = &parsed["categories"][0];
    assert_eq!(cat_val["category"].as_str(), Some("lock-state"));
    assert!(cat_val["healthy"].as_bool().unwrap());
    assert_eq!(cat_val["checks"].as_array().unwrap().len(), 1);

    let check = &cat_val["checks"][0];
    assert_eq!(check["check"].as_str(), Some("lock"));
    assert_eq!(check["severity"].as_str(), Some("info"));
}

#[test]
fn format_report_json_severity_serialization() {
    for (sev, expected) in [
        (Severity::Info, "info"),
        (Severity::Warn, "warn"),
        (Severity::Error, "error"),
    ] {
        let mut cat = CategoryReport::new(CheckCategory::Workspace);
        cat.push("test", sev, "msg".into());
        let report = DoctorReport {
            project_dir: PathBuf::from("/tmp"),
            categories: vec![cat],
        };
        let json = format_report_json(&report);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed["categories"][0]["checks"][0]["severity"].as_str(),
            Some(expected)
        );
    }
}

#[test]
fn rebuild_format_progress_all_statuses() {
    let listed = RebuildReport {
        projection_id: None,
        rebuild_id: None,
        status: RebuildStatus::Listed(vec!["p1".into(), "p2".into()]),
        events_applied: 0,
        duration_ms: 0,
    };
    assert!(listed.format_progress().contains("p1"));
    assert!(listed.format_progress().contains("p2"));

    let started = RebuildReport {
        projection_id: Some("p1".into()),
        rebuild_id: Some("r1".into()),
        status: RebuildStatus::Started { from_sequence: 42 },
        events_applied: 0,
        duration_ms: 0,
    };
    assert!(started.format_progress().contains("42"));

    let in_progress = RebuildReport {
        projection_id: Some("p1".into()),
        rebuild_id: Some("r1".into()),
        status: RebuildStatus::InProgress {
            progress_percent: 75,
            at_sequence: 999,
        },
        events_applied: 750,
        duration_ms: 100,
    };
    assert!(in_progress.format_progress().contains("75%"));
    assert!(in_progress.format_progress().contains("999"));

    let failed = RebuildReport {
        projection_id: Some("p1".into()),
        rebuild_id: Some("r1".into()),
        status: RebuildStatus::Failed {
            reason: "OOM".into(),
        },
        events_applied: 0,
        duration_ms: 0,
    };
    assert!(failed.format_progress().contains("OOM"));

    let noop = RebuildReport {
        projection_id: Some("p1".into()),
        rebuild_id: Some("r1".into()),
        status: RebuildStatus::NoOp {
            reason: "already up to date".into(),
        },
        events_applied: 0,
        duration_ms: 0,
    };
    assert!(noop.format_progress().contains("already up to date"));
}
