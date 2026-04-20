use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::utils::file_hash;

pub const LOCK_FILE_NAME: &str = "vo.lock";
pub const WORKFLOWS_DIR_NAME: &str = "workflows";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockConfig {
    pub project_dir: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum LockError {
    #[error("project not initialized: no .vo directory in {path}")]
    NotInitialized { path: PathBuf },
    #[error("workflows directory not found: {path}")]
    NoWorkflowsDir { path: PathBuf },
    #[error("I/O error at {path}: {reason}")]
    Io {
        path: PathBuf,
        reason: String,
        #[source]
        source: std::io::Error,
    },
    #[error("lockfile write failed: {reason}")]
    LockWrite { reason: String },
    #[error("no workflow binaries found in {path}")]
    Empty { path: PathBuf },
}

pub fn run_lock(config: &LockConfig) -> Result<BTreeMap<String, String>, LockError> {
    let vo_dir = config.project_dir.join(".vo");
    if !vo_dir.is_dir() {
        return Err(LockError::NotInitialized {
            path: config.project_dir.clone(),
        });
    }

    let wf_dir = vo_dir.join(WORKFLOWS_DIR_NAME);
    if !wf_dir.is_dir() {
        return Err(LockError::NoWorkflowsDir { path: wf_dir });
    }

    let mut entries: Vec<_> = std::fs::read_dir(&wf_dir)
        .map_err(|e| LockError::Io {
            path: wf_dir.clone(),
            reason: "readdir".into(),
            source: e,
        })?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    if entries.is_empty() {
        return Err(LockError::Empty { path: wf_dir });
    }

    let mut lockmap = BTreeMap::new();
    for entry in &entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let hash = file_hash(&entry.path()).map_err(|e| LockError::Io {
            path: entry.path(),
            reason: "hashing".into(),
            source: e,
        })?;
        lockmap.insert(name, hash);
    }

    let lock_path = config.project_dir.join(LOCK_FILE_NAME);
    let content: String = lockmap
        .iter()
        .map(|(name, hash)| format!("{name} {hash}"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    std::fs::write(&lock_path, &content).map_err(|e| LockError::LockWrite {
        reason: format!("write {}: {e}", lock_path.display()),
    })?;

    Ok(lockmap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_init_project(temp_dir: &TempDir) -> PathBuf {
        let config = crate::commands::init::InitConfig {
            project_dir: temp_dir.path().to_path_buf(),
            engine_url: "http://localhost:3000".to_string(),
            storage_path: PathBuf::from(".vo/storage"),
        };
        crate::commands::init::run_init(&config).unwrap();
        let wf_dir = temp_dir.path().join(".vo").join("workflows");
        std::fs::create_dir_all(&wf_dir).unwrap();
        temp_dir.path().to_path_buf()
    }

    #[test]
    fn run_lock_creates_lockfile() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = create_init_project(&temp_dir);

        let wf1 = project_dir.join(".vo/workflows/wf1");
        std::fs::write(&wf1, "content1").unwrap();

        let config = LockConfig {
            project_dir: project_dir.clone(),
        };

        let result = run_lock(&config);
        assert!(result.is_ok());

        let lock_path = project_dir.join("vo.lock");
        assert!(lock_path.exists());

        let content = std::fs::read_to_string(&lock_path).unwrap();
        assert!(content.contains("wf1"));
    }

    #[test]
    fn run_lock_not_initialized_error() {
        let temp_dir = TempDir::new().unwrap();
        let config = LockConfig {
            project_dir: temp_dir.path().to_path_buf(),
        };

        let result = run_lock(&config);
        assert!(matches!(result, Err(LockError::NotInitialized { .. })));
    }

    #[test]
    fn run_lock_empty_workflows_error() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = create_init_project(&temp_dir);

        let config = LockConfig {
            project_dir: project_dir.clone(),
        };

        let result = run_lock(&config);
        assert!(matches!(result, Err(LockError::Empty { .. })));
    }

    #[test]
    fn run_lock_multiple_workflows_sorted() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = create_init_project(&temp_dir);

        let wf_a = project_dir.join(".vo/workflows/a");
        let wf_z = project_dir.join(".vo/workflows/z");
        let wf_m = project_dir.join(".vo/workflows/m");
        std::fs::write(&wf_a, "a").unwrap();
        std::fs::write(&wf_z, "z").unwrap();
        std::fs::write(&wf_m, "m").unwrap();

        let config = LockConfig {
            project_dir: project_dir.clone(),
        };

        let result = run_lock(&config).unwrap();
        let names: Vec<&str> = result.keys().map(|s| s.as_str()).collect();
        assert_eq!(names, vec!["a", "m", "z"]);
    }

    #[test]
    fn lock_error_display_format() {
        let err = LockError::NotInitialized {
            path: PathBuf::from("/test"),
        };
        assert!(err.to_string().contains("not initialized"));

        let err = LockError::Empty {
            path: PathBuf::from("/test"),
        };
        assert!(err.to_string().contains("no workflow binaries"));
    }
}
