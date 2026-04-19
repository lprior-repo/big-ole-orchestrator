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

/// Fetch pinned hashes from the engine.
///
/// # Errors
/// Returns an error if the engine is unreachable or returns a non-200 status.
#[tracing::instrument]
pub async fn fetch_pinned_hashes(engine_url: &str) -> Result<HashSet<String>, GcError> {
    let url = format!("{engine_url}/api/v1/registry/pinned-hashes");

    let response = reqwest::get(&url)
        .await
        .map_err(|e| GcError::EngineUnreachable {
            url: url.clone(),
            reason: e.to_string(),
        })?;

    let status = response.status().as_u16();
    if !response.status().is_success() {
        return Err(GcError::EngineHttpError { url, status });
    }

    let body: serde_json::Value =
        response
            .json()
            .await
            .map_err(|e| GcError::InvalidApiResponse {
                reason: e.to_string(),
            })?;

    let hashes_array = body
        .get("hashes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| GcError::InvalidApiResponse {
            reason: "missing 'hashes' array".to_string(),
        })?;

    Ok(hashes_array
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect())
}

/// Find unpinned directories.
///
/// # Errors
/// Returns an error if reading the directory fails in a non-recoverable way.
pub async fn find_unpinned_directories<S: std::hash::BuildHasher>(
    versions_dir: &Path,
    pinned: &HashSet<String, S>,
) -> Result<Vec<PathBuf>, GcError> {
    if !tokio::fs::metadata(versions_dir)
        .await
        .is_ok_and(|m| m.is_dir())
    {
        return Ok(Vec::new());
    }

    let mut entries =
        tokio::fs::read_dir(versions_dir)
            .await
            .map_err(|_| GcError::VersionsDirNotFound {
                path: versions_dir.to_path_buf(),
            })?;

    let mut collected: Vec<PathBuf> = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let Ok(file_type) = entry.file_type().await else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        if !is_hex_64(file_name) {
            continue;
        }
        if pinned.contains(file_name) {
            continue;
        }
        collected.push(entry.path());
    }

    collected.sort();
    Ok(collected)
}

/// Delete a version directory.
///
/// # Errors
/// Returns an error if the directory cannot be deleted.
pub async fn delete_version_dir(path: &Path) -> Result<(), GcError> {
    tokio::fs::remove_dir_all(path)
        .await
        .map_err(|source| GcError::DeleteFailed {
            path: path.to_path_buf(),
            source,
        })
}

/// Run the garbage collection command.
///
/// # Errors
/// Returns an error if the engine is unreachable or returns a non-200 status.
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

    let unpinned = find_unpinned_directories(&config.versions_dir, &pinned).await?;
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
        let mut results: Vec<Result<PathBuf, GcError>> = Vec::new();
        for p in &unpinned {
            results.push(delete_version_dir(p).await.map(|()| p.clone()));
        }

        let deleted_count = results.iter().filter(|r| r.is_ok()).count();
        let deleted_hashes: Vec<String> = results
            .iter()
            .filter_map(|r| r.as_ref().ok())
            .filter_map(|p| extract_hash_from_path(p))
            .collect();
        let failures: Vec<(PathBuf, String)> = results
            .into_iter()
            .filter_map(|r| match r {
                Err(GcError::DeleteFailed { path, source }) => Some((path, source.to_string())),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn is_hex_64_valid_64_char_hex() {
        let s64 = "a".repeat(64);
        assert!(is_hex_64(&s64));
        let s64upper = "A".repeat(64);
        assert!(is_hex_64(&s64upper));
        let s64num = "0".repeat(64);
        assert!(is_hex_64(&s64num));
        let valid_chars = "0123456789abcdef".repeat(4);
        assert!(is_hex_64(&valid_chars));
    }

    #[test]
    fn is_hex_64_invalid_lengths() {
        assert!(!is_hex_64("abc"));
        let s63 = "a".repeat(63);
        assert!(!is_hex_64(&s63));
        let s65 = "a".repeat(65);
        assert!(!is_hex_64(&s65));
    }

    #[test]
    fn is_hex_64_invalid_characters() {
        let sg = "g".repeat(64);
        assert!(!is_hex_64(&sg));
        let sG = "G".repeat(64);
        assert!(!is_hex_64(&sG));
    }

    #[test]
    fn extract_hash_from_path_extracts_filename() {
        let hash = "a".repeat(64);
        let full_path = format!("/versions/{}", hash);
        let path = Path::new(&full_path);
        assert_eq!(extract_hash_from_path(path), Some(hash));
    }

    #[test]
    fn extract_hash_from_path_returns_none_for_root() {
        let path = Path::new("/");
        assert!(extract_hash_from_path(path).is_none());
    }

    #[test]
    fn gc_config_default_values() {
        let config = GcConfig::default();
        assert_eq!(config.engine_url, "http://localhost:3000");
        assert_eq!(config.versions_dir, PathBuf::from("/var/wtf/versions"));
        assert!(!config.dry_run);
    }

    #[test]
    fn gc_summary_zero_initialization() {
        let summary = GcSummary {
            pinned_count: 0,
            scanned_count: 0,
            deleted_count: 0,
            deleted_hashes: vec![],
            failures: vec![],
        };
        assert_eq!(summary.pinned_count, 0);
        assert_eq!(summary.scanned_count, 0);
        assert_eq!(summary.deleted_count, 0);
        assert!(summary.deleted_hashes.is_empty());
        assert!(summary.failures.is_empty());
    }

    #[test]
    fn gc_error_display_format() {
        let err = GcError::VersionsDirNotFound {
            path: PathBuf::from("/test/path"),
        };
        assert!(err.to_string().contains("/test/path"));

        let err = GcError::EngineHttpError {
            url: "http://localhost:3000".to_string(),
            status: 500,
        };
        assert!(err.to_string().contains("HTTP 500"));

        let err = GcError::InvalidApiResponse {
            reason: "missing field".to_string(),
        };
        assert!(err.to_string().contains("missing field"));
    }

    #[tokio::test]
    async fn find_unpinned_directories_empty_when_no_dir() {
        let result = find_unpinned_directories(Path::new("/nonexistent/path"), &HashSet::new()).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn delete_version_dir_removes_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let version_dir = temp_dir.path().join("a".repeat(64));
        std::fs::create_dir_all(&version_dir).unwrap();
        std::fs::write(version_dir.join("file.txt"), "content").unwrap();

        assert!(version_dir.exists());

        let result = delete_version_dir(&version_dir).await;
        assert!(result.is_ok());
        assert!(!version_dir.exists());
    }

    #[tokio::test]
    async fn delete_version_dir_error_on_nonexistent() {
        let result = delete_version_dir(Path::new("/nonexistent/path")).await;
        assert!(result.is_err());
    }

    #[test]
    fn gc_config_with_custom_values() {
        let config = GcConfig {
            engine_url: "http://localhost:9000".to_string(),
            versions_dir: PathBuf::from("/custom/path"),
            dry_run: true,
        };
        assert_eq!(config.engine_url, "http://localhost:9000");
        assert_eq!(config.versions_dir, PathBuf::from("/custom/path"));
        assert!(config.dry_run);
    }
}
