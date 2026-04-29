use std::path::PathBuf;

pub use super::doctor_checks::{
    check_config_validation, check_lock_state, check_port_availability, check_storage_integrity,
    check_subprocess_liveness, check_workflow_definitions, check_workspace, format_report,
    format_report_json, CategoryReport, CheckCategory, CheckResult,
    DoctorReport as ComprehensiveDoctorReport, Severity,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorConfig {
    pub project_dir: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum DoctorError {
    #[error("project not initialized: {path}")]
    NotInitialized { path: PathBuf },
    #[error("I/O error at {path}: {reason}")]
    Io {
        path: PathBuf,
        reason: String,
        #[source]
        source: std::io::Error,
    },
}

/// Run comprehensive diagnostics on a veloxide project.
///
/// Checks seven categories:
/// - **Workspace integrity**: `.vo/` directory, workflows, storage, permissions
/// - **Lock state**: lockfile integrity, hash verification, orphan binaries
/// - **Subprocess liveness**: PID file validation, process alive checks
/// - **Storage integrity**: storage directory health, WAL files, partition check
/// - **Config validation**: TOML parsing, required sections, path validation
/// - **Workflow validation**: JSON workflow definition parsing and validation
/// - **Port availability**: check if serve ports are available
///
/// # Errors
/// Returns `DoctorError` if the project is not initialized (no `.vo/` dir).
pub fn run_doctor(config: &DoctorConfig) -> Result<ComprehensiveDoctorReport, DoctorError> {
    let vo_dir = config.project_dir.join(".vo");
    if !vo_dir.is_dir() {
        return Err(DoctorError::NotInitialized {
            path: config.project_dir.clone(),
        });
    }

    let categories = vec![
        check_workspace(&config.project_dir, &vo_dir),
        check_lock_state(&config.project_dir, &vo_dir),
        check_subprocess_liveness(&vo_dir),
        check_storage_integrity(&vo_dir, &config.project_dir),
        check_config_validation(&config.project_dir),
        check_workflow_definitions(&config.project_dir, &vo_dir),
        check_port_availability(&config.project_dir, &vo_dir),
    ];

    Ok(ComprehensiveDoctorReport {
        project_dir: config.project_dir.clone(),
        categories,
    })
}
