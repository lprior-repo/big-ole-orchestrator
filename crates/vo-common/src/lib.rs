//! Common utilities and types for vo-engine.
//!
//! Shared functionality used across multiple crates including
//! type aliases and common event definitions.

<<<<<<< HEAD
pub mod error;
pub mod events;
pub mod structures;
pub mod types;

pub use error::VoError;
pub use events::WorkflowEvent;
pub use types::{InstanceId, NamespaceId, TimerId};
=======
use serde::{Deserialize, Serialize};

pub type InstanceId = String;
pub type NamespaceId = String;
pub type TimerId = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkflowEvent {
    TimerFired { timer_id: String, timestamp_ms: u64 },
}

pub type VoError = String;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_id_behaves_as_string() {
        let id: InstanceId = "test-instance-123".into();
        assert_eq!(id.len(), 17);
        assert_eq!(id.as_str(), "test-instance-123");
    }

    #[test]
    fn namespace_id_behaves_as_string() {
        let ns: NamespaceId = "namespace-abc".into();
        assert_eq!(ns.len(), 13);
        assert_eq!(ns.as_str(), "namespace-abc");
    }

    #[test]
    fn timer_id_behaves_as_string() {
        let timer: TimerId = "timer-xyz".into();
        assert_eq!(timer.len(), 9);
        assert_eq!(timer.as_str(), "timer-xyz");
    }

    #[test]
    fn vo_error_behaves_as_string() {
        let err: VoError = "something went wrong".into();
        assert_eq!(err.len(), 20);
        assert_eq!(err.as_str(), "something went wrong");
    }

    #[test]
    fn workflow_event_timer_fired_construction() {
        let event = WorkflowEvent::TimerFired {
            timer_id: "timer-abc".into(),
            timestamp_ms: 1234567890,
        };
        match event {
            WorkflowEvent::TimerFired {
                timer_id,
                timestamp_ms,
            } => {
                assert_eq!(timer_id, "timer-abc");
                assert_eq!(timestamp_ms, 1234567890);
            }
        }
    }

    #[test]
    fn workflow_event_json_serialization_roundtrip() {
        let event = WorkflowEvent::TimerFired {
            timer_id: "timer-test-123".into(),
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
                assert_eq!(timer_id, "t1");
                assert_eq!(timestamp_ms, 42);
            }
        }
    }

    #[test]
    fn instance_id_empty_string() {
        let id: InstanceId = "".into();
        assert_eq!(id.len(), 0);
    }

    #[test]
    fn instance_id_unicode() {
        let id: InstanceId = "实例-123-🔱".into();
        assert_eq!(id.len(), 15); // UTF-8 bytes: 6 + 1 + 3 + 1 + 4
        assert_eq!(id.as_str(), "实例-123-🔱");
    }

    #[test]
    fn workflow_event_clone_preserves_data() {
        let event = WorkflowEvent::TimerFired {
            timer_id: "timer-clone-test".into(),
            timestamp_ms: 1111111111,
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }
}
>>>>>>> origin/vo-worker-tests
