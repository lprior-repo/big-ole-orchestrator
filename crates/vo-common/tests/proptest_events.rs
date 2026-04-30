//! Proptest suite for vo-common events.
//!
//! Property-based tests covering WorkflowEvent, EventDedup, and DuplicateResult.
//! These complement the inline unit tests in events.rs and the blackhat
//! adversarial test files.

use proptest::proptest;
use proptest::{prop_assert, prop_assert_eq, prop_assert_ne, prop_assume};
use vo_common::{DuplicateResult, EventDedup, EventId, WorkflowEvent};

// ============================================================================
// WorkflowEvent Property Tests
// ============================================================================

proptest! {
    #[test]
    fn workflow_event_timer_fired_construction(event_id: String, timer_id: String, timestamp_ms: u64) {
        let event = WorkflowEvent::TimerFired {
            event_id: event_id.clone(),
            timer_id: timer_id.clone(),
            timestamp_ms,
        };
        let WorkflowEvent::TimerFired { event_id: eid, timer_id: tid, timestamp_ms: ts } = event;
        prop_assert_eq!(eid, event_id);
        prop_assert_eq!(tid, timer_id);
        prop_assert_eq!(ts, timestamp_ms);
    }

    #[test]
    fn workflow_event_serde_roundtrip(event_id: String, timer_id: String, timestamp_ms: u64) {
        let event = WorkflowEvent::TimerFired {
            event_id: event_id.clone(),
            timer_id: timer_id.clone(),
            timestamp_ms,
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: WorkflowEvent = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(event, deserialized);
    }

    #[test]
    fn workflow_event_clone_preserves_data(event_id: String, timer_id: String, timestamp_ms: u64) {
        let event = WorkflowEvent::TimerFired {
            event_id: event_id.clone(),
            timer_id: timer_id.clone(),
            timestamp_ms,
        };
        let cloned = event.clone();
        prop_assert_eq!(event, cloned);
    }
}

// ============================================================================
// EventDedup Property Tests
// ============================================================================

proptest! {
    #[test]
    fn event_dedup_new_is_empty(event_ids: Vec<String>) {
        let dedup = EventDedup::new();
        prop_assert!(dedup.is_empty());
        prop_assert_eq!(dedup.len(), 0);
        let _ = event_ids;
    }

    #[test]
    fn event_dedup_track_new_events(event_ids: Vec<String>) {
        let mut dedup = EventDedup::new();
        let mut count = 0;
        for id in &event_ids {
            if dedup.track(id.clone()) {
                count += 1;
            }
            prop_assert!(!dedup.is_empty());
        }
        prop_assert_eq!(dedup.len(), count as usize);
    }

    #[test]
    fn event_dedup_track_same_id_returns_false(event_id: String) {
        let mut dedup = EventDedup::new();
        prop_assert!(dedup.track(event_id.clone()));
        prop_assert!(!dedup.track(event_id.clone()));
        prop_assert_eq!(dedup.len(), 1);
    }

    #[test]
    fn event_dedup_is_duplicate_reflects_tracking(event_ids: Vec<String>) {
        let mut dedup = EventDedup::new();
        for id in &event_ids {
            dedup.track(id.clone());
        }
        for id in &event_ids {
            prop_assert!(dedup.is_duplicate(id));
        }
        let never_seen: EventId = "never-seen-id".into();
        prop_assert!(!dedup.is_duplicate(&never_seen));
    }

    #[test]
    fn event_dedup_check_and_track_new_event(event_id: String) {
        let mut dedup = EventDedup::new();
        let result = dedup.check_and_track(event_id.clone());
        prop_assert_eq!(result, DuplicateResult::New);
        prop_assert_eq!(dedup.len(), 1);
    }

    #[test]
    fn event_dedup_check_and_track_duplicate(event_id: String) {
        let mut dedup = EventDedup::new();
        let _ = dedup.check_and_track(event_id.clone());
        let result = dedup.check_and_track(event_id.clone());
        prop_assert_eq!(result, DuplicateResult::Duplicate(event_id.clone()));
        prop_assert_eq!(dedup.len(), 1);
    }

    #[test]
    fn event_dedup_len_accurate_after_mixed_operations(event_ids: Vec<String>) {
        let mut dedup = EventDedup::new();
        let mut expected_len = 0;
        for (i, id) in event_ids.iter().enumerate() {
            let was_new = dedup.track(id.clone());
            if was_new {
                expected_len += 1;
            }
            prop_assert_eq!(dedup.len(), expected_len);

            if i % 2 == 0 && i > 0 {
                let prev_id = &event_ids[i / 2];
                let was_new_again = dedup.track(prev_id.clone());
                prop_assert!(!was_new_again);
                prop_assert_eq!(dedup.len(), expected_len);
            }
        }
    }

    #[test]
    fn event_dedup_multiple_unique_events_accepted(event_ids: Vec<String>) {
        let mut dedup = EventDedup::new();
        let unique_events: Vec<String> = event_ids.into_iter().collect::<std::collections::HashSet<_>>().into_iter().collect();
        let mut accepted = 0;
        for id in &unique_events {
            if dedup.track(id.clone()) {
                accepted += 1;
            }
        }
        prop_assert_eq!(dedup.len(), accepted);
    }
}

// ============================================================================
// DuplicateResult Property Tests
// ============================================================================

proptest! {
    #[test]
    fn duplicate_result_new_equality(event_id: String) {
        let r1 = DuplicateResult::New;
        let r2 = DuplicateResult::New;
        prop_assert_eq!(r1, r2);
        let _ = event_id;
    }

    #[test]
    fn duplicate_result_duplicate_equality(event_id: String) {
        let r1 = DuplicateResult::Duplicate(event_id.clone());
        let r2 = DuplicateResult::Duplicate(event_id.clone());
        prop_assert_eq!(r1, r2);
    }

    #[test]
    fn duplicate_result_new_and_duplicate_unequal(event_id: String) {
        let r1 = DuplicateResult::New;
        let r2 = DuplicateResult::Duplicate(event_id);
        prop_assert_ne!(r1, r2);
    }

    #[test]
    fn duplicate_result_duplicate_unequal_different_ids(id_a: String, id_b: String) {
        prop_assume!(id_a != id_b);
        let r1 = DuplicateResult::Duplicate(id_a);
        let r2 = DuplicateResult::Duplicate(id_b);
        prop_assert_ne!(r1, r2);
    }
}

// ============================================================================
// EventDedup + DuplicateResult Integration Tests
// ============================================================================

proptest! {
    fn event_dedup_check_and_track_semantics(event_ids: Vec<String>) {
        let mut dedup = EventDedup::new();
        for id in &event_ids {
            let result = dedup.check_and_track(id.clone());
            match result {
                DuplicateResult::New => {
                    prop_assert!(!dedup.is_duplicate(id));
                }
                DuplicateResult::Duplicate(_) => {
                    prop_assert!(dedup.is_duplicate(id));
                }
            }
        }
    }

    #[test]
    fn event_dedup_empty_string_event_id() {
        let mut dedup = EventDedup::new();
        let result = dedup.check_and_track(String::new());
        prop_assert_eq!(result, DuplicateResult::New);
        prop_assert_eq!(dedup.len(), 1);
        let result2 = dedup.check_and_track(String::new());
        prop_assert_eq!(result2, DuplicateResult::Duplicate(String::new()));
        prop_assert_eq!(dedup.len(), 1);
    }

    #[test]
    fn event_dedup_unicode_event_ids(ids: Vec<String>) {
        prop_assume!(ids.iter().any(|s| s.chars().any(|c| !c.is_ascii())));
        let mut dedup = EventDedup::new();
        for id in &ids {
            dedup.track(id.clone());
        }
        for id in &ids {
            prop_assert!(dedup.is_duplicate(id));
        }
    }

    #[test]
    fn event_dedup_interleaved_check_and_track(ids_a: Vec<String>, ids_b: Vec<String>) {
        let mut dedup = EventDedup::new();
        for (a, b) in ids_a.iter().zip(ids_b.iter()) {
            let r1 = dedup.check_and_track(a.clone());
            if matches!(r1, DuplicateResult::New) {
                prop_assert!(!dedup.is_duplicate(a));
            }
            let r2 = dedup.check_and_track(b.clone());
            if matches!(r2, DuplicateResult::New) {
                prop_assert!(!dedup.is_duplicate(b));
            }
        }
    }
}

// ============================================================================
// WorkflowEvent + EventDedup Interaction Tests
// ============================================================================

proptest! {
    #[test]
    fn workflow_event_with_event_dedup(event_id: String, timer_id: String, timestamp_ms: u64) {
        let mut dedup = EventDedup::new();
        let _event = WorkflowEvent::TimerFired {
            event_id: event_id.clone(),
            timer_id: timer_id.clone(),
            timestamp_ms,
        };
        let result = dedup.check_and_track(event_id.clone());
        prop_assert_eq!(result, DuplicateResult::New);

        let _dup_event = WorkflowEvent::TimerFired {
            event_id: event_id.clone(),
            timer_id: timer_id.clone(),
            timestamp_ms,
        };
        let result2 = dedup.check_and_track(event_id.clone());
        prop_assert_eq!(result2, DuplicateResult::Duplicate(event_id));

        prop_assert_eq!(dedup.len(), 1);
    }

    #[test]
    fn many_events_dedup_tracking(count in 1u32..1000) {
        let mut dedup = EventDedup::new();
        let mut accepted = 0;
        for i in 0..count {
            let event_id: EventId = format!("evt-{}", i).into();
            if matches!(dedup.check_and_track(event_id), DuplicateResult::New) {
                accepted += 1;
            }
        }
        prop_assert_eq!(dedup.len(), accepted as usize);
        prop_assert_eq!(accepted, count);
    }
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn event_dedup_with_empty_string_id() {
    let mut dedup = EventDedup::new();
    assert!(dedup.track(String::new()));
    assert!(!dedup.track(String::new()));
    assert_eq!(dedup.len(), 1);
}

#[test]
fn workflow_event_cloneable() {
    fn assert_clone<T: Clone>() {}
    assert_clone::<WorkflowEvent>();
    assert_clone::<EventDedup>();
    assert_clone::<DuplicateResult>();
}

#[test]
fn workflow_event_debug() {
    let event = WorkflowEvent::TimerFired {
        event_id: "e1".into(),
        timer_id: "t1".into(),
        timestamp_ms: 42,
    };
    let debug = format!("{:?}", event);
    assert!(debug.contains("TimerFired"));
    assert!(debug.contains("e1"));
}
