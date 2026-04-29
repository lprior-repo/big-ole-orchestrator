//! Event types for vo-common.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::types::TimerId;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WorkflowEvent {
    TimerFired {
        timer_id: TimerId,
        timestamp_ms: u64,
    },
}

pub struct EventDedup {
    seen: HashSet<TimerId>,
}

pub enum DuplicateResult {
    New,
    Duplicate(TimerId),
}

impl EventDedup {
    #[must_use]
    pub fn new() -> Self {
        Self {
            seen: HashSet::new(),
        }
    }

    pub fn check_and_track(&mut self, timer_id: TimerId) -> DuplicateResult {
        if self.seen.contains(&timer_id) {
            DuplicateResult::Duplicate(timer_id)
        } else {
            self.seen.insert(timer_id.clone());
            DuplicateResult::New
        }
    }

    pub fn track(&mut self, timer_id: TimerId) {
        self.seen.insert(timer_id);
    }

    #[must_use]
    pub fn is_duplicate(&self, timer_id: &TimerId) -> bool {
        self.seen.contains(timer_id)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.seen.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }
}

impl Default for EventDedup {
    fn default() -> Self {
        Self::new()
    }
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
        let json = r#"{"type":"TimerFired","timer_id":"t1","timestamp_ms":42}"#;
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
    fn workflow_event_internal_tag_format() {
        let event = WorkflowEvent::TimerFired {
            timer_id: TimerId::new("timer-internal"),
            timestamp_ms: 123,
        };
        let json = serde_json::to_string(&event).expect("should serialize");
        assert!(json.contains(r#""type":"TimerFired""#), "JSON should have type field: {}", json);
    }

    #[test]
    fn workflow_event_rejects_unknown_variant_with_error_message() {
        let json = r#"{"type":"UnknownVariant123","timer_id":"t1","timestamp_ms":42}"#;
        let result: Result<WorkflowEvent, _> = serde_json::from_str(json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("UnknownVariant123"),
            "error message should contain unknown variant name, got: {}",
            err_msg
        );
    }

    #[test]
    fn workflow_event_rejects_unknown_variant_short_form() {
        let json = r#"{"type":"UnknownVariant123"}"#;
        let result: Result<WorkflowEvent, _> = serde_json::from_str(json);
        assert!(result.is_err());
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

    #[test]
    fn event_dedup_new_event_accepted() {
        let mut dedup = EventDedup::new();
        assert!(matches!(dedup.check_and_track(TimerId::new("evt-a")), DuplicateResult::New));
        assert_eq!(dedup.len(), 1);
    }

    #[test]
    fn event_dedup_duplicate_detected() {
        let mut dedup = EventDedup::new();
        dedup.check_and_track(TimerId::new("evt-x"));
        let result = dedup.check_and_track(TimerId::new("evt-x"));
        assert!(matches!(result, DuplicateResult::Duplicate(_)));
        assert_eq!(dedup.len(), 1);
    }

    #[test]
    fn event_dedup_different_events_both_accepted() {
        let mut dedup = EventDedup::new();
        assert!(matches!(dedup.check_and_track(TimerId::new("evt-1")), DuplicateResult::New));
        assert!(matches!(dedup.check_and_track(TimerId::new("evt-2")), DuplicateResult::New));
        assert_eq!(dedup.len(), 2);
    }

    #[test]
    fn event_dedup_empty_string_event_id() {
        let mut dedup = EventDedup::new();
        dedup.check_and_track(TimerId::new(""));
        assert!(matches!(dedup.check_and_track(TimerId::new("")), DuplicateResult::Duplicate(_)));
    }

    #[test]
    fn event_dedup_is_duplicate_before_tracking() {
        let mut dedup = EventDedup::new();
        dedup.track(TimerId::new("evt-pending"));
        assert!(dedup.is_duplicate(&TimerId::new("evt-pending")));
        assert!(!dedup.is_duplicate(&TimerId::new("evt-other")));
    }

    #[test]
    fn event_dedup_initial_empty() {
        let dedup = EventDedup::new();
        assert!(dedup.is_empty());
        assert_eq!(dedup.len(), 0);
    }
}
