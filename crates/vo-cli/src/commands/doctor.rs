use sha2::{Sha256, Digest};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorConfig {
    pub project_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub healthy: bool,
    pub issues: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum DoctorError {
    #[error("project not initialized: {path}")]
    NotInitialized { path: PathBuf },
    #[error("I/O error at {path}: {reason}")]
    Io { path: PathBuf, reason: String, #[source] source: std::io::Error },
}

fn file_hash(path: &Path) -> Result<String, std::io::Error> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn parse_lockfile(content: &str) -> BTreeMap<String, String> {
    content.lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| {
            let (name, hash) = l.split_once(' ')?;
            Some((name.to_string(), hash.to_string()))
        })
        .collect()
}

/// Run diagnostics on a veloxide project.
///
/// Checks:
/// - `.vo/` directory exists
/// - `config.toml` exists and is parseable
/// - `.vo/workflows/` exists
/// - If `vo.lock` exists, verifies each binary hash matches
///
/// # Errors
/// Returns `DoctorError` if the project is not initialized.
pub fn run_doctor(config: &DoctorConfig) -> Result<DoctorReport, DoctorError> {
    let vo_dir = config.project_dir.join(".vo");
    if !vo_dir.is_dir() {
        return Err(DoctorError::NotInitialized { path: config.project_dir.clone() });
    }

    let mut issues = Vec::new();

    // Check config.toml
    let config_path = config.project_dir.join("config.toml");
    if !config_path.exists() {
        issues.push("config.toml missing".to_string());
    } else if std::fs::read_to_string(&config_path).map_err(|e| DoctorError::Io {
        path: config_path.clone(), reason: "reading config".into(), source: e,
    })?.is_empty() {
        issues.push("config.toml is empty".to_string());
    }

    // Check workflows dir
    let wf_dir = vo_dir.join("workflows");
    if !wf_dir.is_dir() {
        issues.push("workflows directory missing".to_string());
    }

    // Check lockfile integrity
    let lock_path = config.project_dir.join("vo.lock");
    if lock_path.exists() && wf_dir.is_dir() {
        let lock_content = std::fs::read_to_string(&lock_path).map_err(|e| DoctorError::Io {
            path: lock_path.clone(), reason: "reading lockfile".into(), source: e,
        })?;
        let lockmap = parse_lockfile(&lock_content);

        for (name, expected_hash) in &lockmap {
            let bin_path = wf_dir.join(name);
            if !bin_path.exists() {
                issues.push(format!("{name}: binary missing (referenced in lockfile)"));
                continue;
            }
            match file_hash(&bin_path) {
                Ok(actual) if actual != *expected_hash => {
                    issues.push(format!("{name}: hash mismatch (expected {expected_hash}, got {actual})"));
                }
                Err(e) => {
                    issues.push(format!("{name}: failed to read: {e}"));
                }
                _ => {}
            }
        }
    }

    let healthy = issues.is_empty();
    Ok(DoctorReport { healthy, issues })
}
