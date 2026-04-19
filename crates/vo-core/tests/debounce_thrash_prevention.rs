//! Debounce thrash prevention tests for config hot-reload.
//!
//! Verifies that rapid file changes are collapsed into single reload events,
//! preventing config reload thrash during active file writes.
//!
//! bead_id: ve-q046j

#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::time::Duration;

use vo_core::debounce::{Debouncer, Error, FileEvent};

/// Helper: create a Debouncer with paused tokio time for deterministic tests.
async fn setup(duration: Duration) -> (tokio::sync::mpsc::Sender<FileEvent>, Debouncer) {
    let (tx, rx) = tokio::sync::mpsc::channel(10_000);
    let debouncer = Debouncer::new(duration, rx).expect("debouncer creation");
    (tx, debouncer)
}

/// Helper: poll a debouncer event without blocking.
async fn poll_ready(debouncer: &mut Debouncer) -> Option<Result<PathBuf, Error>> {
    tokio::select! {
        result = debouncer.next_debounced_event() => Some(result),
        _ = tokio::time::sleep(Duration::from_nanos(1)) => None,
    }
}

#[tokio::test(start_paused = true)]
async fn rapid_modify_events_collapse_to_single_yield() {
    // 50 rapid Modify events for the same file within a 100ms debounce window
    // should produce exactly 1 debounced yield.
    let debounce_ms = 100u64;
    let (tx, mut debouncer) = setup(Duration::from_millis(debounce_ms)).await;
    let path = PathBuf::from("config.json");

    let event_count = 50;
    for _ in 0..event_count {
        tx.send(FileEvent::Modify(path.clone())).await.unwrap();
        tokio::time::advance(Duration::from_millis(1)).await;
    }

    // Not yet yielded — still within debounce window
    assert!(poll_ready(&mut debouncer).await.is_none(),
        "Should not yield during debounce window");

    // Advance past the debounce window
    tokio::time::advance(Duration::from_millis(debounce_ms + 1)).await;

    // Exactly one yield
    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Ok(path.clone()), "Should yield exactly one event");

    // No second yield — all 50 events collapsed into one
    assert!(poll_ready(&mut debouncer).await.is_none(),
        "Should not yield again after collapse");
}

#[tokio::test(start_paused = true)]
async fn stable_state_no_spurious_yields() {
    let (tx, mut debouncer) = setup(Duration::from_millis(100)).await;
    let path = PathBuf::from("stable.json");

    tx.send(FileEvent::Modify(path.clone())).await.unwrap();
    tokio::time::advance(Duration::from_millis(101)).await;

    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Ok(path));

    // Advance a long time with no events — stable state
    tokio::time::advance(Duration::from_secs(60)).await;

    assert!(poll_ready(&mut debouncer).await.is_none(),
        "Stable state must not produce spurious yields");
}

#[tokio::test(start_paused = true)]
async fn debounce_window_separates_distinct_bursts() {
    // Two bursts of events for the same file, separated by more than
    // the debounce window, must produce two separate yields.
    let debounce_ms = 100u64;
    let (tx, mut debouncer) = setup(Duration::from_millis(debounce_ms)).await;
    let path = PathBuf::from("bursts.json");

    // Burst 1: 10 rapid events
    for _ in 0..10 {
        tx.send(FileEvent::Modify(path.clone())).await.unwrap();
        tokio::time::advance(Duration::from_millis(1)).await;
    }

    // Let debounce window expire for burst 1
    tokio::time::advance(Duration::from_millis(debounce_ms + 1)).await;

    let result1 = debouncer.next_debounced_event().await;
    assert_eq!(result1, Ok(path.clone()), "Burst 1 should yield");

    // Gap between bursts — longer than debounce window
    tokio::time::advance(Duration::from_millis(debounce_ms * 3)).await;

    // Burst 2: 10 more rapid events
    for _ in 0..10 {
        tx.send(FileEvent::Modify(path.clone())).await.unwrap();
        tokio::time::advance(Duration::from_millis(1)).await;
    }

    // Let debounce window expire for burst 2
    tokio::time::advance(Duration::from_millis(debounce_ms + 1)).await;

    let result2 = debouncer.next_debounced_event().await;
    assert_eq!(result2, Ok(path.clone()), "Burst 2 should yield as separate event");

    // No third yield
    assert!(poll_ready(&mut debouncer).await.is_none(),
        "Only two bursts should produce two yields");
}
