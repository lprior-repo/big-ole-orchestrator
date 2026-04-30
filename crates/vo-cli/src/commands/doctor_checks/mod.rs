use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::utils::file_hash;

pub use self::config::check_config_validation;
pub use self::display::{format_report, format_report_json};
pub use self::lock::check_lock_state;
pub use self::port::check_port_availability;
pub use self::storage::check_storage_integrity;
pub use self::subprocess::check_subprocess_liveness;
pub use self::workflow::check_workflow_definitions;
pub use self::workspace::check_workspace;

pub mod agents;
pub mod config;
pub mod display;
pub mod dolt;
pub mod git;
pub mod lock;
pub mod port;
pub mod storage;
pub mod subprocess;
pub mod workflow;
pub mod workspace;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckCategory {
    Workspace,
    LockState,
    SubprocessLiveness,
    StorageIntegrity,
    ConfigValidation,
    WorkflowValidation,
    PortAvailability,
}

impl std::fmt::Display for CheckCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Workspace => write!(f, "workspace"),
            Self::LockState => write!(f, "lock-state"),
            Self::SubprocessLiveness => write!(f, "subprocess-liveness"),
            Self::StorageIntegrity => write!(f, "storage-integrity"),
            Self::ConfigValidation => write!(f, "config-validation"),
            Self::WorkflowValidation => write!(f, "workflow-validation"),
            Self::PortAvailability => write!(f, "port-availability"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    pub check: &'static str,
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryReport {
    pub category: CheckCategory,
    pub checks: Vec<CheckResult>,
}

impl CategoryReport {
    pub fn new(category: CheckCategory) -> Self {
        Self {
            category,
            checks: Vec::new(),
        }
    }

    pub fn is_healthy(&self) -> bool {
        self.checks.iter().all(|c| c.severity <= Severity::Warn)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &CheckResult> {
        self.checks.iter().filter(|c| c.severity == Severity::Warn)
    }

    pub fn push(&mut self, check: &'static str, severity: Severity, message: String) {
        self.checks.push(CheckResult {
            check,
            severity,
            message,
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub project_dir: PathBuf,
    pub categories: Vec<CategoryReport>,
}

impl DoctorReport {
    pub fn is_healthy(&self) -> bool {
        self.categories.iter().all(|c| c.is_healthy())
    }

    pub fn errors(&self) -> impl Iterator<Item = &CheckResult> {
        self.categories
            .iter()
            .flat_map(|c| c.checks.iter())
            .filter(|c| c.severity == Severity::Error)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &CheckResult> {
        self.categories
            .iter()
            .flat_map(|c| c.checks.iter())
            .filter(|c| c.severity == Severity::Warn)
    }
}

fn parse_lockfile(content: &str) -> BTreeMap<String, String> {
    content
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| {
            let (name, hash) = l.split_once(' ')?;
            Some((name.to_string(), hash.to_string()))
        })
        .collect()
}

fn pid_alive(pid: u32) -> Option<bool> {
    unsafe {
        let ret = libc::kill(pid as libc::pid_t, 0);
        if ret == 0 {
            Some(true)
        } else {
            let errno = *libc::__errno_location();
            match errno {
                libc::ESRCH => Some(false),
                libc::EPERM => Some(true),
                _ => None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::fs;

    fn make_project_dir() -> PathBuf {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_path_buf();
        std::mem::forget(dir);
        p
    }

    fn init_project(project_dir: &Path) {
        let vo_dir = project_dir.join(".vo");
        fs::create_dir_all(vo_dir.join("workflows")).unwrap();
        fs::create_dir_all(vo_dir.join("storage")).unwrap();
        fs::write(
            project_dir.join("config.toml"),
            "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \".vo/storage\"\n",
        )
        .unwrap();
    }

    #[test]
    fn check_category_has_seven_variants() {
        let all = [
            CheckCategory::Workspace,
            CheckCategory::LockState,
            CheckCategory::SubprocessLiveness,
            CheckCategory::StorageIntegrity,
            CheckCategory::ConfigValidation,
            CheckCategory::WorkflowValidation,
            CheckCategory::PortAvailability,
        ];
        assert_eq!(all.len(), 7);
    }

    #[test]
    fn severity_ordering() {
        assert!(Severity::Error > Severity::Warn);
        assert!(Severity::Warn > Severity::Info);
    }

    #[test]
    fn category_report_empty_is_healthy() {
        assert!(CategoryReport::new(CheckCategory::Workspace).is_healthy());
    }

    #[test]
    fn category_report_with_error_is_unhealthy() {
        let mut r = CategoryReport::new(CheckCategory::Workspace);
        r.push("t", Severity::Error, "b".into());
        assert!(!r.is_healthy());
    }

    #[test]
    fn format_report_json_is_valid() {
        let report = DoctorReport {
            project_dir: PathBuf::from("/tmp/test"),
            categories: vec![CategoryReport::new(CheckCategory::Workspace)],
        };
        let json = format_report_json(&report);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["healthy"].as_bool().unwrap());
        assert_eq!(parsed["categories"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn format_report_produces_output() {
        let report = DoctorReport {
            project_dir: PathBuf::from("/tmp/test"),
            categories: vec![CategoryReport::new(CheckCategory::Workspace)],
        };
        let (stdout, _stderr) = format_report(&report);
        assert!(stdout.contains("Doctor Report"));
        assert!(stdout.contains("workspace"));
    }
}
