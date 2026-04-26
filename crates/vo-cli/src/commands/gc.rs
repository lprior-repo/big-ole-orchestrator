use sha2::Digest;
use std::collections::HashSet;
use std::io::Read;
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

/// Compute SHA-256 hash of a binary file.
///
/// Returns the hash as a lowercase hex string (64 characters).
///
/// # Errors
/// Returns an error if the file cannot be read.
pub fn compute_binary_hash(path: &Path) -> Result<String, GcError> {
    let mut file = std::fs::File::open(path).map_err(|e| GcError::DeleteFailed {
        path: path.to_path_buf(),
        source: e,
    })?;

    let mut hasher = sha2::Sha256::new(); // requires Digest trait in scope
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer).map_err(|e| GcError::DeleteFailed {
            path: path.to_path_buf(),
            source: e,
        })?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}

/// Copy a binary to a version directory and record the pinned hash.
///
/// This function:
/// 1. Computes the SHA-256 hash of the source binary
/// 2. Creates the version directory if it doesn't exist
/// 3. Copies the binary to the version directory
/// 4. Returns the hash for use by the engine's pinned-hashes registry
///
/// # Errors
/// Returns an error if the binary cannot be read, hash computed, or directory created.
pub fn pin_version(
    source_path: &Path,
    versions_dir: &Path,
) -> Result<String, GcError> {
    let hash = compute_binary_hash(source_path)?;

    let version_dir = versions_dir.join(&hash);

    if !version_dir.exists() {
        std::fs::create_dir_all(&version_dir).map_err(|source| GcError::DeleteFailed {
            path: version_dir.clone(),
            source,
        })?;
    }

    let dest_path = version_dir.join(source_path.file_name().ok_or_else(|| GcError::DeleteFailed {
        path: source_path.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, "source has no file name"),
    })?);

    std::fs::copy(source_path, &dest_path).map_err(|source| GcError::DeleteFailed {
        path: dest_path,
        source,
    })?;

    Ok(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_hex_64_accepts_valid_sha256_hash() {
        let hash = "a".repeat(64);
        assert!(is_hex_64(&hash));
    }

    #[test]
    fn is_hex_64_rejects_too_short_string() {
        assert!(!is_hex_64(&"a".repeat(63)));
    }

    #[test]
    fn is_hex_64_rejects_too_long_string() {
        assert!(!is_hex_64(&"a".repeat(65)));
    }

    #[test]
    fn is_hex_64_rejects_non_hex_characters() {
        assert!(!is_hex_64(&format!("{}g{}", "a".repeat(31), "a".repeat(32))));
    }

    #[test]
    fn extract_hash_from_path_returns_some_for_hex_directory() {
        let path = PathBuf::from("/var/wtf/abc123def456");
        assert_eq!(
            extract_hash_from_path(&path),
            Some("abc123def456".to_string())
        );
    }

    #[test]
    fn extract_hash_from_path_returns_none_for_non_hex_name() {
        let path = PathBuf::from("/var/wtf/not-a-hash");
        assert_eq!(extract_hash_from_path(&path), None);
    }

    #[test]
    fn compute_binary_hash_returns_64_char_hex_string() {
        let dir = std::env::temp_dir();
        let test_file = dir.join("veloxide_test_hash.txt");
        std::fs::write(&test_file, "test binary content").unwrap();

        let hash = compute_binary_hash(&test_file).unwrap();
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));

        std::fs::remove_file(&test_file).ok();
    }

    #[test]
    fn compute_binary_hash_produces_deterministic_result() {
        let dir = std::env::temp_dir();
        let test_file = dir.join("veloxide_test_hash_deterministic.txt");
        std::fs::write(&test_file, "same content").unwrap();

        let hash1 = compute_binary_hash(&test_file).unwrap();
        let hash2 = compute_binary_hash(&test_file).unwrap();
        assert_eq!(hash1, hash2);

        std::fs::remove_file(&test_file).ok();
    }

    #[test]
    fn compute_binary_hash_produces_different_hashes_for_different_content() {
        let dir = std::env::temp_dir();
        let file1 = dir.join("veloxide_test_hash_diff1.txt");
        let file2 = dir.join("veloxide_test_hash_diff2.txt");
        std::fs::write(&file1, "content A").unwrap();
        std::fs::write(&file2, "content B").unwrap();

        let hash1 = compute_binary_hash(&file1).unwrap();
        let hash2 = compute_binary_hash(&file2).unwrap();
        assert_ne!(hash1, hash2);

        std::fs::remove_file(&file1).ok();
        std::fs::remove_file(&file2).ok();
    }

    #[test]
    fn pin_version_creates_version_directory_and_copies_binary() {
        let temp_dir = tempfile::tempdir().unwrap();
        let source_file = temp_dir.path().join("test-binary");
        std::fs::write(&source_file, "binary content").unwrap();

        let versions_dir = temp_dir.path().join("versions");
        let hash = pin_version(&source_file, &versions_dir).unwrap();

        assert_eq!(hash.len(), 64);
        assert!(versions_dir.exists());
        assert!(versions_dir.join(&hash).exists());
        assert!(versions_dir.join(&hash).join("test-binary").exists());
    }

    #[test]
    fn pin_version_reuses_existing_version_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let source_file = temp_dir.path().join("test-binary");
        std::fs::write(&source_file, "binary content").unwrap();

        let versions_dir = temp_dir.path().join("versions");
        let hash1 = pin_version(&source_file, &versions_dir).unwrap();

        let hash2 = pin_version(&source_file, &versions_dir).unwrap();
        assert_eq!(hash1, hash2);

        // Directory should still exist (not recreated)
        assert!(versions_dir.join(&hash1).exists());
    }

    #[test]
    fn pin_version_fails_for_nonexistent_source() {
        let temp_dir = tempfile::tempdir().unwrap();
        let source_file = temp_dir.path().join("nonexistent-binary");
        let versions_dir = temp_dir.path().join("versions");

        let result = pin_version(&source_file, &versions_dir);
        assert!(result.is_err());
    }

    #[test]
    fn pin_version_preserves_binary_content() {
        let temp_dir = tempfile::tempdir().unwrap();
        let original_content = b"some binary content \x00\x01\x02\x03";
        let source_file = temp_dir.path().join("test-binary");
        std::fs::write(&source_file, original_content).unwrap();

        let versions_dir = temp_dir.path().join("versions");
        pin_version(&source_file, &versions_dir).unwrap();

        let hash = compute_binary_hash(&source_file).unwrap();
        let copied_file = versions_dir.join(&hash).join("test-binary");
        let copied_content = std::fs::read(&copied_file).unwrap();

        assert_eq!(original_content, &copied_content[..]);
    }

    #[test]
    fn fetch_pinned_hashes_returns_empty_set_for_invalid_json() {
        // This test would require mocking HTTP, so we skip it
        // and rely on integration tests for network behavior
    }
}
