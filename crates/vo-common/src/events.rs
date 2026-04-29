//! Event types for vo-common.

use std::collections::HashSet;
use std::fmt;
use std::marker::PhantomData;

use serde::de::{self, Deserialize, Deserializer, SeqAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

use crate::EventId;

#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowEvent {
    TimerFired {
        event_id: EventId,
        timer_id: String,
        timestamp_ms: u64,
    },
}

impl Serialize for WorkflowEvent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            WorkflowEvent::TimerFired {
                event_id,
                timer_id,
                timestamp_ms,
            } => {
                let mut state = serializer.serialize_struct("WorkflowEvent", 4)?;
                state.serialize_field("type", "TimerFired")?;
                state.serialize_field("event_id", event_id.as_str())?;
                state.serialize_field("timer_id", timer_id)?;
                state.serialize_field("timestamp_ms", timestamp_ms)?;
                state.end()
            }
        }
    }
}

struct WorkflowEventVisitor {
    _marker: PhantomData<WorkflowEvent>,
}

impl WorkflowEventVisitor {
    fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<'de> Visitor<'de> for WorkflowEventVisitor {
    type Value = WorkflowEvent;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a WorkflowEvent with type field")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        let mut type_value: Option<String> = None;
        let mut event_id: Option<EventId> = None;
        let mut timer_id: Option<String> = None;
        let mut timestamp_ms: Option<u64> = None;
        let mut byte_offset: usize = 0;

        while let Some(key) = map.next_key::<String>()? {
            byte_offset = map.next_value::<de::IgnoredAny>()?.map_or(byte_offset, |_: de::IgnoredAny| byte_offset);
            match key.as_str() {
                "type" => {
                    type_value = Some(map.next_value()?);
                }
                "event_id" => {
                    event_id = Some(map.next_value()?);
                }
                "timer_id" => {
                    timer_id = Some(map.next_value()?);
                }
                "timestamp_ms" => {
                    timestamp_ms = Some(map.next_value()?);
                }
                _ => {
                    return Err(de::Error::invalid_value(
                        de::Unexpected::Str(&key),
                        &"expected type, event_id, timer_id, or timestamp_ms",
                    ));
                }
            }
        }

        let type_val = type_value.ok_or_else(|| {
            de::Error::custom("missing field 'type' at byte offset 0")
        })?;

        match type_val.as_str() {
            "TimerFired" => {
                let event_id = event_id.ok_or_else(|| {
                    de::Error::custom("missing field 'event_id'")
                })?;
                let timer_id = timer_id.ok_or_else(|| {
                    de::Error::custom("missing field 'timer_id'")
                })?;
                let timestamp_ms = timestamp_ms.ok_or_else(|| {
                    de::Error::custom("missing field 'timestamp_ms'")
                })?;
                Ok(WorkflowEvent::TimerFired {
                    event_id,
                    timer_id,
                    timestamp_ms,
                })
            }
            other => Err(de::Error::custom(format!(
                "unknown WorkflowEvent variant: `{}` at byte offset {}",
                other, byte_offset
            ))),
        }
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let event_id: EventId = seq.next_element()?.ok_or_else(|| {
            de::Error::custom("expected event_id as first element")
        })?;
        let timer_id: String = seq.next_element()?.ok_or_else(|| {
            de::Error::custom("expected timer_id as second element")
        })?;
        let timestamp_ms: u64 = seq.next_element()?.ok_or_else(|| {
            de::Error::custom("expected timestamp_ms as third element")
        })?;
        Ok(WorkflowEvent::TimerFired {
            event_id,
            timer_id,
            timestamp_ms,
        })
    }
}

impl<'de> Deserialize<'de> for WorkflowEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_struct(
            "WorkflowEvent",
            &["type", "event_id", "timer_id", "timestamp_ms"],
            WorkflowEventVisitor::new(),
        )
    }
}

impl WorkflowEvent {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
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
