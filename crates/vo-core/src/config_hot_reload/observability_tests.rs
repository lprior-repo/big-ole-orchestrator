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
fn reload_event_reload_succeeded_records_duration() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let metrics = Arc::new(HotReloadMetrics::new());
    let received_events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let received_events_clone = received_events.clone();

    let callback = Arc::new(move |event: ReloadEvent| {
        received_events_clone.lock().unwrap().push(event);
    });

    let config = HotReloadConfig::<serde_json::Value>::new_with_observability(
        serde_json::json!({"key": "value"}),
        path,
        Arc::new(AlwaysValid),
        metrics.clone(),
        callback,
    )
    .unwrap();

    fs::write(config.path(), r#"{"updated": true}"#).unwrap();
    let old = config.reload_from_file().unwrap();

    assert_eq!(old, serde_json::json!({"key": "value"}));
    assert_eq!(config.current(), serde_json::json!({"updated": true}));

    assert_eq!(metrics.reload_success_total.get(), 1);
    assert!(metrics.reload_latency_ms.count() >= 1);

    let events = received_events.lock().unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        ReloadEvent::Reloaded { path: _, duration_ms } => {
            assert!(*duration_ms >= 0);
        }
        ReloadEvent::ReloadFailed { .. } => {
            panic!("Expected Reloaded event, got ReloadFailed");
        }
    }
}

#[test]
fn reload_event_reload_failed_records_error() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"updated": true}"#).unwrap();

    let metrics = Arc::new(HotReloadMetrics::new());
    let received_events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let received_events_clone = received_events.clone();

    let callback = Arc::new(move |event: ReloadEvent| {
        received_events_clone.lock().unwrap().push(event);
    });

    let config = HotReloadConfig::<serde_json::Value>::new_with_observability(
        serde_json::json!({"key": "value"}),
        path,
        Arc::new(AlwaysInvalid),
        metrics.clone(),
        callback,
    )
    .unwrap();

    let result = config.reload_from_file();

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::ValidationFailed(_)));

    assert_eq!(metrics.reload_error_total.get(), 1);
    assert_eq!(metrics.reload_success_total.get(), 0);

    let events = received_events.lock().unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        ReloadEvent::Reloaded { .. } => {
            panic!("Expected ReloadFailed event, got Reloaded");
        }
        ReloadEvent::ReloadFailed { path: _, error, duration_ms: _ } => {
            assert!(matches!(error, Error::ValidationFailed(_)));
        }
    }
}

#[test]
fn reload_metrics_success_increments_counter_and_histogram() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let metrics = Arc::new(HotReloadMetrics::new());

    let config = HotReloadConfig::<serde_json::Value>::new_with_observability(
        serde_json::json!({"key": "value"}),
        path,
        Arc::new(AlwaysValid),
        metrics.clone(),
        Arc::new(|_| {}),
    )
    .unwrap();

    assert_eq!(metrics.reload_success_total.get(), 0);

    fs::write(config.path(), r#"{"updated": 1}"#).unwrap();
    config.reload_from_file().unwrap();
    assert_eq!(metrics.reload_success_total.get(), 1);

    fs::write(config.path(), r#"{"updated": 2}"#).unwrap();
    config.reload_from_file().unwrap();
    assert_eq!(metrics.reload_success_total.get(), 2);

    assert_eq!(metrics.reload_latency_ms.count(), 2);
}

#[test]
fn reload_metrics_error_increments_error_counter() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"updated": true}"#).unwrap();

    let metrics = Arc::new(HotReloadMetrics::new());

    let config = HotReloadConfig::<serde_json::Value>::new_with_observability(
        serde_json::json!({"key": "value"}),
        path,
        Arc::new(AlwaysInvalid),
        metrics.clone(),
        Arc::new(|_| {}),
    )
    .unwrap();

    assert_eq!(metrics.reload_error_total.get(), 0);

    let _ = config.reload_from_file();
    assert_eq!(metrics.reload_error_total.get(), 1);

    let _ = config.reload_from_file();
    assert_eq!(metrics.reload_error_total.get(), 2);

    assert_eq!(metrics.reload_success_total.get(), 0);
}

#[test]
fn reload_event_parse_error_emits_error_event() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let metrics = Arc::new(HotReloadMetrics::new());
    let received_events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let received_events_clone = received_events.clone();

    let callback = Arc::new(move |event: ReloadEvent| {
        received_events_clone.lock().unwrap().push(event);
    });

    let config = HotReloadConfig::<serde_json::Value>::new_with_observability(
        serde_json::json!({"key": "value"}),
        path,
        Arc::new(AlwaysValid),
        metrics.clone(),
        callback,
    )
    .unwrap();

    fs::write(config.path(), "not valid json").unwrap();
    let result = config.reload_from_file();

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::ParseError(_)));
    assert_eq!(metrics.reload_error_total.get(), 1);

    let events = received_events.lock().unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        ReloadEvent::ReloadFailed { error, .. } => {
            assert!(matches!(error, Error::ParseError(_)));
        }
        ReloadEvent::Reloaded { .. } => {
            panic!("Expected ReloadFailed event");
        }
    }
}