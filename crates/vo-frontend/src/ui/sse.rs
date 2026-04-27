//! SSE (Server-Sent Events) client for real-time workflow updates.
//!
//! This module provides the frontend client for connecting to the vo-api
//! SSE endpoint at `/api/v1/watch/{instance_id}`. It mirrors the event types
//! defined in `vo_api::handlers::sse::WorkflowSseEvent` and provides
//! Dioxus-compatible reactive state updates.
//!
//! ## Event Types
//!
//! Matches the server-side `WorkflowSseEvent` enum:
//! - `step_completed` — A workflow step finished successfully
//! - `step_failed` — A workflow step failed with an error
//! - `timer_fired` — A timer event fired
//! - `signal_received` — A signal was received
//! - `phase_changed` — Workflow phase changed
//! - `instance_completed` — Workflow instance completed
//! - `instance_failed` — Workflow instance failed

use std::fmt;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// SSE Event Types (mirrors vo_api::handlers::sse::WorkflowSseEvent)
// ============================================================================

/// A parsed SSE event from the workflow watch endpoint.
/// Matches the server-side `WorkflowSseEvent` enum exactly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SseWorkflowEvent {
    StepCompleted {
        node_name: String,
        sequence: u64,
    },
    StepFailed {
        node_name: String,
        sequence: u64,
        error: String,
    },
    TimerFired {
        timer_id: String,
    },
    SignalReceived {
        signal_name: String,
    },
    PhaseChanged {
        phase: String,
    },
    InstanceCompleted,
    InstanceFailed {
        error: String,
    },
}

impl fmt::Display for SseWorkflowEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SseWorkflowEvent::StepCompleted {
                node_name,
                sequence,
            } => {
                write!(f, "StepCompleted(node={node_name}, seq={sequence})")
            }
            SseWorkflowEvent::StepFailed {
                node_name,
                sequence,
                error,
            } => {
                write!(
                    f,
                    "StepFailed(node={node_name}, seq={sequence}, error={error})"
                )
            }
            SseWorkflowEvent::TimerFired { timer_id } => {
                write!(f, "TimerFired(id={timer_id})")
            }
            SseWorkflowEvent::SignalReceived { signal_name } => {
                write!(f, "SignalReceived(name={signal_name})")
            }
            SseWorkflowEvent::PhaseChanged { phase } => {
                write!(f, "PhaseChanged(phase={phase})")
            }
            SseWorkflowEvent::InstanceCompleted => {
                write!(f, "InstanceCompleted")
            }
            SseWorkflowEvent::InstanceFailed { error } => {
                write!(f, "InstanceFailed(error={error})")
            }
        }
    }
}

/// The event type label used in SSE payloads.
impl SseWorkflowEvent {
    /// Returns the event type string (e.g., "step_completed").
    #[must_use]
    pub fn event_type(&self) -> &'static str {
        match self {
            SseWorkflowEvent::StepCompleted { .. } => "step_completed",
            SseWorkflowEvent::StepFailed { .. } => "step_failed",
            SseWorkflowEvent::TimerFired { .. } => "timer_fired",
            SseWorkflowEvent::SignalReceived { .. } => "signal_received",
            SseWorkflowEvent::PhaseChanged { .. } => "phase_changed",
            SseWorkflowEvent::InstanceCompleted => "instance_completed",
            SseWorkflowEvent::InstanceFailed { .. } => "instance_failed",
        }
    }

    /// Returns the node name if this event is node-associated.
    #[must_use]
    pub fn node_name(&self) -> Option<&str> {
        match self {
            SseWorkflowEvent::StepCompleted { node_name, .. }
            | SseWorkflowEvent::StepFailed { node_name, .. } => Some(node_name),
            _ => None,
        }
    }

    /// Returns true if this event represents a terminal workflow state.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            SseWorkflowEvent::InstanceCompleted | SseWorkflowEvent::InstanceFailed { .. }
        )
    }
}

// ============================================================================
// Node Status Updates
// ============================================================================

/// A status update derived from an SSE event, targeting a specific workflow node.
/// This is the bridge between raw SSE events and the frontend node state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeStatusUpdate {
    /// The node name that changed status.
    pub node_name: String,
    /// The new execution state.
    pub new_state: ExecutionState,
    /// Optional error message (for failures).
    pub error: Option<String>,
    /// The sequence number from the SSE event, for ordering.
    pub sequence: u64,
}

/// Execution state that maps to the frontend's `ExecutionState` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionState {
    Idle,
    Running,
    Queued,
    Completed,
    Failed,
    Skipped,
}

impl Default for ExecutionState {
    fn default() -> Self {
        Self::Idle
    }
}

impl fmt::Display for ExecutionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionState::Idle => write!(f, "idle"),
            ExecutionState::Running => write!(f, "running"),
            ExecutionState::Queued => write!(f, "queued"),
            ExecutionState::Completed => write!(f, "completed"),
            ExecutionState::Failed => write!(f, "failed"),
            ExecutionState::Skipped => write!(f, "skipped"),
        }
    }
}

impl ExecutionState {
    /// Returns the CSS status badge class for this state.
    #[must_use]
    pub const fn badge_class(&self) -> &'static str {
        match self {
            ExecutionState::Idle | ExecutionState::Queued => {
                "bg-slate-100 text-slate-700 border-slate-200"
            }
            ExecutionState::Running => "bg-blue-100 text-blue-700 border-blue-200",
            ExecutionState::Completed => "bg-green-100 text-green-700 border-green-200",
            ExecutionState::Failed => "bg-red-100 text-red-700 border-red-200",
            ExecutionState::Skipped => "bg-slate-100 text-slate-500 border-slate-200",
        }
    }

    /// Returns the display label for this state.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            ExecutionState::Idle | ExecutionState::Queued => "pending",
            ExecutionState::Running => "running",
            ExecutionState::Completed => "completed",
            ExecutionState::Failed => "failed",
            ExecutionState::Skipped => "skipped",
        }
    }
}

impl From<&SseWorkflowEvent> for Option<NodeStatusUpdate> {
    fn from(event: &SseWorkflowEvent) -> Self {
        match event {
            SseWorkflowEvent::StepCompleted {
                node_name,
                sequence,
            } => Some(NodeStatusUpdate {
                node_name: node_name.clone(),
                new_state: ExecutionState::Completed,
                error: None,
                sequence: *sequence,
            }),
            SseWorkflowEvent::StepFailed {
                node_name,
                sequence,
                error,
            } => Some(NodeStatusUpdate {
                node_name: node_name.clone(),
                new_state: ExecutionState::Failed,
                error: Some(error.clone()),
                sequence: *sequence,
            }),
            _ => None,
        }
    }
}

// ============================================================================
// SSE Connection Configuration
// ============================================================================

/// Configuration for an SSE connection to the workflow watch endpoint.
#[derive(Debug, Clone)]
pub struct SseConfig {
    /// Base URL of the vo-api server (e.g., "http://localhost:8080").
    pub base_url: String,
    /// Workflow instance ID in format "namespace/instance_id".
    pub instance_id: String,
    /// Maximum reconnect delay (exponential backoff cap).
    pub max_reconnect_delay: Duration,
    /// Initial reconnect delay.
    pub initial_reconnect_delay: Duration,
}

impl SseConfig {
    /// Creates a new SSE config with default reconnect settings.
    #[must_use]
    pub fn new(base_url: String, instance_id: String) -> Self {
        Self {
            base_url,
            instance_id,
            max_reconnect_delay: Duration::from_secs(30),
            initial_reconnect_delay: Duration::from_millis(500),
        }
    }

    /// Returns the full SSE endpoint URL.
    #[must_use]
    pub fn endpoint_url(&self) -> String {
        format!("{}/api/v1/watch/{}", self.base_url, self.instance_id)
    }
}

impl Default for SseConfig {
    fn default() -> Self {
        Self::new("http://localhost:8080".to_string(), "default/00000000000000000000000000".to_string())
    }
}

// ============================================================================
// SSE Connection Status
// ============================================================================

/// The current status of the SSE connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SseConnectionStatus {
    /// Not connected — initial state or after disconnect.
    Disconnected,
    /// Actively receiving events.
    Connected,
    /// Attempting to reconnect after a disconnect.
    Reconnecting,
}

impl fmt::Display for SseConnectionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SseConnectionStatus::Disconnected => write!(f, "disconnected"),
            SseConnectionStatus::Connected => write!(f, "connected"),
            SseConnectionStatus::Reconnecting => write!(f, "reconnecting"),
        }
    }
}

// ============================================================================
// Event Processing
// ============================================================================

/// Attempts to parse an SSE event payload (JSON string) into a typed event.
pub fn parse_sse_event(payload: &str) -> Option<SseWorkflowEvent> {
    serde_json::from_str(payload).ok()
}

/// Attempts to parse a raw SSE data line into a NodeStatusUpdate.
pub fn parse_node_update(payload: &str) -> Option<NodeStatusUpdate> {
    let event = parse_sse_event(payload)?;
    NodeStatusUpdate::from(&event)
}

/// Extracts the node name and new state from an SSE event, if applicable.
pub fn event_to_status_change(event: &SseWorkflowEvent) -> Option<(String, ExecutionState)> {
    match event {
        SseWorkflowEvent::StepCompleted {
            node_name,
            sequence: _,
        } => Some((node_name.clone(), ExecutionState::Completed)),
        SseWorkflowEvent::StepFailed {
            node_name,
            sequence: _,
            error: _,
        } => Some((node_name.clone(), ExecutionState::Failed)),
        _ => None,
    }
}

// ============================================================================
// Backoff Strategy
// ============================================================================

/// Calculates the reconnect delay using exponential backoff with deterministic jitter.
///
/// Uses the formula: `initial * 2^attempt + jitter`, capped at `max_delay`.
/// The jitter is +/- 25% of the base delay, derived from `attempt` for determinism.
#[must_use]
pub fn calculate_backoff_delay(attempt: u32, initial: Duration, max: Duration) -> Duration {
    let base = initial.saturating_mul(2u32.pow(attempt));
    let jitter_factor = if attempt % 2 == 0 { 1i64 } else { -1i64 };
    let jitter_amount = (base.as_millis() as i64 / 4) * jitter_factor;
    let jittered_ms = (base.as_millis() as i64).saturating_add(jitter_amount).max(0) as u64;
    Duration::from_millis(jittered_ms).min(max)
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_step_completed_when_event_type_then_returns_correct_label() {
        let event = SseWorkflowEvent::StepCompleted {
            node_name: "build".to_string(),
            sequence: 42,
        };
        assert_eq!(event.event_type(), "step_completed");
    }

    #[test]
    fn given_step_failed_when_event_type_then_returns_correct_label() {
        let event = SseWorkflowEvent::StepFailed {
            node_name: "test".to_string(),
            sequence: 3,
            error: "timeout".to_string(),
        };
        assert_eq!(event.event_type(), "step_failed");
    }

    #[test]
    fn given_instance_completed_when_is_terminal_then_returns_true() {
        let event = SseWorkflowEvent::InstanceCompleted;
        assert!(event.is_terminal());
    }

    #[test]
    fn given_instance_failed_when_is_terminal_then_returns_true() {
        let event = SseWorkflowEvent::InstanceFailed {
            error: "oom".to_string(),
        };
        assert!(event.is_terminal());
    }

    #[test]
    fn given_step_completed_when_is_terminal_then_returns_false() {
        let event = SseWorkflowEvent::StepCompleted {
            node_name: "step-1".to_string(),
            sequence: 1,
        };
        assert!(!event.is_terminal());
    }

    #[test]
    fn given_step_completed_when_node_name_then_returns_node() {
        let event = SseWorkflowEvent::StepCompleted {
            node_name: "deploy".to_string(),
            sequence: 5,
        };
        assert_eq!(event.node_name(), Some("deploy"));
    }

    #[test]
    fn given_timer_fired_when_node_name_then_returns_none() {
        let event = SseWorkflowEvent::TimerFired {
            timer_id: "timer-1".to_string(),
        };
        assert_eq!(event.node_name(), None);
    }

    #[test]
    fn given_json_payload_when_parse_then_step_completed_parses() {
        let json = r#"{"type":"step_completed","node_name":"build","sequence":42}"#;
        let event = parse_sse_event(json);
        assert!(event.is_some());
        let event = event.unwrap();
        assert!(matches!(
            event,
            SseWorkflowEvent::StepCompleted {
                node_name,
                sequence
            } if node_name == "build" && sequence == 42
        ));
    }

    #[test]
    fn given_json_payload_when_parse_then_step_failed_parses() {
        let json =
            r#"{"type":"step_failed","node_name":"test","sequence":3,"error":"timeout"}"#;
        let event = parse_sse_event(json);
        assert!(event.is_some());
        let event = event.unwrap();
        assert!(matches!(
            event,
            SseWorkflowEvent::StepFailed {
                node_name,
                sequence,
                error
            } if node_name == "test" && sequence == 3 && error == "timeout"
        ));
    }

    #[test]
    fn given_json_payload_when_parse_then_instance_completed_parses() {
        let json = r#"{"type":"instance_completed"}"#;
        let event = parse_sse_event(json);
        assert!(event.is_some());
        assert!(matches!(
            event.unwrap(),
            SseWorkflowEvent::InstanceCompleted
        ));
    }

    #[test]
    fn given_json_payload_when_parse_then_instance_failed_parses() {
        let json = r#"{"type":"instance_failed","error":"disk full"}"#;
        let event = parse_sse_event(json);
        assert!(event.is_some());
        let event = event.unwrap();
        assert!(matches!(
            event,
            SseWorkflowEvent::InstanceFailed { ref error } if error == "disk full"
        ));
    }

    #[test]
    fn given_json_payload_when_parse_then_phase_changed_parses() {
        let json = r#"{"type":"phase_changed","phase":"live"}"#;
        let event = parse_sse_event(json);
        assert!(event.is_some());
        assert!(matches!(
            event.unwrap(),
            SseWorkflowEvent::PhaseChanged { ref phase } if phase == "live"
        ));
    }

    #[test]
    fn given_step_completed_when_converted_to_node_update_then_state_is_completed() {
        let event = SseWorkflowEvent::StepCompleted {
            node_name: "build".to_string(),
            sequence: 10,
        };
        let update = NodeStatusUpdate::from(&event);
        assert!(update.is_some());
        let update = update.unwrap();
        assert_eq!(update.node_name, "build");
        assert_eq!(update.new_state, ExecutionState::Completed);
        assert!(update.error.is_none());
        assert_eq!(update.sequence, 10);
    }

    #[test]
    fn given_step_failed_when_converted_to_node_update_then_state_is_failed() {
        let event = SseWorkflowEvent::StepFailed {
            node_name: "deploy".to_string(),
            sequence: 15,
            error: "connection refused".to_string(),
        };
        let update = NodeStatusUpdate::from(&event);
        assert!(update.is_some());
        let update = update.unwrap();
        assert_eq!(update.node_name, "deploy");
        assert_eq!(update.new_state, ExecutionState::Failed);
        assert_eq!(update.error, Some("connection refused".to_string()));
    }

    #[test]
    fn given_timer_fired_when_converted_to_node_update_then_returns_none() {
        let event = SseWorkflowEvent::TimerFired {
            timer_id: "t1".to_string(),
        };
        let update = NodeStatusUpdate::from(&event);
        assert!(update.is_none());
    }

    #[test]
    fn given_execution_state_when_badge_class_then_returns_correct_css() {
        assert!(ExecutionState::Running.badge_class().contains("blue"));
        assert!(ExecutionState::Completed.badge_class().contains("green"));
        assert!(ExecutionState::Failed.badge_class().contains("red"));
        assert!(
            ExecutionState::Idle.badge_class().contains("slate")
                || ExecutionState::Queued.badge_class().contains("slate")
        );
    }

    #[test]
    fn given_execution_state_when_label_then_returns_lowercase_label() {
        assert_eq!(ExecutionState::Running.label(), "running");
        assert_eq!(ExecutionState::Completed.label(), "completed");
        assert_eq!(ExecutionState::Failed.label(), "failed");
        assert_eq!(ExecutionState::Idle.label(), "pending");
        assert_eq!(ExecutionState::Queued.label(), "pending");
        assert_eq!(ExecutionState::Skipped.label(), "skipped");
    }

    #[test]
    fn given_sse_config_when_endpoint_url_then_returns_correct_path() {
        let config = SseConfig::new(
            "http://localhost:8080".to_string(),
            "payments/01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
        );
        assert_eq!(
            config.endpoint_url(),
            "http://localhost:8080/api/v1/watch/payments/01ARZ3NDEKTSV4RRFFQ69G5FAV"
        );
    }

    #[test]
    fn given_backoff_attempt_0_when_calculate_then_returns_initial() {
        let initial = Duration::from_millis(500);
        let max = Duration::from_secs(30);
        let delay = calculate_backoff_delay(0, initial, max);
        // With jitter, delay should be between initial - initial/4 and initial + initial/4
        let lower = initial.saturating_sub(initial / 4);
        let upper = initial + (initial / 4);
        assert!(delay >= lower && delay <= upper, "delay={delay:?}");
    }

    #[test]
    fn given_backoff_attempt_5_when_calculate_then_exponential_growth() {
        let initial = Duration::from_millis(500);
        let max = Duration::from_secs(30);
        let delay = calculate_backoff_delay(5, initial, max);
        // 500 * 2^5 = 16000ms, with jitter +/- 25%
        let base = initial.saturating_mul(32);
        let jitter = base / 4;
        let expected_min = base.saturating_sub(jitter);
        assert!(delay >= expected_min, "delay should grow exponentially: {delay:?}");
        assert!(delay <= max, "delay should be capped at max: {delay:?}");
    }

    #[test]
    fn given_backoff_exceeds_max_when_calculate_then_returns_max() {
        let initial = Duration::from_millis(500);
        let max = Duration::from_millis(1000);
        let delay = calculate_backoff_delay(10, initial, max);
        assert_eq!(delay, max);
    }

    #[test]
    fn given_node_status_update_display_when_completed_then_shows_state() {
        let event = SseWorkflowEvent::StepCompleted {
            node_name: "build".to_string(),
            sequence: 1,
        };
        let display = format!("{event}");
        assert!(display.contains("StepCompleted"));
        assert!(display.contains("build"));
    }

    #[test]
    fn given_all_event_types_when_event_type_then_all_labels_match() {
        let events: Vec<SseWorkflowEvent> = vec![
            SseWorkflowEvent::StepCompleted {
                node_name: "a".to_string(),
                sequence: 1,
            },
            SseWorkflowEvent::StepFailed {
                node_name: "b".to_string(),
                sequence: 2,
                error: "err".to_string(),
            },
            SseWorkflowEvent::TimerFired {
                timer_id: "t1".to_string(),
            },
            SseWorkflowEvent::SignalReceived {
                signal_name: "sig1".to_string(),
            },
            SseWorkflowEvent::PhaseChanged {
                phase: "live".to_string(),
            },
            SseWorkflowEvent::InstanceCompleted,
            SseWorkflowEvent::InstanceFailed {
                error: "fail".to_string(),
            },
        ];

        let expected: &[&str] = &[
            "step_completed",
            "step_failed",
            "timer_fired",
            "signal_received",
            "phase_changed",
            "instance_completed",
            "instance_failed",
        ];

        for (event, &expected_label) in events.iter().zip(expected.iter()) {
            assert_eq!(event.event_type(), expected_label);
        }
    }

    #[test]
    fn given_invalid_json_when_parse_sse_event_then_returns_none() {
        let result = parse_sse_event("not json at all");
        assert!(result.is_none());
    }

    #[test]
    fn given_unknown_type_when_parse_sse_event_then_returns_none() {
        let result = parse_sse_event(r#"{"type":"unknown_event"}"#);
        assert!(result.is_none());
    }

    #[test]
    fn given_event_to_status_change_when_step_completed_then_returns_node_and_state() {
        let event = SseWorkflowEvent::StepCompleted {
            node_name: "compile".to_string(),
            sequence: 5,
        };
        let result = event_to_status_change(&event);
        assert!(result.is_some());
        let (name, state) = result.unwrap();
        assert_eq!(name, "compile");
        assert_eq!(state, ExecutionState::Completed);
    }

    #[test]
    fn given_event_to_status_change_when_timer_fired_then_returns_none() {
        let event = SseWorkflowEvent::TimerFired {
            timer_id: "t1".to_string(),
        };
        assert!(event_to_status_change(&event).is_none());
    }

    #[test]
    fn given_event_to_status_change_when_instance_completed_then_returns_none() {
        let event = SseWorkflowEvent::InstanceCompleted;
        assert!(event_to_status_change(&event).is_none());
    }
}
