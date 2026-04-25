#![allow(clippy::redundant_pattern_matching)]
use common::make_temp_dir;
use vo_cli::{CheckCategory, DoctorConfig, DoctorError};
use vo_cli::commands::rebuild::{RebuildConfig, RebuildError, RebuildStatus};

#[test]
fn doctor_fails_without_vo_dir() {
    let dir = make_temp_dir();
    let config = DoctorConfig {
        project_dir: dir.clone(),
    };
    let result = vo_cli::run_doctor(&config);
    assert!(matches!(result, Err(DoctorError::NotInitialized { .. })));
}

#[test]
fn doctor_succeeds_on_initialized_project() {
    let dir = make_temp_dir();
    common::setup_project(&dir);
    let config = DoctorConfig {
        project_dir: dir.clone(),
    };
    let report = vo_cli::run_doctor(&config).unwrap();
    assert_eq!(report.project_dir, dir);
    assert_eq!(report.categories.len(), 5);
}

#[test]
fn doctor_report_has_all_five_categories() {
    let dir = make_temp_dir();
    common::setup_project(&dir);
    let config = DoctorConfig {
        project_dir: dir.clone(),
    };
    let report = vo_cli::run_doctor(&config).unwrap();
    let cats: Vec<_> = report.categories.iter().map(|c| c.category).collect();
    assert!(cats.contains(&CheckCategory::Workspace));
    assert!(cats.contains(&CheckCategory::LockState));
    assert!(cats.contains(&CheckCategory::SubprocessLiveness));
    assert!(cats.contains(&CheckCategory::StorageIntegrity));
    assert!(cats.contains(&CheckCategory::ConfigValidation));
}

#[test]
fn doctor_healthy_project_is_healthy() {
    let dir = make_temp_dir();
    common::setup_project(&dir);
    let config = DoctorConfig {
        project_dir: dir.clone(),
    };
    let report = vo_cli::run_doctor(&config).unwrap();
    assert!(report.is_healthy());
}

#[test]
fn rebuild_fails_without_vo_dir() {
    let dir = make_temp_dir();
    let config = RebuildConfig {
        project_dir: dir.clone(),
        projection_id: Some("p1".into()),
        list_projections: false,
        force: false,
        schema_version: None,
    };
    let result = vo_cli::commands::rebuild::run_rebuild(&config);
    assert!(matches!(result, Err(RebuildError::NotInitialized { .. })));
}

#[test]
fn rebuild_list_returns_ok() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo")).unwrap();
    let config = RebuildConfig {
        project_dir: dir.clone(),
        projection_id: None,
        list_projections: true,
        force: false,
        schema_version: None,
    };
    let result = vo_cli::commands::rebuild::run_rebuild(&config);
    assert!(result.is_ok());
    let report = result.unwrap();
    assert!(matches!(report.status, RebuildStatus::Listed(_)));
}

#[test]
fn rebuild_without_projection_id_fails() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo")).unwrap();
    let config = RebuildConfig {
        project_dir: dir.clone(),
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
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo")).unwrap();
    let config = RebuildConfig {
        project_dir: dir.clone(),
        projection_id: Some("proj-1".into()),
        list_projections: false,
        force: true,
        schema_version: None,
    };
    let result = vo_cli::commands::rebuild::run_rebuild(&config);
    assert!(result.is_ok());
    let report = result.unwrap();
    assert_eq!(report.projection_id.as_deref(), Some("proj-1"));
    assert!(matches!(report.status, RebuildStatus::Completed));
}
