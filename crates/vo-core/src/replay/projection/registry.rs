//! Projection registry for managing rebuildable projections (ADR-037).
//!
//! The registry coordinates projection lifecycle: registration, staleness detection,
//! rebuild triggering, and idempotent rebuild execution.
//!
//! ## Architecture
//!
//! ```ignore
//! let registry = ProjectionRegistry::new(engine.clone());
//! registry.register("instances", projector, max_version)?;
//! registry.check_and_rebuild_if_stale("instances", current_sequence)?;
//! ```

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::replay::projection::{
    ProjectionEngine, ProjectionError, ProjectionRebuilder, ProjectionRecord, ProjectionResult,
    ProjectionState, ProjectionStateManager, Projector, StaleReason,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionClass {
    Operational,
    Operator,
}

pub struct RegisteredProjection<S, E, P>
where
    S: Clone + Default + serde::Serialize,
    E: Clone,
    P: Projector<S, E>,
{
    pub class: ProjectionClass,
    pub projector: P,
    pub state_manager: ProjectionStateManager,
    max_supported_version: u8,
    _phantom: std::marker::PhantomData<(S, E)>,
}

impl<S, E, P> RegisteredProjection<S, E, P>
where
    S: Clone + Default + serde::Serialize,
    E: Clone,
    P: Projector<S, E>,
{
    pub fn new(projector: P, class: ProjectionClass, max_supported_version: u8) -> Self {
        Self {
            class,
            projector,
            state_manager: ProjectionStateManager::new(),
            max_supported_version,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn schema_version(&self) -> u8 {
        self.projector.schema_version()
    }

    pub fn is_schema_compatible(&self, version: u8) -> bool {
        version >= 1 && version <= self.max_supported_version
    }
}

#[derive(Debug)]
pub struct RebuildTrigger {
    pub projection_id: String,
    pub reason: StaleReason,
    pub detected_at: u64,
    pub rebuild_id: String,
}

#[derive(Debug)]
pub struct RegistryMetrics {
    pub total_projections: usize,
    pub ready_count: usize,
    pub stale_count: usize,
    pub rebuilding_count: usize,
    pub failed_count: usize,
    pub active_rebuilds: usize,
}

pub struct ProjectionRegistry {
    engine: Arc<ProjectionEngine>,
    projections: RwLock<HashMap<String, Box<dyn std::any::Any + Send + Sync>>>,
    rebuild_triggers: RwLock<Vec<RebuildTrigger>>,
    current_sequence: RwLock<u64>,
}

impl ProjectionRegistry {
    pub fn new(engine: Arc<ProjectionEngine>) -> Self {
        Self {
            engine,
            projections: RwLock::new(HashMap::new()),
            rebuild_triggers: RwLock::new(Vec::new()),
            current_sequence: RwLock::new(0),
        }
    }

    pub fn register<S, E, P>(
        &self,
        projection_id: &str,
        projector: P,
        class: ProjectionClass,
    ) -> Result<(), ProjectionError>
    where
        S: Clone + Default + serde::Serialize + 'static,
        E: Clone + 'static,
        P: Projector<S, E> + 'static,
    {
        let max_version = projector.schema_version();
        let registered = RegisteredProjection::<S, E, P>::new(projector, class, max_version);

        registered
            .state_manager
            .set_state(projection_id, ProjectionState::Building)?;

        let mut projections = self.projections.write().map_err(|_| {
            ProjectionError::InvalidState("projection registry lock poisoned".to_string())
        })?;

        projections.insert(projection_id.to_string(), Box::new(registered));
        Ok(())
    }

    pub fn get_record<S, E, P>(
        &self,
        projection_id: &str,
    ) -> Result<ProjectionRecord, ProjectionError>
    where
        S: Clone + Default + serde::Serialize,
        E: Clone,
        P: Projector<S, E>,
    {
        let projections = self.projections.read().map_err(|_| {
            ProjectionError::InvalidState("projection registry lock poisoned".to_string())
        })?;

        let registered = projections
            .get(projection_id)
            .ok_or_else(|| ProjectionError::ProjectionNotFound(projection_id.to_string()))?;

        let registered = registered
            .downcast_ref::<RegisteredProjection<S, E, P>>()
            .ok_or_else(|| ProjectionError::InvalidState("projection type mismatch".to_string()))?;

        let state = registered
            .state_manager
            .get_state(projection_id)
            .ok_or_else(|| ProjectionError::ProjectionNotFound(projection_id.to_string()))?;

        Ok(ProjectionRecord::new(
            projection_id.to_string(),
            registered.schema_version(),
            vec![],
            (0, 0),
            0,
            0,
            0,
        ))
    }

    pub fn set_ready(&self, projection_id: &str) -> Result<(), ProjectionError> {
        let projections = self.projections.read().map_err(|_| {
            ProjectionError::InvalidState("projection registry lock poisoned".to_string())
        })?;

        if let Some(registered) = projections.get(projection_id) {
            let _registered = registered
                .downcast_ref::<RegisteredProjection<(), (), ()>>()
                .map_err(|_| {
                    ProjectionError::InvalidState("projection type mismatch".to_string())
                })?;
        }
        Ok(())
    }

    pub fn mark_stale(
        &self,
        projection_id: &str,
        reason: StaleReason,
    ) -> Result<(), ProjectionError> {
        let projections = self.projections.read().map_err(|_| {
            ProjectionError::InvalidState("projection registry lock poisoned".to_string())
        })?;

        let _registered = projections
            .get(projection_id)
            .ok_or_else(|| ProjectionError::ProjectionNotFound(projection_id.to_string()))?;

        let _registered = _registered
            .downcast_ref::<RegisteredProjection<(), (), ()>>()
            .map_err(|_| ProjectionError::InvalidState("projection type mismatch".to_string()))?;

        let now = self.current_sequence();
        _registered.state_manager.transition_to(
            projection_id,
            ProjectionState::Stale {
                detected_at: now,
                reason: reason.clone(),
            },
        )?;

        drop(projections);

        let trigger = RebuildTrigger {
            projection_id: projection_id.to_string(),
            reason,
            detected_at: now,
            rebuild_id: format!("{}-{}", projection_id, now),
        };

        let mut triggers = self.rebuild_triggers.write().map_err(|_| {
            ProjectionError::InvalidState("rebuild triggers lock poisoned".to_string())
        })?;
        triggers.push(trigger);

        Ok(())
    }

    pub fn update_sequence(&self, sequence: u64) {
        let mut current = self.current_sequence.write().unwrap_or_else(|e| e.into_inner());
        *current = sequence;
    }

    pub fn current_sequence(&self) -> u64 {
        *self.current_sequence.read().unwrap_or_else(|e| e.into_inner())
    }

    pub fn detect_staleness(
        &self,
        projection_id: &str,
        record: &ProjectionRecord,
    ) -> Result<Option<StaleReason>, ProjectionError> {
        let stale_reason = self
            .engine
            .detect_staleness(record, self.current_sequence());
        Ok(stale_reason)
    }

    pub fn check_and_rebuild_if_stale<S, E, P>(
        &self,
        projection_id: &str,
        record: &ProjectionRecord,
        events: impl IntoIterator<Item = E>,
    ) -> Result<Option<ProjectionResult<S>>, ProjectionError>
    where
        S: Clone + Default + serde::Serialize,
        E: Clone,
        P: Projector<S, E> + 'static,
    {
        let projections = self.projections.read().map_err(|_| {
            ProjectionError::InvalidState("projection registry lock poisoned".to_string())
        })?;

        let registered = projections
            .get(projection_id)
            .ok_or_else(|| ProjectionError::ProjectionNotFound(projection_id.to_string()))?;

        let registered = registered
            .downcast_ref::<RegisteredProjection<S, E, P>>()
            .ok_or_else(|| ProjectionError::InvalidState("projection type mismatch".to_string()))?;

        if !self.engine.is_idle() {
            return Err(ProjectionError::ThrottleExceeded(100));
        }

        self.engine.try_acquire_rebuild_slot(projection_id)?;

        let from_sequence = record.sequence_range.1 + 1;
        let rebuilder = ProjectionRebuilder::new(
            &self.engine,
            &registered.projector,
            projection_id.to_string(),
            from_sequence,
        );

        registered.state_manager.transition_to(
            projection_id,
            ProjectionState::Rebuilding {
                progress: 0,
                from_sequence,
            },
        )?;

        drop(projections);

        let result = rebuilder.rebuild_full(events);

        self.engine.release_rebuild_slot(projection_id);

        match result {
            Ok(res) => {
                let projections = self.projections.read().map_err(|_| {
                    ProjectionError::InvalidState("projection registry lock poisoned".to_string())
                })?;

                if let Some(registered) = projections.get(projection_id) {
                    let registered = registered
                        .downcast_ref::<RegisteredProjection<S, E, P>>()
                        .map_err(|_| {
                            ProjectionError::InvalidState("projection type mismatch".to_string())
                        })?;
                    registered
                        .state_manager
                        .transition_to(projection_id, ProjectionState::Ready)?;
                }
                Ok(Some(res))
            }
            Err(e) => {
                let projections = self.projections.read().map_err(|_| {
                    ProjectionError::InvalidState("projection registry lock poisoned".to_string())
                })?;

                if let Some(registered) = projections.get(projection_id) {
                    let registered = registered
                        .downcast_ref::<RegisteredProjection<S, E, P>>()
                        .map_err(|_| {
                            ProjectionError::InvalidState("projection type mismatch".to_string())
                        })?;
                    let now = self.current_sequence();
                    registered.state_manager.transition_to(
                        projection_id,
                        ProjectionState::Failed {
                            reason: e.to_string(),
                            attempted_at: now,
                        },
                    )?;
                }
                Err(e)
            }
        }
    }

    pub fn pending_triggers(&self) -> Result<Vec<RebuildTrigger>, ProjectionError> {
        let triggers = self.rebuild_triggers.read().map_err(|_| {
            ProjectionError::InvalidState("rebuild triggers lock poisoned".to_string())
        })?;
        Ok(triggers.clone())
    }

    pub fn clear_trigger(&self, rebuild_id: &str) -> Result<(), ProjectionError> {
        let mut triggers = self.rebuild_triggers.write().map_err(|_| {
            ProjectionError::InvalidState("rebuild triggers lock poisoned".to_string())
        })?;
        triggers.retain(|t| t.rebuild_id != rebuild_id);
        Ok(())
    }

    pub fn metrics(&self) -> Result<RegistryMetrics, ProjectionError> {
        let projections = self.projections.read().map_err(|_| {
            ProjectionError::InvalidState("projection registry lock poisoned".to_string())
        })?;

        let mut ready_count = 0;
        let mut stale_count = 0;
        let mut rebuilding_count = 0;
        let mut failed_count = 0;

        for registered in projections.values() {
            let state = registered
                .downcast_ref::<RegisteredProjection<(), (), ()>>()
                .map_err(|_| {
                    ProjectionError::InvalidState("projection type mismatch".to_string())
                })?;

            let state = state
                .state_manager
                .get_state("")
                .unwrap_or(ProjectionState::Building);
            match state {
                ProjectionState::Ready => ready_count += 1,
                ProjectionState::Stale { .. } => stale_count += 1,
                ProjectionState::Rebuilding { .. } => rebuilding_count += 1,
                ProjectionState::Failed { .. } => failed_count += 1,
                ProjectionState::Building => {}
            }
        }

        Ok(RegistryMetrics {
            total_projections: projections.len(),
            ready_count,
            stale_count,
            rebuilding_count,
            failed_count,
            active_rebuilds: self.engine.active_rebuild_count(),
        })
    }

    pub fn list_projections(&self) -> Result<Vec<String>, ProjectionError> {
        let projections = self.projections.read().map_err(|_| {
            ProjectionError::InvalidState("projection registry lock poisoned".to_string())
        })?;
        Ok(projections.keys().cloned().collect())
    }

    pub fn engine(&self) -> &Arc<ProjectionEngine> {
        &self.engine
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::replay::projection::{
        ProjectionEngine, ProjectionError, Projector, RebuildThrottleConfig,
    };

    struct TestProjector;

    impl Projector<String, String> for TestProjector {
        type Error = String;

        fn project(&self, state: String, event: &String) -> Result<String, Self::Error> {
            Ok(format!("{}+{}", state, event))
        }

        fn initial_state() -> String {
            String::new()
        }

        fn schema_version(&self) -> u8 {
            1
        }
    }

    fn make_engine() -> Arc<ProjectionEngine> {
        Arc::new(
            ProjectionEngine::builder(1)
                .throttle_config(RebuildThrottleConfig::new(5, 100, 1))
                .build(),
        )
    }

    #[test]
    fn registry_register_and_list() {
        let engine = make_engine();
        let registry = ProjectionRegistry::new(engine);

        registry
            .register::<String, String, TestProjector>(
                "test-projection",
                TestProjector,
                ProjectionClass::Operational,
            )
            .unwrap();

        let projections = registry.list_projections().unwrap();
        assert_eq!(projections, vec!["test-projection"]);
    }

    #[test]
    fn registry_update_sequence() {
        let engine = make_engine();
        let registry = ProjectionRegistry::new(engine);

        assert_eq!(registry.current_sequence(), 0);
        registry.update_sequence(100);
        assert_eq!(registry.current_sequence(), 100);
    }

    #[test]
    fn registry_mark_stale_creates_trigger() {
        let engine = make_engine();
        let registry = ProjectionRegistry::new(engine);

        registry
            .register::<String, String, TestProjector>(
                "test-projection",
                TestProjector,
                ProjectionClass::Operational,
            )
            .unwrap();

        registry
            .mark_stale("test-projection", StaleReason::ManualInvalidation)
            .unwrap();

        let triggers = registry.pending_triggers().unwrap();
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].projection_id, "test-projection");
    }

    #[test]
    fn registry_metrics() {
        let engine = make_engine();
        let registry = ProjectionRegistry::new(engine);

        registry
            .register::<String, String, TestProjector>(
                "proj-1",
                TestProjector,
                ProjectionClass::Operational,
            )
            .unwrap();

        let metrics = registry.metrics().unwrap();
        assert_eq!(metrics.total_projections, 1);
    }
}
