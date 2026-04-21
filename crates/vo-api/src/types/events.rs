use axum::response::sse::Event;

#[derive(Debug, Clone)]
pub enum WorkflowEvent {
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

impl WorkflowEvent {
    pub fn to_sse_event(&self) -> Event {
        let data = match self {
            WorkflowEvent::StepCompleted {
                node_name,
                sequence,
            } => {
                serde_json::json!({
                    "type": "step_completed",
                    "node_name": node_name,
                    "sequence": sequence,
                })
            }
            WorkflowEvent::StepFailed {
                node_name,
                sequence,
                error,
            } => {
                serde_json::json!({
                    "type": "step_failed",
                    "node_name": node_name,
                    "sequence": sequence,
                    "error": error,
                })
            }
            WorkflowEvent::TimerFired { timer_id } => {
                serde_json::json!({
                    "type": "timer_fired",
                    "timer_id": timer_id,
                })
            }
            WorkflowEvent::SignalReceived { signal_name } => {
                serde_json::json!({
                    "type": "signal_received",
                    "signal_name": signal_name,
                })
            }
            WorkflowEvent::PhaseChanged { phase } => {
                serde_json::json!({
                    "type": "phase_changed",
                    "phase": phase,
                })
            }
            WorkflowEvent::InstanceCompleted => {
                serde_json::json!({
                    "type": "instance_completed",
                })
            }
            WorkflowEvent::InstanceFailed { error } => {
                serde_json::json!({
                    "type": "instance_failed",
                    "error": error,
                })
            }
        };
        Event::default()
            .event("workflow-event")
            .data(data.to_string())
    }

    pub fn to_json_string(&self) -> String {
        let data = match self {
            WorkflowEvent::StepCompleted {
                node_name,
                sequence,
            } => {
                serde_json::json!({
                    "type": "step_completed",
                    "node_name": node_name,
                    "sequence": sequence,
                })
            }
            WorkflowEvent::StepFailed {
                node_name,
                sequence,
                error,
            } => {
                serde_json::json!({
                    "type": "step_failed",
                    "node_name": node_name,
                    "sequence": sequence,
                    "error": error,
                })
            }
            WorkflowEvent::TimerFired { timer_id } => {
                serde_json::json!({
                    "type": "timer_fired",
                    "timer_id": timer_id,
                })
            }
            WorkflowEvent::SignalReceived { signal_name } => {
                serde_json::json!({
                    "type": "signal_received",
                    "signal_name": signal_name,
                })
            }
            WorkflowEvent::PhaseChanged { phase } => {
                serde_json::json!({
                    "type": "phase_changed",
                    "phase": phase,
                })
            }
            WorkflowEvent::InstanceCompleted => {
                serde_json::json!({
                    "type": "instance_completed",
                })
            }
            WorkflowEvent::InstanceFailed { error } => {
                serde_json::json!({
                    "type": "instance_failed",
                    "error": error,
                })
            }
        };
        data.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_completed_to_sse_event() {
        let event = WorkflowEvent::StepCompleted {
            node_name: "build-step".to_string(),
            sequence: 42,
        };
        let _sse = event.to_sse_event();
    }

    #[test]
    fn step_completed_to_json_string() {
        let event = WorkflowEvent::StepCompleted {
            node_name: "build-step".to_string(),
            sequence: 42,
        };
        let json = event.to_json_string();
        assert!(json.contains("\"type\":\"step_completed\""));
        assert!(json.contains("\"node_name\":\"build-step\""));
        assert!(json.contains("\"sequence\":42"));
    }

    #[test]
    fn timer_fired_to_sse_event() {
        let event = WorkflowEvent::TimerFired {
            timer_id: "timer-123".to_string(),
        };
        let _sse = event.to_sse_event();
    }

    #[test]
    fn timer_fired_to_json_string() {
        let event = WorkflowEvent::TimerFired {
            timer_id: "timer-123".to_string(),
        };
        let json = event.to_json_string();
        assert!(json.contains("\"type\":\"timer_fired\""));
        assert!(json.contains("\"timer_id\":\"timer-123\""));
    }
}
