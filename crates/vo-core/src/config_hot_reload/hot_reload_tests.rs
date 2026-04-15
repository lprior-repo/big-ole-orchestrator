#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::sync::Arc;

use tempfile::TempDir;

use super::*;

pub struct AlwaysValid;
impl<T: Clone + Send + Sync> ConfigValidator<T> for AlwaysValid {
    fn validate(&self, _config: &T) -> Result<(), String> {
        Ok(())
    }
}

pub struct AlwaysInvalid;
impl<T: Clone + Send + Sync> ConfigValidator<T> for AlwaysInvalid {
    fn validate(&self, _config: &T) -> Result<(), String> {
        Err("always invalid".to_string())
    }
}

#[test]
fn hot_reload_config_new_returns_error_when_file_missing() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("missing.json");
    let result = HotReloadConfig::<serde_json::Value>::new(
        serde_json::json!({}),
        path,
        Arc::new(AlwaysValid),
    );
    assert!(matches!(result, Err(Error::ConfigFileNotFound(_))));
}

#[test]
fn hot_reload_config_new_succeeds_when_file_exists() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let result = HotReloadConfig::<serde_json::Value>::new(
        serde_json::json!({"key": "value"}),
        path,
        Arc::new(AlwaysValid),
    );
    assert!(result.is_ok());
}

#[test]
fn hot_reload_config_current_returns_initial_value() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"key": "value"}),
        path,
        Arc::new(AlwaysValid),
    )
    .unwrap();

    assert_eq!(config.current(), serde_json::json!({"key": "value"}));
}

#[test]
fn hot_reload_config_try_update_succeeds_with_valid_config() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"key": "value"}),
        path,
        Arc::new(AlwaysValid),
    )
    .unwrap();

    let result = config.try_update(serde_json::json!({"new": "config"}));
    assert!(result.is_ok());
}

#[test]
fn hot_reload_config_try_update_fails_with_invalid_config() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"key": "value"}),
        path,
        Arc::new(AlwaysInvalid),
    )
    .unwrap();

    let result = config.try_update(serde_json::json!({"new": "config"}));
    assert!(matches!(result, Err(Error::ValidationFailed(_))));
}

#[test]
fn hot_reload_config_commit_applies_pending_config() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"key": "value"}),
        path,
        Arc::new(AlwaysValid),
    )
    .unwrap();

    config
        .try_update(serde_json::json!({"new": "config"}))
        .unwrap();
    let old = config.commit().unwrap();

    assert_eq!(old, serde_json::json!({"key": "value"}));
    assert_eq!(config.current(), serde_json::json!({"new": "config"}));
}

#[test]
fn hot_reload_config_commit_returns_error_when_no_pending() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"key": "value"}),
        path,
        Arc::new(AlwaysValid),
    )
    .unwrap();

    let result = config.commit();
    assert!(matches!(result, Err(Error::SwapFailed)));
}

#[test]
fn hot_reload_config_rollback_clears_pending() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"key": "value"}),
        path,
        Arc::new(AlwaysValid),
    )
    .unwrap();

    config
        .try_update(serde_json::json!({"new": "config"}))
        .unwrap();
    config.rollback();

    let result = config.commit();
    assert!(matches!(result, Err(Error::SwapFailed)));
}

#[test]
fn hot_reload_config_reload_from_file_updates_config() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"updated": true}"#).unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"key": "value"}),
        path,
        Arc::new(AlwaysValid),
    )
    .unwrap();

    let old = config.reload_from_file().unwrap();
    assert_eq!(old, serde_json::json!({"key": "value"}));
    assert_eq!(config.current(), serde_json::json!({"updated": true}));
}

#[test]
fn hot_reload_config_reload_from_file_validates_before_update() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"updated": true}"#).unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"key": "value"}),
        path,
        Arc::new(AlwaysInvalid),
    )
    .unwrap();

    let result = config.reload_from_file();
    assert!(matches!(result, Err(Error::ValidationFailed(_))));
    assert_eq!(config.current(), serde_json::json!({"key": "value"}));
}

#[test]
fn hot_reload_config_path_returns_exact_provided_path() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("subdir").join("config.json");
    fs::create_dir_all(temp_dir.path().join("subdir")).unwrap();
    fs::write(&path, "{}").unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"key": "value"}),
        path.clone(),
        Arc::new(AlwaysValid),
    )
    .unwrap();

    assert_eq!(config.path(), path);
}

#[test]
fn hot_reload_config_try_update_does_not_modify_current() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"key": "value"}),
        path,
        Arc::new(AlwaysValid),
    )
    .unwrap();

    config
        .try_update(serde_json::json!({"new": "config"}))
        .unwrap();

    assert_eq!(config.current(), serde_json::json!({"key": "value"}));
}
