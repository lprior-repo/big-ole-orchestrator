//! Dioxus SSE connection manager with reactive state updates.
//!
//! This module provides a Dioxus component and hooks for connecting to the
//! vo-api SSE watch endpoint. It manages:
//! - EventSource connection lifecycle
//! - Automatic reconnection with exponential backoff
//! - Reactive node status updates via Dioxus signals
//! - Terminal event detection (workflow completion/failure)
//!
//! ## Usage
//!
//! ```rust,ignore
//! // In your Dioxus app component:
//! let config = use_ref_cell(|| SseConfig::new(
//!     "http://localhost:8080".to_string(),
//!     "payments/abc123".to_string(),
//! ));
//! let node_states = use_ref_cell::<HashMap<String, ExecutionState>>(HashMap::new);
//! let connection_status = use_signal(|| SseConnectionStatus::Disconnected);
//!
//! SseConnection {
//!     config,
//!     node_states,
//!     on_event: EventHandler::new(|_event| {}),
//! }
//! ```

#![cfg(target_arch = "wasm32")]

use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use dioxus::prelude::*;
use wasm_bindgen::closure::JsClosure;
use web_sys::{Event as JsEvent, EventSource};

use crate::ui::sse::{
    calculate_backoff_delay, parse_sse_event, SseConfig, SseConnectionStatus, SseWorkflowEvent,
};

// ============================================================================
// SSE State Hook
// ============================================================================

/// Reactive state for SSE-driven node status tracking.
///
/// This is the shared state that both the SSE connection manager and the
/// DAG visualization component read from. Updates flow:
///
/// SSE Event → parse → node_states.update() → DAG re-renders
pub struct SseState {
    /// Map of node_name → current execution state.
    /// Updated reactively as SSE events arrive.
    pub node_states: Rc<Signal<HashMap<String, crate::ui::sse::ExecutionState>>>,

    /// Current SSE connection status.
    pub connection_status: Signal<SseConnectionStatus>,

    /// Latest event received (for logging/inspection).
    pub latest_event: Signal<Option<SseWorkflowEvent>>,

    /// Total event count received.
    pub event_count: Signal<u64>,

    /// Whether the workflow has reached a terminal state.
    pub is_terminal: Signal<bool>,
}

impl SseState {
    /// Returns a clone suitable for closure capture.
    pub fn clone_for_closure(&self) -> Self {
        Self {
            node_states: self.node_states.clone(),
            connection_status: self.connection_status.clone(),
            latest_event: self.latest_event.clone(),
            event_count: self.event_count.clone(),
            is_terminal: self.is_terminal.clone(),
        }
    }
}

impl SseState {
    /// Creates a new SSE state with initial disconnected status.
    #[must_use]
    pub fn new() -> Self {
        Self {
            node_states: Rc::new(Signal::new(HashMap::new())),
            connection_status: Signal::new(SseConnectionStatus::Disconnected),
            latest_event: Signal::new(None),
            event_count: Signal::new(0),
            is_terminal: Signal::new(false),
        }
    }

    /// Updates state from a received SSE event.
    /// Returns true if the event was a terminal state change.
    pub fn on_event(&mut self, event: SseWorkflowEvent) {
        let is_terminal = event.is_terminal();

        // Update latest event
        self.latest_event.set(Some(event.clone()));
        self.event_count.set(self.event_count.read() + 1);
        self.is_terminal.set(is_terminal);

        // Update node states if this event has a node name
        if let Some((node_name, new_state)) = crate::ui::sse::event_to_status_change(&event) {
            self.node_states
                .write()
                .insert(node_name, new_state);
        }
    }
}

impl Default for SseState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// EventSource Connection
// ============================================================================

/// Attempts to create a new EventSource connection.
/// Returns Ok(EventSource) on success, Err(JsValue) on failure.
fn create_event_source(url: &str) -> Result<EventSource, wasm_bindgen::JsValue> {
    let source = EventSource::new(url)?;
    source.set_with_namespace_handling(false);
    Ok(source)
}

/// Converts a Duration to milliseconds for JavaScript timers.
fn duration_to_ms(duration: Duration) -> f64 {
    duration.as_millis() as f64
}

// ============================================================================
// Connection Management
// ============================================================================

/// The internal state for an SSE connection managed by Dioxus effects.
struct SseConnectionManager {
    config: SseConfig,
    state: SseState,
    on_event: Option<Rc<dyn Fn(&SseWorkflowEvent)>>,
    reconnect_attempt: Rc<Signal<u32>>,
    closed: Rc<Signal<bool>>,
}

impl SseConnectionManager {
    fn new(config: SseConfig, state: SseState, on_event: Option<Rc<dyn Fn(&SseWorkflowEvent)>>) -> Self {
        Self {
            config,
            state,
            on_event,
            reconnect_attempt: Rc::new(Signal::new(0)),
            closed: Rc::new(Signal::new(false)),
        }
    }

    /// Opens the SSE connection. This is the main entry point.
    fn connect(&self) {
        let config = self.config.clone();
        let state = self.state.clone_for_closure();
        let reconnect_attempt = self.reconnect_attempt.clone();
        let closed = self.closed.clone();
        let on_event = self.on_event.clone();

        // Schedule connection as a wasm future
        wasm_bindgen_futures::spawn_local(async move {
            if closed.read().clone() {
                return;
            }

            state.connection_status.set(SseConnectionStatus::Reconnecting);

            // Open the EventSource
            let event_source = match create_event_source(&config.endpoint_url()) {
                Ok(es) => es,
                Err(e) => {
                    tracing::warn!("Failed to create EventSource: {:?}", e);
                    schedule_reconnect(&config, &state, &reconnect_attempt, &closed, &on_event);
                    return;
                }
            };

            if closed.read().clone() {
                event_source.close();
                return;
            }

            state.connection_status.set(SseConnectionStatus::Connected);

            // Register event handlers (captures config for reconnection)
            let result = register_handlers(&event_source, &state, &config, &closed, &on_event);
            if let Err(e) = result {
                tracing::warn!("Failed to register handlers: {:?}", e);
                event_source.close();
                schedule_reconnect(&config, &state, &reconnect_attempt, &closed, &on_event);
                return;
            }

            // Keep connection alive — loop until closed or error
            loop {
                if closed.read().clone() {
                    event_source.close();
                    break;
                }
                // Spin wait — EventSource is event-driven, not polled
                gloo_timers::future::sleep(Duration::from_secs(1)).await;
            }
        });
    }

    /// Closes the connection and stops reconnection attempts.
    fn disconnect(&self) {
        self.closed.write().clone_from(&true);
        self.state.connection_status.set(SseConnectionStatus::Disconnected);
    }
}

/// Registers all EventSource event handlers.
fn register_handlers(
    source: &EventSource,
    state: &SseState,
    config: &SseConfig,
    closed: &Signal<bool>,
    on_event: &Option<Rc<dyn Fn(&SseWorkflowEvent)>>,
) -> Result<(), wasm_bindgen::JsValue> {
    // Message handler — receives all unnamed events
    let message_handler = {
        let state = state.clone_for_closure();
        let closed = closed.clone();
        let on_event = on_event.clone();
        Closure::wrap(Box::new(move |js_event: JsEvent| {
            if *closed.read() {
                return;
            }
            if let Some(data) = js_event.data() {
                if let Some(event) = parse_sse_event(&data) {
                    state.on_event(event.clone());
                    if let Some(ref handler) = on_event {
                        handler(&event);
                    }
                }
            }
        })) as Box<dyn FnMut(_)>
    };
    source.add_event_listener_with_callback("message", message_handler.as_ref().unchecked_ref())?;
    message_handler.forget();

    // Open handler
    let open_handler = {
        let state = state.clone_for_closure();
        Closure::wrap(Box::new(move |_js_event: JsEvent| {
            state.connection_status.set(SseConnectionStatus::Connected);
        }) as Box<dyn FnMut(_)>
    );
    source.add_event_listener_with_callback("open", open_handler.as_ref().unchecked_ref())?;
    open_handler.forget();

    // Error handler — triggers reconnection
    let error_config = config.clone();
    let error_state = state.clone_for_closure();
    let error_closed = closed.clone();
    let error_on_event = on_event.clone();
    let error_reconnect_attempt = Rc::new(Signal::new(0u32));
    let error_reconnect_handle = Rc::new(Signal::new(None::<std::rc::Rc<dyn Fn()>>));
    let error_handle = {
        let state = error_state.clone_for_closure();
        let config = error_config.clone();
        let closed = error_closed.clone();
        let on_event = error_on_event.clone();
        let reconnect_attempt = error_reconnect_attempt.clone();
        Closure::wrap(Box::new(move |js_event: JsEvent| {
            if *closed.read() {
                return;
            }
            let _ = js_event; // unused
            state.connection_status.set(SseConnectionStatus::Reconnecting);
            if let Some(es) = source.dyn_ref::<EventSource>() {
                es.close();
            }
            // Schedule reconnection with backoff using async sleep
            let attempt = reconnect_attempt.read().clone();
            let delay = calculate_backoff_delay(attempt, config.initial_reconnect_delay, config.max_reconnect_delay);
            reconnect_attempt.set(attempt + 1);
            let config_clone = config.clone();
            let state_clone = state.clone_for_closure();
            let closed_clone = closed.clone();
            let on_event_clone = on_event.clone();
            let attempt_clone = reconnect_attempt.clone();
            // Spawn async task to handle the reconnect flow
            wasm_bindgen_futures::spawn_local(async move {
                gloo_timers::future::sleep(delay).await;
                if *closed_clone.read() {
                    return;
                }
                let es = match create_event_source(&config_clone.endpoint_url()) {
                    Ok(es) => es,
                    Err(e) => {
                        tracing::warn!("Reconnect failed: {:?}", e);
                        let a = attempt_clone.read().clone();
                        let d = calculate_backoff_delay(a, config_clone.initial_reconnect_delay, config_clone.max_reconnect_delay);
                        attempt_clone.set(a + 1);
                        let c = config_clone.clone();
                        let s = state_clone.clone_for_closure();
                        let cl = closed_clone.clone();
                        let oe = on_event_clone.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            gloo_timers::future::sleep(d).await;
                            if !*cl.read() {
                                if let Ok(es) = create_event_source(&c.endpoint_url()) {
                                    let _ = register_handlers(&es, &s, &c, &cl, &oe);
                                }
                            }
                        });
                        return;
                    }
                };
                if !*closed_clone.read() {
                    let _ = register_handlers(&es, &state_clone, &config_clone, &closed_clone, &on_event_clone);
                }
            });
        })) as Box<dyn FnMut(_)>
    };
    source.add_event_listener_with_callback("error", error_handle.as_ref().unchecked_ref())?;
    error_handle.forget();

    Ok(())
}

/// Opens a connection (used by reconnect logic).
fn open_connection(
    config: SseConfig,
    state: SseState,
    closed: Signal<bool>,
    on_event: Option<Rc<dyn Fn(&SseWorkflowEvent)>>,
    resolve: &js_sys::Function,
) {
    let cb = move || {
        state.connection_status.set(SseConnectionStatus::Connected);
        resolve.call0(&js_sys::undefined().into());
    };
    cb();
}

// ============================================================================
// Dioxus Component
// ============================================================================

/// A Dioxus component that manages an SSE connection to the workflow watch endpoint.
///
/// This component:
/// 1. Creates an EventSource connection when mounted
/// 2. Listens for workflow events and updates reactive state
/// 3. Handles reconnection automatically on disconnect
/// 4. Cleans up the connection when unmounted
///
/// ## Props
///
/// - `config` — The SSE endpoint configuration
/// - `on_event` — Callback for each received event
#[component]
pub fn SseConnection(
    /// SSE endpoint configuration.
    config: UseRef<SseConfig>,
    /// Callback invoked for each SSE event. Use this to trigger side effects.
    on_event: EventHandler<SseWorkflowEvent>,
) -> Element {
    let state = use_state(|| SseState::new());
    let closed = use_state(|| false);

    // Connect when component mounts
    use_effect(move || {
        let state = state.clone();
        let closed = closed.clone();
        let config = config.clone();

        let config_val = config.read().clone();
        let mgr = SseConnectionManager::new(
            config_val,
            state.clone(),
            Some(Rc::new(move |event: &SseWorkflowEvent| {
                on_event.emit(event.clone());
            })),
        );

        mgr.connect();

        // Cleanup on unmount
        move || {
            closed.write().clone_from(&true);
            mgr.disconnect();
        }
    });

    // Render nothing — this is a management component
    None
}

// ============================================================================
// Utility: Read raw event count from state
// ============================================================================

/// Returns the current connection status from SSE state.
#[must_use]
pub fn connection_status(state: &SseState) -> SseConnectionStatus {
    state.connection_status.read().clone()
}

/// Returns the current event count from SSE state.
#[must_use]
pub fn event_count(state: &SseState) -> u64 {
    state.event_count.read().clone()
}

/// Returns true if the workflow has reached a terminal state.
#[must_use]
pub fn is_terminal(state: &SseState) -> bool {
    state.is_terminal.read().clone()
}

/// Returns a snapshot of all node states.
#[must_use]
pub fn node_states_snapshot(state: &SseState) -> HashMap<String, crate::ui::sse::ExecutionState> {
    state.node_states.read().clone()
}

/// Returns the latest event from SSE state.
#[must_use]
pub fn latest_event(state: &SseState) -> Option<SseWorkflowEvent> {
    state.latest_event.read().clone()
}
