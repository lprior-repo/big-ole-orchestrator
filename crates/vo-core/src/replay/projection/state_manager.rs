//! Projection state manager — tracks and transitions projection states.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::replay::projection::error::{ProjectionError, ProjectionStateError};
use crate::replay::projection::types::ProjectionState;

#[derive(Debug, Clone)]
pub struct ProjectionStateManager {
    states: Arc<Mutex<HashMap<String, ProjectionState>>>,
}

impl ProjectionStateManager {
    pub fn new() -> Self {
        Self {
            states: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get_state(&self, projection_id: &str) -> Option<ProjectionState> {
        self.states
            .lock()
            .ok()
            .and_then(|states| states.get(projection_id).cloned())
    }

    pub fn set_state(
        &self,
        projection_id: &str,
        state: ProjectionState,
    ) -> Result<(), ProjectionError> {
        self.states
            .lock()
            .map_err(|_| ProjectionError::InvalidState("lock poisoned".to_string()))?
            .insert(projection_id.to_string(), state);
        Ok(())
    }

    pub fn transition_to(
        &self,
        projection_id: &str,
        new_state: ProjectionState,
    ) -> Result<(), ProjectionStateError> {
        let mut states =
            self.states
                .lock()
                .map_err(|_| ProjectionStateError::InvalidTransition {
                    from: "lock poisoned".to_string(),
                    to: format!("{:?}", new_state),
                })?;

        let current = states.get(projection_id);

        let valid = matches!(
            (&current, &new_state),
            (None, _)
                | (Some(ProjectionState::Building), ProjectionState::Ready)
                | (
                    Some(ProjectionState::Building),
                    ProjectionState::Failed { .. }
                )
                | (Some(ProjectionState::Ready), ProjectionState::Stale { .. })
                | (
                    Some(ProjectionState::Ready),
                    ProjectionState::Rebuilding { .. }
                )
                | (Some(ProjectionState::Ready), ProjectionState::Failed { .. })
                | (
                    Some(ProjectionState::Stale { .. }),
                    ProjectionState::Rebuilding { .. }
                )
                | (
                    Some(ProjectionState::Stale { .. }),
                    ProjectionState::Failed { .. }
                )
                | (
                    Some(ProjectionState::Rebuilding { .. }),
                    ProjectionState::Ready
                )
                | (
                    Some(ProjectionState::Rebuilding { .. }),
                    ProjectionState::Failed { .. }
                )
                | (
                    Some(ProjectionState::Failed { .. }),
                    ProjectionState::Rebuilding { .. }
                )
        );

        if !valid {
            return Err(ProjectionStateError::InvalidTransition {
                from: format!("{:?}", current),
                to: format!("{:?}", new_state),
            });
        }

        states.insert(projection_id.to_string(), new_state);
        Ok(())
    }

    pub fn is_ready(&self, projection_id: &str) -> bool {
        matches!(self.get_state(projection_id), Some(ProjectionState::Ready))
    }

    pub fn is_stale(&self, projection_id: &str) -> bool {
        self.get_state(projection_id)
            .map(|s| s.is_stale())
            .unwrap_or(false)
    }

    pub fn is_failed(&self, projection_id: &str) -> bool {
        matches!(
            self.get_state(projection_id),
            Some(ProjectionState::Failed { .. })
        )
    }
}

impl Default for ProjectionStateManager {
    fn default() -> Self {
        Self::new()
    }
}
