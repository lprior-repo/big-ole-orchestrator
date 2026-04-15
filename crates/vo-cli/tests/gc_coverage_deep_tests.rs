use std::path::PathBuf;
use vo_cli::commands::gc::{GcConfig, GcError, GcSummary};
use vo_cli::commands::gc::{delete_version_dir, find_unpinned_directories};
use std::collections::HashSet;

#[test]
fn gc_config_default_engine_url() {
    let config = GcConfig::default();
    assert_eq!(config.engine_url, "http://localhost:3000");
}

#[test]
fn gc_config_default_versions_dir() {
    let config = GcConfig::default();
    assert_eq!(config.versions_dir, PathBuf::from("/var/wtf/versions"));
}

#[test]
fn gc_config_default_dry_run_false() {
    let config = GcConfig::default();
    assert!(!config.dry_run);
}

#[test]
fn gc_error_engine_http_display_includes_url() {
    let err = GcError::EngineHttpError {
        url: "http://engine:3000/api".to_string(),
        status: 503,
    };
    let msg = err.to_string();
    assert!(msg.contains("503"));
    assert!(msg.contains("http://engine:3000/api"));
}

#[test]
fn gc_error_invalid_api_response_display() {
    let err = GcError::InvalidApiResponse {
        reason: "missing hashes".to_string(),
    };
    assert!(err.to_string().contains("missing hashes"));
}

#[test]
fn gc_error_versions_dir_display_includes_path() {
    let err = GcError::VersionsDirNotFound {
        path: PathBuf::from("/data/versions"),
    };
    assert!(err.to_string().contains("/data/versions"));
}

#[test]
fn gc_error_delete_failed_display_includes_path() {
    let err = GcError::DeleteFailed {
        path: PathBuf::from("/data/versions/abc123"),
        source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/data/versions/abc123"));
    assert!(msg.contains("denied"));
}

#[test]
fn gc_summary_fields() {
    let summary = GcSummary {
        pinned_count: 10,
        scanned_count: 5,
        deleted_count: 3,
        deleted_hashes: vec!["abc".to_string(), "def".to_string()],
        failures: vec![(PathBuf::from("/x"), "err".to_string())],
    };
    assert_eq!(summary.pinned_count, 10);
    assert_eq!(summary.scanned_count, 5);
    assert_eq!(summary.deleted_count, 3);
    assert_eq!(summary.deleted_hashes.len(), 2);
    assert_eq!(summary.failures.len(), 1);
}

#[test]
fn gc_summary_equality() {
    let a = GcSummary {
        pinned_count: 1,
        scanned_count: 2,
        deleted_count: 0,
        deleted_hashes: vec![],
        failures: vec![],
    };
    let b = GcSummary {
        pinned_count: 1,
        scanned_count: 2,
        deleted_count: 0,
        deleted_hashes: vec![],
        failures: vec![],
    };
    assert_eq!(a, b);
}

#[test]
fn gc_summary_inequality_different_counts() {
    let a = GcSummary {
        pinned_count: 1,
        scanned_count: 2,
        deleted_count: 0,
        deleted_hashes: vec![],
        failures: vec![],
    };
    let b = GcSummary {
        pinned_count: 1,
        scanned_count: 3,
        deleted_count: 0,
        deleted_hashes: vec![],
        failures: vec![],
    };
    assert_ne!(a, b);
}

#[tokio::test]
async fn delete_version_dir_nonexistent_returns_error() {
    let path = PathBuf::from("/tmp/vo-cli-gc-test-nonexistent-dir-xyz");
    let result = delete_version_dir(&path).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn find_unpinned_empty_dir_returns_empty() {
    let dir = tempfile::tempdir().unwrap();
    let pinned: HashSet<String> = HashSet::new();
    let result = find_unpinned_directories(dir.path(), &pinned).await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn find_unpinned_nonexistent_dir_returns_empty() {
    let path = PathBuf::from("/tmp/vo-cli-gc-no-such-dir-xyz");
    let pinned: HashSet<String> = HashSet::new();
    let result = find_unpinned_directories(&path, &pinned).await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn delete_version_dir_success() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("abc123");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("data.bin"), b"content").unwrap();
    delete_version_dir(&sub).await.unwrap();
    assert!(!sub.exists());
}

#[tokio::test]
async fn find_unpinned_skips_non_hex_directories() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("not-a-hash")).unwrap();
    std::fs::create_dir_all(dir.path().join("README.md")).unwrap();
    let pinned: HashSet<String> = HashSet::new();
    let result = find_unpinned_directories(dir.path(), &pinned).await.unwrap();
    assert!(result.is_empty());
}
