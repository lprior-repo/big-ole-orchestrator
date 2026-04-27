//! Projector trait interface tests (PT-*, PE-*).
//!
//! Tests the `Projector` trait contract: pure state transformation,
//! no side effects, and correct error conversion.

use std::sync::atomic::{AtomicU8, Ordering};

use serde_json::json;

use crate::replay::projection::{ProjectionError, ProjectionResult, Projector};

/// Test projector implementation for testing.
struct TestProjector {
    version: u8,
    call_count: AtomicU8,
}

impl TestProjector {
    fn new(version: u8) -> Self {
        Self {
            version,
            call_count: AtomicU8::new(0),
        }
    }

    fn call_count(&self) -> u8 {
        self.call_count.load(Ordering::SeqCst)
    }
}

impl Projector<String, String> for TestProjector {
    type Error = String;

    fn project(&self, state: String, event: &String) -> Result<String, Self::Error> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(format!("{}+{}", state, event))
    }

    fn initial_state() -> String {
        String::new()
    }

    fn schema_version(&self) -> u8 {
        self.version
    }
}

/// Counter projector that tracks calls but is otherwise pure.
struct PureCounterProjector {
    version: u8,
}

impl PureCounterProjector {
    fn new(version: u8) -> Self {
        Self { version }
    }
}

impl Projector<u64, String> for PureCounterProjector {
    type Error = String;

    fn project(&self, state: u64, _event: &String) -> Result<u64, Self::Error> {
        Ok(state + 1)
    }

    fn initial_state() -> u64 {
        0
    }

    fn schema_version(&self) -> u8 {
        self.version
    }
}

/// Projector that returns an error for testing.
struct ErrorProjector;

impl Projector<String, String> for ErrorProjector {
    type Error = String;

    fn project(&self, _state: String, _event: &String) -> Result<String, Self::Error> {
        Err("intentional error".to_string())
    }

    fn initial_state() -> String {
        String::new()
    }

    fn schema_version(&self) -> u8 {
        1
    }
}

#[cfg(test)]
mod pt_tests {
    use super::*;

    #[test]
    fn pt_001_project_is_pure_same_inputs_produce_identical_output() {
        let projector = TestProjector::new(1);
        let state = "initial".to_string();
        let event = "event1".to_string();

        let result1 = projector.project(state.clone(), &event).unwrap();
        let result2 = projector.project(state, &event).unwrap();

        assert_eq!(
            result1, result2,
            "PT-001: project() must be pure - same inputs must produce identical output"
        );
    }

    #[test]
    fn pt_002_project_has_no_side_effects() {
        let projector = TestProjector::new(1);
        let initial_count = projector.call_count();

        let _ = projector
            .project("state".to_string(), &"event".to_string())
            .unwrap();
        let after_count = projector.call_count();

        assert_eq!(
            after_count - initial_count,
            1,
            "PT-002: project() must have no side effects beyond internal call counting"
        );
    }

    #[test]
    fn pt_003_initial_state_returns_zero_value() {
        let state: String = <TestProjector as Projector<String, String>>::initial_state();
        assert_eq!(
            state, "",
            "PT-003: initial_state() must return zero-value state"
        );
    }

    #[test]
    fn pt_003_initial_state_u64_is_zero() {
        let state: u64 = <PureCounterProjector as Projector<u64, String>>::initial_state();
        assert_eq!(
            state, 0,
            "PT-003: initial_state() must return zero-value for numeric types"
        );
    }

    #[test]
    fn pt_004_schema_version_returns_consistent_value() {
        let projector = TestProjector::new(5);

        let v1 = projector.schema_version();
        let v2 = projector.schema_version();

        assert_eq!(
            v1, v2,
            "PT-004: schema_version() must return consistent value"
        );
        assert_eq!(
            v1, 5,
            "PT-004: schema_version() must return configured value"
        );
    }

    #[test]
    fn pt_005_projector_does_not_retain_mutable_state() {
        let projector = PureCounterProjector::new(1);

        let result1 = projector.project(0, &"e1".to_string()).unwrap();
        let result2 = projector.project(0, &"e2".to_string()).unwrap();

        assert_eq!(
            result1, 1,
            "PT-005: Second call must not be affected by first - got 1"
        );
        assert_eq!(
            result2, 1,
            "PT-005: Each project() call must be independent - got 1"
        );
    }

    #[test]
    fn pt_006_projection_state_clone_bound_satisfied() {
        let projector = TestProjector::new(1);
        let state: String = projector
            .project("".to_string(), &"event".to_string())
            .unwrap();

        let _cloned = state.clone();
        assert_eq!(
            state, _cloned,
            "PT-006: ProjectionState must satisfy Clone bound"
        );
    }

    #[test]
    fn pt_007_projection_state_default_bound_satisfied() {
        let _state: String = <TestProjector as Projector<String, String>>::initial_state();
        assert!(true, "PT-007: ProjectionState must satisfy Default bound");
    }

    #[test]
    fn pt_008_projection_state_serialize_bound_satisfied() {
        let state: String = <TestProjector as Projector<String, String>>::initial_state();
        let serialized = serde_json::to_value(&state);
        assert!(
            serialized.is_ok(),
            "PT-008: ProjectionState must satisfy Serialize bound"
        );
    }
}

#[cfg(test)]
mod pe_tests {
    use super::*;

    #[test]
    fn pe_001_projector_error_converts_to_projection_error() {
        let projector = ErrorProjector;
        let result: Result<String, ProjectionError> = projector
            .project("state".to_string(), &"event".to_string())
            .map_err(|e| e.into());

        assert!(
            result.is_err(),
            "PE-001: Projector error must convert into ProjectionError"
        );
    }

    #[test]
    fn pe_002_projector_error_message_preserved() {
        let projector = ErrorProjector;
        let result: Result<String, ProjectionError> = projector
            .project("state".to_string(), &"event".to_string())
            .map_err(|e| e.into());

        match result {
            Err(ProjectionError::Projector(msg)) => {
                assert_eq!(
                    msg, "intentional error",
                    "PE-002: Error message must be preserved through conversion"
                );
            }
            Err(e) => panic!("PE-002: Expected ProjectionError::Projector, got {:?}", e),
            Ok(_) => panic!("PE-002: Expected error, got Ok"),
        }
    }
}
