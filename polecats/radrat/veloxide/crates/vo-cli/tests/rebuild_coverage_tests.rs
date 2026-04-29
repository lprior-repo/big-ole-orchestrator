use std::path::PathBuf;
use vo_cli::commands::rebuild::{
    run_rebuild, RebuildConfig, RebuildError, RebuildReport, RebuildStatus,
};
use vo_cli::{interpret_cli_from, map_error_to_exit_code, CliError, Command};

fn make_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir vo");
    std::fs::create_dir_all(dir.path().join(".vo/workflows")).expect("mkdir wf");
    dir
}

#[test]
fn rebuild_rejects_uninit_project() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = RebuildConfig {
        project_dir: dir.path().to_path_buf(),
        projection_id: Some("proj-1".into()),
        list_projections: false,
        force: false,
        schema_version: None,
    };
    let result = run_rebuild(&config);
    assert!(matches!(result, Err(RebuildError::NotInitialized { .. })));
}

#[test]
fn rebuild_list_projections_returns_ok() {
    let dir = make_project();
    let config = RebuildConfig {
        project_dir: dir.path().to_path_buf(),
        projection_id: None,
        list_projections: true,
        force: false,
        schema_version: None,
    };
    let report = run_rebuild(&config).expect("rebuild list");
    assert!(matches!(report.status, RebuildStatus::Listed(_)));
}

#[test]
fn rebuild_requires_projection_id_when_not_listing() {
    let dir = make_project();
    let config = RebuildConfig {
        project_dir: dir.path().to_path_buf(),
        projection_id: None,
        list_projections: false,
        force: false,
        schema_version: None,
    };
    let result = run_rebuild(&config);
    assert!(result.is_err());
}

#[test]
fn rebuild_with_projection_id_returns_completed() {
    let dir = make_project();
    let config = RebuildConfig {
        project_dir: dir.path().to_path_buf(),
        projection_id: Some("my-projection".into()),
        list_projections: false,
        force: false,
        schema_version: None,
    };
    let report = run_rebuild(&config).expect("rebuild");
    assert_eq!(report.projection_id.as_deref(), Some("my-projection"));
    assert!(report.rebuild_id.is_some());
    assert_eq!(report.status, RebuildStatus::Completed);
}

#[test]
fn rebuild_force_flag_in_config() {
    let config = RebuildConfig {
        project_dir: PathBuf::from("/tmp"),
        projection_id: Some("p".into()),
        list_projections: false,
        force: true,
        schema_version: None,
    };
    assert!(config.force);
}

#[test]
fn rebuild_schema_version_in_config() {
    let config = RebuildConfig {
        project_dir: PathBuf::from("/tmp"),
        projection_id: None,
        list_projections: true,
        force: false,
        schema_version: Some(2),
    };
    assert_eq!(config.schema_version, Some(2));
}

#[test]
fn rebuild_status_format_listed() {
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
    assert!(output.contains("Registered projections"));
}

#[test]
fn rebuild_status_format_started() {
    let report = RebuildReport {
        projection_id: Some("p".into()),
        rebuild_id: Some("r-1".into()),
        status: RebuildStatus::Started { from_sequence: 42 },
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("started"));
    assert!(output.contains("42"));
}

#[test]
fn rebuild_status_format_failed() {
    let report = RebuildReport {
        projection_id: Some("p".into()),
        rebuild_id: Some("r-1".into()),
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
fn rebuild_status_format_noop() {
    let report = RebuildReport {
        projection_id: Some("p".into()),
        rebuild_id: Some("r-1".into()),
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
fn rebuild_error_not_initialized_display() {
    let err = RebuildError::NotInitialized {
        path: PathBuf::from("/no/project"),
    };
    assert!(err.to_string().contains("/no/project"));
    assert!(err.to_string().contains("not initialized"));
}

#[test]
fn rebuild_error_projection_not_found_display() {
    let err = RebuildError::ProjectionNotFound("missing-proj".into());
    assert!(err.to_string().contains("missing-proj"));
    assert!(err.to_string().contains("not found"));
}

#[test]
fn rebuild_error_rebuild_failed_display() {
    let err = RebuildError::RebuildFailed("timeout".into());
    assert!(err.to_string().contains("timeout"));
}

#[test]
fn rebuild_error_unsupported_schema_version_display() {
    let err = RebuildError::UnsupportedSchemaVersion(99);
    assert!(err.to_string().contains("99"));
    assert!(err.to_string().contains("not supported"));
}

#[test]
fn rebuild_error_rebuild_in_progress_display() {
    let err = RebuildError::RebuildInProgress("proj-x".into());
    assert!(err.to_string().contains("proj-x"));
    assert!(err.to_string().contains("in progress"));
}

#[test]
fn rebuild_error_idempotency_mismatch_display() {
    let err = RebuildError::IdempotencyMismatch {
        expected: "abc".into(),
        actual: "xyz".into(),
    };
    let msg = err.to_string();
    assert!(msg.contains("abc"));
    assert!(msg.contains("xyz"));
    assert!(msg.contains("mismatch"));
}

#[test]
fn rebuild_error_engine_display() {
    let err = RebuildError::Engine("connection refused".into());
    assert!(err.to_string().contains("connection refused"));
}

#[test]
fn rebuild_error_from_io_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "broken");
    let rebuild_err: RebuildError = io_err.into();
    assert!(matches!(rebuild_err, RebuildError::Io { .. }));
    assert!(rebuild_err.to_string().contains("broken"));
}

#[test]
fn cli_error_rebuild_exit_code_is_1() {
    assert_eq!(
        map_error_to_exit_code(&CliError::Rebuild(RebuildError::ProjectionNotFound(
            "x".into()
        ))),
        1
    );
}

#[test]
fn cli_error_rebuild_variant_display() {
    let err = CliError::Rebuild(RebuildError::Engine("fail".into()));
    assert!(err.to_string().contains("engine"));
}

#[test]
fn parse_rebuild_defaults() {
    let cli = interpret_cli_from(vec!["vo", "rebuild"]).expect("parse");
    match cli.command {
        Command::Rebuild {
            project_dir,
            projection_id,
            list_projections,
            force,
        } => {
            assert_eq!(project_dir, PathBuf::from("."));
            assert!(projection_id.is_none());
            assert!(!list_projections);
            assert!(!force);
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_with_projection_id() {
    let cli =
        interpret_cli_from(vec!["vo", "rebuild", "--projection-id", "my-proj"]).expect("parse");
    match cli.command {
        Command::Rebuild { projection_id, .. } => {
            assert_eq!(projection_id.as_deref(), Some("my-proj"));
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_with_list_flag() {
    let cli = interpret_cli_from(vec!["vo", "rebuild", "--list"]).expect("parse");
    match cli.command {
        Command::Rebuild {
            list_projections, ..
        } => {
            assert!(list_projections);
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_with_force_flag() {
    let cli = interpret_cli_from(vec!["vo", "rebuild", "--force"]).expect("parse");
    match cli.command {
        Command::Rebuild { force, .. } => {
            assert!(force);
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_with_project_dir() {
    let cli =
        interpret_cli_from(vec!["vo", "rebuild", "--project-dir", "/my/project"]).expect("parse");
    match cli.command {
        Command::Rebuild { project_dir, .. } => {
            assert_eq!(project_dir, PathBuf::from("/my/project"));
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_all_flags() {
    let cli = interpret_cli_from(vec![
        "vo",
        "rebuild",
        "--project-dir",
        "/proj",
        "--projection-id",
        "p1",
        "--list",
        "--force",
    ])
    .expect("parse");
    match cli.command {
        Command::Rebuild {
            project_dir,
            projection_id,
            list_projections,
            force,
        } => {
            assert_eq!(project_dir, PathBuf::from("/proj"));
            assert_eq!(projection_id.as_deref(), Some("p1"));
            assert!(list_projections);
            assert!(force);
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn rebuild_report_equality() {
    let r1 = RebuildReport {
        projection_id: Some("p".into()),
        rebuild_id: Some("r".into()),
        status: RebuildStatus::Completed,
        events_applied: 10,
        duration_ms: 5,
    };
    let r2 = r1.clone();
    assert_eq!(r1, r2);
}

#[test]
fn rebuild_status_equality() {
    assert_eq!(
        RebuildStatus::Started { from_sequence: 1 },
        RebuildStatus::Started { from_sequence: 1 }
    );
    assert_eq!(
        RebuildStatus::Failed { reason: "x".into() },
        RebuildStatus::Failed { reason: "x".into() }
    );
    assert_ne!(
        RebuildStatus::Started { from_sequence: 1 },
        RebuildStatus::Started { from_sequence: 2 }
    );
}
