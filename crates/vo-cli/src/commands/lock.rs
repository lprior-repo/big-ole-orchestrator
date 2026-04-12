use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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

fn file_hash(path: &Path) -> Result<String, std::io::Error> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
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
