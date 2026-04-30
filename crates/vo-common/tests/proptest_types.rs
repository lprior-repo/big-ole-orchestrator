//! Proptest suite for vo-common types.
//!
//! Property-based tests covering TimestampMs, InstanceId, NamespaceId, TimerId,
//! and EventId type aliases. These complement the inline unit tests in types.rs
//! and the blackhat/QA test files.

use proptest::prelude::*;
use proptest::proptest;
use proptest::{prop_assert, prop_assert_eq};
use vo_common::types::TimestampMs;
use vo_common::{EventId, InstanceId, NamespaceId, TimerId, VoError};

// ============================================================================
// TimestampMs Property Tests
// ============================================================================

proptest! {
    #[test]
    fn timestamp_ms_as_u64_roundtrip(value: u64) {
        let ts = TimestampMs::new_unchecked(value);
        prop_assert_eq!(ts.as_u64(), value);
    }

    #[test]
    fn timestamp_ms_ordering_antisymmetric(a: u64, b: u64) {
        let ts_a = TimestampMs::new_unchecked(a);
        let ts_b = TimestampMs::new_unchecked(b);
        if a < b {
            prop_assert!(ts_a < ts_b);
            prop_assert!(ts_b > ts_a);
        } else if a > b {
            prop_assert!(ts_a > ts_b);
            prop_assert!(ts_b < ts_a);
        } else {
            prop_assert_eq!(ts_a, ts_b);
        }
    }

    #[test]
    fn timestamp_ms_ordering_transitive(a: u64, b: u64, c: u64) {
        let ts_a = TimestampMs::new_unchecked(a);
        let ts_b = TimestampMs::new_unchecked(b);
        let ts_c = TimestampMs::new_unchecked(c);
        if a <= b && b <= c {
            prop_assert!(ts_a <= ts_b);
            prop_assert!(ts_b <= ts_c);
            prop_assert!(ts_a <= ts_c);
        }
    }

    #[test]
    fn timestamp_ms_clone_preserves_value(value: u64) {
        let ts = TimestampMs::new_unchecked(value);
        let cloned = ts;
        prop_assert_eq!(ts.as_u64(), cloned.as_u64());
    }

    #[test]
    fn timestamp_ms_serde_roundtrip(value: u64) {
        let ts = TimestampMs::new_unchecked(value);
        let json = serde_json::to_string(&ts).unwrap();
        let deserialized: TimestampMs = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(ts.as_u64(), deserialized.as_u64());
    }

    #[test]
    fn timestamp_ms_boundary_min(value: u64) {
        let ts = TimestampMs::new_unchecked(value);
        prop_assert_eq!(ts.as_u64(), value);
        prop_assert!(ts.as_u64() >= 0);
    }

    #[test]
    fn timestamp_ms_boundary_max(value: u64) {
        let ts = TimestampMs::new_unchecked(value);
        prop_assert!(ts.as_u64() <= u64::MAX);
    }
}

// ============================================================================
// Type Alias Property Tests
// ============================================================================

proptest! {
    fn instance_id_is_string(content: String) {
        let id: InstanceId = content.clone();
        prop_assert_eq!(id.as_str(), content.as_str());
        let s: String = id;
        prop_assert_eq!(s, content);
    }

    #[test]
    fn namespace_id_is_string(content: String) {
        let ns: NamespaceId = content.clone();
        prop_assert_eq!(ns.as_str(), content.as_str());
        let s: String = ns;
        prop_assert_eq!(s, content);
    }

    #[test]
    fn timer_id_is_string(content: String) {
        let t: TimerId = content.clone();
        prop_assert_eq!(t.as_str(), content.as_str());
        let s: String = t;
        prop_assert_eq!(s, content);
    }

    #[test]
    fn event_id_is_string(content: String) {
        let e: EventId = content.clone();
        prop_assert_eq!(e.as_str(), content.as_str());
        let s: String = e;
        prop_assert_eq!(s, content);
    }

    #[test]
    fn type_aliases_zero_cost(content: String) {
        prop_assert_eq!(std::mem::size_of::<InstanceId>(), std::mem::size_of::<String>());
        prop_assert_eq!(std::mem::size_of::<NamespaceId>(), std::mem::size_of::<String>());
        prop_assert_eq!(std::mem::size_of::<TimerId>(), std::mem::size_of::<String>());
        prop_assert_eq!(std::mem::size_of::<EventId>(), std::mem::size_of::<String>());
        let _ = content;
    }

    #[test]
    fn instance_id_as_ref_str(content: String) {
        let id: InstanceId = content.clone();
        prop_assert_eq!(<InstanceId as AsRef<str>>::as_ref(&id), content.as_str());
    }

    #[test]
    fn namespace_id_as_ref_str(content: String) {
        let ns: NamespaceId = content.clone();
        prop_assert_eq!(<NamespaceId as AsRef<str>>::as_ref(&ns), content.as_str());
    }

    #[test]
    fn timer_id_as_ref_str(content: String) {
        let t: TimerId = content.clone();
        prop_assert_eq!(<TimerId as AsRef<str>>::as_ref(&t), content.as_str());
    }

    #[test]
    fn event_id_as_ref_str(content: String) {
        let e: EventId = content.clone();
        prop_assert_eq!(<EventId as AsRef<str>>::as_ref(&e), content.as_str());
    }

    #[test]
    fn type_aliases_accept_into_string(content: String) {
        let id: InstanceId = content.clone();
        prop_assert_eq!(String::from(id.clone()), content.clone());
        let ns: NamespaceId = content.clone();
        prop_assert_eq!(String::from(ns.clone()), content.clone());
        let t: TimerId = content.clone();
        prop_assert_eq!(String::from(t.clone()), content.clone());
        let e: EventId = content.clone();
        prop_assert_eq!(String::from(e), content);
    }

    #[test]
    fn type_aliases_equality(content: String) {
        let a: InstanceId = content.clone();
        let b: InstanceId = content.clone();
        prop_assert_eq!(a, b);

        let na: NamespaceId = content.clone();
        let nb: NamespaceId = content.clone();
        prop_assert_eq!(na, nb);
    }

    #[test]
    fn type_aliases_inequality(a: String, b: String) {
        prop_assume!(a != b);
        let id_a: InstanceId = a.clone();
        let id_b: InstanceId = b.clone();
        prop_assert_ne!(id_a, id_b);
    }
}

#[test]
fn empty_string_aliases() {
    use vo_common::{EventId, InstanceId, NamespaceId, TimerId};
    let i: InstanceId = String::new();
    let n: NamespaceId = String::new();
    let t: TimerId = String::new();
    let e: EventId = String::new();
    assert_eq!(i.as_str(), "");
    assert_eq!(n.as_str(), "");
    assert_eq!(t.as_str(), "");
    assert_eq!(e.as_str(), "");
}

proptest! {
    fn unicode_aliases(content: String) {
        prop_assume!(content.chars().any(|c| !c.is_ascii()));
        let id: InstanceId = content.clone();
        prop_assert_eq!(id.as_str(), content.as_str());
        let ns: NamespaceId = content.clone();
        prop_assert_eq!(ns.as_str(), content.as_str());
        let t: TimerId = content.clone();
        prop_assert_eq!(t.as_str(), content.as_str());
        let e: EventId = content.clone();
        prop_assert_eq!(e.as_str(), content.as_str());
    }

    #[test]
    fn very_long_aliases(len in 0u32..100_000) {
        let content = "x".repeat(len as usize);
        let id: InstanceId = content.clone();
        prop_assert_eq!(id.len(), len as usize);
        let s: String = id;
        prop_assert_eq!(s.len(), len as usize);
    }
}

// ============================================================================
// TimestampMs + Type Alias Interaction Tests
// ============================================================================

proptest! {
    #[test]
    fn timestamp_ms_with_various_ids(
        ts_value: u64,
        id_content: String,
        ns_content: String,
        timer_content: String,
        event_content: String,
    ) {
        let ts = TimestampMs::new_unchecked(ts_value);
        let _id: InstanceId = id_content;
        let _ns: NamespaceId = ns_content;
        let _t: TimerId = timer_content;
        let _e: EventId = event_content;
        prop_assert_eq!(ts.as_u64(), ts_value);
    }
}
