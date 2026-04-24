//! End-to-end tests for Config hot-reload system.
//!
//! TDD Red Phase: Full workflow tests verifying complete hot-reload cycles.
//! These should FAIL until the full implementation is complete.

use std::fs;
use std::sync::Arc;
use std::time::Duration;

use tempfile::TempDir;
use vo_core::config_hot_reload::{ConfigValidator, Error, HotReloadConfig, WatcherConfig};

struct VersionValidator;
impl ConfigValidator<serde_json::Value> for VersionValidator {
    fn validate(&self, config: &serde_json::Value) -> Result<(), String> {
        if config.get("version").and_then(|v| v.as_u64()).is_some() {
            Ok(())
        } else {
            Err("missing version field".to_string())
        }
    }
}

// ============================================================
// E2E 1: Full hot-reload cycle with file watching
// ============================================================

#[test]
fn e2e_full_hot_reload_cycle_with_file_modification() {
    let temp_dir = TempDir::new().expect("temp dir creation");
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"version": 1, "name": "initial"}"#).expect("should succeed");

    let config = HotReloadConfig::new(
        serde_json::json!({"version": 1, "name": "initial"}),
        path.clone(),
        Arc::new(VersionValidator),
    )
    .expect("should succeed");

    assert_eq!(
        config.current(),
        serde_json::json!({"version": 1, "name": "initial"})
    );

    config
        .try_update(serde_json::json!({"version": 2, "name": "updated"}))
        .expect("should succeed");
    assert_eq!(
        config.current(),
        serde_json::json!({"version": 1, "name": "initial"})
    );

    let old = config.commit().expect("should succeed");
    assert_eq!(old, serde_json::json!({"version": 1, "name": "initial"}));
    assert_eq!(
        config.current(),
        serde_json::json!({"version": 2, "name": "updated"})
    );

    fs::write(&path, r#"{"version": 3, "name": "from-file"}"#).expect("should succeed");
    let old2 = config.reload_from_file().expect("should succeed");
    assert_eq!(old2, serde_json::json!({"version": 2, "name": "updated"}));
    assert_eq!(
        config.current(),
        serde_json::json!({"version": 3, "name": "from-file"})
    );

    let no_pending_result = config.commit();
    assert!(matches!(no_pending_result, Err(Error::SwapFailed)));
}

// ============================================================
// E2E 2: Rollback workflow end-to-end
// ============================================================

#[test]
fn e2e_rollback_preserves_current_through_failed_update_cycle() {
    let temp_dir = TempDir::new().expect("temp dir creation");
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"version": 1}"#).expect("should succeed");

    let config = HotReloadConfig::new(
        serde_json::json!({"version": 1}),
        path,
        Arc::new(VersionValidator),
    )
    .expect("should succeed");

    config
        .try_update(serde_json::json!({"version": 2}))
        .expect("should succeed");
    config.rollback();
    assert_eq!(config.current(), serde_json::json!({"version": 1}));

    config
        .try_update(serde_json::json!({"version": 3}))
        .expect("should succeed");
    let old = config.commit().expect("should succeed");
    assert_eq!(old, serde_json::json!({"version": 1}));
    assert_eq!(config.current(), serde_json::json!({"version": 3}));

    config
        .try_update(serde_json::json!({"version": 4}))
        .expect("should succeed");
    config.rollback();
    assert_eq!(config.current(), serde_json::json!({"version": 3}));

    let result = config.commit();
    assert!(matches!(result, Err(Error::SwapFailed)));
}

// ============================================================
// E2E 3: Concurrent update and commit scenarios
// ============================================================

#[test]
fn e2e_interleaved_try_update_commit_rollback_operations() {
    let temp_dir = TempDir::new().expect("temp dir creation");
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"version": 1}"#).expect("should succeed");

    let config = HotReloadConfig::new(
        serde_json::json!({"version": 1}),
        path,
        Arc::new(VersionValidator),
    )
    .expect("should succeed");

    config
        .try_update(serde_json::json!({"version": 2}))
        .expect("should succeed");
    config.rollback();

    config
        .try_update(serde_json::json!({"version": 3}))
        .expect("should succeed");
    config
        .try_update(serde_json::json!({"version": 4}))
        .expect("should succeed");
    let old = config.commit().expect("should succeed");
    assert_eq!(old, serde_json::json!({"version": 1}));
    assert_eq!(config.current(), serde_json::json!({"version": 4}));

    let no_pending = config.commit();
    assert!(matches!(no_pending, Err(Error::SwapFailed)));

    config
        .try_update(serde_json::json!({"version": 5}))
        .expect("should succeed");
    let old2 = config.commit().expect("should succeed");
    assert_eq!(old2, serde_json::json!({"version": 4}));
    assert_eq!(config.current(), serde_json::json!({"version": 5}));
}

#[test]
fn e2e_reload_from_file_interleaved_with_pending_state() {
    let temp_dir = TempDir::new().expect("temp dir creation");
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"version": 1}"#).expect("should succeed");

    let config = HotReloadConfig::new(
        serde_json::json!({"version": 1}),
        path.clone(),
        Arc::new(VersionValidator),
    )
    .expect("should succeed");

    config
        .try_update(serde_json::json!({"version": 99}))
        .expect("should succeed");

    fs::write(&path, r#"{"version": 2}"#).expect("should succeed");
    let old = config.reload_from_file().expect("should succeed");
    assert_eq!(old, serde_json::json!({"version": 1}));
    assert_eq!(config.current(), serde_json::json!({"version": 2}));

    let committed = config.commit().expect("should succeed");
    assert_eq!(committed, serde_json::json!({"version": 2}));
    assert_eq!(config.current(), serde_json::json!({"version": 99}));
}

#[test]
fn e2e_watcher_config_defaults_match_spec() {
    let config = WatcherConfig::default();

    assert!(config.recursive, "default recursive should be true");
    assert_eq!(
        config.debounce_duration,
        Some(Duration::from_millis(300)),
        "default debounce should be 300ms"
    );
    assert_eq!(
        config.patterns,
        vec!["*".to_string()],
        "default patterns should be [*]"
    );
}
