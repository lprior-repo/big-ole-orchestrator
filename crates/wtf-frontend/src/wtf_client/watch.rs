#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use std::time::Duration;

use dioxus::prelude::{use_future, use_signal, ReadSignal, ReadableExt, WritableExt};
use futures::{stream, Stream, StreamExt};
use serde_json::Value;
use tap::Pipe;
use thiserror::Error;

use super::types::InstanceView;

#[derive(Debug, Error)]
pub enum WatchError {
    #[error("request failed: {0}")]
    Request(String),
    #[error("invalid SSE payload: {0}")]
    InvalidPayload(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackoffPolicy {
    initial: Duration,
    max: Duration,
}

impl BackoffPolicy {
    #[must_use]
    pub const fn new(initial: Duration, max: Duration) -> Self {
        Self { initial, max }
    }

    #[must_use]
    fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let bounded_shift = attempt.min(10);
        self.initial
            .checked_mul(1_u32 << bounded_shift)
            .map_or(self.max, |delay| delay.min(self.max))
    }
}

impl Default for BackoffPolicy {
    fn default() -> Self {
        Self {
            initial: Duration::from_millis(250),
            max: Duration::from_secs(3),
        }
    }
}

#[derive(Debug, Clone)]
struct WatchState {
    client: reqwest::Client,
    url: String,
    backoff: BackoffPolicy,
    attempt: u32,
}

pub fn watch_namespace(
    base_url: &str,
    namespace: &str,
) -> impl Stream<Item = Result<InstanceView, WatchError>> {
    watch_namespace_with_policy(base_url, namespace, BackoffPolicy::default())
}

fn watch_namespace_with_policy(
    base_url: &str,
    namespace: &str,
    backoff: BackoffPolicy,
) -> impl Stream<Item = Result<InstanceView, WatchError>> {
    let base = base_url.trim_end_matches('/').to_owned();
    let ns = namespace.to_owned();
    let url = format!("{base}/api/v1/watch/{ns}");
    let state = WatchState {
        client: reqwest::Client::new(),
        url,
        backoff,
        attempt: 0,
    };

    stream::unfold(state, |state| async move {
        let event = fetch_one_event(&state.client, &state.url).await;

        let next_state = if let Ok(_) = &event {
            WatchState {
                attempt: 0,
                ..state
            }
        } else {
            let delay = state.backoff.delay_for_attempt(state.attempt);
            sleep_for(delay).await;
            WatchState {
                attempt: state.attempt.saturating_add(1),
                ..state
            }
        };

        Some((event, next_state))
    })
}

async fn fetch_one_event(client: &reqwest::Client, url: &str) -> Result<InstanceView, WatchError> {
    let response = client
        .get(url)
        .header("Accept", "text/event-stream")
        .send()
        .await
        .map_err(|error| WatchError::Request(error.to_string()))?
        .error_for_status()
        .map_err(|error| WatchError::Request(error.to_string()))?;

    let payload = read_first_sse_data_payload(response).await?;
    parse_first_instance_payload(&payload)
}

async fn read_first_sse_data_payload(response: reqwest::Response) -> Result<String, WatchError> {
    response
        .text()
        .await
        .map_err(|error| WatchError::Request(error.to_string()))
        .and_then(|body| parse_first_sse_data_payload(&body))
}

fn parse_first_sse_data_payload(body: &str) -> Result<String, WatchError> {
    body.split("\n\n")
        .find_map(|event| {
            let parts = event
                .lines()
                .filter_map(|line| line.trim_start().strip_prefix("data:").map(str::trim))
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();
            parts.pipe(|parts| (!parts.is_empty()).then(|| parts.join("\n")))
        })
        .ok_or_else(|| WatchError::InvalidPayload("missing data: line in SSE stream".to_owned()))
}

fn parse_first_instance_payload(payload: &str) -> Result<InstanceView, WatchError> {
    let trimmed = payload.trim_start();
    if trimmed.starts_with('{') {
        return serde_json::from_str::<InstanceView>(trimmed)
            .map_err(|error| WatchError::InvalidPayload(error.to_string()));
    }

    if let Some((key, value)) = payload.split_once(':') {
        let parsed: Value = serde_json::from_str(value.trim())
            .map_err(|error| WatchError::InvalidPayload(error.to_string()))?;

        return Ok(InstanceView {
            instance_id: key
                .rsplit('/')
                .next()
                .map_or_else(|| key.to_owned(), ToOwned::to_owned),
            workflow_type: parsed
                .get("workflow_type")
                .and_then(Value::as_str)
                .map_or_else(String::new, ToOwned::to_owned),
            status: parsed
                .get("phase")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_owned(),
            current_state: parsed
                .get("current_state")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            last_event_seq: parsed
                .get("events_applied")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            updated_at: parsed
                .get("updated_at")
                .and_then(Value::as_str)
                .map_or_else(String::new, ToOwned::to_owned),
        });
    }

    serde_json::from_str::<InstanceView>(trimmed)
        .map_err(|error| WatchError::InvalidPayload(error.to_string()))
}

#[cfg(test)]
fn parse_first_instance_line(raw: &str) -> Result<InstanceView, WatchError> {
    raw.lines()
        .find_map(|line| line.trim_start().strip_prefix("data:").map(str::trim))
        .ok_or_else(|| WatchError::InvalidPayload("missing data: line".to_owned()))
        .and_then(parse_first_instance_payload)
}

#[must_use]
pub fn use_instance_watch(namespace: String) -> ReadSignal<Vec<InstanceView>> {
    let instances = use_signal(Vec::<InstanceView>::new);
    let ns = namespace;
    let instances_signal = instances;

    use_future(move || {
        let stream = watch_namespace("http://localhost:8080", &ns);
        async move {
            stream
                .for_each(|item| {
                    let mut signal = instances_signal;
                    async move {
                        if let Ok(next_instance) = item {
                            let current = signal.read().clone();
                            signal.set(upsert_instance(current, next_instance));
                        }
                    }
                })
                .await;
        }
    });

    instances_signal.into()
}

fn upsert_instance(current: Vec<InstanceView>, next: InstanceView) -> Vec<InstanceView> {
    let next_id = next.instance_id.clone();
    let mut merged = current
        .into_iter()
        .filter(|instance| instance.instance_id != next_id)
        .chain(std::iter::once(next))
        .collect::<Vec<_>>();
    merged.sort_by(|left, right| left.instance_id.cmp(&right.instance_id));
    merged
}

async fn sleep_for(duration: Duration) {
    #[cfg(target_arch = "wasm32")]
    {
        gloo_timers::future::sleep(duration).await;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::sleep(duration);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;
    use std::time::Instant;

    use futures::pin_mut;
    use futures::StreamExt;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;

    use super::{
        parse_first_instance_line, parse_first_sse_data_payload, watch_namespace_with_policy,
        BackoffPolicy,
    };

    #[test]
    fn parses_key_prefixed_payload() {
        let raw = "event: put\ndata: payments/01ABC:{\"workflow_type\":\"checkout\",\"phase\":\"live\",\"events_applied\":12}\n\n";
        let parsed = parse_first_instance_line(raw);
        assert!(parsed.is_ok());

        if let Ok(parsed) = parsed {
            assert_eq!(parsed.instance_id, "01ABC");
            assert_eq!(parsed.workflow_type, "checkout");
            assert_eq!(parsed.status, "live");
            assert_eq!(parsed.last_event_seq, 12);
        }
    }

    #[test]
    fn parses_plain_json_payload() {
        let raw = "data: {\"instance_id\":\"abc\",\"workflow_type\":\"wf\",\"status\":\"running\",\"current_state\":null,\"last_event_seq\":2,\"updated_at\":\"2026-01-01T00:00:00Z\"}\n\n";
        let parsed = parse_first_instance_line(raw);
        assert!(parsed.is_ok());

        if let Ok(parsed) = parsed {
            assert_eq!(parsed.instance_id, "abc");
            assert_eq!(parsed.workflow_type, "wf");
            assert_eq!(parsed.status, "running");
        }
    }

    #[test]
    fn backoff_policy_caps_delay_at_max() {
        let policy = BackoffPolicy::new(Duration::from_millis(100), Duration::from_millis(400));
        assert_eq!(policy.delay_for_attempt(0), Duration::from_millis(100));
        assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(200));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(400));
        assert_eq!(policy.delay_for_attempt(9), Duration::from_millis(400));
    }

    #[tokio::test]
    async fn reconnects_with_backoff_and_recovers() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let (base_url, _server_handle) = spawn_sse_server(attempts).await;
        let backoff = BackoffPolicy::new(Duration::from_millis(10), Duration::from_millis(20));

        let watch_stream = watch_namespace_with_policy(&base_url, "payments", backoff);
        pin_mut!(watch_stream);
        let started = Instant::now();

        let first = watch_stream.next().await;
        let second = watch_stream.next().await;
        let third = watch_stream.next().await;
        let elapsed = started.elapsed();

        assert!(matches!(first, Some(Err(_))));
        assert!(matches!(second, Some(Err(_))));
        assert!(elapsed >= Duration::from_millis(30));

        let maybe_instance = third.and_then(Result::ok);
        assert!(maybe_instance.is_some());

        if let Some(instance) = maybe_instance {
            assert_eq!(instance.instance_id, "01BACKOFF");
            assert_eq!(instance.workflow_type, "checkout");
            assert_eq!(instance.status, "live");
            assert_eq!(instance.last_event_seq, 3);
        }
    }

    #[test]
    fn parses_multiline_sse_payload() {
        let body = "event: put\ndata: payments/01ABC:{\"workflow_type\":\"checkout\"\ndata: ,\"phase\":\"live\",\"events_applied\":9}\n\n";
        let payload = parse_first_sse_data_payload(body);
        assert!(payload.is_ok());

        if let Ok(payload) = payload {
            assert!(payload.contains("payments/01ABC"));
            assert!(payload.contains("\"phase\":\"live\""));
        }
    }

    async fn spawn_sse_server(
        attempts: Arc<AtomicUsize>,
    ) -> (String, JoinHandle<Result<(), std::io::Error>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await;
        assert!(listener.is_ok());

        if let Ok(listener) = listener {
            let addr = listener.local_addr();
            assert!(addr.is_ok());

            if let Ok(addr) = addr {
                let server = tokio::spawn(async move {
                    loop {
                        let accepted = listener.accept().await;
                        let (mut socket, _) = match accepted {
                            Ok(value) => value,
                            Err(error) => return Err(error),
                        };

                        let call_count = attempts.fetch_add(1, Ordering::SeqCst);

                        let mut request_buf = [0_u8; 512];
                        let _ = socket.read(&mut request_buf).await;

                        let response = if call_count < 2 {
                            "HTTP/1.1 503 Service Unavailable\r\ncontent-length: 9\r\nconnection: close\r\n\r\nnot ready"
                                .to_owned()
                        } else {
                            let body = "event: put\ndata: payments/01BACKOFF:{\"workflow_type\":\"checkout\",\"phase\":\"live\",\"events_applied\":3}\n\n";
                            format!(
                                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                                body.len(),
                                body
                            )
                        };

                        let _ = socket.write_all(response.as_bytes()).await;
                        let _ = socket.shutdown().await;

                        if call_count >= 2 {
                            break;
                        }
                    }

                    Ok(())
                });

                return (format!("http://{addr}"), server);
            }
        }

        let fallback = tokio::spawn(async { Ok::<(), std::io::Error>(()) });
        ("http://127.0.0.1:0".to_owned(), fallback)
    }
}
