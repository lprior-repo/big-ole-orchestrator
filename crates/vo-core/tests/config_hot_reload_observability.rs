#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::sync::Arc;

use tempfile::TempDir;
use vo_core::config_hot_reload::{
    ConfigValidator, Error, HotReloadConfig, HotReloadMetrics, ReloadEvent,
};

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

#[test]
fn reload_event_read_error_emits_failed_event() {
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

    fs::remove_file(config.path()).unwrap();
    let result = config.reload_from_file();

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::ReadError(_)));
    assert_eq!(metrics.reload_error_total.get(), 1);
    assert_eq!(metrics.reload_success_total.get(), 0);

    let events = received_events.lock().unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        ReloadEvent::ReloadFailed { error, .. } => {
            assert!(matches!(error, Error::ReadError(_)));
        }
        ReloadEvent::Reloaded { .. } => {
            panic!("Expected ReloadFailed event for ReadError");
        }
    }
}

#[test]
fn reload_event_contains_correct_path() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let received_events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let received_events_clone = received_events.clone();
    let callback = Arc::new(move |event: ReloadEvent| {
        received_events_clone.lock().unwrap().push(event);
    });

    let config = HotReloadConfig::<serde_json::Value>::new_with_observability(
        serde_json::json!({"v": 1}),
        path.clone(),
        Arc::new(AlwaysValid),
        Arc::new(HotReloadMetrics::new()),
        callback,
    )
    .unwrap();

    fs::write(config.path(), r#"{"v": 2}"#).unwrap();
    config.reload_from_file().unwrap();

    let events = received_events.lock().unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        ReloadEvent::Reloaded { path: event_path, .. } => {
            assert_eq!(event_path, &path);
        }
        ReloadEvent::ReloadFailed { .. } => panic!("Expected Reloaded"),
    }
}

#[test]
fn reload_error_event_contains_correct_path() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let received_events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let received_events_clone = received_events.clone();
    let callback = Arc::new(move |event: ReloadEvent| {
        received_events_clone.lock().unwrap().push(event);
    });

    let config = HotReloadConfig::<serde_json::Value>::new_with_observability(
        serde_json::json!({"v": 1}),
        path.clone(),
        Arc::new(AlwaysInvalid),
        Arc::new(HotReloadMetrics::new()),
        callback,
    )
    .unwrap();

    let _ = config.reload_from_file();

    let events = received_events.lock().unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        ReloadEvent::ReloadFailed { path: event_path, .. } => {
            assert_eq!(event_path, &path);
        }
        ReloadEvent::Reloaded { .. } => panic!("Expected ReloadFailed"),
    }
}

#[test]
fn reload_event_duration_is_reasonable() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let received_events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let received_events_clone = received_events.clone();
    let callback = Arc::new(move |event: ReloadEvent| {
        received_events_clone.lock().unwrap().push(event);
    });

    let config = HotReloadConfig::<serde_json::Value>::new_with_observability(
        serde_json::json!({}),
        path,
        Arc::new(AlwaysValid),
        Arc::new(HotReloadMetrics::new()),
        callback,
    )
    .unwrap();

    fs::write(config.path(), r#"{"x": 1}"#).unwrap();
    config.reload_from_file().unwrap();

    let events = received_events.lock().unwrap();
    match &events[0] {
        ReloadEvent::Reloaded { duration_ms, .. } => {
            assert!(*duration_ms < 5000, "duration_ms should be reasonable");
        }
        ReloadEvent::ReloadFailed { .. } => panic!("Expected Reloaded"),
    }
}

#[test]
fn reload_error_event_has_duration() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let received_events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let received_events_clone = received_events.clone();
    let callback = Arc::new(move |event: ReloadEvent| {
        received_events_clone.lock().unwrap().push(event);
    });

    let config = HotReloadConfig::<serde_json::Value>::new_with_observability(
        serde_json::json!({}),
        path,
        Arc::new(AlwaysInvalid),
        Arc::new(HotReloadMetrics::new()),
        callback,
    )
    .unwrap();

    let _ = config.reload_from_file();

    let events = received_events.lock().unwrap();
    match &events[0] {
        ReloadEvent::ReloadFailed { duration_ms, .. } => {
            assert!(*duration_ms < 5000, "error duration_ms should be reasonable");
        }
        ReloadEvent::Reloaded { .. } => panic!("Expected ReloadFailed"),
    }
}

#[test]
fn multiple_reload_events_arrive_in_order() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"v": 0}"#).unwrap();

    let received_events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let received_events_clone = received_events.clone();
    let callback = Arc::new(move |event: ReloadEvent| {
        received_events_clone.lock().unwrap().push(event);
    });

    let config = HotReloadConfig::<serde_json::Value>::new_with_observability(
        serde_json::json!({"v": 0}),
        path,
        Arc::new(AlwaysValid),
        Arc::new(HotReloadMetrics::new()),
        callback,
    )
    .unwrap();

    // Reload 1: success
    fs::write(config.path(), r#"{"v": 1}"#).unwrap();
    config.reload_from_file().unwrap();

    // Reload 2: parse error
    fs::write(config.path(), "bad").unwrap();
    let _ = config.reload_from_file();

    // Reload 3: success
    fs::write(config.path(), r#"{"v": 3}"#).unwrap();
    config.reload_from_file().unwrap();

    let events = received_events.lock().unwrap();
    assert_eq!(events.len(), 3);
    assert!(matches!(&events[0], ReloadEvent::Reloaded { .. }));
    assert!(matches!(&events[1], ReloadEvent::ReloadFailed { .. }));
    assert!(matches!(&events[2], ReloadEvent::Reloaded { .. }));
}

#[test]
fn mixed_reload_metrics_track_independently() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let metrics = Arc::new(HotReloadMetrics::new());

    let config = HotReloadConfig::<serde_json::Value>::new_with_observability(
        serde_json::json!({}),
        path,
        Arc::new(AlwaysInvalid),
        metrics.clone(),
        Arc::new(|_| {}),
    )
    .unwrap();

    // 3 failed reloads
    let _ = config.reload_from_file();
    let _ = config.reload_from_file();
    let _ = config.reload_from_file();

    assert_eq!(metrics.reload_error_total.get(), 3);
    assert_eq!(metrics.reload_success_total.get(), 0);
    assert_eq!(metrics.reload_latency_ms.count(), 0);
}

#[test]
fn shared_metrics_aggregate_across_configs() {
    let temp_dir = TempDir::new().unwrap();
    let path_a = temp_dir.path().join("a.json");
    let path_b = temp_dir.path().join("b.json");
    fs::write(&path_a, r#"{"id": "a"}"#).unwrap();
    fs::write(&path_b, r#"{"id": "b"}"#).unwrap();

    let shared_metrics = Arc::new(HotReloadMetrics::new());

    let config_a = HotReloadConfig::<serde_json::Value>::new_with_observability(
        serde_json::json!({"id": "a"}),
        path_a,
        Arc::new(AlwaysValid),
        shared_metrics.clone(),
        Arc::new(|_| {}),
    )
    .unwrap();

    let config_b = HotReloadConfig::<serde_json::Value>::new_with_observability(
        serde_json::json!({"id": "b"}),
        path_b,
        Arc::new(AlwaysValid),
        shared_metrics.clone(),
        Arc::new(|_| {}),
    )
    .unwrap();

    fs::write(config_a.path(), r#"{"id": "a2"}"#).unwrap();
    config_a.reload_from_file().unwrap();

    fs::write(config_b.path(), r#"{"id": "b2"}"#).unwrap();
    config_b.reload_from_file().unwrap();

    fs::write(config_a.path(), r#"{"id": "a3"}"#).unwrap();
    config_a.reload_from_file().unwrap();

    assert_eq!(shared_metrics.reload_success_total.get(), 3);
    assert_eq!(shared_metrics.reload_latency_ms.count(), 3);
    assert_eq!(shared_metrics.reload_error_total.get(), 0);
}

#[test]
fn reload_success_records_latency_in_histogram() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let metrics = Arc::new(HotReloadMetrics::new());

    let config = HotReloadConfig::<serde_json::Value>::new_with_observability(
        serde_json::json!({}),
        path,
        Arc::new(AlwaysValid),
        metrics.clone(),
        Arc::new(|_| {}),
    )
    .unwrap();

    fs::write(config.path(), r#"{"a": 1}"#).unwrap();
    config.reload_from_file().unwrap();
    fs::write(config.path(), r#"{"a": 2}"#).unwrap();
    config.reload_from_file().unwrap();

    assert_eq!(metrics.reload_latency_ms.count(), 2);
    assert!(
        metrics.reload_latency_ms.sum() < 10_000,
        "latency sum should be reasonable"
    );
    assert!(metrics.reload_latency_ms.average() < 5_000);
}

#[test]
fn reload_failure_does_not_record_latency() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let metrics = Arc::new(HotReloadMetrics::new());

    let config = HotReloadConfig::<serde_json::Value>::new_with_observability(
        serde_json::json!({}),
        path,
        Arc::new(AlwaysInvalid),
        metrics.clone(),
        Arc::new(|_| {}),
    )
    .unwrap();

    let _ = config.reload_from_file();
    let _ = config.reload_from_file();

    assert_eq!(metrics.reload_error_total.get(), 2);
    assert_eq!(metrics.reload_latency_ms.count(), 0);
    assert_eq!(metrics.reload_latency_ms.sum(), 0);
}

#[test]
fn reload_event_callback_receives_all_error_variants() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let received_events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let received_events_clone = received_events.clone();
    let callback = Arc::new(move |event: ReloadEvent| {
        received_events_clone.lock().unwrap().push(event);
    });

    let config = HotReloadConfig::<serde_json::Value>::new_with_observability(
        serde_json::json!({"v": 0}),
        path,
        Arc::new(AlwaysValid),
        Arc::new(HotReloadMetrics::new()),
        callback,
    )
    .unwrap();

    // 1: success
    fs::write(config.path(), r#"{"v": 1}"#).unwrap();
    config.reload_from_file().unwrap();

    // 2: parse error
    fs::write(config.path(), "{{{bad").unwrap();
    let _ = config.reload_from_file();

    // 3: read error (file deleted)
    fs::remove_file(config.path()).unwrap();
    let _ = config.reload_from_file();

    let events = received_events.lock().unwrap();
    assert_eq!(events.len(), 3);

    // Verify error types
    match &events[0] {
        ReloadEvent::Reloaded { .. } => {}
        _ => panic!("Expected Reloaded for event 0"),
    }
    match &events[1] {
        ReloadEvent::ReloadFailed { error, .. } => {
            assert!(matches!(error, Error::ParseError(_)));
        }
        _ => panic!("Expected ReloadFailed for event 1"),
    }
    match &events[2] {
        ReloadEvent::ReloadFailed { error, .. } => {
            assert!(matches!(error, Error::ReadError(_)));
        }
        _ => panic!("Expected ReloadFailed for event 2"),
    }
}
