#![allow(clippy::redundant_pattern_matching)]
//! Red Queen adversarial tests for the Filesystem watcher with debounce.
//!
//! These tests attempt to break the debounce logic from every angle,
//! targeting debounce invariants (DEB-001 through DEB-025).

use std::path::PathBuf;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time;
use vo_core::debounce::{Debouncer, Error, FileEvent};

struct PollFuture<'a, F>(&'a mut F);
impl<'a, F: Future + Unpin> Future for PollFuture<'a, F> {
    type Output = Poll<F::Output>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.0).poll(cx) {
            Poll::Ready(val) => Poll::Ready(Poll::Ready(val)),
            Poll::Pending => Poll::Ready(Poll::Pending),
        }
    }
}

async fn poll_next(debouncer: &mut Debouncer) -> Poll<Result<PathBuf, Error>> {
    let mut fut = Box::pin(debouncer.next_debounced_event());
    PollFuture(&mut fut).await
}

fn setup(duration: Duration) -> (mpsc::Sender<FileEvent>, Debouncer) {
    let (tx, rx) = mpsc::channel(100);
    let debouncer = Debouncer::new(duration, rx).unwrap_or_else(|_| panic!("setup failed"));
    (tx, debouncer)
}

#[tokio::test(start_paused = true)]
async fn attack_deb001_rapid_burst_same_file_collapses_to_one() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);

    let file_path = PathBuf::from("/test/file.txt");

    for _ in 0..1000 {
        tx.send(FileEvent::Modify(file_path.clone())).await.unwrap();
    }

    assert_eq!(poll_next(&mut debouncer).await, Poll::Pending);

    time::advance(Duration::from_millis(101)).await;

    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Ok(file_path));

    assert_eq!(poll_next(&mut debouncer).await, Poll::Pending);
}

#[tokio::test(start_paused = true)]
async fn attack_deb002_very_long_debounce_does_not_overflow() {
    let duration = Duration::from_secs(3600);
    let (_tx, rx) = mpsc::channel(100);
    let result = Debouncer::new(duration, rx);
    assert!(result.is_ok(), "Long debounce should not cause overflow");
}

#[tokio::test(start_paused = true)]
async fn attack_deb003_many_distinct_files_yield_separate_events() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);

    let file_count = 50;
    let mut expected_paths = Vec::new();

    for i in 0..file_count {
        let file_path = PathBuf::from(format!("/test/file_{}.txt", i));
        expected_paths.push(file_path.clone());
        tx.send(FileEvent::Modify(file_path)).await.unwrap();
    }

    assert_eq!(poll_next(&mut debouncer).await, Poll::Pending);

    time::advance(Duration::from_millis(101)).await;

    let mut received_paths = Vec::new();
    for _ in 0..file_count {
        let result = debouncer.next_debounced_event().await;
        assert!(result.is_ok());
        received_paths.push(result.unwrap());
    }

    received_paths.sort();
    expected_paths.sort();

    assert_eq!(received_paths, expected_paths);
}

#[tokio::test(start_paused = true)]
async fn attack_deb004_delete_cancels_pending_modify() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);

    let file_path = PathBuf::from("/test/file.txt");

    tx.send(FileEvent::Modify(file_path.clone())).await.unwrap();
    tx.send(FileEvent::Delete(file_path.clone())).await.unwrap();

    time::advance(Duration::from_millis(101)).await;

    assert_eq!(poll_next(&mut debouncer).await, Poll::Pending);
}

#[tokio::test(start_paused = true)]
async fn attack_deb005_delete_nonexistent_path_is_noop() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);

    let unrelated_path = PathBuf::from("/completely/unrelated/path.txt");
    tx.send(FileEvent::Delete(unrelated_path)).await.unwrap();

    let watched_path = PathBuf::from("/test/file.txt");
    tx.send(FileEvent::Modify(watched_path.clone())).await.unwrap();

    time::advance(Duration::from_millis(101)).await;

    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Ok(watched_path));
}

#[tokio::test(start_paused = true)]
async fn attack_deb006_rapid_toggle_final_delete_wins() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);

    let file_path = PathBuf::from("/test/file.txt");

    for i in 0..100 {
        let event = if i % 2 == 0 {
            FileEvent::Modify(file_path.clone())
        } else {
            FileEvent::Delete(file_path.clone())
        };
        tx.send(event).await.unwrap();
    }

    time::advance(Duration::from_millis(101)).await;

    assert_eq!(poll_next(&mut debouncer).await, Poll::Pending);
}

#[tokio::test(start_paused = true)]
async fn attack_deb007_event_after_debounce_boundary() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);

    let file_path = PathBuf::from("/test/file.txt");

    tx.send(FileEvent::Modify(file_path.clone())).await.unwrap();

    assert_eq!(poll_next(&mut debouncer).await, Poll::Pending);

    time::advance(Duration::from_millis(101)).await;

    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Ok(file_path));
}

#[tokio::test(start_paused = true)]
async fn attack_deb008_event_before_boundary_not_ready() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);

    let file_path = PathBuf::from("/test/file.txt");

    tx.send(FileEvent::Modify(file_path.clone())).await.unwrap();

    assert_eq!(poll_next(&mut debouncer).await, Poll::Pending);

    time::advance(Duration::from_millis(99)).await;

    assert_eq!(poll_next(&mut debouncer).await, Poll::Pending);
}

#[tokio::test(start_paused = true)]
async fn attack_deb009_empty_path_handled() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);

    let empty_path = PathBuf::from("");
    tx.send(FileEvent::Modify(empty_path.clone())).await.unwrap();

    time::advance(Duration::from_millis(101)).await;

    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Ok(empty_path));
}

#[tokio::test(start_paused = true)]
async fn attack_deb010_very_long_path_handled() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);

    let long_path_str = "a".repeat(4096);
    let long_path = PathBuf::from(long_path_str);

    tx.send(FileEvent::Modify(long_path.clone())).await.unwrap();

    time::advance(Duration::from_millis(101)).await;

    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Ok(long_path));
}

#[tokio::test(start_paused = true)]
async fn attack_deb011_concurrent_sends_no_panics() {
    use std::sync::Arc;

    let duration = Duration::from_millis(100);
    let (tx, rx) = mpsc::channel(1000);
    let mut debouncer = Debouncer::new(duration, rx).unwrap();
    let tx = Arc::new(tx);

    let mut handles = vec![];

    for i in 0..10 {
        let tx = Arc::clone(&tx);
        let handle = tokio::spawn(async move {
            for j in 0..100 {
                let file_path = PathBuf::from(format!("/test/concurrent_{}_{}.txt", i, j));
                let event = FileEvent::Modify(file_path);
                let _ = tx.send(event).await;
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    time::advance(Duration::from_millis(101)).await;

    while let Ok(Ok(_)) = time::timeout(Duration::from_millis(10), debouncer.next_debounced_event()).await {}
}

#[test]
fn attack_deb012_zero_duration_rejected() {
    let (_tx, rx) = mpsc::channel::<FileEvent>(10);
    let result = Debouncer::new(Duration::from_nanos(0), rx);
    assert_eq!(result, Err(vo_core::debounce::Error::InvalidDebounceDuration));
}

#[tokio::test(start_paused = true)]
async fn attack_deb013_channel_capacity_exhaustion_handled() {
    let duration = Duration::from_millis(100);
    let (tx, rx) = mpsc::channel(100);
    let mut debouncer = Debouncer::new(duration, rx).unwrap();

    for i in 0..2000 {
        let file_path = PathBuf::from(format!("/test/overflow_{}.txt", i));
        let event = FileEvent::Modify(file_path);
        let _ = tx.try_send(event);
    }

    time::advance(Duration::from_millis(101)).await;

    while let Ok(Ok(_)) = time::timeout(Duration::from_millis(10), debouncer.next_debounced_event()).await {}
}

#[tokio::test(start_paused = true)]
async fn attack_deb014_event_ordering_different_files() {
    let duration = Duration::from_millis(50);
    let (tx, mut debouncer) = setup(duration);

    let file_a = PathBuf::from("/test/file_a.txt");
    let file_b = PathBuf::from("/test/file_b.txt");

    tx.send(FileEvent::Modify(file_a.clone())).await.unwrap();
    time::advance(Duration::from_millis(20)).await;

    tx.send(FileEvent::Modify(file_b.clone())).await.unwrap();
    time::advance(Duration::from_millis(20)).await;

    tx.send(FileEvent::Modify(file_a.clone())).await.unwrap();
    time::advance(Duration::from_millis(101)).await;

    let result_a = debouncer.next_debounced_event().await;
    assert!(result_a.is_ok());

    let result_b = debouncer.next_debounced_event().await;
    assert!(result_b.is_ok());
}

#[tokio::test(start_paused = true)]
async fn attack_deb015_modified_timer_reset() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);

    let file_path = PathBuf::from("/test/file.txt");

    tx.send(FileEvent::Modify(file_path.clone())).await.unwrap();
    time::advance(Duration::from_millis(90)).await;

    tx.send(FileEvent::Modify(file_path.clone())).await.unwrap();

    assert_eq!(poll_next(&mut debouncer).await, Poll::Pending);

    time::advance(Duration::from_millis(99)).await;
    assert_eq!(poll_next(&mut debouncer).await, Poll::Pending);

    time::advance(Duration::from_millis(2)).await;

    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Ok(file_path));
}

#[tokio::test(start_paused = true)]
async fn attack_deb016_max_duration_causes_internal_error() {
    let duration = Duration::MAX;
    let (tx, mut debouncer) = setup(duration);

    let file_path = PathBuf::from("/test/overflow.txt");
    tx.send(FileEvent::Modify(file_path)).await.unwrap();

    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Err(vo_core::debounce::Error::DebouncerInternal));
}

#[tokio::test(start_paused = true)]
async fn attack_deb017_closed_channel_error() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);

    let file_path = PathBuf::from("/test/file.txt");
    tx.send(FileEvent::Modify(file_path)).await.unwrap();

    time::advance(Duration::from_millis(101)).await;

    let result1 = debouncer.next_debounced_event().await;
    assert!(result1.is_ok());

    drop(tx);

    let result2 = debouncer.next_debounced_event().await;
    assert_eq!(result2, Err(vo_core::debounce::Error::WatcherChannelClosed));
}

#[tokio::test(start_paused = true)]
async fn attack_deb018_pending_then_closed_channel() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);

    let file_path = PathBuf::from("/test/pending.txt");
    tx.send(FileEvent::Modify(file_path.clone())).await.unwrap();

    time::advance(Duration::from_millis(101)).await;

    drop(tx);

    let result1 = debouncer.next_debounced_event().await;
    assert_eq!(result1, Ok(file_path));

    let result2 = debouncer.next_debounced_event().await;
    assert_eq!(result2, Err(vo_core::debounce::Error::WatcherChannelClosed));
}

#[tokio::test(start_paused = true)]
async fn attack_deb019_modify_after_delete_reappears() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);

    let file_path = PathBuf::from("/test/file.txt");

    tx.send(FileEvent::Modify(file_path.clone())).await.unwrap();
    time::advance(Duration::from_millis(50)).await;

    tx.send(FileEvent::Delete(file_path.clone())).await.unwrap();
    time::advance(Duration::from_millis(101)).await;

    tx.send(FileEvent::Modify(file_path.clone())).await.unwrap();
    time::advance(Duration::from_millis(101)).await;

    let result = debouncer.next_debounced_event().await;
    assert!(result.is_ok());
}

#[tokio::test(start_paused = true)]
async fn attack_deb020_very_short_debounce_burst() {
    let duration = Duration::from_millis(1);
    let (tx, mut debouncer) = setup(duration);

    let file_path = PathBuf::from("/test/file.txt");

    for _ in 0..100 {
        tx.send(FileEvent::Modify(file_path.clone())).await.unwrap();
    }

    time::advance(Duration::from_millis(10)).await;

    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Ok(file_path));

    assert_eq!(poll_next(&mut debouncer).await, Poll::Pending);
}

#[tokio::test(start_paused = true)]
async fn attack_deb021_multiple_files_all_arrive() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);

    let paths: Vec<PathBuf> = (0..5).map(|i| PathBuf::from(format!("/test/file_{}.txt", i))).collect();

    for path in &paths {
        tx.send(FileEvent::Modify(path.clone())).await.unwrap();
    }

    time::advance(Duration::from_millis(101)).await;

    let mut received = Vec::new();
    for _ in 0..5 {
        let result = debouncer.next_debounced_event().await;
        assert!(result.is_ok());
        received.push(result.unwrap());
    }

    received.sort();
    let mut expected = paths.clone();
    expected.sort();
    assert_eq!(received, expected);
}

#[tokio::test(start_paused = true)]
async fn attack_deb022_delete_cancels_immediately() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);

    let file_path = PathBuf::from("/test/file.txt");

    tx.send(FileEvent::Modify(file_path.clone())).await.unwrap();
    tx.send(FileEvent::Delete(file_path.clone())).await.unwrap();

    time::advance(Duration::from_millis(101)).await;

    assert_eq!(poll_next(&mut debouncer).await, Poll::Pending);
}

#[tokio::test(start_paused = true)]
async fn attack_deb023_separate_files_independent_timers() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);

    let file_a = PathBuf::from("/test/a.txt");
    let file_b = PathBuf::from("/test/b.txt");

    tx.send(FileEvent::Modify(file_a.clone())).await.unwrap();
    time::advance(Duration::from_millis(101)).await;

    let result_a = debouncer.next_debounced_event().await;
    assert_eq!(result_a, Ok(file_a));

    tx.send(FileEvent::Modify(file_b.clone())).await.unwrap();
    time::advance(Duration::from_millis(101)).await;

    let result_b = debouncer.next_debounced_event().await;
    assert_eq!(result_b, Ok(file_b));
}

#[tokio::test(start_paused = true)]
async fn attack_deb024_no_pending_no_events() {
    let duration = Duration::from_millis(100);
    let (_tx, mut debouncer) = setup(duration);

    time::advance(Duration::from_secs(1)).await;

    assert_eq!(poll_next(&mut debouncer).await, Poll::Pending);
}

#[tokio::test(start_paused = true)]
async fn attack_deb025_timer_reset_behavior() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);

    let file_path = PathBuf::from("/test/file.txt");

    tx.send(FileEvent::Modify(file_path.clone())).await.unwrap();
    time::advance(Duration::from_millis(50)).await;

    tx.send(FileEvent::Modify(file_path.clone())).await.unwrap();
    time::advance(Duration::from_millis(50)).await;

    tx.send(FileEvent::Modify(file_path.clone())).await.unwrap();
    time::advance(Duration::from_millis(50)).await;

    tx.send(FileEvent::Modify(file_path.clone())).await.unwrap();
    time::advance(Duration::from_millis(101)).await;

    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Ok(file_path));
}
