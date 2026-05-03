//! Proptest suite for vo-common types.
//!
//! Property-based tests covering TimestampMs, InstanceId, NamespaceId, TimerId,
//! type aliases and parse/format edge cases. These complement the inline unit
//! tests in types.rs and the blackhat/QA test files.

use proptest::prelude::*;
use proptest::proptest;
use proptest::{prop_assert, prop_assert_eq};
use vo_types::TimestampMs;
use vo_common::{InstanceId, NamespaceId, TimerId, VoError};

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
        let id: InstanceId = content.clone().into();
        prop_assert_eq!(id.as_str(), content.as_str());
        let s: String = id.into();
        prop_assert_eq!(s, content);
    }

    #[test]
    fn namespace_id_is_string(content: String) {
        let ns: NamespaceId = content.clone().into();
        prop_assert_eq!(ns.as_str(), content.as_str());
        let s: String = ns.into();
        prop_assert_eq!(s, content);
    }

    #[test]
    fn timer_id_is_string(content: String) {
        let t: TimerId = content.clone().into();
        prop_assert_eq!(t.as_str(), content.as_str());
        let s: String = t.into();
        prop_assert_eq!(s, content);
    }

    #[test]
    fn type_aliases_zero_cost(content: String) {
        prop_assert_eq!(std::mem::size_of::<InstanceId>(), std::mem::size_of::<String>());
        prop_assert_eq!(std::mem::size_of::<NamespaceId>(), std::mem::size_of::<String>());
        prop_assert_eq!(std::mem::size_of::<TimerId>(), std::mem::size_of::<String>());
        let _ = content;
    }

    #[test]
    fn instance_id_as_ref_str(content: String) {
        let id: InstanceId = content.clone().into();
        prop_assert_eq!(<InstanceId as AsRef<str>>::as_ref(&id), content.as_str());
    }

    #[test]
    fn namespace_id_as_ref_str(content: String) {
        let ns: NamespaceId = content.clone().into();
        prop_assert_eq!(<NamespaceId as AsRef<str>>::as_ref(&ns), content.as_str());
    }

    #[test]
    fn timer_id_as_ref_str(content: String) {
        let t: TimerId = content.clone().into();
        prop_assert_eq!(<TimerId as AsRef<str>>::as_ref(&t), content.as_str());
    }

    #[test]
    fn type_aliases_accept_into_string(content: String) {
        let id: InstanceId = content.clone().into();
        prop_assert_eq!(String::from(id.clone()), content.clone());
        let ns: NamespaceId = content.clone().into();
        prop_assert_eq!(String::from(ns.clone()), content.clone());
        let t: TimerId = content.clone().into();
        prop_assert_eq!(String::from(t.clone()), content.clone());
    }

    #[test]
    fn type_aliases_equality(content: String) {
        let a: InstanceId = content.clone().into();
        let b: InstanceId = content.clone().into();
        prop_assert_eq!(a, b);

        let na: NamespaceId = content.clone().into();
        let nb: NamespaceId = content.clone().into();
        prop_assert_eq!(na, nb);
    }

    #[test]
    fn type_aliases_inequality(a: String, b: String) {
        prop_assume!(a != b);
        let id_a: InstanceId = a.clone().into();
        let id_b: InstanceId = b.clone().into();
        prop_assert_ne!(id_a, id_b);
    }
}

#[test]
fn empty_string_aliases() {
    use vo_common::{InstanceId, NamespaceId, TimerId};
    let i: InstanceId = String::new().into();
    let n: NamespaceId = String::new().into();
    let t: TimerId = String::new().into();
    assert_eq!(i.as_str(), "");
    assert_eq!(n.as_str(), "");
    assert_eq!(t.as_str(), "");
}

proptest! {
    fn unicode_aliases(content: String) {
        prop_assume!(content.chars().any(|c| !c.is_ascii()));
        let id: InstanceId = content.clone().into();
        prop_assert_eq!(id.as_str(), content.as_str());
        let ns: NamespaceId = content.clone().into();
        prop_assert_eq!(ns.as_str(), content.as_str());
        let t: TimerId = content.clone().into();
        prop_assert_eq!(t.as_str(), content.as_str());
    }

    #[test]
    fn very_long_aliases(len in 0u32..100_000) {
        let content = "x".repeat(len as usize);
        let id: InstanceId = content.clone().into();
        prop_assert_eq!(id.len(), len as usize);
        let s: String = id.into();
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
    ) {
        let ts = TimestampMs::new_unchecked(ts_value);
        let _id: InstanceId = id_content.into();
        let _ns: NamespaceId = ns_content.into();
        let _t: TimerId = timer_content.into();
        prop_assert_eq!(ts.as_u64(), ts_value);
    }
}

// ============================================================================
// InstanceId Parse/Format Edge Cases
// ============================================================================

proptest! {
    #[test]
    fn instance_id_ulid_format_roundtrip(content: String) {
        let id: InstanceId = content.clone().into();
        let displayed = id.to_string();
        prop_assert_eq!(displayed.clone(), content.clone());
        let reparsed: InstanceId = displayed.into();
        prop_assert_eq!(reparsed.as_str(), content);
    }

    #[test]
    fn instance_id_debug_format(content: String) {
        let id: InstanceId = content.clone().into();
        let debug = format!("{:?}", id);
        prop_assert!(!debug.is_empty());
    }

    #[test]
    fn instance_id_serde_roundtrip(content: String) {
        let id: InstanceId = content.clone().into();
        let json = serde_json::to_string(&id).unwrap();
        let restored: InstanceId = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(id.as_str(), restored.as_str());
    }

    #[test]
    fn instance_id_from_str_equivalence(content: String) {
        let id: InstanceId = content.clone().into();
        let from_str: InstanceId = content.as_str().into();
        prop_assert_eq!(id, from_str);
    }

    #[test]
    fn instance_id_binary_content(len in 0u32..256) {
        let bytes: Vec<u8> = (0..len).map(|i| i as u8).collect();
        let content = String::from_utf8_lossy(&bytes).to_string();
        let id: InstanceId = content.clone().into();
        prop_assert_eq!(id.as_str(), content);
    }

    #[test]
    fn instance_id_null_bytes(len in 0u32..64) {
        let content = "\0".repeat(len as usize);
        let id: InstanceId = content.clone().into();
        prop_assert_eq!(id.as_str(), content);
    }

    #[test]
    fn instance_id_whitespace_variations(content: String) {
        prop_assume!(!content.contains('\0'));
        let with_prefix = format!(" {}", content);
        let with_suffix = format!("{} ", content);
        let with_both = format!(" {} ", content);

        let id_orig: InstanceId = content.clone().into();
        let id_prefix: InstanceId = with_prefix.clone().into();
        let id_suffix: InstanceId = with_suffix.clone().into();
        let id_both: InstanceId = with_both.clone().into();

        prop_assert_eq!(id_orig.as_str(), content);
        prop_assert_eq!(id_prefix.as_str(), with_prefix);
        prop_assert_eq!(id_suffix.as_str(), with_suffix);
        prop_assert_eq!(id_both.as_str(), with_both);
    }

    #[test]
    fn instance_id_newline_tab_content(len in 1u32..64) {
        let base: String = "abcdefghijklmnopqrstuvwxyz".chars().take(len as usize).collect();
        let content = base + "\n\t";
        let id: InstanceId = content.clone().into();
        prop_assert_eq!(id.as_str(), content);
    }

    #[test]
    fn instance_id_deref_target(content: String) {
        use std::ops::Deref;
        let id: InstanceId = content.clone().into();
        let deref_str: &String = Deref::deref(&id);
        prop_assert_eq!(deref_str.as_str(), content);
    }
}
