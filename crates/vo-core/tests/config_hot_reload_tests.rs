//! ConfigValidator and HotReloadConfig integration tests.

use std::fs;
use std::sync::Arc;
use tempfile::TempDir;
use vo_core::config_hot_reload::{ConfigValidator, HotReloadConfig};

struct AlwaysValid;
impl<T: Clone + Send + Sync> ConfigValidator<T> for AlwaysValid {
    fn validate(&self, _config: &T) -> Result<(), String> {
        Ok(())
    }
}

struct AlwaysInvalid;
impl<T: Clone + Send + Sync> ConfigValidator<T> for AlwaysInvalid {
    fn validate(&self, _config: &T) -> Result<(), String> {
        Err("always invalid".to_string())
    }
}

struct ThresholdValidator {
    threshold: u64,
}
impl ConfigValidator<serde_json::Value> for ThresholdValidator {
    fn validate(&self, config: &serde_json::Value) -> Result<(), String> {
        let value = config.get("value").and_then(|v| v.as_u64()).unwrap_or(0);
        if value >= self.threshold {
            Ok(())
        } else {
            Err(format!(
                "value {} below threshold {}",
                value, self.threshold
            ))
        }
    }
}

struct MultiFieldValidator;
impl ConfigValidator<serde_json::Value> for MultiFieldValidator {
    fn validate(&self, config: &serde_json::Value) -> Result<(), String> {
        let name = config.get("name").and_then(|v| v.as_str());
        let version = config.get("version").and_then(|v| v.as_u64());
        let enabled = config.get("enabled").and_then(|v| v.as_bool());

        if name.is_none() {
            return Err("missing required field: name".to_string());
        }
        if version.is_none() {
            return Err("missing required field: version".to_string());
        }
        if enabled.is_none() {
            return Err("missing required field: enabled".to_string());
        }
        Ok(())
    }
}

#[test]
fn config_validator_trait_always_valid_accepts_anything() {
    let validator = AlwaysValid;
    let result = validator.validate(&serde_json::json!({"any": "value"}));
    assert!(result.is_ok(), "AlwaysValid should accept any config");
}

#[test]
fn config_validator_trait_always_invalid_rejects_anything() {
    let validator = AlwaysInvalid;
    let result = validator.validate(&serde_json::json!({"any": "value"}));
    assert!(result.is_err(), "AlwaysInvalid should reject any config");
    assert_eq!(result.unwrap_err(), "always invalid");
}

#[test]
fn hot_reload_config_with_threshold_validator_accepts_above_threshold() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"value": 100}"#).unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"value": 100}),
        path,
        Arc::new(ThresholdValidator { threshold: 50 }),
    )
    .unwrap();

    let result = config.try_update(serde_json::json!({"value": 75}));
    assert!(result.is_ok(), "value above threshold should be accepted");
}

#[test]
fn hot_reload_config_with_threshold_validator_rejects_below_threshold() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"value": 100}"#).unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"value": 100}),
        path,
        Arc::new(ThresholdValidator { threshold: 50 }),
    )
    .unwrap();

    let result = config.try_update(serde_json::json!({"value": 25}));
    assert!(result.is_err(), "value below threshold should be rejected");
}

#[test]
fn hot_reload_config_with_multi_field_validator_requires_all_fields() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"name": "test", "version": 1, "enabled": true}"#).unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"name": "test", "version": 1, "enabled": true}),
        path,
        Arc::new(MultiFieldValidator),
    )
    .unwrap();

    let result = config.try_update(serde_json::json!({"name": "test2", "version": 2}));
    assert!(result.is_err(), "missing enabled field should be rejected");

    let result = config.try_update(serde_json::json!({"name": "test3"}));
    assert!(
        result.is_err(),
        "missing version and enabled fields should be rejected"
    );

    let result = config.try_update(serde_json::json!({
        "name": "test4",
        "version": 4,
        "enabled": false
    }));
    assert!(result.is_ok(), "all fields present should be accepted");
}

#[test]
fn hot_reload_config_validator_error_preserves_current_on_reject() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"value": 100}"#).unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"value": 100}),
        path,
        Arc::new(ThresholdValidator { threshold: 50 }),
    )
    .unwrap();

    assert_eq!(config.current(), serde_json::json!({"value": 100}));

    let result = config.try_update(serde_json::json!({"value": 25}));
    assert!(result.is_err());

    assert_eq!(
        config.current(),
        serde_json::json!({"value": 100}),
        "current config should be unchanged after rejected update"
    );
}

#[test]
fn hot_reload_config_commit_returns_old_config_for_rollback() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"value": 50}"#).unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"value": 50}),
        path,
        Arc::new(AlwaysValid),
    )
    .unwrap();

    config.try_update(serde_json::json!({"value": 75})).unwrap();
    let old = config.commit().unwrap();

    assert_eq!(old, serde_json::json!({"value": 50}));
    assert_eq!(config.current(), serde_json::json!({"value": 75}));
}

#[test]
fn hot_reload_config_rollback_then_update() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"value": 50}"#).unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"value": 50}),
        path,
        Arc::new(ThresholdValidator { threshold: 30 }),
    )
    .unwrap();

    config
        .try_update(serde_json::json!({"value": 100}))
        .unwrap();
    config.rollback();

    let result = config.commit();
    assert!(result.is_err(), "commit should fail after rollback");

    config
        .try_update(serde_json::json!({"value": 200}))
        .unwrap();
    let old = config.commit().unwrap();
    assert_eq!(old, serde_json::json!({"value": 50}));
    assert_eq!(config.current(), serde_json::json!({"value": 200}));
}

#[test]
fn hot_reload_config_reload_from_file_updates_current() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"value": 50}"#).unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"value": 50}),
        path.clone(),
        Arc::new(AlwaysValid),
    )
    .unwrap();

    fs::write(&path, r#"{"value": 100}"#).unwrap();
    let old = config.reload_from_file().unwrap();

    assert_eq!(old, serde_json::json!({"value": 50}));
    assert_eq!(config.current(), serde_json::json!({"value": 100}));
}

#[test]
fn hot_reload_config_reload_from_file_rejects_invalid() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"value": 50}"#).unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"value": 50}),
        path.clone(),
        Arc::new(ThresholdValidator { threshold: 30 }),
    )
    .unwrap();

    fs::write(&path, r#"{"value": 10}"#).unwrap();
    let result = config.reload_from_file();

    assert!(result.is_err(), "reload should fail for invalid config");
    assert_eq!(
        config.current(),
        serde_json::json!({"value": 50}),
        "current should be unchanged after failed reload"
    );
}

#[test]
fn hot_reload_config_reload_from_file_preserves_pending() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"value": 50}"#).unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"value": 50}),
        path.clone(),
        Arc::new(AlwaysValid),
    )
    .unwrap();

    config
        .try_update(serde_json::json!({"value": 999}))
        .unwrap();

    fs::write(&path, r#"{"value": 100}"#).unwrap();
    let old = config.reload_from_file().unwrap();

    assert_eq!(old, serde_json::json!({"value": 50}));
    assert_eq!(config.current(), serde_json::json!({"value": 100}));

    let committed = config.commit().unwrap();
    assert_eq!(
        committed,
        serde_json::json!({"value": 100}),
        "committed value should be from reload, not pending"
    );
    assert_eq!(
        config.current(),
        serde_json::json!({"value": 999}),
        "after commit, current should be the pending value"
    );
}
