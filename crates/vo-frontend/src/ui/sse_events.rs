#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use serde::Deserialize;

use crate::ui::edges::graph_types::ExecutionState;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SseConnectionState {
    Connecting,
    Connected,
    Reconnecting,
    Closed,
    Error,
}

impl WorkflowSseEvent {
    #[must_use]
    pub fn node_name(&self) -> Option<&str> {
        match self {
            Self::StepCompleted { node_name, .. } | Self::StepFailed { node_name, .. } => {
                Some(node_name)
            }
            _ => None,
        }
    }

    #[must_use]
    pub fn to_execution_state(&self) -> Option<ExecutionState> {
        match self {
            Self::StepCompleted { .. } => Some(ExecutionState::Completed),
            Self::StepFailed { .. } => Some(ExecutionState::Failed),
            Self::InstanceCompleted => None,
            Self::InstanceFailed { .. } => None,
            Self::PhaseChanged { .. } => None,
            Self::TimerFired { .. } => None,
            Self::SignalReceived { .. } => None,
        }
    }
}

#[must_use]
pub fn parse_sse_data(data: &str) -> Option<WorkflowSseEvent> {
    if data.starts_with(':') {
        return None;
    }
    serde_json::from_str(data).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_step_completed() {
        let json = r#"{"type":"step_completed","node_name":"build","sequence":1}"#;
        let event = parse_sse_data(json).expect("should parse");
        assert_eq!(event.node_name(), Some("build"));
        assert_eq!(event.to_execution_state(), Some(ExecutionState::Completed));
    }

    #[test]
    fn parse_step_failed() {
        let json = r#"{"type":"step_failed","node_name":"deploy","sequence":2,"error":"timeout"}"#;
        let event = parse_sse_data(json).expect("should parse");
        assert_eq!(event.node_name(), Some("deploy"));
        assert_eq!(event.to_execution_state(), Some(ExecutionState::Failed));
    }

    #[test]
    fn parse_timer_fired() {
        let json = r#"{"type":"timer_fired","timer_id":"t1"}"#;
        let event = parse_sse_data(json).expect("should parse");
        assert_eq!(event.node_name(), None);
        assert_eq!(event.to_execution_state(), None);
    }

    #[test]
    fn parse_signal_received() {
        let json = r#"{"type":"signal_received","signal_name":"cancel"}"#;
        let event = parse_sse_data(json).expect("should parse");
        assert_eq!(event.node_name(), None);
    }

    #[test]
    fn parse_phase_changed() {
        let json = r#"{"type":"phase_changed","phase":"live"}"#;
        let event = parse_sse_data(json).expect("should parse");
        assert_eq!(event.node_name(), None);
    }

    #[test]
    fn parse_instance_completed() {
        let json = r#"{"type":"instance_completed"}"#;
        let event = parse_sse_data(json).expect("should parse");
        assert_eq!(event.node_name(), None);
        assert_eq!(event.to_execution_state(), None);
    }

    #[test]
    fn parse_instance_failed() {
        let json = r#"{"type":"instance_failed","error":"oom"}"#;
        let event = parse_sse_data(json).expect("should parse");
        assert_eq!(event.node_name(), None);
        assert_eq!(event.to_execution_state(), None);
    }

    #[test]
    fn ignore_keepalive_comment() {
        assert_eq!(parse_sse_data(":keepalive"), None);
        assert_eq!(parse_sse_data(": ping"), None);
    }

    #[test]
    fn invalid_json_returns_none() {
        assert_eq!(parse_sse_data("not json"), None);
        assert_eq!(parse_sse_data(""), None);
    }

    #[test]
    fn step_completed_to_execution_state() {
        let event = WorkflowSseEvent::StepCompleted {
            node_name: "x".to_string(),
            sequence: 1,
        };
        assert_eq!(event.to_execution_state(), Some(ExecutionState::Completed));
    }

    #[test]
    fn step_failed_to_execution_state() {
        let event = WorkflowSseEvent::StepFailed {
            node_name: "x".to_string(),
            sequence: 1,
            error: "e".to_string(),
        };
        assert_eq!(event.to_execution_state(), Some(ExecutionState::Failed));
    }
}
