//! ProjectionState lifecycle state machine tests (LS-*, LT-*, LI-*, SR-*).
//!
//! Tests the `ProjectionState` state machine: valid/invalid transitions,
//! StaleReason variants, and terminal state handling.

use crate::replay::projection::{ProjectionState, ProjectionStateError, StaleReason};

#[cfg(test)]
mod ls_tests {
    use super::*;

    #[test]
    fn ls_001_projection_state_building_constructs() {
        let state = ProjectionState::Building;
        assert!(
            matches!(state, ProjectionState::Building),
            "LS-001: Building state must construct"
        );
    }

    #[test]
    fn ls_002_projection_state_ready_constructs() {
        let state = ProjectionState::Ready;
        assert!(
            matches!(state, ProjectionState::Ready),
            "LS-002: Ready state must construct"
        );
    }

    #[test]
    fn ls_003_projection_state_stale_constructs() {
        let state = ProjectionState::Stale {
            detected_at: 1000,
            reason: StaleReason::ManualInvalidation,
        };

        match &state {
            ProjectionState::Stale {
                detected_at,
                reason,
            } => {
                assert_eq!(*detected_at, 1000, "LS-003: detected_at must be accessible");
                assert!(
                    matches!(reason, StaleReason::ManualInvalidation),
                    "LS-003: reason must be accessible"
                );
            }
            _ => panic!("LS-003: Expected Stale state"),
        }
    }

    #[test]
    fn ls_004_projection_state_rebuilding_constructs() {
        let state = ProjectionState::Rebuilding {
            progress: 50,
            from_sequence: 100,
        };

        match &state {
            ProjectionState::Rebuilding {
                progress,
                from_sequence,
            } => {
                assert_eq!(*progress, 50, "LS-004: progress must be accessible");
                assert_eq!(
                    *from_sequence, 100,
                    "LS-004: from_sequence must be accessible"
                );
            }
            _ => panic!("LS-004: Expected Rebuilding state"),
        }
    }

    #[test]
    fn ls_005_projection_state_failed_constructs() {
        let state = ProjectionState::Failed {
            reason: "test failure".to_string(),
            attempted_at: 5000,
        };

        match &state {
            ProjectionState::Failed {
                reason,
                attempted_at,
            } => {
                assert_eq!(reason, "test failure", "LS-005: reason must be accessible");
                assert_eq!(
                    *attempted_at, 5000,
                    "LS-005: attempted_at must be accessible"
                );
            }
            _ => panic!("LS-005: Expected Failed state"),
        }
    }
}

#[cfg(test)]
mod sr_tests {
    use super::*;

    #[test]
    fn sr_001_stale_reason_schema_version_mismatch_constructs() {
        let reason = StaleReason::SchemaVersionMismatch {
            expected: 5,
            actual: 3,
        };

        match &reason {
            StaleReason::SchemaVersionMismatch { expected, actual } => {
                assert_eq!(*expected, 5, "SR-001: expected must be 5");
                assert_eq!(*actual, 3, "SR-001: actual must be 3");
            }
            _ => panic!("SR-001: Expected SchemaVersionMismatch"),
        }
    }

    #[test]
    fn sr_002_stale_reason_sequence_gap_detected_constructs() {
        let reason = StaleReason::SequenceGapDetected { gap_at: 42 };

        match &reason {
            StaleReason::SequenceGapDetected { gap_at } => {
                assert_eq!(*gap_at, 42, "SR-002: gap_at must be 42");
            }
            _ => panic!("SR-002: Expected SequenceGapDetected"),
        }
    }

    #[test]
    fn sr_003_stale_reason_corruption_detected_constructs() {
        let reason = StaleReason::CorruptionDetected;
        assert!(
            matches!(reason, StaleReason::CorruptionDetected),
            "SR-003: CorruptionDetected must construct"
        );
    }

    #[test]
    fn sr_004_stale_reason_manual_invalidation_constructs() {
        let reason = StaleReason::ManualInvalidation;
        assert!(
            matches!(reason, StaleReason::ManualInvalidation),
            "SR-004: ManualInvalidation must construct"
        );
    }
}

#[cfg(test)]
mod state_machine_tests {
    use super::*;
    use crate::replay::projection::ProjectionStateManager;

    fn create_state_manager() -> ProjectionStateManager {
        ProjectionStateManager::new()
    }

    #[test]
    fn lt_001_transition_building_to_ready() {
        let mgr = create_state_manager();
        mgr.set_state("p1", ProjectionState::Building).unwrap();

        let result = mgr.transition_to("p1", ProjectionState::Ready);
        assert!(result.is_ok(), "LT-001: Building -> Ready must succeed");
    }

    #[test]
    fn lt_002_transition_building_to_failed() {
        let mgr = create_state_manager();
        mgr.set_state("p1", ProjectionState::Building).unwrap();

        let result = mgr.transition_to(
            "p1",
            ProjectionState::Failed {
                reason: "build failed".to_string(),
                attempted_at: 1000,
            },
        );
        assert!(result.is_ok(), "LT-002: Building -> Failed must succeed");
    }

    #[test]
    fn lt_003_transition_ready_to_stale() {
        let mgr = create_state_manager();
        mgr.set_state("p1", ProjectionState::Ready).unwrap();

        let result = mgr.transition_to(
            "p1",
            ProjectionState::Stale {
                detected_at: 1000,
                reason: StaleReason::ManualInvalidation,
            },
        );
        assert!(result.is_ok(), "LT-003: Ready -> Stale must succeed");
    }

    #[test]
    fn lt_004_transition_stale_to_rebuilding() {
        let mgr = create_state_manager();
        mgr.set_state(
            "p1",
            ProjectionState::Stale {
                detected_at: 1000,
                reason: StaleReason::ManualInvalidation,
            },
        )
        .unwrap();

        let result = mgr.transition_to(
            "p1",
            ProjectionState::Rebuilding {
                progress: 0,
                from_sequence: 1001,
            },
        );
        assert!(result.is_ok(), "LT-004: Stale -> Rebuilding must succeed");
    }

    #[test]
    fn lt_005_transition_rebuilding_to_ready() {
        let mgr = create_state_manager();
        mgr.set_state(
            "p1",
            ProjectionState::Rebuilding {
                progress: 100,
                from_sequence: 1,
            },
        )
        .unwrap();

        let result = mgr.transition_to("p1", ProjectionState::Ready);
        assert!(result.is_ok(), "LT-005: Rebuilding -> Ready must succeed");
    }

    #[test]
    fn lt_006_transition_rebuilding_to_failed() {
        let mgr = create_state_manager();
        mgr.set_state(
            "p1",
            ProjectionState::Rebuilding {
                progress: 50,
                from_sequence: 1,
            },
        )
        .unwrap();

        let result = mgr.transition_to(
            "p1",
            ProjectionState::Failed {
                reason: "rebuild failed".to_string(),
                attempted_at: 2000,
            },
        );
        assert!(result.is_ok(), "LT-006: Rebuilding -> Failed must succeed");
    }

    #[test]
    fn lt_007_transition_failed_to_building() {
        let mgr = create_state_manager();
        mgr.set_state(
            "p1",
            ProjectionState::Failed {
                reason: "previous failure".to_string(),
                attempted_at: 1000,
            },
        )
        .unwrap();

        let result = mgr.transition_to("p1", ProjectionState::Building);
        assert!(
            result.is_ok(),
            "LT-007: Failed -> Building (manual reset) must succeed"
        );
    }

    #[test]
    fn li_001_invalid_failed_to_ready() {
        let mgr = create_state_manager();
        mgr.set_state(
            "p1",
            ProjectionState::Failed {
                reason: "terminal".to_string(),
                attempted_at: 1000,
            },
        )
        .unwrap();

        let result = mgr.transition_to("p1", ProjectionState::Ready);
        assert!(
            result.is_err(),
            "LI-001: Failed -> Ready must be invalid (terminal state)"
        );

        match result {
            Err(ProjectionStateError::TerminalStateTransition { .. }) => {}
            Err(e) => panic!("LI-001: Expected TerminalStateTransition, got {:?}", e),
            Ok(_) => panic!("LI-001: Expected error, got Ok"),
        }
    }

    #[test]
    fn li_002_invalid_ready_to_rebuilding() {
        let mgr = create_state_manager();
        mgr.set_state("p1", ProjectionState::Ready).unwrap();

        let result = mgr.transition_to(
            "p1",
            ProjectionState::Rebuilding {
                progress: 0,
                from_sequence: 1,
            },
        );
        assert!(
            result.is_err(),
            "LI-002: Ready -> Rebuilding must fail (not stale)"
        );
    }

    #[test]
    fn li_003_invalid_building_to_stale() {
        let mgr = create_state_manager();
        mgr.set_state("p1", ProjectionState::Building).unwrap();

        let result = mgr.transition_to(
            "p1",
            ProjectionState::Stale {
                detected_at: 1000,
                reason: StaleReason::ManualInvalidation,
            },
        );
        assert!(result.is_err(), "LI-003: Building -> Stale must be invalid");
    }

    #[test]
    fn li_004_invalid_ready_to_building() {
        let mgr = create_state_manager();
        mgr.set_state("p1", ProjectionState::Ready).unwrap();

        let result = mgr.transition_to("p1", ProjectionState::Building);
        assert!(result.is_err(), "LI-004: Ready -> Building must be invalid");
    }

    #[test]
    fn li_005_invalid_stale_to_failed() {
        let mgr = create_state_manager();
        mgr.set_state(
            "p1",
            ProjectionState::Stale {
                detected_at: 1000,
                reason: StaleReason::ManualInvalidation,
            },
        )
        .unwrap();

        let result = mgr.transition_to(
            "p1",
            ProjectionState::Failed {
                reason: "direct fail".to_string(),
                attempted_at: 2000,
            },
        );
        assert!(result.is_err(), "LI-005: Stale -> Failed must be invalid");
    }

    #[test]
    fn state_is_terminal() {
        assert!(
            !ProjectionState::Building.is_terminal(),
            "Building must not be terminal"
        );
        assert!(
            !ProjectionState::Ready.is_terminal(),
            "Ready must not be terminal"
        );
        assert!(
            !ProjectionState::Stale {
                detected_at: 0,
                reason: StaleReason::ManualInvalidation
            }
            .is_terminal(),
            "Stale must not be terminal"
        );
        assert!(
            !ProjectionState::Rebuilding {
                progress: 50,
                from_sequence: 1
            }
            .is_terminal(),
            "Rebuilding must not be terminal"
        );
        assert!(
            ProjectionState::Failed {
                reason: "test".to_string(),
                attempted_at: 100
            }
            .is_terminal(),
            "Failed must be terminal"
        );
    }

    #[test]
    fn state_is_stale() {
        assert!(
            !ProjectionState::Building.is_stale(),
            "Building must not be stale"
        );
        assert!(
            !ProjectionState::Ready.is_stale(),
            "Ready must not be stale"
        );
        assert!(
            ProjectionState::Stale {
                detected_at: 0,
                reason: StaleReason::ManualInvalidation
            }
            .is_stale(),
            "Stale must be stale"
        );
        assert!(
            ProjectionState::Rebuilding {
                progress: 50,
                from_sequence: 1
            }
            .is_stale(),
            "Rebuilding must be stale"
        );
        assert!(
            !ProjectionState::Failed {
                reason: "test".to_string(),
                attempted_at: 100
            }
            .is_stale(),
            "Failed must not be stale"
        );
    }
}
