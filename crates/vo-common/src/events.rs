//! Event types for vo-common.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::EventId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkflowEvent {
    TimerFired {
        event_id: EventId,
        timer_id: String,
        timestamp_ms: u64,
    },
}

#[derive(Debug, Clone, Default)]
pub struct EventDedup {
    seen: HashSet<EventId>,
}

impl EventDedup {
    pub fn new() -> Self {
        Self {
            seen: HashSet::new(),
        }
    }

    pub fn is_duplicate(&self, event_id: &EventId) -> bool {
        self.seen.contains(event_id)
    }

    pub fn track(&mut self, event_id: EventId) -> bool {
        self.seen.insert(event_id)
    }

    pub fn check_and_track(&mut self, event_id: EventId) -> DuplicateResult {
        if self.seen.insert(event_id.clone()) {
            DuplicateResult::New
        } else {
            DuplicateResult::Duplicate(event_id)
        }
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

#[derive(Debug, Clone, PartialEq)]
pub enum DuplicateResult {
    New,
    Duplicate(EventId),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(event_id: &str, timer_id: &str, timestamp_ms: u64) -> WorkflowEvent {
        WorkflowEvent::TimerFired {
            event_id: event_id.into(),
            timer_id: timer_id.into(),
            timestamp_ms,
        }
    }

    #[test]
    fn workflow_event_timer_fired_construction() {
        let event = make_event("evt-1", "timer-abc", 1234567890);
        match event {
            WorkflowEvent::TimerFired {
                event_id,
                timer_id,
                timestamp_ms,
            } => {
                assert_eq!(event_id, "evt-1");
                assert_eq!(timer_id, "timer-abc");
                assert_eq!(timestamp_ms, 1234567890);
            }
        }
    }

    #[test]
    fn workflow_event_json_serialization_roundtrip() {
        let event = make_event("evt-rt", "timer-test-123", 9876543210);
        let json = serde_json::to_string(&event).expect("should serialize");
        let deserialized: WorkflowEvent = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(event, deserialized);
    }

    #[test]
    fn workflow_event_json_deserialization() {
        let json = r#"{"TimerFired":{"event_id":"e1","timer_id":"t1","timestamp_ms":42}}"#;
        let event: WorkflowEvent = serde_json::from_str(json).expect("should deserialize");
        match event {
            WorkflowEvent::TimerFired {
                event_id,
                timer_id,
                timestamp_ms,
            } => {
                assert_eq!(event_id, "e1");
                assert_eq!(timer_id, "t1");
                assert_eq!(timestamp_ms, 42);
            }
        }
    }

    #[test]
    fn workflow_event_clone_preserves_data() {
        let event = make_event("evt-clone", "timer-clone-test", 1111111111);
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn event_dedup_new_event_accepted() {
        let mut dedup = EventDedup::new();
        assert_eq!(dedup.check_and_track("evt-a".into()), DuplicateResult::New);
        assert_eq!(dedup.len(), 1);
    }

    #[test]
    fn event_dedup_duplicate_detected() {
        let mut dedup = EventDedup::new();
        dedup.check_and_track("evt-x".into());
        let result = dedup.check_and_track("evt-x".into());
        assert_eq!(result, DuplicateResult::Duplicate("evt-x".into()));
        assert_eq!(dedup.len(), 1);
    }

    #[test]
    fn event_dedup_different_events_both_accepted() {
        let mut dedup = EventDedup::new();
        assert_eq!(dedup.check_and_track("evt-1".into()), DuplicateResult::New);
        assert_eq!(dedup.check_and_track("evt-2".into()), DuplicateResult::New);
        assert_eq!(dedup.len(), 2);
    }

    #[test]
    fn event_dedup_empty_string_event_id() {
        let mut dedup = EventDedup::new();
        dedup.check_and_track("".into());
        assert_eq!(
            dedup.check_and_track("".into()),
            DuplicateResult::Duplicate("".into())
        );
    }

    #[test]
    fn event_dedup_is_duplicate_before_tracking() {
        let mut dedup = EventDedup::new();
        dedup.track("evt-pending".into());
        assert!(dedup.is_duplicate(&"evt-pending".into()));
        assert!(!dedup.is_duplicate(&"evt-other".into()));
    }

    #[test]
    fn event_dedup_initial_empty() {
        let dedup = EventDedup::new();
        assert!(dedup.is_empty());
        assert_eq!(dedup.len(), 0);
    }

    #[test]
    fn same_timer_different_event_ids_not_duplicate() {
        let a = make_event("evt-a", "timer-same", 100);
        let b = make_event("evt-b", "timer-same", 100);
        assert_ne!(a, b);
    }

    #[test]
    fn different_timer_same_event_id_are_equal() {
        let a = make_event("evt-x", "timer-alpha", 100);
        let b = make_event("evt-x", "timer-beta", 200);
        assert_ne!(a, b);
    }
}
