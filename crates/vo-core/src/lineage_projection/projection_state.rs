//! Projection state machine and transitions (ADR-038).
//!
//! Pure functions for validating state transitions and atomic swaps.

use crate::lineage_projection::types::*;

/// Simulate an atomic projection swap: validates new state before replacing old.
pub fn atomic_projection_swap(
    current_state: &ProjectionState,
    new_state: ProjectionState,
) -> ProjectionSwapResult {
    let projection_id = String::from("test-projection");
    let old_state = current_state.clone();

    let valid_transition = is_valid_state_transition(current_state, &new_state);

    if valid_transition {
        ProjectionSwapResult {
            projection_id,
            old_state,
            new_state: new_state.clone(),
            swapped: true,
        }
    } else {
        ProjectionSwapResult {
            projection_id,
            old_state,
            new_state,
            swapped: false,
        }
    }
}

/// Check if a state transition is valid.
///
/// Valid transitions:
/// - Building -> Ready (build completed)
/// - Building -> Failed (build failed)
/// - Ready -> Stale (staleness detected)
/// - Stale -> Rebuilding (rebuild initiated)
/// - Rebuilding -> Ready (rebuild completed)
/// - Rebuilding -> Failed (rebuild failed)
/// - Failed -> Building (manual reset)
pub fn is_valid_state_transition(from: &ProjectionState, to: &ProjectionState) -> bool {
    matches!(
        (from, to),
        (ProjectionState::Building, ProjectionState::Ready { .. })
            | (ProjectionState::Building, ProjectionState::Failed { .. })
            | (ProjectionState::Ready { .. }, ProjectionState::Stale { .. })
            | (
                ProjectionState::Stale { .. },
                ProjectionState::Rebuilding { .. }
            )
            | (
                ProjectionState::Rebuilding { .. },
                ProjectionState::Ready { .. }
            )
            | (
                ProjectionState::Rebuilding { .. },
                ProjectionState::Failed { .. }
            )
            | (ProjectionState::Failed { .. }, ProjectionState::Building)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_swap_valid_transition_building_to_ready() {
        let current = ProjectionState::Building;
        let new = ProjectionState::Ready {
            schema_version: 1,
            last_sequence: 100,
        };
        let result = atomic_projection_swap(&current, new.clone());
        assert!(result.swapped);
        assert_eq!(result.old_state, ProjectionState::Building);
        assert_eq!(result.new_state, new);
    }

    #[test]
    fn atomic_swap_invalid_transition_ready_to_building() {
        let current = ProjectionState::Ready {
            schema_version: 1,
            last_sequence: 100,
        };
        let new = ProjectionState::Building;
        let result = atomic_projection_swap(&current, new.clone());
        assert!(!result.swapped);
    }

    #[test]
    fn atomic_swap_invalid_transition_failed_to_ready() {
        let current = ProjectionState::Failed {
            reason: "error".to_string(),
            attempted_at: 100,
        };
        let new = ProjectionState::Ready {
            schema_version: 1,
            last_sequence: 0,
        };
        let result = atomic_projection_swap(&current, new.clone());
        assert!(!result.swapped);
    }

    #[test]
    fn valid_transition_building_to_ready() {
        assert!(is_valid_state_transition(
            &ProjectionState::Building,
            &ProjectionState::Ready {
                schema_version: 1,
                last_sequence: 0,
            }
        ));
    }

    #[test]
    fn valid_transition_stale_to_rebuilding() {
        assert!(is_valid_state_transition(
            &ProjectionState::Stale {
                reason: "mismatch".to_string(),
                detected_at: 0,
            },
            &ProjectionState::Rebuilding {
                progress: 0.0,
                from_sequence: 0,
            }
        ));
    }

    #[test]
    fn valid_transition_rebuilding_to_failed() {
        assert!(is_valid_state_transition(
            &ProjectionState::Rebuilding {
                progress: 0.5,
                from_sequence: 0,
            },
            &ProjectionState::Failed {
                reason: "error".to_string(),
                attempted_at: 100,
            }
        ));
    }

    #[test]
    fn valid_transition_failed_to_building() {
        assert!(is_valid_state_transition(
            &ProjectionState::Failed {
                reason: "error".to_string(),
                attempted_at: 100,
            },
            &ProjectionState::Building
        ));
    }

    #[test]
    fn invalid_transition_ready_to_building() {
        assert!(!is_valid_state_transition(
            &ProjectionState::Ready {
                schema_version: 1,
                last_sequence: 0,
            },
            &ProjectionState::Building
        ));
    }

    #[test]
    fn invalid_transition_failed_to_ready() {
        assert!(!is_valid_state_transition(
            &ProjectionState::Failed {
                reason: "error".to_string(),
                attempted_at: 100,
            },
            &ProjectionState::Ready {
                schema_version: 1,
                last_sequence: 0,
            }
        ));
    }
}
