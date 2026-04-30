//! Proptest suite for VoError.
//!
//! Property-based tests covering VoError construction, Display, Debug,
//! From conversions, equality, and serialization. These complement the
//! inline unit tests in error.rs.

use proptest::proptest;
use proptest::{prop_assert, prop_assert_eq, prop_assert_ne, prop_assume};
use vo_common::VoError;

// ============================================================================
// VoError Constructor Property Tests
// ============================================================================

proptest! {
    #[test]
    fn vo_error_config_roundtrip(msg: String) {
        let err = VoError::config(msg.clone());
        let display = err.to_string();
        prop_assert!(display.contains("configuration error"));
        prop_assert!(display.contains(&msg));
    }

    #[test]
    fn vo_error_internal_roundtrip(msg: String) {
        let err = VoError::internal(msg.clone());
        let display = err.to_string();
        prop_assert!(display.contains("internal error"));
        prop_assert!(display.contains(&msg));
    }

    #[test]
    fn vo_error_not_found_roundtrip(msg: String) {
        let err = VoError::not_found(msg.clone());
        let display = err.to_string();
        prop_assert!(display.contains("not found"));
        prop_assert!(display.contains(&msg));
    }

    #[test]
    fn vo_error_validation_roundtrip(msg: String) {
        let err = VoError::validation(msg.clone());
        let display = err.to_string();
        prop_assert!(display.contains("validation failed"));
        prop_assert!(display.contains(&msg));
    }

    #[test]
    fn vo_error_timeout_roundtrip(msg: String) {
        let err = VoError::timeout(msg.clone());
        let display = err.to_string();
        prop_assert!(display.contains("operation timed out"));
        prop_assert!(display.contains(&msg));
    }
}

// ============================================================================
// VoError Equality Property Tests
// ============================================================================

proptest! {
    #[test]
    fn vo_error_same_variant_equality(msg_a: String, msg_b: String) {
        if msg_a == msg_b {
            prop_assert_eq!(VoError::config(msg_a.clone()), VoError::config(msg_b.clone()));
            prop_assert_eq!(VoError::internal(msg_a.clone()), VoError::internal(msg_b.clone()));
            prop_assert_eq!(VoError::not_found(msg_a.clone()), VoError::not_found(msg_b.clone()));
            prop_assert_eq!(VoError::validation(msg_a.clone()), VoError::validation(msg_b.clone()));
            prop_assert_eq!(VoError::timeout(msg_a.clone()), VoError::timeout(msg_b.clone()));
        }
    }

    #[test]
    fn vo_error_same_variant_inequality(msg_a: String, msg_b: String) {
        prop_assume!(msg_a != msg_b);
        prop_assert_ne!(VoError::config(msg_a.clone()), VoError::config(msg_b.clone()));
        prop_assert_ne!(VoError::internal(msg_a.clone()), VoError::internal(msg_b.clone()));
        prop_assert_ne!(VoError::not_found(msg_a.clone()), VoError::not_found(msg_b.clone()));
        prop_assert_ne!(VoError::validation(msg_a.clone()), VoError::validation(msg_b.clone()));
        prop_assert_ne!(VoError::timeout(msg_a.clone()), VoError::timeout(msg_b.clone()));
    }

    #[test]
    fn vo_error_different_variants_always_unequal(msg: String) {
        let variants = [
            VoError::config(msg.clone()),
            VoError::internal(msg.clone()),
            VoError::not_found(msg.clone()),
            VoError::validation(msg.clone()),
            VoError::timeout(msg.clone()),
        ];
        for (i, a) in variants.iter().enumerate() {
            for (j, b) in variants.iter().enumerate() {
                if i != j {
                    prop_assert_ne!(a, b);
                }
            }
        }
    }
}

// ============================================================================
// VoError Clone Property Tests
// ============================================================================

proptest! {
    #[test]
    fn vo_error_clone_preserves_all_variants(msg: String) {
        for err in [
            VoError::config(msg.clone()),
            VoError::internal(msg.clone()),
            VoError::not_found(msg.clone()),
            VoError::validation(msg.clone()),
            VoError::timeout(msg.clone()),
        ] {
            let cloned = err.clone();
            prop_assert_eq!(err, cloned);
        }
    }
}

// ============================================================================
// VoError Serialization Property Tests
// ============================================================================

proptest! {
    #[test]
    fn vo_error_serde_roundtrip_all_variants(msg: String) {
        let variants = [
            VoError::config(msg.clone()),
            VoError::internal(msg.clone()),
            VoError::not_found(msg.clone()),
            VoError::validation(msg.clone()),
            VoError::timeout(msg.clone()),
        ];
        for err in variants {
            let json = serde_json::to_string(&err).unwrap();
            let deserialized: VoError = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(err, deserialized);
        }
    }
}

// ============================================================================
// VoError Debug Property Tests
// ============================================================================

proptest! {
    #[test]
    fn vo_error_debug_contains_variant_name(msg: String) {
        let err = VoError::config(msg.clone());
        let debug = format!("{:?}", err);
        prop_assert!(debug.contains("Config"));

        let err = VoError::internal(msg.clone());
        let debug = format!("{:?}", err);
        prop_assert!(debug.contains("Internal"));

        let err = VoError::not_found(msg.clone());
        let debug = format!("{:?}", err);
        prop_assert!(debug.contains("NotFound"));

        let err = VoError::validation(msg.clone());
        let debug = format!("{:?}", err);
        prop_assert!(debug.contains("Validation"));

        let err = VoError::timeout(msg);
        let debug = format!("{:?}", err);
        prop_assert!(debug.contains("Timeout"));
    }
}

// ============================================================================
// VoError Display Property Tests
// ============================================================================

proptest! {
    #[test]
    fn vo_error_display_contains_actual_message(msg: String) {
        let msg_clone = msg.clone();
        let tests = [
            VoError::config(msg.clone()),
            VoError::internal(msg.clone()),
            VoError::not_found(msg.clone()),
            VoError::validation(msg.clone()),
            VoError::timeout(msg),
        ];
        for err in tests {
            let display = err.to_string();
            prop_assert!(
                display.contains(&msg_clone),
                "Display '{}' should contain '{}'",
                display,
                msg_clone
            );
        }
    }

    #[test]
    fn vo_error_display_not_empty(msg: String) {
        let tests = [
            VoError::config(msg.clone()),
            VoError::internal(msg.clone()),
            VoError::not_found(msg.clone()),
            VoError::validation(msg.clone()),
            VoError::timeout(msg),
        ];
        for err in tests {
            let display = err.to_string();
            prop_assert!(!display.is_empty());
        }
    }
}

// ============================================================================
// VoError Empty String Edge Cases
// ============================================================================

#[test]
fn vo_error_empty_message_still_displays() {
    let tests = [
        (VoError::config(String::new()), "configuration error"),
        (VoError::internal(String::new()), "internal error"),
        (VoError::not_found(String::new()), "not found"),
        (VoError::validation(String::new()), "validation failed"),
        (VoError::timeout(String::new()), "operation timed out"),
    ];
    for (err, expected_prefix) in tests {
        let display = err.to_string();
        assert!(display.contains(expected_prefix));
    }
}

proptest! {
    fn vo_error_unicode_message_roundtrip(msg: String) {
        prop_assume!(msg.chars().any(|c| !c.is_ascii()));
        let err = VoError::internal(msg.clone());
        let display = err.to_string();
        prop_assert!(display.contains(&msg));
    }
}

// ============================================================================
// VoError Send + Sync Bounds
// ============================================================================

#[test]
fn vo_error_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<VoError>();
}

// ============================================================================
// VoError std::error::Error Contract
// ============================================================================

#[test]
fn vo_error_is_std_error() {
    fn check<E: std::error::Error + Send + Sync + Clone>(_e: E) {}
    check(VoError::config("test"));
    check(VoError::internal("test"));
    check(VoError::not_found("test"));
    check(VoError::validation("test"));
    check(VoError::timeout("test"));
}
