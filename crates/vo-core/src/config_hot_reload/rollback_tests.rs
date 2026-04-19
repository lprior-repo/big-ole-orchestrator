//! Tests for config hot-reload rollback behavior (ve-o0l7i).
//!
//! Verifies that invalid configs (syntax errors, semantic errors, missing fields)
//! roll back to the previous valid state without corrupting internal state.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tempfile::TempDir;

use super::hot_reload::ConfigValidator;
use super::{Error, HotReloadConfig};

// -- Typed config for missing-field tests --

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AppSettings {
    version: u32,
    name: String,
    max_connections: u64,
}

struct SemanticValidator;
impl ConfigValidator<AppSettings> for SemanticValidator {
    fn validate(&self, config: &AppSettings) -> Result<(), String> {
        if config.max_connections == 0 {
            return Err("max_connections must be > 0".to_string());
        }
        if config.name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        Ok(())
    }
}

// -- Tests --

#[test]
fn rollback_syntax_error_preserves_current_config() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    let initial = AppSettings {
        version: 1,
        name: "production".to_string(),
        max_connections: 100,
    };
    fs::write(&path, serde_json::to_string(&initial).unwrap()).unwrap();

    let config =
        HotReloadConfig::new(initial.clone(), path.clone(), Arc::new(SemanticValidator)).unwrap();

    // Write malformed JSON (syntax error)
    fs::write(&path, "{invalid json {{{").unwrap();

    let result = config.reload_from_file();
    assert!(matches!(result, Err(Error::ParseError(_))));

    // Current config must be preserved
    assert_eq!(config.current(), initial);
}

#[test]
fn rollback_semantic_error_preserves_current_config() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    let initial = AppSettings {
        version: 1,
        name: "production".to_string(),
        max_connections: 100,
    };
    fs::write(&path, serde_json::to_string(&initial).unwrap()).unwrap();

    let config =
        HotReloadConfig::new(initial.clone(), path.clone(), Arc::new(SemanticValidator)).unwrap();

    // Write valid JSON but semantically invalid (max_connections = 0)
    fs::write(
        &path,
        r#"{"version": 2, "name": "staging", "max_connections": 0}"#,
    )
    .unwrap();

    let result = config.reload_from_file();
    assert!(matches!(result, Err(Error::ValidationFailed(_))));

    // Current config must be preserved
    assert_eq!(config.current(), initial);
}

#[test]
fn rollback_missing_required_field_preserves_current_config() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    let initial = AppSettings {
        version: 1,
        name: "production".to_string(),
        max_connections: 100,
    };
    fs::write(&path, serde_json::to_string(&initial).unwrap()).unwrap();

    let config =
        HotReloadConfig::new(initial.clone(), path.clone(), Arc::new(SemanticValidator)).unwrap();

    // Write JSON missing required field "max_connections"
    fs::write(&path, r#"{"version": 2, "name": "staging"}"#).unwrap();

    let result = config.reload_from_file();
    assert!(matches!(result, Err(Error::ParseError(_))));

    // Current config must be preserved
    assert_eq!(config.current(), initial);
}

#[test]
fn rollback_consecutive_failures_preserve_current_config() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    let initial = AppSettings {
        version: 1,
        name: "production".to_string(),
        max_connections: 100,
    };
    fs::write(&path, serde_json::to_string(&initial).unwrap()).unwrap();

    let config =
        HotReloadConfig::new(initial.clone(), path.clone(), Arc::new(SemanticValidator)).unwrap();

    // Multiple consecutive failures of different types
    fs::write(&path, "{broken").unwrap();
    assert!(config.reload_from_file().is_err());
    assert_eq!(config.current(), initial);

    fs::write(
        &path,
        r#"{"version": 2, "name": "staging", "max_connections": 0}"#,
    )
    .unwrap();
    assert!(config.reload_from_file().is_err());
    assert_eq!(config.current(), initial);

    fs::write(&path, r#"{"version": 2, "name": "staging"}"#).unwrap();
    assert!(config.reload_from_file().is_err());
    assert_eq!(config.current(), initial);
}

#[test]
fn rollback_then_successful_reload_works() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    let initial = AppSettings {
        version: 1,
        name: "production".to_string(),
        max_connections: 100,
    };
    fs::write(&path, serde_json::to_string(&initial).unwrap()).unwrap();

    let config =
        HotReloadConfig::new(initial.clone(), path.clone(), Arc::new(SemanticValidator)).unwrap();

    // Failed reload
    fs::write(&path, "{broken").unwrap();
    assert!(config.reload_from_file().is_err());
    assert_eq!(config.current(), initial);

    // Successful reload after failure
    let updated = AppSettings {
        version: 2,
        name: "staging".to_string(),
        max_connections: 200,
    };
    fs::write(&path, serde_json::to_string(&updated).unwrap()).unwrap();
    let old = config.reload_from_file().unwrap();
    assert_eq!(old, initial);
    assert_eq!(config.current(), updated);
}

#[test]
fn rollback_does_not_corrupt_pending_state() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    let initial = AppSettings {
        version: 1,
        name: "production".to_string(),
        max_connections: 100,
    };
    fs::write(&path, serde_json::to_string(&initial).unwrap()).unwrap();

    let config =
        HotReloadConfig::new(initial.clone(), path.clone(), Arc::new(SemanticValidator)).unwrap();

    // Stage a pending config via try_update
    let pending = AppSettings {
        version: 3,
        name: "canary".to_string(),
        max_connections: 50,
    };
    config.try_update(pending.clone()).unwrap();

    // Failed reload from file should not affect pending
    fs::write(&path, "{broken").unwrap();
    assert!(config.reload_from_file().is_err());

    // Pending should still be available for commit
    let old = config.commit().unwrap();
    assert_eq!(old, initial);
    assert_eq!(config.current(), pending);
}

#[test]
fn rollback_try_update_validation_failure_does_not_affect_current() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    let initial = AppSettings {
        version: 1,
        name: "production".to_string(),
        max_connections: 100,
    };
    fs::write(&path, serde_json::to_string(&initial).unwrap()).unwrap();

    let config =
        HotReloadConfig::new(initial.clone(), path, Arc::new(SemanticValidator)).unwrap();

    // Try to update with semantically invalid config
    let invalid = AppSettings {
        version: 2,
        name: "".to_string(),
        max_connections: 100,
    };
    let result = config.try_update(invalid);
    assert!(result.is_err());

    // Current must be preserved
    assert_eq!(config.current(), initial);

    // Pending must be empty
    let commit_result = config.commit();
    assert!(matches!(commit_result, Err(Error::SwapFailed)));
}
