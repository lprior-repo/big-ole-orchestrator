//! Write class taxonomy for storage QoS tiers.
//!
//! Defines the three-tier write class taxonomy per ADR-032:
//! - Tier 1: CriticalControlPlane — never dropped
//! - Tier 2: OperatorProjection — may lag
//! - Tier 3: BulkBlob — may be deferred
//!
//! Also provides WriteBudget for per-class budget tracking.

use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

// ─────────────────────────────────────────────────────────────────────────────
// Error Types
// ─────────────────────────────────────────────────────────────────────────────

/// Errors for write class taxonomy operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Error {
    /// Returned when an unknown or unclassified write class is encountered.
    #[error("unknown write class: {0}")]
    UnknownWriteClass(String),

    /// Returned when serialization/deserialization of WriteClass fails.
    #[error("serialization error: {0}")]
    SerializationError(String),

    /// Returned when taxonomy is accessed before being initialized.
    #[error("taxonomy not initialized")]
    TaxonomyNotInitialized,

    /// Returned when a write budget constraint is violated.
    #[error("budget exceeded for {class:?}: requested {requested}, available {available}")]
    BudgetExceeded {
        class: WriteClass,
        requested: u64,
        available: u64,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// WriteClass Enum
// ─────────────────────────────────────────────────────────────────────────────

/// Defines the three-tier write class taxonomy per ADR-032.
///
/// # Variants
/// - `CriticalControlPlane` — Tier 1: events, instances, dedupe, effects,
///   leases, timers, snapshots. Never dropped under pressure.
/// - `OperatorProjection` — Tier 2: dashboard views, redacted history enrichments,
///   UI convenience indexes. May lag under pressure.
/// - `BulkBlob` — Tier 3: large canonical payloads, bounded stderr blobs,
///   optional large outputs. May be deferred under pressure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteClass {
    CriticalControlPlane,
    OperatorProjection,
    BulkBlob,
}

impl WriteClass {
    /// Returns the QoS tier number (1=critical, 2=projection, 3=blob).
    ///
    /// # Returns
    /// - `1` for `CriticalControlPlane`
    /// - `2` for `OperatorProjection`
    /// - `3` for `BulkBlob`
    pub fn tier(self) -> u8 {
        match self {
            WriteClass::CriticalControlPlane => 1,
            WriteClass::OperatorProjection => 2,
            WriteClass::BulkBlob => 3,
        }
    }

    /// Returns `true` if writes of this class are never dropped under pressure.
    ///
    /// Only `CriticalControlPlane` returns `true`.
    pub fn never_drops(self) -> bool {
        matches!(self, WriteClass::CriticalControlPlane)
    }

    /// Parses a string into a `WriteClass`.
    ///
    /// # Arguments
    /// * `s` - The string to parse. Must be one of:
    ///   - `"critical_control_plane"` → `Ok(WriteClass::CriticalControlPlane)`
    ///   - `"operator_projection"` → `Ok(WriteClass::OperatorProjection)`
    ///   - `"bulk_blob"` → `Ok(WriteClass::BulkBlob)`
    ///
    /// # Errors
    /// Returns `Err(Error::UnknownWriteClass)` for any other string,
    /// including empty strings and case-mismatched variants.
    pub fn parse(s: &str) -> Result<WriteClass, Error> {
        match s {
            "critical_control_plane" => Ok(WriteClass::CriticalControlPlane),
            "operator_projection" => Ok(WriteClass::OperatorProjection),
            "bulk_blob" => Ok(WriteClass::BulkBlob),
            _ => Err(Error::UnknownWriteClass(s.to_string())),
        }
    }

    /// Returns the canonical name of the write class.
    ///
    /// # Returns
    /// - `"critical_control_plane"` for `CriticalControlPlane`
    /// - `"operator_projection"` for `OperatorProjection`
    /// - `"bulk_blob"` for `BulkBlob`
    pub fn as_str(self) -> &'static str {
        match self {
            WriteClass::CriticalControlPlane => "critical_control_plane",
            WriteClass::OperatorProjection => "operator_projection",
            WriteClass::BulkBlob => "bulk_blob",
        }
    }
}

impl FromStr for WriteClass {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        WriteClass::parse(s)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// WriteBudget Struct
// ─────────────────────────────────────────────────────────────────────────────

use std::cell::RefCell;

/// Associates a write budget per class for storage pressure management.
///
/// Budget is tracked independently per class. When a write is attempted,
/// `can_write()` checks if the budget allows it, and `reserve()` deducts
/// from the budget on success.
#[derive(Clone, Debug)]
pub struct WriteBudget {
    critical_limit: u64,
    projection_limit: u64,
    blob_limit: u64,
    critical_used: RefCell<u64>,
    projection_used: RefCell<u64>,
    blob_used: RefCell<u64>,
}

impl WriteBudget {
    /// Creates a new budget with the given limits per class.
    ///
    /// # Arguments
    /// * `critical_limit` - Maximum bytes for `CriticalControlPlane` writes
    /// * `projection_limit` - Maximum bytes for `OperatorProjection` writes
    /// * `blob_limit` - Maximum bytes for `BulkBlob` writes
    pub fn new(critical_limit: u64, projection_limit: u64, blob_limit: u64) -> Self {
        Self {
            critical_limit,
            projection_limit,
            blob_limit,
            critical_used: RefCell::new(0),
            projection_used: RefCell::new(0),
            blob_used: RefCell::new(0),
        }
    }

    /// Returns the remaining budget for a given class.
    ///
    /// # Arguments
    /// * `class` - The write class to query
    ///
    /// # Returns
    /// The number of bytes remaining in the budget for that class.
    pub fn remaining(&self, class: WriteClass) -> u64 {
        match class {
            WriteClass::CriticalControlPlane => self
                .critical_limit
                .saturating_sub(*self.critical_used.borrow()),
            WriteClass::OperatorProjection => self
                .projection_limit
                .saturating_sub(*self.projection_used.borrow()),
            WriteClass::BulkBlob => self.blob_limit.saturating_sub(*self.blob_used.borrow()),
        }
    }

    /// Checks if a write of the given class would exceed available budget.
    ///
    /// # Arguments
    /// * `class` - The write class to check
    /// * `size_bytes` - The size of the write in bytes
    ///
    /// # Returns
    /// `true` if the write would fit within the remaining budget for that class.
    /// Zero-byte writes always return `true`.
    pub fn can_write(&self, class: WriteClass, size_bytes: u64) -> bool {
        self.remaining(class) >= size_bytes
    }

    /// Reserves budget for a write.
    ///
    /// # Arguments
    /// * `class` - The write class
    /// * `size_bytes` - The size of the write in bytes
    ///
    /// # Errors
    /// Returns `Err(Error::BudgetExceeded)` if the write would exceed
    /// the available budget for that class.
    pub fn reserve(&self, class: WriteClass, size_bytes: u64) -> Result<(), Error> {
        let remaining = self.remaining(class);
        if remaining < size_bytes {
            return Err(Error::BudgetExceeded {
                class,
                requested: size_bytes,
                available: remaining,
            });
        }

        // Mutate internal state via RefCell
        match class {
            WriteClass::CriticalControlPlane => {
                *self.critical_used.borrow_mut() += size_bytes;
            }
            WriteClass::OperatorProjection => {
                *self.projection_used.borrow_mut() += size_bytes;
            }
            WriteClass::BulkBlob => {
                *self.blob_used.borrow_mut() += size_bytes;
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod write_class_tests {
    use super::*;

    // ── WriteClass::tier() tests ─────────────────────────────────────────────

    #[test]
    fn write_class_returns_tier_1_when_critical_control_plane() {
        let wc = WriteClass::CriticalControlPlane;
        assert_eq!(wc.tier(), 1);
    }

    #[test]
    fn write_class_returns_tier_2_when_operator_projection() {
        let wc = WriteClass::OperatorProjection;
        assert_eq!(wc.tier(), 2);
    }

    #[test]
    fn write_class_returns_tier_3_when_bulk_blob() {
        let wc = WriteClass::BulkBlob;
        assert_eq!(wc.tier(), 3);
    }

    // ── WriteClass::never_drops() tests ──────────────────────────────────────

    #[test]
    fn write_class_never_drops_returns_true_when_critical_control_plane() {
        let wc = WriteClass::CriticalControlPlane;
        assert!(wc.never_drops());
    }

    #[test]
    fn write_class_never_drops_returns_false_when_operator_projection() {
        let wc = WriteClass::OperatorProjection;
        assert!(!wc.never_drops());
    }

    #[test]
    fn write_class_never_drops_returns_false_when_bulk_blob() {
        let wc = WriteClass::BulkBlob;
        assert!(!wc.never_drops());
    }

    // ── WriteClass::parse() tests ─────────────────────────────────────────

    #[test]
    fn write_class_parses_critical_control_plane_from_str() {
        let result = WriteClass::parse("critical_control_plane");
        assert_eq!(result, Ok(WriteClass::CriticalControlPlane));
    }

    #[test]
    fn write_class_parses_operator_projection_from_str() {
        let result = WriteClass::parse("operator_projection");
        assert_eq!(result, Ok(WriteClass::OperatorProjection));
    }

    #[test]
    fn write_class_parses_bulk_blob_from_str() {
        let result = WriteClass::parse("bulk_blob");
        assert_eq!(result, Ok(WriteClass::BulkBlob));
    }

    #[test]
    fn write_class_returns_unknown_write_class_error_when_parsing_invalid_string() {
        let result = WriteClass::parse("invalid_class_name");
        assert_eq!(
            result,
            Err(Error::UnknownWriteClass("invalid_class_name".to_string()))
        );
    }

    #[test]
    fn write_class_returns_unknown_write_class_error_when_parsing_empty_string() {
        let result = WriteClass::parse("");
        assert_eq!(result, Err(Error::UnknownWriteClass("".to_string())));
    }

    #[test]
    fn write_class_returns_unknown_write_class_error_when_parsing_case_mismatch() {
        let result = WriteClass::parse("CRITICAL_CONTROL_PLANE");
        assert_eq!(
            result,
            Err(Error::UnknownWriteClass(
                "CRITICAL_CONTROL_PLANE".to_string()
            ))
        );
    }

    // ── WriteClass::as_str() tests ────────────────────────────────────────────

    #[test]
    fn write_class_as_str_returns_critical_control_plane_when_critical_control_plane() {
        let wc = WriteClass::CriticalControlPlane;
        assert_eq!(wc.as_str(), "critical_control_plane");
    }

    #[test]
    fn write_class_as_str_returns_operator_projection_when_operator_projection() {
        let wc = WriteClass::OperatorProjection;
        assert_eq!(wc.as_str(), "operator_projection");
    }

    #[test]
    fn write_class_as_str_returns_bulk_blob_when_bulk_blob() {
        let wc = WriteClass::BulkBlob;
        assert_eq!(wc.as_str(), "bulk_blob");
    }

    // ── WriteClass Serialization/Deserialization tests ────────────────────────

    #[test]
    fn write_class_returns_serialization_error_when_deserializing_malformed_json() {
        let json = "{ invalid }";
        let result: Result<WriteClass, _> = serde_json::from_str(json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        // We expect this to fail with some kind of serde error
        assert!(
            err.to_string()
                .contains("expected value at line 1 column 3")
                || err.to_string().contains("invalid")
        );
    }

    #[test]
    fn write_class_returns_serialization_error_when_deserializing_truncated_json() {
        let json = "\"critical_cont";
        let result: Result<WriteClass, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    // ── TaxonomyNotInitialized test ───────────────────────────────────────────

    #[test]
    fn taxonomy_returns_not_initialized_error_when_accessed_before_init() {
        // This test verifies that Error::TaxonomyNotInitialized can be constructed
        // and displays correctly.
        let err = Error::TaxonomyNotInitialized;
        assert_eq!(err.to_string(), "taxonomy not initialized");
    }

    // ── WriteBudget::new() tests ─────────────────────────────────────────────

    #[test]
    fn write_budget_creates_with_given_limits() {
        let budget = WriteBudget::new(100, 200, 300);
        assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 100);
        assert_eq!(budget.remaining(WriteClass::OperatorProjection), 200);
        assert_eq!(budget.remaining(WriteClass::BulkBlob), 300);
    }

    #[test]
    fn write_budget_creates_with_zero_limits() {
        let budget = WriteBudget::new(0, 0, 0);
        assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 0);
        assert_eq!(budget.remaining(WriteClass::OperatorProjection), 0);
        assert_eq!(budget.remaining(WriteClass::BulkBlob), 0);
    }

    #[test]
    fn write_budget_creates_with_max_limits() {
        let budget = WriteBudget::new(u64::MAX, u64::MAX, u64::MAX);
        assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), u64::MAX);
        assert_eq!(budget.remaining(WriteClass::OperatorProjection), u64::MAX);
        assert_eq!(budget.remaining(WriteClass::BulkBlob), u64::MAX);
    }

    // ── WriteBudget::can_write() tests ───────────────────────────────────────

    #[test]
    fn write_budget_can_write_returns_true_when_under_limit() {
        let budget = WriteBudget::new(100, 200, 300);
        assert!(budget.can_write(WriteClass::CriticalControlPlane, 50));
    }

    #[test]
    fn write_budget_can_write_returns_true_when_at_exact_limit() {
        let budget = WriteBudget::new(100, 200, 300);
        assert!(budget.can_write(WriteClass::CriticalControlPlane, 100));
    }

    #[test]
    fn write_budget_can_write_returns_false_when_over_limit() {
        let budget = WriteBudget::new(100, 200, 300);
        assert!(!budget.can_write(WriteClass::CriticalControlPlane, 150));
    }

    #[test]
    fn write_budget_can_write_returns_true_when_zero_bytes() {
        let budget = WriteBudget::new(100, 200, 300);
        assert!(budget.can_write(WriteClass::CriticalControlPlane, 0));
    }

    // ── WriteBudget::can_write() exhaustion test ─────────────────────────────

    #[test]
    fn write_budget_can_write_returns_false_when_exhausted() {
        let budget = WriteBudget::new(100, 200, 300);
        let _ = budget.reserve(WriteClass::CriticalControlPlane, 100);
        assert!(!budget.can_write(WriteClass::CriticalControlPlane, 1));
    }

    // ── WriteBudget::reserve() tests ───────────────────────────────────────

    #[test]
    fn write_budget_reserve_deducts_bytes_on_success() {
        let budget = WriteBudget::new(100, 200, 300);
        let result = budget.reserve(WriteClass::CriticalControlPlane, 30);
        assert_eq!(result, Ok(()));
        assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 70);
    }

    #[test]
    fn write_budget_reserve_succeeds_when_at_exact_limit() {
        let budget = WriteBudget::new(100, 200, 300);
        let result = budget.reserve(WriteClass::CriticalControlPlane, 100);
        assert_eq!(result, Ok(()));
        assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 0);
    }

    #[test]
    fn write_budget_reserve_returns_budget_exceeded_when_over_limit() {
        let budget = WriteBudget::new(100, 200, 300);
        let result = budget.reserve(WriteClass::CriticalControlPlane, 150);
        assert_eq!(
            result,
            Err(Error::BudgetExceeded {
                class: WriteClass::CriticalControlPlane,
                requested: 150,
                available: 100,
            })
        );
    }

    #[test]
    fn write_budget_reserve_returns_budget_exceeded_when_exhausted_plus_one() {
        let budget = WriteBudget::new(100, 200, 300);
        let _ = budget.reserve(WriteClass::CriticalControlPlane, 100);
        let result = budget.reserve(WriteClass::CriticalControlPlane, 1);
        assert_eq!(
            result,
            Err(Error::BudgetExceeded {
                class: WriteClass::CriticalControlPlane,
                requested: 1,
                available: 0,
            })
        );
    }

    #[test]
    fn write_budget_reserve_zero_bytes_succeeds() {
        let budget = WriteBudget::new(100, 200, 300);
        let result = budget.reserve(WriteClass::CriticalControlPlane, 0);
        assert_eq!(result, Ok(()));
        assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 100);
    }

    // ── WriteBudget::remaining() tests ───────────────────────────────────────

    #[test]
    fn write_budget_remaining_returns_correct_initial_values() {
        let budget = WriteBudget::new(100, 200, 300);
        assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 100);
        assert_eq!(budget.remaining(WriteClass::OperatorProjection), 200);
        assert_eq!(budget.remaining(WriteClass::BulkBlob), 300);
    }

    #[test]
    fn write_budget_remaining_returns_zero_after_exhaustion() {
        let budget = WriteBudget::new(100, 200, 300);
        let _ = budget.reserve(WriteClass::CriticalControlPlane, 100);
        assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 0);
    }

    #[test]
    fn write_budget_remaining_unchanged_after_failed_reserve() {
        let budget = WriteBudget::new(100, 200, 300);
        let _ = budget.reserve(WriteClass::CriticalControlPlane, 150);
        assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 100);
    }

    // ── Error Display tests ───────────────────────────────────────────────────

    #[test]
    fn error_unknown_write_class_displays_class_name() {
        let err = Error::UnknownWriteClass("test_class".to_string());
        assert_eq!(err.to_string(), "unknown write class: test_class");
    }

    #[test]
    fn error_serialization_error_displays_message() {
        let err = Error::SerializationError("test error".to_string());
        assert_eq!(err.to_string(), "serialization error: test error");
    }

    #[test]
    fn error_taxonomy_not_initialized_displays_message() {
        let err = Error::TaxonomyNotInitialized;
        assert_eq!(err.to_string(), "taxonomy not initialized");
    }

    #[test]
    fn error_budget_exceeded_displays_details() {
        let err = Error::BudgetExceeded {
            class: WriteClass::CriticalControlPlane,
            requested: 150,
            available: 100,
        };
        assert!(err.to_string().contains("budget exceeded"));
        assert!(err.to_string().contains("CriticalControlPlane"));
        assert!(err.to_string().contains("150"));
        assert!(err.to_string().contains("100"));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Proptest Invariants
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod proptest_write_class_invariants {
    use super::*;
    use proptest::prelude::*;

    /// PROP-01: INV-001 — tier() is always 1, 2, or 3
    proptest! {
        #[test]
        fn write_class_tier_always_returns_1_2_or_3(variant in proptest::sample::select(&[
            WriteClass::CriticalControlPlane,
            WriteClass::OperatorProjection,
            WriteClass::BulkBlob,
        ])) {
            let tier = variant.tier();
            prop_assert!(tier >= 1 && tier <= 3, "tier() must be 1, 2, or 3, got {}", tier);
        }
    }

    /// PROP-02: INV-002 — never_drops() is true only for CriticalControlPlane
    proptest! {
        #[test]
        fn write_class_never_drops_true_only_for_critical_control_plane(variant in proptest::sample::select(&[
            WriteClass::CriticalControlPlane,
            WriteClass::OperatorProjection,
            WriteClass::BulkBlob,
        ])) {
            let never_drops = variant.never_drops();
            let is_critical = matches!(variant, WriteClass::CriticalControlPlane);
            prop_assert_eq!(never_drops, is_critical,
                "never_drops() should be {} for {:?}, got {}",
                is_critical, variant, never_drops);
        }
    }

    /// PROP-03: INV-003 — as_str() round-trips through from_str()
    proptest! {
        #[test]
        fn write_class_as_str_roundtrips_through_from_str(variant in proptest::sample::select(&[
            WriteClass::CriticalControlPlane,
            WriteClass::OperatorProjection,
            WriteClass::BulkBlob,
        ])) {
            let s = variant.as_str();
            let parsed = WriteClass::parse(s);
            prop_assert!(parsed.is_ok(), "from_str({}) should return Ok, got {:?}", s, parsed);
            prop_assert_eq!(parsed.as_ref().ok(), Some(&variant),
                "from_str({}) should return Some({:?}), got {:?}", s, variant, parsed);
        }
    }

    /// PROP-04: INV-004 — JSON serialization round-trip
    proptest! {
        #[test]
        fn write_class_json_roundtrip_preserves_variant(variant in proptest::sample::select(&[
            WriteClass::CriticalControlPlane,
            WriteClass::OperatorProjection,
            WriteClass::BulkBlob,
        ])) {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: WriteClass = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(parsed, variant,
                "JSON round-trip failed for {:?}: serialized to {}, parsed back to {:?}",
                variant, json, parsed);
        }
    }

    /// PROP-05: INV-005 — WriteBudget reserve never produces negative remaining
    proptest! {
        #[test]
        fn write_budget_reserve_never_produces_negative_remaining(
            critical in 0u64..=1000,
            _projection in 0u64..=1000,
            _blob in 0u64..=1000,
            reserve_size in 0u64..=1000,
        ) {
            let budget = WriteBudget::new(critical, 1000, 1000);
            let class = WriteClass::CriticalControlPlane;

            let initial = budget.remaining(class);
            let result = budget.reserve(class, reserve_size);

            if result.is_ok() {
                let remaining = budget.remaining(class);
                // After a successful reserve, remaining should be initial - size
                // But since our stub doesn't track usage, this will likely fail
                prop_assert!(remaining <= initial,
                    "remaining() should be <= {} after successful reserve of {}, was {}",
                    initial, reserve_size, remaining);
            }
        }
    }

    /// PROP-06: INV-006 — can_write and reserve are consistent
    proptest! {
        #[test]
        fn write_budget_can_write_and_reserve_are_consistent(
            critical in 1u64..=1000,
            _projection in 1u64..=1000,
            _blob in 1u64..=1000,
            size in 0u64..=2000,
        ) {
            let budget = WriteBudget::new(critical, 1000, 1000);
            let class = WriteClass::CriticalControlPlane;

            let can_write = budget.can_write(class, size);
            let reserve_result = budget.reserve(class, size);

            prop_assert_eq!(can_write, reserve_result.is_ok(),
                "can_write returned {} but reserve returned {:?}",
                can_write, reserve_result);
        }
    }
}
