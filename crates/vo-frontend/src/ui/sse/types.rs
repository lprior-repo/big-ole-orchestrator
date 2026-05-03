//! SSE event types for workflow real-time updates.
//!
//! These types mirror [`WorkflowSseEvent`] from `vo_api::handlers::sse` and represent
//! the event payloads sent over the SSE endpoint at `/api/v1/watch/{instance_id}`.
//!
//! The frontend consumes these events and updates the DAG visualization reactively
//! as node status changes, completions, and failures arrive.

use serde::{Deserialize, Serialize};

use crate::ui::edges::graph_types::ExecutionState;

// ── WorkflowSseEvent ─────────────────────────────────────────────────────────

/// An SSE event emitted by the vo-api SSE handler during workflow execution.
///
/// Mirrors `WorkflowSseEvent` from `vo_api::handlers::sse`.
/// Events are received as JSON payloads with a `"type"` field that identifies
/// the variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkflowSseEvent {
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

impl WorkflowSseEvent {
    /// Returns the human-readable label for this event type.
    #[must_use]
    pub fn event_label(&self) -> &'static str {
        match self {
            Self::StepCompleted { .. } => "step_completed",
            Self::StepFailed { .. } => "step_failed",
            Self::TimerFired { .. } => "timer_fired",
            Self::SignalReceived { .. } => "signal_received",
            Self::PhaseChanged { .. } => "phase_changed",
            Self::InstanceCompleted => "instance_completed",
            Self::InstanceFailed { .. } => "instance_failed",
        }
    }

    /// Returns the node name if this event is associated with a specific node.
    #[must_use]
    pub fn node_name(&self) -> Option<&str> {
        match self {
            Self::StepCompleted { node_name, .. } | Self::StepFailed { node_name, .. } => {
                Some(node_name)
            }
            Self::TimerFired { .. }
            | Self::SignalReceived { .. }
            | Self::PhaseChanged { .. }
            | Self::InstanceCompleted
            | Self::InstanceFailed { .. } => None,
        }
    }

    /// Returns the execution state to apply to a node for this event.
    #[must_use]
    pub fn node_state(&self) -> Option<ExecutionState> {
        match self {
            Self::StepCompleted { .. } => Some(ExecutionState::Completed),
            Self::StepFailed { .. } => Some(ExecutionState::Failed),
            Self::TimerFired { .. }
            | Self::SignalReceived { .. }
            | Self::PhaseChanged { .. }
            | Self::InstanceCompleted
            | Self::InstanceFailed { .. } => None,
        }
    }

    /// Returns the workflow-level state change, if any.
    #[must_use]
    pub fn instance_state(&self) -> Option<WorkflowInstanceState> {
        match self {
            Self::InstanceCompleted => Some(WorkflowInstanceState::Completed),
            Self::InstanceFailed { .. } => Some(WorkflowInstanceState::Failed),
            Self::PhaseChanged { phase } => Some(WorkflowInstanceState::Phase(phase.clone())),
            Self::StepCompleted { .. }
            | Self::StepFailed { .. }
            | Self::TimerFired { .. }
            | Self::SignalReceived { .. } => None,
        }
    }

    /// Check if this event indicates the workflow instance has finished.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::InstanceCompleted | Self::InstanceFailed { .. })
    }
}

// ── WorkflowInstanceState ────────────────────────────────────────────────────

/// The high-level state of a workflow instance, as reported by SSE events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkflowInstanceState {
    Phase(String),
    Completed,
    Failed,
}

impl WorkflowInstanceState {
    /// Returns a human-readable status label.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Phase(phase) => format!("phase:{phase}"),
            Self::Completed => "completed".to_string(),
            Self::Failed => "failed".to_string(),
        }
    }
}

// ── NodeStateDelta ───────────────────────────────────────────────────────────

/// A delta describing how a node's state should be updated from an SSE event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeStateDelta {
    pub node_name: String,
    pub new_state: ExecutionState,
    pub error: Option<String>,
    pub sequence: u64,
}

impl NodeStateDelta {
    #[must_use]
    pub fn from_event(event: &WorkflowSseEvent) -> Option<Self> {
        match event {
            WorkflowSseEvent::StepCompleted {
                node_name,
                sequence,
            } => Some(NodeStateDelta {
                node_name: node_name.clone(),
                new_state: ExecutionState::Completed,
                error: None,
                sequence: *sequence,
            }),
            WorkflowSseEvent::StepFailed {
                node_name,
                sequence,
                error,
            } => Some(NodeStateDelta {
                node_name: node_name.clone(),
                new_state: ExecutionState::Failed,
                error: Some(error.clone()),
                sequence: *sequence,
            }),
            _ => None,
        }
    }
}

// ── WorkflowEventLog ─────────────────────────────────────────────────────────

/// A chronologically ordered log of SSE events for a workflow instance.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkflowEventLog {
    events: Vec<LoggedEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LoggedEvent {
    event: WorkflowSseEvent,
    received_at: u64,
}

impl WorkflowEventLog {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append an event to the log.
    pub fn append(&mut self, event: WorkflowSseEvent) {
        // Use a monotonic counter as timestamp placeholder (real impl would use std::time)
        let timestamp = self.events.len() as u64;
        self.events.push(LoggedEvent {
            event,
            received_at: timestamp,
        });
    }

    /// Returns the number of events in the log.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Returns true if the log is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Returns all events in the log.
    #[must_use]
    pub fn events(&self) -> Vec<WorkflowSseEvent> {
        self.events.iter().map(|e| e.event.clone()).collect()
    }

    /// Returns the last event, if any.
    #[must_use]
    pub fn last_event(&self) -> Option<&WorkflowSseEvent> {
        self.events.last().map(|e| &e.event)
    }
}

// ── SSE Connection Status ────────────────────────────────────────────────────

/// The connection status of an SSE stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SseConnectionStatus {
    Connecting,
    Connected,
    Disconnected,
    Error(String),
}

impl Default for SseConnectionStatus {
    fn default() -> Self {
        Self::Connecting
    }
}

impl SseConnectionStatus {
    #[must_use]
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected)
    }

    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    #[must_use]
    pub fn error_message(&self) -> Option<&str> {
        match self {
            Self::Error(msg) => Some(msg),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_completed_event_serializes_correctly() {
        let event = WorkflowSseEvent::StepCompleted {
            node_name: "build".to_string(),
            sequence: 42,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("step_completed"));
        assert!(json.contains("build"));
        assert!(json.contains("42"));
    }

    #[test]
    fn step_failed_event_serializes_correctly() {
        let event = WorkflowSseEvent::StepFailed {
            node_name: "test".to_string(),
            sequence: 1,
            error: "timeout".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("step_failed"));
        assert!(json.contains("timeout"));
    }

    #[test]
    fn instance_completed_deserializes() {
        let json = r#"{"type":"instance_completed"}"#;
        let event: WorkflowSseEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(event, WorkflowSseEvent::InstanceCompleted));
    }

    #[test]
    fn instance_failed_deserializes() {
        let json = r#"{"type":"instance_failed","error":"oom"}"#;
        let event: WorkflowSseEvent = serde_json::from_str(json).unwrap();
        match event {
            WorkflowSseEvent::InstanceFailed { error } => assert_eq!(error, "oom"),
            _ => panic!("expected InstanceFailed"),
        }
    }

    #[test]
    fn step_completed_node_name_returns_some() {
        let event = WorkflowSseEvent::StepCompleted {
            node_name: "deploy".to_string(),
            sequence: 5,
        };
        assert_eq!(event.node_name(), Some("deploy"));
    }

    #[test]
    fn instance_completed_node_name_returns_none() {
        let event = WorkflowSseEvent::InstanceCompleted;
        assert_eq!(event.node_name(), None);
    }

    #[test]
    fn step_completed_node_state_returns_completed() {
        let event = WorkflowSseEvent::StepCompleted {
            node_name: "build".to_string(),
            sequence: 1,
        };
        assert_eq!(event.node_state(), Some(ExecutionState::Completed));
    }

    #[test]
    fn step_failed_node_state_returns_failed() {
        let event = WorkflowSseEvent::StepFailed {
            node_name: "test".to_string(),
            sequence: 1,
            error: "fail".to_string(),
        };
        assert_eq!(event.node_state(), Some(ExecutionState::Failed));
    }

    #[test]
    fn instance_completed_is_terminal() {
        let event = WorkflowSseEvent::InstanceCompleted;
        assert!(event.is_terminal());
    }

    #[test]
    fn step_completed_not_terminal() {
        let event = WorkflowSseEvent::StepCompleted {
            node_name: "build".to_string(),
            sequence: 1,
        };
        assert!(!event.is_terminal());
    }

    #[test]
    fn instance_state_returns_correct_variant() {
        let event = WorkflowSseEvent::InstanceCompleted;
        assert!(matches!(
            event.instance_state(),
            Some(WorkflowInstanceState::Completed)
        ));
    }

    #[test]
    fn phase_changed_instance_state() {
        let event = WorkflowSseEvent::PhaseChanged {
            phase: "executing".to_string(),
        };
        assert!(matches!(
            event.instance_state(),
            Some(WorkflowInstanceState::Phase(_))
        ));
    }

    #[test]
    fn step_completed_instance_state_returns_none() {
        let event = WorkflowSseEvent::StepCompleted {
            node_name: "build".to_string(),
            sequence: 1,
        };
        assert!(event.instance_state().is_none());
    }

    #[test]
    fn node_state_delta_from_step_completed() {
        let event = WorkflowSseEvent::StepCompleted {
            node_name: "build".to_string(),
            sequence: 10,
        };
        let delta = NodeStateDelta::from_event(&event).unwrap();
        assert_eq!(delta.node_name, "build");
        assert_eq!(delta.new_state, ExecutionState::Completed);
        assert!(delta.error.is_none());
        assert_eq!(delta.sequence, 10);
    }

    #[test]
    fn node_state_delta_from_step_failed() {
        let event = WorkflowSseEvent::StepFailed {
            node_name: "test".to_string(),
            sequence: 3,
            error: "assertion failed".to_string(),
        };
        let delta = NodeStateDelta::from_event(&event).unwrap();
        assert_eq!(delta.error, Some("assertion failed".to_string()));
    }

    #[test]
    fn node_state_delta_from_phase_changed_returns_none() {
        let event = WorkflowSseEvent::PhaseChanged {
            phase: "planning".to_string(),
        };
        assert!(NodeStateDelta::from_event(&event).is_none());
    }

    #[test]
    fn event_log_appends_and_counts() {
        let mut log = WorkflowEventLog::new();
        assert!(log.is_empty());
        log.append(WorkflowSseEvent::StepCompleted {
            node_name: "build".to_string(),
            sequence: 1,
        });
        assert_eq!(log.len(), 1);
        log.append(WorkflowSseEvent::InstanceCompleted);
        assert_eq!(log.len(), 2);
    }

    #[test]
    fn event_log_last_event_returns_last() {
        let mut log = WorkflowEventLog::new();
        log.append(WorkflowSseEvent::StepCompleted {
            node_name: "a".to_string(),
            sequence: 1,
        });
        log.append(WorkflowSseEvent::StepCompleted {
            node_name: "b".to_string(),
            sequence: 2,
        });
        match log.last_event().unwrap() {
            WorkflowSseEvent::StepCompleted {
                node_name,
                sequence,
            } => {
                assert_eq!(node_name, "b");
                assert_eq!(*sequence, 2);
            }
            _ => panic!("expected StepCompleted"),
        }
    }

    #[test]
    fn sse_connection_status_defaults_to_connecting() {
        assert_eq!(
            SseConnectionStatus::default(),
            SseConnectionStatus::Connecting
        );
    }

    #[test]
    fn sse_connection_status_is_connected_only_for_connected() {
        assert!(SseConnectionStatus::Connected.is_connected());
        assert!(!SseConnectionStatus::Connecting.is_connected());
        assert!(!SseConnectionStatus::Disconnected.is_connected());
        assert!(!SseConnectionStatus::Error("fail".to_string()).is_connected());
    }

    #[test]
    fn sse_connection_status_error_message() {
        let err = SseConnectionStatus::Error("connection refused".to_string());
        assert!(err.is_error());
        assert_eq!(err.error_message(), Some("connection refused"));

        let connected = SseConnectionStatus::Connected;
        assert!(!connected.is_error());
        assert!(connected.error_message().is_none());
    }

    #[test]
    fn workflow_instance_state_label() {
        assert_eq!(WorkflowInstanceState::Completed.label(), "completed");
        assert_eq!(WorkflowInstanceState::Failed.label(), "failed");
        assert_eq!(
            WorkflowInstanceState::Phase("executing".to_string()).label(),
            "phase:executing"
        );
    }
}
