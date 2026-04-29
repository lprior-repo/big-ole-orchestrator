#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use tempfile::TempDir;
use tokio::time;

use super::*;

#[tokio::test(start_paused = true)]
async fn debounced_file_watcher_reports_single_event_for_rapid_changes() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");
    fs::write(&config_path, "{}").unwrap();

    let config = WatcherConfig {
        recursive: false,
        debounce_duration: Some(Duration::from_millis(100)),
        patterns: vec!["*.json".to_string()],
    };

    let (mut watcher, mut rx) = DebouncedFileWatcher::new(&config_path, config).unwrap();

    fs::write(&config_path, r#"{"v":1}"#).unwrap();
    time::advance(Duration::from_millis(20)).await;
    fs::write(&config_path, r#"{"v":2}"#).unwrap();
    time::advance(Duration::from_millis(20)).await;
    fs::write(&config_path, r#"{"v":3}"#).unwrap();
    time::advance(Duration::from_millis(20)).await;

    time::advance(Duration::from_millis(101)).await;

    let result = rx.recv().await.unwrap();
    assert!(result.is_ok());

    let result2 = rx.recv().await;
    assert!(result2.is_none());

    watcher.unwatch().unwrap();
}

#[tokio::test(start_paused = true)]
async fn debounced_file_watcher_yields_event_after_stable_state() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");
    fs::write(&config_path, "{}").unwrap();

    let config = WatcherConfig {
        recursive: false,
        debounce_duration: Some(Duration::from_millis(50)),
        patterns: vec!["*.json".to_string()],
    };

    let (mut watcher, mut rx) = DebouncedFileWatcher::new(&config_path, config).unwrap();

    fs::write(&config_path, r#"{"initial":true}"#).unwrap();
    time::advance(Duration::from_millis(20)).await;

    fs::write(&config_path, r#"{"updated":true}"#).unwrap();
    time::advance(Duration::from_millis(20)).await;

    fs::write(&config_path, r#"{"final":true}"#).unwrap();

    time::advance(Duration::from_millis(51)).await;

    let result = rx.recv().await.unwrap();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), config_path);

    watcher.unwatch().unwrap();
}

#[tokio::test(start_paused = true)]
async fn debounced_file_watcher_respects_exact_debounce_window() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");
    fs::write(&config_path, "{}").unwrap();

    let debounce_ms = 100;
    let config = WatcherConfig {
        recursive: false,
        debounce_duration: Some(Duration::from_millis(debounce_ms)),
        patterns: vec!["*.json".to_string()],
    };

    let (mut watcher, mut rx) = DebouncedFileWatcher::new(&config_path, config).unwrap();

    fs::write(&config_path, r#"{"t":0}"#).unwrap();

    time::advance(Duration::from_millis(99)).await;
    let pending_result = rx.try_recv();
    assert!(pending_result.is_err());

    time::advance(Duration::from_millis(1)).await;
    let result = rx.recv().await.unwrap();
    assert!(result.is_ok());

    watcher.unwatch().unwrap();
}

#[tokio::test(start_paused = true)]
async fn debounced_file_watcher_multiple_distinct_files_each_yield_separate_event() {
    let temp_dir = TempDir::new().unwrap();
    let config_a = temp_dir.path().join("a.json");
    let config_b = temp_dir.path().join("b.json");
    fs::write(&config_a, "{}").unwrap();
    fs::write(&config_b, "{}").unwrap();

    let config = WatcherConfig {
        recursive: false,
        debounce_duration: Some(Duration::from_millis(100)),
        patterns: vec!["*.json".to_string()],
    };

    let (mut watcher_a, mut rx_a) = DebouncedFileWatcher::new(&config_a, config.clone()).unwrap();
    let (mut watcher_b, mut rx_b) = DebouncedFileWatcher::new(&config_b, config.clone()).unwrap();

    fs::write(&config_a, r#"{"a":1}"#).unwrap();
    time::advance(Duration::from_millis(50)).await;
    fs::write(&config_b, r#"{"b":2}"#).unwrap();
    time::advance(Duration::from_millis(51)).await;

    let result_a = rx_a.recv().await.unwrap();
    let result_b = rx_b.recv().await.unwrap();

    assert!(result_a.is_ok());
    assert!(result_b.is_ok());

    watcher_a.unwatch().unwrap();
    watcher_b.unwatch().unwrap();
}

#[tokio::test(start_paused = true)]
async fn debounced_file_watcher_deletion_cancels_pending_modify() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");
    fs::write(&config_path, "{}").unwrap();

    let config = WatcherConfig {
        recursive: false,
        debounce_duration: Some(Duration::from_millis(100)),
        patterns: vec!["*.json".to_string()],
    };

    let (mut watcher, mut rx) = DebouncedFileWatcher::new(&config_path, config).unwrap();

    fs::write(&config_path, r#"{"modified":true}"#).unwrap();
    time::advance(Duration::from_millis(50)).await;

    fs::remove_file(&config_path).unwrap();

    time::advance(Duration::from_millis(101)).await;

    let pending_result = rx.try_recv();
    assert!(pending_result.is_err());

    watcher.unwatch().unwrap();
}

#[tokio::test(start_paused = true)]
async fn debounced_file_watcher_stable_then_new_change_resets_debounce() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");
    fs::write(&config_path, "{}").unwrap();

    let config = WatcherConfig {
        recursive: false,
        debounce_duration: Some(Duration::from_millis(50)),
        patterns: vec!["*.json".to_string()],
    };

    let (mut watcher, mut rx) = DebouncedFileWatcher::new(&config_path, config).unwrap();

    fs::write(&config_path, r#"{"v":1}"#).unwrap();
    time::advance(Duration::from_millis(40)).await;

    fs::write(&config_path, r#"{"v":2}"#).unwrap();
    time::advance(Duration::from_millis(30)).await;

    fs::write(&config_path, r#"{"v":3}"#).unwrap();
    time::advance(Duration::from_millis(51)).await;

    let result = rx.recv().await.unwrap();
    assert!(result.is_ok());

    time::advance(Duration::from_millis(60)).await;
    let second_result = rx.recv().await;
    assert!(second_result.is_none());

    watcher.unwatch().unwrap();
}
