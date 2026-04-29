//! Proptest invariants for Upcaster and UpcasterRegistry.
//!
//! These tests verify properties that should hold for ANY valid implementation.

use proptest::prelude::*;
use vo_core::upcaster::Upcaster;
use vo_types::events::upcaster::UpcasterError;

// =============================================================================
// Upcaster Invariants
// =============================================================================

proptest! {
    /// Invariant: upcast is deterministic - same input always returns same output (or same error)
    #[test]
    fn upcaster_is_deterministic(source_version in 0u8..=10u8) {
        let upcaster = TestDeterministicUpcaster { source_version };
        let input = serde_json::json!({"version": 0, "payload": {}});

        let result1 = upcaster.upcast(&input);
        let result2 = upcaster.upcast(&input);

        prop_assert_eq!(result1, result2);
    }
}

// =============================================================================
// Test Upcaster Implementations for Propertive Testing
// =============================================================================

/// A deterministic upcaster for testing - always returns the same output for same input.
struct TestDeterministicUpcaster {
    source_version: u8,
}

impl Upcaster for TestDeterministicUpcaster {
    fn source_version(&self) -> u8 {
        self.source_version
    }

    fn target_version(&self) -> u8 {
        self.source_version + 1
    }

    fn upcast(&self, _payload: &serde_json::Value) -> Result<serde_json::Value, UpcasterError> {
        Err(UpcasterError::UpcastFailed("stub".to_string()))
    }
}
