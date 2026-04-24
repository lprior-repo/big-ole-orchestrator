//! Event types for vo-common.

use serde::{Deserialize, Serialize};

use crate::types::TimerId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkflowEvent {
    TimerFired {
        timer_id: TimerId,
        timestamp_ms: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_event_timer_fired_construction() {
        let event = WorkflowEvent::TimerFired {
            timer_id: TimerId::new("timer-abc"),
            timestamp_ms: 1234567890,
        };
        match event {
            WorkflowEvent::TimerFired {
                timer_id,
                timestamp_ms,
            } => {
                assert_eq!(timer_id.as_str(), "timer-abc");
                assert_eq!(timestamp_ms, 1234567890);
            }
        }
    }

    #[test]
    fn workflow_event_json_serialization_roundtrip() {
        let event = WorkflowEvent::TimerFired {
            timer_id: TimerId::new("timer-test-123"),
            timestamp_ms: 9876543210,
        };
        let json = serde_json::to_string(&event).expect("should serialize");
        let deserialized: WorkflowEvent = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(event, deserialized);
    }

    #[test]
    fn workflow_event_json_deserialization() {
        let json = r#"{"TimerFired":{"timer_id":"t1","timestamp_ms":42}}"#;
        let event: WorkflowEvent = serde_json::from_str(json).expect("should deserialize");
        match event {
            WorkflowEvent::TimerFired {
                timer_id,
                timestamp_ms,
            } => {
                assert_eq!(timer_id.as_str(), "t1");
                assert_eq!(timestamp_ms, 42);
            }
        }
    }

    #[test]
    fn workflow_event_clone_preserves_data() {
        let event = WorkflowEvent::TimerFired {
            timer_id: TimerId::new("timer-clone-test"),
            timestamp_ms: 1111111111,
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }
}
