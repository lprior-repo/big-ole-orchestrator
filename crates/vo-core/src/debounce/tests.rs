use super::*;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc;
use tokio::time;

fn setup(duration: Duration) -> (mpsc::Sender<FileEvent>, Debouncer) {
    let (tx, rx) = mpsc::channel(100);
    let debouncer = Debouncer::new(duration, rx).unwrap_or_else(|_| panic!("setup failed"));
    (tx, debouncer)
}

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

#[test]
fn debouncer_new_returns_invalid_duration_error_when_duration_is_zero() {
    let (_tx, rx) = mpsc::channel(10);
    let result = Debouncer::new(Duration::from_nanos(0), rx);
    assert_eq!(result, Err(Error::InvalidDebounceDuration));
}

#[tokio::test]
async fn debouncer_new_returns_ok_instance_when_duration_is_one_nanosecond() {
    let (_tx, rx) = mpsc::channel(10);
    let duration = Duration::from_nanos(1);
    let result = Debouncer::new(duration, rx);
    assert_eq!(result.unwrap().duration, duration);
}

#[tokio::test]
async fn debouncer_new_returns_ok_instance_when_duration_is_max() {
    let (_tx, rx) = mpsc::channel(10);
    let duration = Duration::MAX;
    let result = Debouncer::new(duration, rx);
    assert_eq!(result.unwrap().duration, duration);
}

#[tokio::test]
async fn debouncer_new_returns_channel_closed_error_when_receiver_is_already_closed() {
    let (tx, rx) = mpsc::channel(10);
    drop(tx);
    let result = Debouncer::new(Duration::from_millis(100), rx);
    assert_eq!(result, Err(Error::WatcherChannelClosed));
}

#[test]
fn debouncer_new_returns_no_runtime_error_outside_tokio() {
    let handle = std::thread::spawn(|| {
        let (_tx, rx) = tokio::sync::mpsc::channel::<FileEvent>(10);
        Debouncer::new(Duration::from_millis(100), rx)
    });
    let result = handle.join().unwrap();
    assert_eq!(result, Err(Error::NoRuntime));
}

#[tokio::test(start_paused = true)]
async fn next_debounced_event_yields_exact_path_when_debounce_duration_elapses() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);

    let path = PathBuf::from("workflow.bin");
    tx.send(FileEvent::Modify(path.clone())).await.unwrap();

    assert_eq!(poll_next(&mut debouncer).await, Poll::Pending);

    time::advance(Duration::from_millis(101)).await;

    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Ok(path));
}

#[tokio::test(start_paused = true)]
async fn next_debounced_event_yields_ok_path_after_continuous_writes_cease() {
    let duration = Duration::from_millis(50);
    let (tx, mut debouncer) = setup(duration);

    let path = PathBuf::from("workflow_c.bin");

    tx.send(FileEvent::Modify(path.clone())).await.unwrap();
    assert_eq!(poll_next(&mut debouncer).await, Poll::Pending);
    time::advance(Duration::from_millis(20)).await;

    tx.send(FileEvent::Modify(path.clone())).await.unwrap();
    assert_eq!(poll_next(&mut debouncer).await, Poll::Pending);
    time::advance(Duration::from_millis(20)).await;

    tx.send(FileEvent::Modify(path.clone())).await.unwrap();
    assert_eq!(poll_next(&mut debouncer).await, Poll::Pending);
    time::advance(Duration::from_millis(20)).await;

    time::advance(Duration::from_millis(100)).await;

    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Ok(path));
}

#[tokio::test(start_paused = true)]
async fn next_debounced_event_collapses_multiple_events_for_same_file_into_single_yield() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);
    let path = PathBuf::from("workflow_a.bin");

    tx.send(FileEvent::Modify(path.clone())).await.unwrap();
    time::advance(Duration::from_millis(20)).await;

    tx.send(FileEvent::Modify(path.clone())).await.unwrap();
    time::advance(Duration::from_millis(20)).await;

    tx.send(FileEvent::Modify(path.clone())).await.unwrap();
    time::advance(Duration::from_millis(20)).await;

    time::advance(Duration::from_millis(101)).await;

    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Ok(path));

    assert_eq!(poll_next(&mut debouncer).await, Poll::Pending);
}

#[tokio::test(start_paused = true)]
async fn next_debounced_event_yields_multiple_distinct_files_when_events_interleaved() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);

    let path_a = PathBuf::from("file_a.bin");
    let path_b = PathBuf::from("file_b.bin");

    tx.send(FileEvent::Modify(path_a.clone())).await.unwrap();
    time::advance(Duration::from_millis(50)).await;
    tx.send(FileEvent::Modify(path_b.clone())).await.unwrap();

    time::advance(Duration::from_millis(51)).await;

    let res_a = debouncer.next_debounced_event().await;
    assert_eq!(res_a, Ok(path_a));

    time::advance(Duration::from_millis(50)).await;

    let res_b = debouncer.next_debounced_event().await;
    assert_eq!(res_b, Ok(path_b));
}

#[tokio::test(start_paused = true)]
async fn next_debounced_event_resets_timer_when_event_arrives_exactly_at_debounce_duration() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);
    let path = PathBuf::from("boundary.bin");

    tx.send(FileEvent::Modify(path.clone())).await.unwrap();
    time::advance(Duration::from_millis(100)).await;

    tx.send(FileEvent::Modify(path.clone())).await.unwrap();
    assert_eq!(poll_next(&mut debouncer).await, Poll::Pending);

    time::advance(Duration::from_millis(101)).await;

    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Ok(path));
}

#[tokio::test(start_paused = true)]
async fn next_debounced_event_drops_pending_path_when_deletion_event_arrives() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);
    let path = PathBuf::from("deleted.bin");

    tx.send(FileEvent::Modify(path.clone())).await.unwrap();
    time::advance(Duration::from_millis(50)).await;

    tx.send(FileEvent::Delete(path.clone())).await.unwrap();

    time::advance(Duration::from_millis(150)).await;

    assert_eq!(poll_next(&mut debouncer).await, Poll::Pending);
}

#[tokio::test(start_paused = true)]
async fn next_debounced_event_yields_pending_events_before_returning_channel_closed_error() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);
    let path = PathBuf::from("pending.bin");

    tx.send(FileEvent::Modify(path.clone())).await.unwrap();
    time::advance(Duration::from_millis(101)).await;

    drop(tx);

    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Ok(path));

    let result2 = debouncer.next_debounced_event().await;
    assert_eq!(result2, Err(Error::WatcherChannelClosed));
}

#[tokio::test(start_paused = true)]
async fn next_debounced_event_returns_channel_closed_error_when_sender_dropped() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);

    drop(tx);

    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Err(Error::WatcherChannelClosed));
}

#[tokio::test(start_paused = true)]
async fn next_debounced_event_remains_pending_when_polled_with_no_events() {
    let duration = Duration::from_millis(100);
    let (_tx, mut debouncer) = setup(duration);

    time::advance(Duration::from_secs(1)).await;

    assert_eq!(poll_next(&mut debouncer).await, Poll::Pending);
}

#[tokio::test(start_paused = true)]
async fn next_debounced_event_yields_empty_path_when_received() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);
    let path = PathBuf::from("");

    tx.send(FileEvent::Modify(path.clone())).await.unwrap();
    time::advance(Duration::from_millis(101)).await;

    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Ok(path));
}

#[tokio::test(start_paused = true)]
async fn next_debounced_event_yields_ok_path_when_path_length_is_maximum() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);
    let path_str = "a".repeat(4096);
    let path = PathBuf::from(path_str);

    tx.send(FileEvent::Modify(path.clone())).await.unwrap();
    time::advance(Duration::from_millis(101)).await;

    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Ok(path));
}

#[tokio::test(start_paused = true)]
async fn next_debounced_event_handles_multiple_concurrent_distinct_files() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);

    let path1 = PathBuf::from("file_1.bin");
    let path2 = PathBuf::from("file_2.bin");
    let path3 = PathBuf::from("file_3.bin");

    tx.send(FileEvent::Modify(path1.clone())).await.unwrap();
    tx.send(FileEvent::Modify(path2.clone())).await.unwrap();
    tx.send(FileEvent::Modify(path3.clone())).await.unwrap();

    time::advance(Duration::from_millis(101)).await;

    let mut results = [
        debouncer.next_debounced_event().await,
        debouncer.next_debounced_event().await,
        debouncer.next_debounced_event().await,
    ];
    results.sort();

    assert_eq!(results[0], Ok(path1));
    assert_eq!(results[1], Ok(path2));
    assert_eq!(results[2], Ok(path3));
}

#[tokio::test(start_paused = true)]
async fn next_debounced_event_returns_internal_error_on_timer_overflow() {
    let duration = Duration::MAX;
    let (tx, mut debouncer) = setup(duration);

    tx.send(FileEvent::Modify(PathBuf::from("overflow.bin")))
        .await
        .unwrap();

    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Err(Error::DebouncerInternal));
}

#[tokio::test(start_paused = true)]
async fn next_debounced_event_returns_debouncer_internal_error_when_timer_fails() {
    let duration = Duration::MAX;
    let (tx, mut debouncer) = setup(duration);

    tx.send(FileEvent::Modify(PathBuf::from("overflow2.bin")))
        .await
        .unwrap();

    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Err(Error::DebouncerInternal));
}

#[tokio::test(start_paused = true)]
async fn component_timeout_handling_graceful_on_silent_deadline() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);

    let path = PathBuf::from("timeout_test.bin");
    tx.send(FileEvent::Modify(path.clone())).await.unwrap();

    assert_eq!(poll_next(&mut debouncer).await, Poll::Pending);

    time::advance(Duration::from_millis(50)).await;

    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Ok(path));
}

#[tokio::test(start_paused = true)]
async fn component_timeout_handling_error_reporting_on_timeout() {
    let duration = Duration::from_millis(50);
    let (tx, mut debouncer) = setup(duration);

    let path = PathBuf::from("error_report.bin");
    tx.send(FileEvent::Modify(path.clone())).await.unwrap();

    time::advance(Duration::from_millis(51)).await;

    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Ok(path));
}

#[tokio::test(start_paused = true)]
async fn component_timeout_multiple_deadlines_fire_correctly() {
    let duration = Duration::from_millis(50);
    let (tx, mut debouncer) = setup(duration);

    let path1 = PathBuf::from("first.bin");
    let path2 = PathBuf::from("second.bin");

    tx.send(FileEvent::Modify(path1.clone())).await.unwrap();
    time::advance(Duration::from_millis(51)).await;

    let result1 = debouncer.next_debounced_event().await;
    assert_eq!(result1, Ok(path1));

    tx.send(FileEvent::Modify(path2.clone())).await.unwrap();
    time::advance(Duration::from_millis(51)).await;

    let result2 = debouncer.next_debounced_event().await;
    assert_eq!(result2, Ok(path2));
}

#[tokio::test(start_paused = true)]
async fn component_timeout_channel_close_during_wait_returns_graceful_error() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);

    let path = PathBuf::from("close_during_wait.bin");
    tx.send(FileEvent::Modify(path.clone())).await.unwrap();

    time::advance(Duration::from_millis(101)).await;

    let result1 = debouncer.next_debounced_event().await;
    assert_eq!(result1, Ok(path));

    drop(tx);

    let result2 = debouncer.next_debounced_event().await;
    assert_eq!(result2, Err(Error::WatcherChannelClosed));
}
