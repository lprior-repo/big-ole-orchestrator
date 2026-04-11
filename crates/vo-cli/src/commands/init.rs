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
    Io { path: PathBuf, reason: String, #[source] source: std::io::Error },
    #[error("symlink at {path} — refusing to init")]
    SymlinkTarget { path: PathBuf },
}

pub fn run_init(config: &InitConfig) -> Result<PathBuf, InitError> {
    let project_dir = &config.project_dir;

    // Validate: must be an existing directory
    let meta = std::fs::symlink_metadata(project_dir).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            InitError::DirNotFound { path: project_dir.clone() }
        } else if e.kind() == std::io::ErrorKind::PermissionDenied {
            InitError::PermissionDenied { path: project_dir.clone(), reason: e.to_string() }
        } else {
            InitError::Io { path: project_dir.clone(), reason: "reading metadata".into(), source: e }
        }
    })?;

    if meta.file_type().is_symlink() {
        return Err(InitError::SymlinkTarget { path: project_dir.clone() });
    }
    if !meta.is_dir() {
        return Err(InitError::NotDirectory { path: project_dir.clone() });
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
        path: vo_dir.clone(), reason: "creating .vo dir".into(), source: e,
    })?;
    std::fs::create_dir_all(vo_dir.join(WORKFLOWS_DIR_NAME)).map_err(|e| InitError::Io {
        path: vo_dir.join(WORKFLOWS_DIR_NAME), reason: "creating workflows dir".into(), source: e,
    })?;

    let toml = format!(
        "[engine]\nurl = \"{}\"\n\n[storage]\npath = \"{}\"\n",
        config.engine_url,
        config.storage_path.display()
    );
    std::fs::write(project_dir.join(CONFIG_FILE_NAME), &toml).map_err(|e| {
        InitError::Io { path: project_dir.join(CONFIG_FILE_NAME), reason: "writing config".into(), source: e }
    })?;

    Ok(vo_dir)
}
