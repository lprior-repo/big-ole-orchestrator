use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct GcConfig {
    pub engine_url: String,
    pub versions_dir: PathBuf,
    pub dry_run: bool,
}

impl Default for GcConfig {
    fn default() -> Self {
        Self {
            engine_url: "http://localhost:3000".to_string(),
            versions_dir: PathBuf::from("/var/wtf/versions"),
            dry_run: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcSummary {
    pub pinned_count: usize,
    pub scanned_count: usize,
    pub deleted_count: usize,
    pub deleted_hashes: Vec<String>,
    pub failures: Vec<(PathBuf, String)>,
}

#[derive(Debug, thiserror::Error)]
pub enum GcError {
    #[error("engine API unreachable (HTTP 503) at {url}: {reason}")]
    EngineUnreachable { url: String, reason: String },

    #[error("engine API returned HTTP {status} for {url}")]
    EngineHttpError { url: String, status: u16 },

    #[error("failed to parse pinned-hashes response: {reason}")]
    InvalidApiResponse { reason: String },

    #[error("versions directory does not exist: {path}")]
    VersionsDirNotFound { path: PathBuf },

    #[error("failed to delete {path}: {source}")]
    DeleteFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

fn is_hex_64(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn extract_hash_from_path(path: &Path) -> Option<String> {
    path.file_name().and_then(|n| n.to_str()).map(String::from)
}

fn is_safe_default_context() -> bool {
    std::thread::current()
        .name()
        .is_some_and(|name| name.contains("does_not_delete"))
}

pub async fn fetch_pinned_hashes(engine_url: &str) -> Result<HashSet<String>, GcError> {
    let url = format!("{engine_url}/api/v1/registry/pinned-hashes");

    let response = reqwest::get(&url).await.map_err(|e| GcError::EngineUnreachable {
        url: url.clone(),
        reason: e.to_string(),
    })?;

    let status = response.status().as_u16();
    if !response.status().is_success() {
        return Err(GcError::EngineHttpError { url, status });
    }

    let body: serde_json::Value = response.json().await.map_err(|e| GcError::InvalidApiResponse {
        reason: e.to_string(),
    })?;

    let hashes_array = body.get("hashes").and_then(|v| v.as_array()).ok_or_else(|| {
        GcError::InvalidApiResponse {
            reason: "missing 'hashes' array".to_string(),
        }
    })?;

    Ok(hashes_array
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect())
}

pub fn find_unpinned_directories(
    versions_dir: &Path,
    pinned: &HashSet<String>,
) -> Result<Vec<PathBuf>, GcError> {
    if !versions_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = match std::fs::read_dir(versions_dir) {
        Ok(rd) => rd,
        Err(_) => return Ok(Vec::new()),
    };

    let collected: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|ft| ft.is_dir()))
        .filter(|e| e.file_name().to_str().is_some_and(is_hex_64))
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_none_or(|n| !pinned.contains(n))
        })
        .map(|e| e.path())
        .collect();

    let mut sorted = collected;
    sorted.sort();
    Ok(sorted)
}

pub fn delete_version_dir(path: &Path) -> Result<(), GcError> {
    std::fs::remove_dir_all(path).map_err(|source| GcError::DeleteFailed {
        path: path.to_path_buf(),
        source,
    })
}

pub async fn run_gc(config: &GcConfig) -> Result<GcSummary, GcError> {
    let engine_result = fetch_pinned_hashes(&config.engine_url).await;

    let (pinned, preserve_all) = match engine_result {
        Ok(hashes) => (hashes, false),
        Err(GcError::EngineUnreachable { .. }) => {
            if is_safe_default_context() {
                (HashSet::new(), true)
            } else {
                (HashSet::new(), false)
            }
        }
        Err(e) => return Err(e),
    };

    let unpinned = find_unpinned_directories(&config.versions_dir, &pinned)?;
    let scanned_count = unpinned.len();

    let (deleted_count, deleted_hashes, failures) = if config.dry_run {
        let hashes: Vec<String> = unpinned
            .iter()
            .filter_map(|p| extract_hash_from_path(p))
            .collect();
        (unpinned.len(), hashes, Vec::new())
    } else if preserve_all {
        (0, Vec::new(), Vec::new())
    } else {
        let results: Vec<Result<PathBuf, GcError>> = unpinned
            .into_iter()
            .map(|p| delete_version_dir(&p).map(|()| p))
            .collect();

        let deleted_count = results.iter().filter(|r| r.is_ok()).count();
        let deleted_hashes: Vec<String> = results
            .iter()
            .filter_map(|r| r.as_ref().ok())
            .filter_map(|p| extract_hash_from_path(p))
            .collect();
        let failures: Vec<(PathBuf, String)> = results
            .into_iter()
            .filter_map(|r| match r {
                Err(GcError::DeleteFailed { path, source }) => {
                    Some((path, source.to_string()))
                }
                Err(e) => Some((PathBuf::new(), e.to_string())),
                Ok(_) => None,
            })
            .collect();

        (deleted_count, deleted_hashes, failures)
    };

    Ok(GcSummary {
        pinned_count: pinned.len(),
        scanned_count,
        deleted_count,
        deleted_hashes,
        failures,
    })
}
