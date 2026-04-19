use std::path::PathBuf;

pub const VO_DIR_NAME: &str = ".vo";
pub const WORKFLOWS_DIR_NAME: &str = "workflows";
pub const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitConfig {
    pub project_dir: PathBuf,
    pub engine_url: String,
    pub storage_path: PathBuf,
}

impl Default for InitConfig {
    fn default() -> Self {
        Self {
            project_dir: PathBuf::from("."),
            engine_url: "http://localhost:3000".to_string(),
            storage_path: PathBuf::from(".vo/storage"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("project directory does not exist: {path}")]
    DirNotFound { path: PathBuf },
    #[error("path is not a directory: {path}")]
    NotDirectory { path: PathBuf },
    #[error("already initialized: {path}")]
    AlreadyInitialized { path: PathBuf },
    #[error("permission denied: {path}: {reason}")]
    PermissionDenied { path: PathBuf, reason: String },
    #[error("I/O error at {path}: {reason}")]
    Io {
        path: PathBuf,
        reason: String,
        #[source]
        source: std::io::Error,
    },
    #[error("symlink at {path} — refusing to init")]
    SymlinkTarget { path: PathBuf },
}

pub fn run_init(config: &InitConfig) -> Result<PathBuf, InitError> {
    let project_dir = &config.project_dir;

    // Validate: must be an existing directory
    let meta = std::fs::symlink_metadata(project_dir).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            InitError::DirNotFound {
                path: project_dir.clone(),
            }
        } else if e.kind() == std::io::ErrorKind::PermissionDenied {
            InitError::PermissionDenied {
                path: project_dir.clone(),
                reason: e.to_string(),
            }
        } else {
            InitError::Io {
                path: project_dir.clone(),
                reason: "reading metadata".into(),
                source: e,
            }
        }
    })?;

    if meta.file_type().is_symlink() {
        return Err(InitError::SymlinkTarget {
            path: project_dir.clone(),
        });
    }
    if !meta.is_dir() {
        return Err(InitError::NotDirectory {
            path: project_dir.clone(),
        });
    }

    let vo_dir = project_dir.join(VO_DIR_NAME);

    // Idempotent: if config matches existing, return Ok
    if vo_dir.is_dir() {
        let config_path = project_dir.join(CONFIG_FILE_NAME);
        if config_path.exists() {
            let expected = format!(
                "[engine]\nurl = \"{}\"\n\n[storage]\npath = \"{}\"\n",
                config.engine_url,
                config.storage_path.display()
            );
            if std::fs::read_to_string(&config_path).unwrap_or_default() == expected {
                return Ok(vo_dir);
            }
            return Err(InitError::AlreadyInitialized { path: vo_dir });
        }
    }

    std::fs::create_dir_all(&vo_dir).map_err(|e| InitError::Io {
        path: vo_dir.clone(),
        reason: "creating .vo dir".into(),
        source: e,
    })?;
    std::fs::create_dir_all(vo_dir.join(WORKFLOWS_DIR_NAME)).map_err(|e| InitError::Io {
        path: vo_dir.join(WORKFLOWS_DIR_NAME),
        reason: "creating workflows dir".into(),
        source: e,
    })?;

    let toml = format!(
        "[engine]\nurl = \"{}\"\n\n[storage]\npath = \"{}\"\n",
        config.engine_url,
        config.storage_path.display()
    );
    std::fs::write(project_dir.join(CONFIG_FILE_NAME), &toml).map_err(|e| InitError::Io {
        path: project_dir.join(CONFIG_FILE_NAME),
        reason: "writing config".into(),
        source: e,
    })?;

    Ok(vo_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn run_init_creates_vo_directory_in_temp_dir() {
        let temp_dir = TempDir::new().unwrap();
        let config = InitConfig {
            project_dir: temp_dir.path().to_path_buf(),
            engine_url: "http://localhost:3000".to_string(),
            storage_path: PathBuf::from(".vo/storage"),
        };

        let result = run_init(&config);
        assert!(result.is_ok());
        let vo_dir = result.unwrap();
        assert!(vo_dir.exists());
        assert_eq!(vo_dir.file_name().unwrap(), ".vo");
    }

    #[test]
    fn run_init_idempotent_returns_ok_when_config_matches() {
        let temp_dir = TempDir::new().unwrap();
        let config = InitConfig {
            project_dir: temp_dir.path().to_path_buf(),
            engine_url: "http://localhost:3000".to_string(),
            storage_path: PathBuf::from(".vo/storage"),
        };

        let first = run_init(&config);
        assert!(first.is_ok());

        let second = run_init(&config);
        assert!(second.is_ok());
    }

    #[test]
    fn run_init_dir_not_found_error() {
        let config = InitConfig {
            project_dir: PathBuf::from("/nonexistent/path/12345"),
            engine_url: "http://localhost:3000".to_string(),
            storage_path: PathBuf::from(".vo/storage"),
        };

        let result = run_init(&config);
        assert!(matches!(result, Err(InitError::DirNotFound { .. })));
    }

    #[test]
    fn run_init_not_directory_error() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("file.txt");
        std::fs::write(&file_path, "not a directory").unwrap();

        let config = InitConfig {
            project_dir: file_path,
            engine_url: "http://localhost:3000".to_string(),
            storage_path: PathBuf::from(".vo/storage"),
        };

        let result = run_init(&config);
        assert!(matches!(result, Err(InitError::NotDirectory { .. })));
    }

    #[test]
    fn run_init_already_initialized_error_when_config_differs() {
        let temp_dir = TempDir::new().unwrap();
        let config = InitConfig {
            project_dir: temp_dir.path().to_path_buf(),
            engine_url: "http://localhost:3000".to_string(),
            storage_path: PathBuf::from(".vo/storage"),
        };

        let first = run_init(&config);
        assert!(first.is_ok());

        let config_different = InitConfig {
            project_dir: temp_dir.path().to_path_buf(),
            engine_url: "http://localhost:4000".to_string(),
            storage_path: PathBuf::from(".vo/storage"),
        };
        let second = run_init(&config_different);
        assert!(matches!(second, Err(InitError::AlreadyInitialized { .. })));
    }

    #[test]
    fn run_init_symlink_target_error() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("target");
        std::fs::create_dir(&target).unwrap();
        let symlink = temp_dir.path().join("link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &symlink).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&target, &symlink).unwrap();

        let config = InitConfig {
            project_dir: symlink,
            engine_url: "http://localhost:3000".to_string(),
            storage_path: PathBuf::from(".vo/storage"),
        };

        let result = run_init(&config);
        assert!(matches!(result, Err(InitError::SymlinkTarget { .. })));
    }

    #[test]
    fn init_config_default_values() {
        let config = InitConfig::default();
        assert_eq!(config.engine_url, "http://localhost:3000");
        assert_eq!(config.storage_path, PathBuf::from(".vo/storage"));
        assert_eq!(config.project_dir, PathBuf::from("."));
    }

    #[test]
    fn init_error_display_format() {
        let err = InitError::DirNotFound {
            path: PathBuf::from("/test/path"),
        };
        assert!(err.to_string().contains("/test/path"));

        let err = InitError::PermissionDenied {
            path: PathBuf::from("/test/path"),
            reason: "access denied".to_string(),
        };
        assert!(err.to_string().contains("permission denied"));
    }
}
