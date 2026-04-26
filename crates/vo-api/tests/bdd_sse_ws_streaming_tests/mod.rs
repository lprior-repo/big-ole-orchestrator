//! Shared SSE types and constants for BDD streaming tests.

#[derive(Debug, Clone)]
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
    InstanceFailed {
        error: String,
    },
    InstanceCompleted,
    TimerFired {
        timer_id: String,
    },
    SignalReceived {
        signal_name: String,
    },
    PhaseChanged {
        phase: String,
    },
}

impl WorkflowSseEvent {
    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            WorkflowSseEvent::StepCompleted {
                node_name,
                sequence,
            } => {
                serde_json::json!({
                    "type": "step_completed",
                    "node_name": node_name,
                    "sequence": sequence,
                })
            }
            WorkflowSseEvent::StepFailed {
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
            WorkflowSseEvent::TimerFired { timer_id } => {
                serde_json::json!({
                    "type": "timer_fired",
                    "timer_id": timer_id,
                })
            }
            WorkflowSseEvent::SignalReceived { signal_name } => {
                serde_json::json!({
                    "type": "signal_received",
                    "signal_name": signal_name,
                })
            }
            WorkflowSseEvent::PhaseChanged { phase } => {
                serde_json::json!({
                    "type": "phase_changed",
                    "phase": phase,
                })
            }
            WorkflowSseEvent::InstanceCompleted => {
                serde_json::json!({
                    "type": "instance_completed",
                })
            }
            WorkflowSseEvent::InstanceFailed { error } => {
                serde_json::json!({
                    "type": "instance_failed",
                    "error": error,
                })
            }
        }
    }
}

pub const SSE_BROADCAST_CAPACITY: usize = 1000;
pub const SSE_KEEPALIVE_INTERVAL_SECS: u64 = 15;

#[cfg(test)]
mod tests {
    use super::*;

    mod sse;
    mod ws;
    mod broadcast;
}
