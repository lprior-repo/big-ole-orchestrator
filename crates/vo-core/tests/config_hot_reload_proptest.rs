//! Property-based tests for Config hot-reload system.
//!
//! TDD Red Phase: These proptests verify invariant properties.
//! They should PASS against the current implementation where behaviors exist
//! and expose gaps where they don't.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use proptest::prelude::*;
use tempfile::TempDir;
use vo_core::config_hot_reload::{ConfigValidator, Error, HotReloadConfig, WatcherConfig};

struct AlwaysValid;
impl<T: Clone + Send + Sync> ConfigValidator<T> for AlwaysValid {
    fn validate(&self, _config: &T) -> Result<(), String> {
        Ok(())
    }
}

struct RejectEmpty;
impl ConfigValidator<String> for RejectEmpty {
    fn validate(&self, config: &String) -> Result<(), String> {
        if config.is_empty() {
            Err("empty string rejected".to_string())
        } else {
            Ok(())
        }
    }
}

struct MinLenValidator(usize);
impl ConfigValidator<String> for MinLenValidator {
    fn validate(&self, config: &String) -> Result<(), String> {
        if config.len() >= self.0 {
            Ok(())
        } else {
            Err(format!("string too short: {} < {}", config.len(), self.0))
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn current_returns_independent_clones(
        initial_key in "[a-z]{1,5}",
        initial_val in 0u32..1000,
    ) {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let config = HotReloadConfig::new(
            serde_json::json!({initial_key: initial_val}),
            path,
            Arc::new(AlwaysValid),
        ).unwrap();

        let mut clone = config.current();
        clone.as_object_mut().unwrap().insert("injected".to_string(), serde_json::json!(true));

        prop_assert_eq!(config.current(), serde_json::json!({initial_key: initial_val}));
    }

    #[test]
    fn try_update_only_accepts_valid_configs(
        valid_string in "[a-z]{1,20}",
    ) {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let config = HotReloadConfig::new(
            valid_string.clone(),
            path,
            Arc::new(RejectEmpty),
        ).unwrap();

        let result = config.try_update(valid_string.clone());
        prop_assert!(result.is_ok());

        let result_invalid = config.try_update(String::new());
        prop_assert!(matches!(result_invalid, Err(Error::ValidationFailed(_))));
    }

    #[test]
    fn commit_only_succeeds_when_pending_is_some(
        initial in "[a-z]{1,10}",
        update_val in "[a-z]{1,10}",
    ) {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let config = HotReloadConfig::new(
            initial.clone(),
            path,
            Arc::new(AlwaysValid),
        ).unwrap();

        let no_pending_result = config.commit();
        prop_assert!(matches!(no_pending_result, Err(Error::SwapFailed)));

        config.try_update(update_val.clone()).unwrap();
        let commit_result = config.commit();
        prop_assert!(commit_result.is_ok());
        prop_assert_eq!(commit_result.unwrap(), initial);
    }

    #[test]
    fn rollback_leaves_current_unchanged(
        initial in "[a-z]{1,10}",
        pending_val in "[a-z]{1,10}",
    ) {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let config = HotReloadConfig::new(
            initial.clone(),
            path,
            Arc::new(AlwaysValid),
        ).unwrap();

        config.try_update(pending_val).unwrap();
        config.rollback();

        prop_assert_eq!(config.current(), initial);
    }

    #[test]
    fn reload_from_file_either_updates_or_leaves_unchanged(
        version in 1u64..100u64,
    ) {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");

        let initial = serde_json::json!({"version": 1u64});
        fs::write(&path, serde_json::to_string(&initial).unwrap()).unwrap();

        let config = HotReloadConfig::new(
            initial.clone(),
            path.clone(),
            Arc::new(AlwaysValid),
        ).unwrap();

        fs::write(&path, serde_json::to_string(&serde_json::json!({"version": version})).unwrap()).unwrap();

        let result = config.reload_from_file();

        if result.is_ok() {
            prop_assert_eq!(config.current(), serde_json::json!({"version": version}));
        } else {
            prop_assert_eq!(config.current(), serde_json::json!({"version": 1u64}));
        }
    }

    #[test]
    fn path_clone_independence(
        dir_name in "[a-z]{1,5}",
        file_name in "[a-z]{1,5}\\.json",
    ) {
        let temp_dir = TempDir::new().unwrap();
        let subdir = temp_dir.path().join(&dir_name);
        fs::create_dir_all(&subdir).unwrap();

        let mut path = subdir.join(&file_name);
        fs::write(&path, "{}").unwrap();

        let config = HotReloadConfig::new(
            serde_json::json!({"key": "value"}),
            path.clone(),
            Arc::new(AlwaysValid),
        ).unwrap();

        path.set_file_name("different.json");

        prop_assert!(config.path().file_name().unwrap().to_str().unwrap().contains(&file_name.replace(".json", "")));
    }

    #[test]
    fn watcher_config_default_debounce_is_300ms() {
        let config = WatcherConfig::default();
        prop_assert_eq!(config.debounce_duration, Some(std::time::Duration::from_millis(300)));
    }

    #[test]
    fn min_len_validator_accepts_and_rejects_correctly(
        len in 1usize..20,
        char in 'a'..='z',
    ) {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let config = HotReloadConfig::new(
            String::new(),
            path,
            Arc::new(MinLenValidator(len)),
        ).unwrap();

        let too_short = char.to_string().repeat(len.saturating_sub(1));
        let result = config.try_update(too_short);
        prop_assert!(result.is_err());

        let just_right = char.to_string().repeat(len);
        let result = config.try_update(just_right);
        prop_assert!(result.is_ok());
    }

    #[test]
    fn try_update_twice_only_second_survives(
        first in "[a-z]{1,10}",
        second in "[a-z]{1,10}",
    ) {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let config = HotReloadConfig::new(
            String::new(),
            path,
            Arc::new(AlwaysValid),
        ).unwrap();

        config.try_update(first).unwrap();
        config.try_update(second.clone()).unwrap();

        let old = config.commit().unwrap();
        prop_assert_eq!(old, String::new());
        prop_assert_eq!(config.current(), second);
    }
}
