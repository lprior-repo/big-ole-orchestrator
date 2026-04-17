//! Common utilities and types for vo-engine.
//!
//! Shared functionality used across multiple crates including
//! type aliases and common event definitions.

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type InstanceId = String;
pub type NamespaceId = String;
pub type TimerId = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkflowEvent {
    TimerFired {
        timer_id: String,
        timestamp_ms: u64,
    },
    TaskCompleted {
        task_id: String,
        result_json: String,
    },
    TaskFailed {
        task_id: String,
        error: String,
    },
    SignalReceived {
        signal_name: String,
        payload_json: String,
    },
    WorkflowStarted {
        workflow_id: String,
        input_json: String,
    },
    WorkflowCompleted {
        workflow_id: String,
        result_json: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VoError {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("internal error: {0}")]
    Internal(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("operation timed out: {0}")]
    Timeout(String),
}

impl VoError {
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    pub fn timeout(msg: impl Into<String>) -> Self {
        Self::Timeout(msg.into())
    }
}

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
    fn vo_error_config_constructs() {
        let err = VoError::config("bad config");
        assert!(matches!(err, VoError::Config(msg) if msg == "bad config"));
    }

    #[test]
    fn vo_error_internal_constructs() {
        let err = VoError::internal("oops");
        assert!(matches!(err, VoError::Internal(msg) if msg == "oops"));
    }

    #[test]
    fn vo_error_not_found_constructs() {
        let err = VoError::not_found("missing");
        assert!(matches!(err, VoError::NotFound(msg) if msg == "missing"));
    }

    #[test]
    fn vo_error_validation_constructs() {
        let err = VoError::validation("invalid");
        assert!(matches!(err, VoError::Validation(msg) if msg == "invalid"));
    }

    #[test]
    fn vo_error_timeout_constructs() {
        let err = VoError::timeout("30s");
        assert!(matches!(err, VoError::Timeout(msg) if msg == "30s"));
    }

    #[test]
    fn vo_error_displays_message() {
        let err = VoError::Internal("something went wrong".to_string());
        let msg = err.to_string();
        assert!(msg.contains("something went wrong"));
    }

    #[test]
    fn workflow_event_timer_fired_construction() {
        let event = WorkflowEvent::TimerFired {
            timer_id: "timer-abc".into(),
            timestamp_ms: 1234567890,
        };
        if let WorkflowEvent::TimerFired {
            timer_id,
            timestamp_ms,
        } = event
        {
            assert_eq!(timer_id, "timer-abc");
            assert_eq!(timestamp_ms, 1234567890);
        } else {
            panic!("expected TimerFired variant");
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
        if let WorkflowEvent::TimerFired {
            timer_id,
            timestamp_ms,
        } = event
        {
            assert_eq!(timer_id, "t1");
            assert_eq!(timestamp_ms, 42);
        } else {
            panic!("expected TimerFired variant");
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

    #[test]
    fn task_completed_construction() {
        let event = WorkflowEvent::TaskCompleted {
            task_id: "task-123".into(),
            result_json: r#"{"status":"success"}"#.into(),
        };
        match event {
            WorkflowEvent::TaskCompleted {
                task_id,
                result_json,
            } => {
                assert_eq!(task_id, "task-123");
                assert_eq!(result_json, r#"{"status":"success"}"#);
            }
            _ => panic!("expected TaskCompleted variant"),
        }
    }

    #[test]
    fn task_failed_construction() {
        let event = WorkflowEvent::TaskFailed {
            task_id: "task-456".into(),
            error: "connection timeout".into(),
        };
        match event {
            WorkflowEvent::TaskFailed { task_id, error } => {
                assert_eq!(task_id, "task-456");
                assert_eq!(error, "connection timeout");
            }
            _ => panic!("expected TaskFailed variant"),
        }
    }

    #[test]
    fn signal_received_construction() {
        let event = WorkflowEvent::SignalReceived {
            signal_name: "pause".into(),
            payload_json: r#"{"reason":"maintenance"}"#.into(),
        };
        match event {
            WorkflowEvent::SignalReceived {
                signal_name,
                payload_json,
            } => {
                assert_eq!(signal_name, "pause");
                assert_eq!(payload_json, r#"{"reason":"maintenance"}"#);
            }
            _ => panic!("expected SignalReceived variant"),
        }
    }

    #[test]
    fn workflow_started_construction() {
        let event = WorkflowEvent::WorkflowStarted {
            workflow_id: "wf-789".into(),
            input_json: r#"{"name":"test"}"#.into(),
        };
        match event {
            WorkflowEvent::WorkflowStarted {
                workflow_id,
                input_json,
            } => {
                assert_eq!(workflow_id, "wf-789");
                assert_eq!(input_json, r#"{"name":"test"}"#);
            }
            _ => panic!("expected WorkflowStarted variant"),
        }
    }

    #[test]
    fn workflow_completed_construction() {
        let event = WorkflowEvent::WorkflowCompleted {
            workflow_id: "wf-abc".into(),
            result_json: r#"{"duration_ms":5000}"#.into(),
        };
        match event {
            WorkflowEvent::WorkflowCompleted {
                workflow_id,
                result_json,
            } => {
                assert_eq!(workflow_id, "wf-abc");
                assert_eq!(result_json, r#"{"duration_ms":5000}"#);
            }
            _ => panic!("expected WorkflowCompleted variant"),
        }
    }

    #[test]
    fn task_completed_json_serialization() {
        let event = WorkflowEvent::TaskCompleted {
            task_id: "task-tasks".into(),
            result_json: r#"{"output":"data"}"#.into(),
        };
        let json = serde_json::to_string(&event).expect("should serialize");
        let deserialized: WorkflowEvent = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(event, deserialized);
    }

    #[test]
    fn task_failed_json_serialization() {
        let event = WorkflowEvent::TaskFailed {
            task_id: "task-fail".into(),
            error: "panic occurred".into(),
        };
        let json = serde_json::to_string(&event).expect("should serialize");
        let deserialized: WorkflowEvent = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(event, deserialized);
    }

    #[test]
    fn signal_received_json_serialization() {
        let event = WorkflowEvent::SignalReceived {
            signal_name: "resume".into(),
            payload_json: r#"{"force":true}"#.into(),
        };
        let json = serde_json::to_string(&event).expect("should serialize");
        let deserialized: WorkflowEvent = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(event, deserialized);
    }

    #[test]
    fn workflow_started_json_serialization() {
        let event = WorkflowEvent::WorkflowStarted {
            workflow_id: "wf-start".into(),
            input_json: r#"{"args":[1,2,3]}"#.into(),
        };
        let json = serde_json::to_string(&event).expect("should serialize");
        let deserialized: WorkflowEvent = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(event, deserialized);
    }

    #[test]
    fn workflow_completed_json_serialization() {
        let event = WorkflowEvent::WorkflowCompleted {
            workflow_id: "wf-end".into(),
            result_json: r#"{"success":true}"#.into(),
        };
        let json = serde_json::to_string(&event).expect("should serialize");
        let deserialized: WorkflowEvent = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(event, deserialized);
    }

    #[test]
    fn workflow_event_all_variants_serialization() {
        let events = vec![
            WorkflowEvent::TimerFired {
                timer_id: "t1".into(),
                timestamp_ms: 100,
            },
            WorkflowEvent::TaskCompleted {
                task_id: "t2".into(),
                result_json: "{}".into(),
            },
            WorkflowEvent::TaskFailed {
                task_id: "t3".into(),
                error: "err".into(),
            },
            WorkflowEvent::SignalReceived {
                signal_name: "s1".into(),
                payload_json: "{}".into(),
            },
            WorkflowEvent::WorkflowStarted {
                workflow_id: "w1".into(),
                input_json: "{}".into(),
            },
            WorkflowEvent::WorkflowCompleted {
                workflow_id: "w2".into(),
                result_json: "{}".into(),
            },
        ];
        for event in events {
            let json = serde_json::to_string(&event).expect("should serialize");
            let deserialized: WorkflowEvent =
                serde_json::from_str(&json).expect("should deserialize");
            assert_eq!(event, deserialized);
        }
    }
}
