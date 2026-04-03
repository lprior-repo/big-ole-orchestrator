//! Proptest invariants for Upcaster and UpcasterRegistry.
//!
//! These tests verify properties that should hold for ANY valid implementation.

use proptest::prelude::*;
use vo_core::upcaster::{Upcaster, UpcasterError};

// =============================================================================
// Upcaster Invariants
// =============================================================================

proptest! {
    /// Invariant: upcast is deterministic - same input always returns same output (or same error)
    #[test]
    fn upcaster_is_deterministic(source_version in 0u8..=10u8) {
        let upcaster = TestDeterministicUpcaster { source_version };
        let input = br#"{"version": 0, "payload": {}}"#;

        let result1 = upcaster.upcast(input);
        let result2 = upcaster.upcast(input);

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

    fn upcast(&self, _input: &[u8]) -> Result<Vec<u8>, UpcasterError> {
        // RED PHASE: This is a stub that returns an error
        Err(UpcasterError::UpcastingFailed("stub".to_string()))
    }
}
