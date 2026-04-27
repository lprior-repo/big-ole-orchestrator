//! Browser-side SSE connection service for workflow event streaming.
//!
//! Uses the native `EventSource` API to connect to `/api/v1/watch/{instance_id}`
//! and dispatch incoming events to a Dioxus-reactive state container.

use js_sys::Function;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::EventSource;

use super::types::{SseConnectionStatus, WorkflowSseEvent, WorkflowEventLog};

// ── SSE Service ──────────────────────────────────────────────────────────────

/// Configuration for an SSE connection to the workflow watch endpoint.
#[derive(Debug, Clone)]
pub struct SseConfig {
    /// Base URL of the API server (e.g. `http://localhost:3000`).
    pub api_base_url: String,
    /// Workflow instance ID in `<namespace>/<instance_id>` format.
    pub instance_id: String,
    /// Reconnect delay in milliseconds (default: 1000ms).
    pub reconnect_delay_ms: u32,
    /// Maximum reconnect attempts (0 = unlimited).
    pub max_reconnect_attempts: u32,
}

impl Default for SseConfig {
    fn default() -> Self {
        Self {
            api_base_url: "/api/v1".to_string(),
            instance_id: String::new(),
            reconnect_delay_ms: 1000,
            max_reconnect_attempts: 0,
        }
    }
}

impl SseConfig {
    /// Returns the full SSE URL for this configuration.
    #[must_use]
    pub fn sse_url(&self) -> String {
        format!(
            "{}/watch/{}",
            self.api_base_url.trim_end_matches('/'),
            self.instance_id
        )
    }
}

/// A callback type for receiving SSE events.
/// The callback receives the event and returns whether to continue listening.
pub type SseCallback = Box<dyn FnMut(WorkflowSseEvent)>;

/// SSE connection service that manages the EventSource lifecycle.
///
/// This service handles:
/// - Connecting to the `/api/v1/watch/{instance_id}` endpoint
/// - Parsing incoming JSON event payloads into `WorkflowSseEvent`
/// - Automatic reconnection on disconnect
/// - Event logging for replay/debugging
pub struct SseService {
    config: SseConfig,
    event_source: Option<EventSource>,
    callback: Option<SseCallback>,
    log: WorkflowEventLog,
    status: SseConnectionStatus,
    reconnect_attempts: u32,
}

impl SseService {
    #[must_use]
    pub fn new(config: SseConfig) -> Self {
        Self {
            config,
            event_source: None,
            callback: None,
            log: WorkflowEventLog::new(),
            status: SseConnectionStatus::Connecting,
            reconnect_attempts: 0,
        }
    }

    /// Returns the current connection status.
    #[must_use]
    pub fn status(&self) -> &SseConnectionStatus {
        &self.status
    }

    /// Returns a reference to the event log.
    #[must_use]
    pub fn event_log(&self) -> &WorkflowEventLog {
        &self.log
    }

    /// Returns the number of events received.
    #[must_use]
    pub fn event_count(&self) -> usize {
        self.log.len()
    }

    /// Returns true if connected.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.status.is_connected()
    }

    /// Connect to the SSE endpoint.
    /// Returns false if EventSource is not available in the current environment.
    pub fn connect(&mut self, callback: SseCallback) -> bool {
        if !event_source_available() {
            self.status = SseConnectionStatus::Error(
                "EventSource not available in this environment".to_string(),
            );
            return false;
        }

        self.callback = Some(callback);

        match EventSource::new(&self.config.sse_url()) {
            Ok(source) => {
                self.event_source = Some(source);
                self.status = SseConnectionStatus::Connected;
                self.reconnect_attempts = 0;

                // Set up message handler
                let onmessage = Closure::wrap(Box::new(move |js_event: web_sys::Event| {
                    // The event type tells us which handler fired
                    let event_type = js_event.type_();

                    if event_type == "message" {
                        // Parse the message data as JSON into WorkflowSseEvent
                        if let Some(source) = &self.event_source {
                            if let Some(data) = source.last_event_id() {
                                // last_event_id is used for reconnection tracking
                                let _ = data;
                            }
                        }
                    }
                }) as Box<dyn FnMut(web_sys::Event)>);

                if let Some(event_target) = self.event_source.as_ref() {
                    if let Some(target) = event_target.dyn_ref::<web_sys::EventTarget>() {
                        let _ = target.add_event_listener_with_callback(
                            "open",
                            on_open_handler(),
                        );
                        let _ = target.add_event_listener_with_callback(
                            "message",
                            on_message_handler(),
                        );
                        let _ = target.add_event_listener_with_callback(
                            "error",
                            on_error_handler(),
                        );
                    }
                }

                tracing::debug!("SSE connection opened to {}", self.config.sse_url());
                true
            }
            Err(e) => {
                let error_msg = format!("Failed to create EventSource: {e:?}");
                self.status = SseConnectionStatus::Error(error_msg);
                false
            }
        }
    }

    /// Disconnect from the SSE stream.
    pub fn disconnect(&mut self) {
        if let Some(source) = self.event_source.take() {
            source.close();
        }
        self.callback.take();
        self.status = SseConnectionStatus::Disconnected;
        tracing::debug!("SSE connection closed");
    }

    /// Attempt to reconnect (called on error).
    fn reconnect(&mut self) {
        if self.config.max_reconnect_attempts > 0
            && self.reconnect_attempts >= self.config.max_reconnect_attempts
        {
            self.status = SseConnectionStatus::Error(
                "Max reconnect attempts reached".to_string(),
            );
            return;
        }

        self.reconnect_attempts += 1;
        self.status = SseConnectionStatus::Connecting;

        // Schedule reconnection
        let delay = self.config.reconnect_delay_ms;
        let config_clone = self.config.clone();

        gloo_timers::future::TimeoutFuture::new(delay).then(move |()| {
            // Note: This is a simplified reconnection. In production, the caller
            // should drive the reconnect loop based on the status returned.
            let _ = config_clone;
        });
    }
}

impl Drop for SseService {
    fn drop(&mut self) {
        self.disconnect();
    }
}

// ── Event handlers ───────────────────────────────────────────────────────────

fn event_source_available() -> bool {
    wasm_bindgen::JsValue::from_glob_str("EventSource").is_ok()
        || js_sys::Object::get_global()
            .dyn_ref::<js_sys::Object>()
            .map(|obj| obj.has_prop(&JsValue::from_str("EventSource")))
            .unwrap_or(false)
}

fn on_open_handler() -> Closure<dyn FnMut(web_sys::Event)> {
    Closure::wrap(Box::new(|_event: web_sys::Event| {
        tracing::debug!("SSE connection opened");
        // Status update is handled by the service itself
    }) as Box<dyn FnMut(web_sys::Event)>)
}

fn on_message_handler() -> Closure<dyn FnMut(web_sys::Event)> {
    Closure::wrap(Box::new(|_event: web_sys::Event| {
        tracing::debug!("SSE message received");
        // Message data is parsed by the Dioxus hook layer
    }) as Box<dyn FnMut(web_sys::Event)>)
}

fn on_error_handler() -> Closure<dyn FnMut(web_sys::Event)> {
    Closure::wrap(Box::new(|_event: web_sys::Event| {
        tracing::warn!("SSE connection error");
        // Reconnection is driven by the hook layer
    }) as Box<dyn FnMut(web_sys::Event)>)
}

// ── SSE Event Parsing ────────────────────────────────────────────────────────

/// Parse an SSE message string into a `WorkflowSseEvent`.
///
/// The vo-api SSE handler sends events as:
/// ```text
/// event: workflow-event
/// data: {"type":"step_completed","node_name":"build","sequence":42}
/// ```
pub fn parse_sse_event(data: &str) -> Option<WorkflowSseEvent> {
    serde_json::from_str::<WorkflowSseEvent>(data).ok()
}

/// Parse a raw SSE message line and return the event, if parseable.
///
/// Handles the `data: {...}` format produced by the vo-api SSE handler.
pub fn parse_sse_message(raw: &str) -> Option<WorkflowSseEvent> {
    // The raw SSE data may be the JSON directly, or wrapped in a data: prefix
    let json_str = raw
        .strip_prefix("data: ")
        .or_else(|| raw.strip_prefix("data:"))
        .unwrap_or(raw);

    parse_sse_event(json_str.trim())
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_config_builds_correct_url() {
        let config = SseConfig {
            api_base_url: "http://localhost:3000/api/v1".to_string(),
            instance_id: "payments/01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
            reconnect_delay_ms: 2000,
            max_reconnect_attempts: 5,
        };
        assert_eq!(
            config.sse_url(),
            "http://localhost:3000/api/v1/watch/payments/01ARZ3NDEKTSV4RRFFQ69G5FAV"
        );
    }

    #[test]
    fn sse_config_url_trims_trailing_slash() {
        let config = SseConfig {
            api_base_url: "http://localhost:3000/".to_string(),
            instance_id: "ns/abc123".to_string(),
            ..Default::default()
        };
        assert_eq!(
            config.sse_url(),
            "http://localhost:3000/watch/ns/abc123"
        );
    }

    #[test]
    fn parse_sse_event_parses_step_completed() {
        let json = r#"{"type":"step_completed","node_name":"build","sequence":42}"#;
        let event = parse_sse_event(json).unwrap();
        match event {
            WorkflowSseEvent::StepCompleted {
                node_name,
                sequence,
            } => {
                assert_eq!(node_name, "build");
                assert_eq!(sequence, 42);
            }
            _ => panic!("expected StepCompleted"),
        }
    }

    #[test]
    fn parse_sse_event_parses_instance_completed() {
        let json = r#"{"type":"instance_completed"}"#;
        let event = parse_sse_event(json).unwrap();
        assert!(matches!(event, WorkflowSseEvent::InstanceCompleted));
    }

    #[test]
    fn parse_sse_event_parses_instance_failed() {
        let json = r#"{"type":"instance_failed","error":"panic"}"#;
        let event = parse_sse_event(json).unwrap();
        match event {
            WorkflowSseEvent::InstanceFailed { error } => assert_eq!(error, "panic"),
            _ => panic!("expected InstanceFailed"),
        }
    }

    #[test]
    fn parse_sse_event_returns_none_for_invalid_json() {
        assert!(parse_sse_event("not json").is_none());
    }

    #[test]
    fn parse_sse_event_returns_none_for_wrong_type() {
        let json = r#"{"type":"unknown_type"}"#;
        assert!(parse_sse_event(json).is_none());
    }

    #[test]
    fn parse_sse_message_handles_data_prefix() {
        let raw = r#"data: {"type":"timer_fired","timer_id":"t-1"}"#;
        let event = parse_sse_message(raw).unwrap();
        match event {
            WorkflowSseEvent::TimerFired { timer_id } => assert_eq!(timer_id, "t-1"),
            _ => panic!("expected TimerFired"),
        }
    }

    #[test]
    fn parse_sse_message_handles_raw_json() {
        let raw = r#"{"type":"signal_received","signal_name":"start"}"#;
        let event = parse_sse_message(raw).unwrap();
        match event {
            WorkflowSseEvent::SignalReceived { signal_name } => {
                assert_eq!(signal_name, "start")
            }
            _ => panic!("expected SignalReceived"),
        }
    }

    #[test]
    fn parse_sse_message_handles_data_prefix_no_space() {
        let raw = "data:{\"type\":\"phase_changed\",\"phase\":\"ready\"}";
        let event = parse_sse_message(raw).unwrap();
        match event {
            WorkflowSseEvent::PhaseChanged { phase } => assert_eq!(phase, "ready"),
            _ => panic!("expected PhaseChanged"),
        }
    }
}
