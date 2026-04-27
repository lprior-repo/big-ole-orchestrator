use std::path::PathBuf;
use vo_cli::commands::rebuild::{RebuildConfig, RebuildError, RebuildReport, RebuildStatus};

#[test]
fn rebuild_not_initialized_returns_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = RebuildConfig {
        project_dir: dir.path().to_path_buf(),
        projection_id: Some("p1".into()),
        list_projections: false,
        force: false,
        schema_version: None,
    };
    let result = vo_cli::commands::rebuild::run_rebuild(&config);
    assert!(matches!(result, Err(RebuildError::NotInitialized { .. })));
}

#[test]
fn rebuild_list_returns_empty_projections() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir");
    let config = RebuildConfig {
        project_dir: dir.path().to_path_buf(),
        projection_id: None,
        list_projections: true,
        force: false,
        schema_version: None,
    };
    let result = vo_cli::commands::rebuild::run_rebuild(&config);
    assert!(result.is_ok());
    let report = result.expect("ok");
    assert!(matches!(report.status, RebuildStatus::Listed(ref v) if v.is_empty()));
}

#[test]
fn rebuild_without_projection_id_returns_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir");
    let config = RebuildConfig {
        project_dir: dir.path().to_path_buf(),
        projection_id: None,
        list_projections: false,
        force: false,
        schema_version: None,
    };
    let result = vo_cli::commands::rebuild::run_rebuild(&config);
    assert!(result.is_err());
}

#[test]
fn rebuild_with_projection_id_succeeds() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir");
    let config = RebuildConfig {
        project_dir: dir.path().to_path_buf(),
        projection_id: Some("order-summary".into()),
        list_projections: false,
        force: false,
        schema_version: None,
    };
    let result = vo_cli::commands::rebuild::run_rebuild(&config);
    assert!(result.is_ok());
    let report = result.expect("ok");
    assert_eq!(report.projection_id.as_deref(), Some("order-summary"));
    assert!(matches!(report.status, RebuildStatus::Completed));
}

#[test]
fn rebuild_report_format_progress_listed() {
    let report = RebuildReport {
        projection_id: None,
        rebuild_id: None,
        status: RebuildStatus::Listed(vec!["p1".into(), "p2".into()]),
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("Registered projections"));
    assert!(output.contains("p1"));
    assert!(output.contains("p2"));
}

#[test]
fn rebuild_report_format_progress_started() {
    let report = RebuildReport {
        projection_id: Some("p".into()),
        rebuild_id: Some("r".into()),
        status: RebuildStatus::Started { from_sequence: 42 },
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("started"));
    assert!(output.contains("42"));
}

#[test]
fn rebuild_report_format_progress_in_progress() {
    let report = RebuildReport {
        projection_id: Some("p".into()),
        rebuild_id: Some("r".into()),
        status: RebuildStatus::InProgress {
            progress_percent: 75,
            at_sequence: 3000,
        },
        events_applied: 3000,
        duration_ms: 200,
    };
    let output = report.format_progress();
    assert!(output.contains("75%"));
    assert!(output.contains("3000"));
}

#[test]
fn rebuild_report_format_progress_completed() {
    let report = RebuildReport {
        projection_id: Some("p".into()),
        rebuild_id: Some("r".into()),
        status: RebuildStatus::Completed,
        events_applied: 500,
        duration_ms: 120,
    };
    let output = report.format_progress();
    assert!(output.contains("completed"));
    assert!(output.contains("500 events"));
    assert!(output.contains("120ms"));
}

#[test]
fn rebuild_report_format_progress_failed() {
    let report = RebuildReport {
        projection_id: Some("p".into()),
        rebuild_id: Some("r".into()),
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
fn rebuild_report_format_progress_noop() {
    let report = RebuildReport {
        projection_id: Some("p".into()),
        rebuild_id: Some("r".into()),
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
