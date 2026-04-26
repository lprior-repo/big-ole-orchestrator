//! Guarantee class classification for workflow execution semantics (ADR-007, ADR-031).
//!
//! This module defines the type system for classifying workflow execution guarantees.
//! Each variant represents a distinct delivery/exactly-once guarantee tier that the
//! UI surfaces as a badge (ADR-007) and that the engine enforces at publish time (ADR-031).
//!
//! No I/O — pure types.

/// Classification of a workflow's execution guarantee tier (ADR-007, ADR-031).
///
/// Determines the delivery semantics the engine guarantees for this workflow:
/// - **ExactOnce**: Deduplication at ingress (ADR-028), idempotent effect replay,
///   crash-safe recovery — the strongest guarantee.
/// - **AtLeastOnce**: Retries may cause duplicate side effects. The engine retries
///   on failure but does not deduplicate ingress or guarantee idempotent replay.
/// - **BestEffort**: No delivery guarantees. Fire-and-forget semantics with no
///   retry or recovery. Useful for logging, telemetry, and non-critical paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum GuaranteeClass {
    /// Exactly-once execution — deduplicated ingress, idempotent replay, crash-safe.
    ExactOnce,
    /// At-least-once execution — retries possible, duplicates may occur.
    AtLeastOnce,
    /// Best-effort execution — no guarantees, fire-and-forget.
    BestEffort,
}

impl GuaranteeClass {
    /// Returns all `GuaranteeClass` variants in declaration order.
    #[must_use]
    #[allow(dead_code)]
    pub const fn all_variants() -> &'static [GuaranteeClass] {
        &[
            GuaranteeClass::ExactOnce,
            GuaranteeClass::AtLeastOnce,
            GuaranteeClass::BestEffort,
        ]
    }

    /// Whether this guarantee tier requires ingress deduplication (ADR-028).
    #[must_use]
    #[allow(dead_code)]
    pub const fn requires_deduplication(self) -> bool {
        matches!(self, GuaranteeClass::ExactOnce)
    }

    /// Whether this guarantee tier permits unsafe nodes.
    ///
    /// Only `BestEffort` workflows may contain `Unsafe` nodes, since
    /// unsafe nodes break all delivery guarantees by definition.
    #[must_use]
    #[allow(dead_code)]
    pub const fn permits_unsafe_nodes(self) -> bool {
        matches!(self, GuaranteeClass::BestEffort)
    }

    /// Returns a short human-readable label for badge display.
    #[must_use]
    #[allow(dead_code)]
    pub const fn label(self) -> &'static str {
        match self {
            GuaranteeClass::ExactOnce => "exact-once",
            GuaranteeClass::AtLeastOnce => "at-least-once",
            GuaranteeClass::BestEffort => "best-effort",
        }
    }

    /// Returns the Tailwind CSS badge class for this guarantee tier (ADR-007).
    #[must_use]
    #[allow(dead_code)]
    pub const fn badge_class(self) -> &'static str {
        match self {
            GuaranteeClass::ExactOnce => "bg-emerald-100 text-emerald-700 border-emerald-300",
            GuaranteeClass::AtLeastOnce => "bg-amber-100 text-amber-700 border-amber-300",
            GuaranteeClass::BestEffort => "bg-red-100 text-red-700 border-red-300",
        }
    }

    /// Returns the icon name for this guarantee tier (ADR-007).
    #[must_use]
    #[allow(dead_code)]
    pub const fn icon(self) -> &'static str {
        match self {
            GuaranteeClass::ExactOnce => "shield-check",
            GuaranteeClass::AtLeastOnce => "shield-alert",
            GuaranteeClass::BestEffort => "shield-off",
        }
    }
}

impl Default for GuaranteeClass {
    fn default() -> Self {
        GuaranteeClass::BestEffort
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn guarantee_class_exact_once_serializes_to_snake_case() {
        let json = serde_json::to_string(&GuaranteeClass::ExactOnce).unwrap();
        assert_eq!(json, "\"exact_once\"");
    }

    #[test]
    fn guarantee_class_at_least_once_serializes_to_snake_case() {
        let json = serde_json::to_string(&GuaranteeClass::AtLeastOnce).unwrap();
        assert_eq!(json, "\"at_least_once\"");
    }

    #[test]
    fn guarantee_class_best_effort_serializes_to_snake_case() {
        let json = serde_json::to_string(&GuaranteeClass::BestEffort).unwrap();
        assert_eq!(json, "\"best_effort\"");
    }

    #[test]
    fn guarantee_class_round_trips_via_serde() {
        let variants = [
            GuaranteeClass::ExactOnce,
            GuaranteeClass::AtLeastOnce,
            GuaranteeClass::BestEffort,
        ];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let recovered: GuaranteeClass = serde_json::from_str(&json).unwrap();
            assert_eq!(recovered, variant, "round-trip failed for {:?}", variant);
        }
    }

    #[test]
    fn guarantee_class_rejects_unknown_variant_with_data_error() {
        let result: Result<GuaranteeClass, serde_json::Error> =
            serde_json::from_str("\"nonexistent\"");
        let err = result.expect_err("should reject unknown variant 'nonexistent'");
        assert!(
            err.is_data(),
            "expected data error for unknown variant, got: {:?}",
            err
        );
    }

    #[test]
    fn guarantee_class_all_variants_returns_three_in_declaration_order() {
        let variants = GuaranteeClass::all_variants();
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0], GuaranteeClass::ExactOnce);
        assert_eq!(variants[1], GuaranteeClass::AtLeastOnce);
        assert_eq!(variants[2], GuaranteeClass::BestEffort);
    }

    #[test]
    fn exact_once_requires_deduplication() {
        assert!(GuaranteeClass::ExactOnce.requires_deduplication());
        assert!(!GuaranteeClass::AtLeastOnce.requires_deduplication());
        assert!(!GuaranteeClass::BestEffort.requires_deduplication());
    }

    #[test]
    fn only_best_effort_permits_unsafe_nodes() {
        assert!(!GuaranteeClass::ExactOnce.permits_unsafe_nodes());
        assert!(!GuaranteeClass::AtLeastOnce.permits_unsafe_nodes());
        assert!(GuaranteeClass::BestEffort.permits_unsafe_nodes());
    }

    #[test]
    fn label_returns_human_readable_string() {
        assert_eq!(GuaranteeClass::ExactOnce.label(), "exact-once");
        assert_eq!(GuaranteeClass::AtLeastOnce.label(), "at-least-once");
        assert_eq!(GuaranteeClass::BestEffort.label(), "best-effort");
    }

    #[test]
    fn guarantee_class_equality_works() {
        assert_eq!(GuaranteeClass::ExactOnce, GuaranteeClass::ExactOnce);
        assert_ne!(GuaranteeClass::ExactOnce, GuaranteeClass::AtLeastOnce);
        assert_ne!(GuaranteeClass::AtLeastOnce, GuaranteeClass::BestEffort);
    }

    #[test]
    fn guarantee_class_hash_is_consistent() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();
        GuaranteeClass::ExactOnce.hash(&mut hasher1);
        GuaranteeClass::ExactOnce.hash(&mut hasher2);
        assert_eq!(hasher1.finish(), hasher2.finish());
    }
}

#[cfg(feature = "proptest")]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn guarantee_class_all_variants_proptest_exhaustive(idx in 0..3usize) {
            let variants = GuaranteeClass::all_variants();
            prop_assert!(idx < variants.len(), "idx {} should be in range", idx);
            let variant = variants[idx];
            let json = serde_json::to_string(variant).expect("serialize");
            let restored: GuaranteeClass = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, *variant, "roundtrip failed for {:?}", variant);
        }

        #[test]
        fn guarantee_class_hash_is_deterministic(variant in 0..3u8) {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};

            let gc = match variant {
                0 => GuaranteeClass::ExactOnce,
                1 => GuaranteeClass::AtLeastOnce,
                2 => GuaranteeClass::BestEffort,
                _ => return prop_assert!(false, "invalid variant index"),
            };

            let mut hasher1 = DefaultHasher::new();
            let mut hasher2 = DefaultHasher::new();

            gc.hash(&mut hasher1);
            gc.hash(&mut hasher2);

            prop_assert_eq!(hasher1.finish(), hasher2.finish(), "hash should be deterministic");
        }

        #[test]
        fn guarantee_class_equality_is_reflexive(variant in 0..3u8) {
            let gc = match variant {
                0 => GuaranteeClass::ExactOnce,
                1 => GuaranteeClass::AtLeastOnce,
                2 => GuaranteeClass::BestEffort,
                _ => return prop_assert!(false, "invalid variant index"),
            };
            prop_assert_eq!(gc, gc, "equality should be reflexive");
        }

        #[test]
        fn label_matches_serde_roundtrip(variant in 0..3u8) {
            let gc = match variant {
                0 => GuaranteeClass::ExactOnce,
                1 => GuaranteeClass::AtLeastOnce,
                2 => GuaranteeClass::BestEffort,
                _ => return prop_assert!(false, "invalid variant index"),
            };
            let json = serde_json::to_string(&gc).expect("serialize");
            let restored: GuaranteeClass = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, gc, "serde roundtrip should preserve variant");
            prop_assert!(!gc.label().is_empty(), "label should not be empty");
        }

        #[test]
        fn deduplication_and_unsafe_are_mutually_exclusive_for_exact_once(variant in 0..3u8) {
            let gc = match variant {
                0 => GuaranteeClass::ExactOnce,
                1 => GuaranteeClass::AtLeastOnce,
                2 => GuaranteeClass::BestEffort,
                _ => return prop_assert!(false, "invalid variant index"),
            };
            if gc.requires_deduplication() {
                prop_assert!(!gc.permits_unsafe_nodes(),
                    "exact-once requiring deduplication should not permit unsafe nodes");
            }
        }
    }
}
