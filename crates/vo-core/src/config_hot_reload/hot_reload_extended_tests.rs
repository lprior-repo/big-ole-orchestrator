#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::sync::Arc;

use tempfile::TempDir;

use super::hot_reload_tests::{AlwaysInvalid, AlwaysValid};
use super::*;

#[test]
fn hot_reload_config_try_update_twice_overwrites_previous_pending() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"key": "original"}),
        path,
        Arc::new(AlwaysValid),
    )
    .unwrap();

    config
        .try_update(serde_json::json!({"key": "first"}))
        .unwrap();
    config
        .try_update(serde_json::json!({"key": "second"}))
        .unwrap();

    let old = config.commit().unwrap();
    assert_eq!(old, serde_json::json!({"key": "original"}));
    assert_eq!(config.current(), serde_json::json!({"key": "second"}));
}

#[test]
fn hot_reload_config_commit_clears_pending_after_promotion() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let config =
        HotReloadConfig::new(serde_json::json!({"v": 1}), path, Arc::new(AlwaysValid)).unwrap();

    config.try_update(serde_json::json!({"v": 2})).unwrap();
    let _old = config.commit().unwrap();

    let result = config.commit();
    assert!(matches!(result, Err(Error::SwapFailed)));
}

#[test]
fn hot_reload_config_rollback_when_no_pending_is_noop() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let config =
        HotReloadConfig::new(serde_json::json!({"v": 1}), path, Arc::new(AlwaysValid)).unwrap();

    config.rollback();
    assert_eq!(config.current(), serde_json::json!({"v": 1}));
}

#[test]
fn hot_reload_config_reload_from_file_returns_parse_error_for_malformed_json() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "not valid json {{{").unwrap();

    let config =
        HotReloadConfig::new(serde_json::json!({"v": 1}), path, Arc::new(AlwaysValid)).unwrap();

    let result = config.reload_from_file();
    assert!(matches!(result, Err(Error::ParseError(_))));
    assert_eq!(config.current(), serde_json::json!({"v": 1}));
}

#[test]
fn hot_reload_config_reload_from_file_does_not_affect_pending() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"v": 1}"#).unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"v": 1}),
        path.clone(),
        Arc::new(AlwaysValid),
    )
    .unwrap();

    config.try_update(serde_json::json!({"v": 999})).unwrap();

    fs::write(&path, r#"{"v": 2}"#).unwrap();
    let old = config.reload_from_file().unwrap();

    assert_eq!(old, serde_json::json!({"v": 1}));
    assert_eq!(config.current(), serde_json::json!({"v": 2}));

    let committed = config.commit().unwrap();
    assert_eq!(committed, serde_json::json!({"v": 2}));
    assert_eq!(config.current(), serde_json::json!({"v": 999}));
}

#[test]
fn hot_reload_config_current_returns_independent_clones() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"key": "value"}),
        path,
        Arc::new(AlwaysValid),
    )
    .unwrap();

    let mut clone1 = config.current();
    let mut clone2 = config.current();

    clone1
        .as_object_mut()
        .unwrap()
        .insert("modified".to_string(), serde_json::json!(true));
    clone2
        .as_object_mut()
        .unwrap()
        .insert("other".to_string(), serde_json::json!(42));

    assert_eq!(config.current(), serde_json::json!({"key": "value"}));
}

#[test]
fn hot_reload_config_commit_returns_previous_current_for_rollback() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let config =
        HotReloadConfig::new(serde_json::json!({"v": 1}), path, Arc::new(AlwaysValid)).unwrap();

    config.try_update(serde_json::json!({"v": 2})).unwrap();
    let old = config.commit().unwrap();

    assert_eq!(old, serde_json::json!({"v": 1}));
    assert_eq!(config.current(), serde_json::json!({"v": 2}));
}

#[test]
fn hot_reload_config_new_initializes_current_with_provided_value() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let initial = serde_json::json!({"nested": {"deep": true}});
    let config = HotReloadConfig::new(initial.clone(), path, Arc::new(AlwaysValid)).unwrap();

    assert_eq!(config.current(), initial);
}

#[test]
fn hot_reload_config_try_update_validates_before_staging() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let config =
        HotReloadConfig::new(serde_json::json!({"v": 1}), path, Arc::new(AlwaysInvalid)).unwrap();

    let result = config.try_update(serde_json::json!({"v": 2}));
    assert!(matches!(result, Err(Error::ValidationFailed(_))));

    let result = config.commit();
    assert!(matches!(result, Err(Error::SwapFailed)));
}

#[test]
fn hot_reload_config_reload_from_file_reads_from_stored_path() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"original": true}"#).unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"original": true}),
        path.clone(),
        Arc::new(AlwaysValid),
    )
    .unwrap();

    assert_eq!(config.path(), path);

    fs::write(&path, r#"{"updated": true}"#).unwrap();
    config.reload_from_file().unwrap();

    assert_eq!(config.current(), serde_json::json!({"updated": true}));
}
