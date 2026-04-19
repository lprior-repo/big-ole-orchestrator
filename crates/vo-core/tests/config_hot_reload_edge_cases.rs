use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use tempfile::TempDir;
use vo_core::config_hot_reload::{ConfigValidator, Error, HotReloadConfig};

struct AlwaysValid;
impl<T: Clone + Send + Sync> ConfigValidator<T> for AlwaysValid {
    fn validate(&self, _config: &T) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn concurrent_try_update_and_commit_from_multiple_threads() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"version": 1}"#).unwrap();

    let config = Arc::new(
        HotReloadConfig::new(
            serde_json::json!({"version": 1}),
            path,
            Arc::new(AlwaysValid),
        )
        .unwrap(),
    );

    let mut handles = Vec::new();
    for i in 0..8 {
        let cfg = Arc::clone(&config);
        handles.push(thread::spawn(move || {
            for j in 0..100 {
                let new_val = serde_json::json!({"version": i * 100 + j});
                let _ = cfg.try_update(new_val);
                let _ = cfg.commit();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let current = config.current();
    assert!(current.is_object());
    assert!(current.get("version").is_some());
}

#[test]
fn concurrent_readers_while_writing() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"version": 1}"#).unwrap();

    let config = Arc::new(
        HotReloadConfig::new(
            serde_json::json!({"version": 1}),
            path,
            Arc::new(AlwaysValid),
        )
        .unwrap(),
    );

    let mut handles = Vec::new();

    let writer_cfg = Arc::clone(&config);
    handles.push(thread::spawn(move || {
        for i in 0..100u64 {
            let new_val = serde_json::json!({"version": i});
            let _ = writer_cfg.try_update(new_val);
            let _ = writer_cfg.commit();
        }
    }));

    for _ in 0..4 {
        let reader_cfg = Arc::clone(&config);
        handles.push(thread::spawn(move || {
            for _ in 0..200 {
                let current = reader_cfg.current();
                assert!(current.is_object());
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}

#[test]
fn reload_from_file_with_empty_file_returns_parse_error() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"version": 1}"#).unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"version": 1}),
        path.clone(),
        Arc::new(AlwaysValid),
    )
    .unwrap();

    fs::write(&path, "").unwrap();
    let result = config.reload_from_file();
    assert!(matches!(result, Err(Error::ParseError(_))));
    assert_eq!(config.current(), serde_json::json!({"version": 1}));
}

#[test]
fn reload_from_file_with_file_deleted_after_creation() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"version": 1}"#).unwrap();

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

    let config = HotReloadConfig::new(
        serde_json::json!({"version": 1}),
        path.clone(),
        Arc::new(VersionValidator),
    )
    .unwrap();

    fs::remove_file(&path).unwrap();
    let result = config.reload_from_file();
    assert!(matches!(result, Err(Error::ReadError(_))));
    assert_eq!(config.current(), serde_json::json!({"version": 1}));
}

#[test]
fn concurrent_reload_from_file_and_current_reads() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"version": 1}"#).unwrap();

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

    let config = Arc::new(
        HotReloadConfig::new(
            serde_json::json!({"version": 1}),
            path.clone(),
            Arc::new(VersionValidator),
        )
        .unwrap(),
    );

    let mut handles = Vec::new();

    let writer_cfg = Arc::clone(&config);
    let writer_path = path.clone();
    handles.push(thread::spawn(move || {
        for i in 2..52u64 {
            fs::write(&writer_path, format!(r#"{{"version": {}}}"#, i)).unwrap();
            let _ = writer_cfg.reload_from_file();
        }
    }));

    for _ in 0..4 {
        let reader_cfg = Arc::clone(&config);
        handles.push(thread::spawn(move || {
            for _ in 0..200 {
                let current = reader_cfg.current();
                assert!(current.is_object());
                assert!(current.get("version").is_some());
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}

#[test]
fn rapid_try_update_overwrites_only_last_pending_survives() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"version": 0}"#).unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"version": 0}),
        path,
        Arc::new(AlwaysValid),
    )
    .unwrap();

    for i in 1..=100u64 {
        config
            .try_update(serde_json::json!({"version": i}))
            .unwrap();
    }

    let old = config.commit().unwrap();
    assert_eq!(old, serde_json::json!({"version": 0}));
    assert_eq!(config.current(), serde_json::json!({"version": 100}));
    assert!(matches!(config.commit(), Err(Error::SwapFailed)));
}
