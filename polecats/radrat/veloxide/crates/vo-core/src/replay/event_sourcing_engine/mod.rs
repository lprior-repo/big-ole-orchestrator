//! Event sourcing engine (ESE).
//!
//! Unified event sourcing engine that combines replay, projection, and
//! snapshot-based recovery for complete state reconstruction.
//!
//! Components: `ReplayEngine` (stateless replay), `ProjectionEngine` (projection management),
//! `SnapshotManager` (snapshot-based recovery).

mod builder;
mod config;
mod recovery;
mod snapshot;

use std::sync::Arc;
use std::time::Instant;

pub use builder::EventSourcingEngineBuilder;
pub use config::EventSourcingConfig;
pub use recovery::{RecoveryResult, RecoveryType};
pub use snapshot::{EventStore, InMemorySnapshotStore, Snapshot, SnapshotStore};

use crate::replay::projection::{
    ProjectionEngine, ProjectionError, ProjectionRebuilder, ProjectionResult, Projector,
};
use crate::replay::ReplayEngine;
use crate::upcaster::UpcasterRegistry;

pub struct EventSourcingEngine {
    pub(super) config: EventSourcingConfig,
    pub(super) replay_engine: ReplayEngine,
    pub(super) projection_engine: ProjectionEngine,
    pub(super) upcaster_registry: Option<Box<dyn UpcasterRegistry>>,
    pub(super) snapshot_store: Option<Arc<dyn SnapshotStore>>,
}

impl EventSourcingEngine {
    pub fn builder(max_schema_version: u8) -> EventSourcingEngineBuilder {
        EventSourcingEngineBuilder::new(max_schema_version)
    }

    #[must_use]
    pub fn new(max_schema_version: u8) -> Self {
        Self::builder(max_schema_version).build()
    }

    pub fn with_upcaster(mut self, registry: Box<dyn UpcasterRegistry>) -> Self {
        self.upcaster_registry = Some(registry);
        self
    }

    pub fn with_snapshot_store(mut self, store: Arc<dyn SnapshotStore>) -> Self {
        self.snapshot_store = Some(store);
        self
    }

    #[must_use]
    pub const fn max_schema_version(&self) -> u8 {
        self.config.max_schema_version
    }

    pub fn create_rebuilder<'a, S, E, P>(
        &'a self,
        projection_id: &str,
        projector: &'a P,
        instance_id: String,
        from_sequence: u64,
    ) -> ProjectionRebuilder<'a, S, E, P>
    where
        S: Clone + Default + serde::Serialize + 'a,
        E: Clone + 'a,
        P: Projector<S, E> + 'a,
    {
        ProjectionRebuilder::new(
            &self.projection_engine,
            projector,
            format!("{}-{}", projection_id, instance_id),
            from_sequence,
        )
    }

    pub fn reconstruct_state(
        &self,
        events: &[vo_types::events::EventEnvelope],
    ) -> Result<RecoveryResult<Option<vo_types::state::LifecycleState>>, crate::replay::ReplayError>
    {
        let start = Instant::now();

        let result = if let Some(registry) = &self.upcaster_registry {
            self.replay_engine
                .replay_with_upcaster(registry.as_ref(), events)?
        } else {
            self.replay_engine.replay(events)?
        };

        let recovery_type = RecoveryType::FullReplay;

        Ok(RecoveryResult::new(
            result.final_state,
            result.events_applied as u64,
            events.first().map(|e| e.sequence).unwrap_or(0),
            events.last().map(|e| e.sequence).unwrap_or(0),
            recovery_type,
            start.elapsed().as_millis() as u64,
            false,
        ))
    }

    pub fn reconstruct_state_with_snapshot(
        &self,
        snapshot: Option<&Snapshot>,
        recent_events: &[vo_types::events::EventEnvelope],
    ) -> Result<RecoveryResult<Option<vo_types::state::LifecycleState>>, ProjectionError> {
        let start = Instant::now();

        if let Some(snap) = snapshot {
            if !snap.is_compatible_with(self.config.max_schema_version) {
                return Err(ProjectionError::IncompatibleSchemaVersion {
                    expected: self.config.max_schema_version,
                    actual: snap.schema_version,
                });
            }

            let events_to_replay: Vec<vo_types::events::EventEnvelope> = recent_events
                .iter()
                .filter(|e| e.sequence > snap.sequence)
                .cloned()
                .collect();

            let starting_sequence = snap.sequence + 1;
            let ending_sequence = recent_events
                .last()
                .map(|e| e.sequence)
                .unwrap_or(snap.sequence);

            if events_to_replay.is_empty() {
                let state: Option<vo_types::state::LifecycleState> =
                    serde_json::from_slice(&snap.state_bytes).map_err(|e| {
                        ProjectionError::Storage(format!("snapshot deserialization failed: {}", e))
                    })?;

                return Ok(RecoveryResult::new(
                    state,
                    0,
                    starting_sequence,
                    ending_sequence,
                    RecoveryType::SnapshotAccelerated,
                    start.elapsed().as_millis() as u64,
                    true,
                ));
            }

            let replay_result = if let Some(registry) = &self.upcaster_registry {
                self.replay_engine
                    .replay_with_upcaster(registry.as_ref(), &events_to_replay)
                    .map_err(|e| ProjectionError::BuildFailed(e.to_string()))?
            } else {
                self.replay_engine
                    .replay(&events_to_replay)
                    .map_err(|e| ProjectionError::BuildFailed(e.to_string()))?
            };

            return Ok(RecoveryResult::new(
                replay_result.final_state,
                replay_result.events_applied as u64,
                starting_sequence,
                ending_sequence,
                RecoveryType::SnapshotAccelerated,
                start.elapsed().as_millis() as u64,
                true,
            ));
        }

        let replay_result = if let Some(registry) = &self.upcaster_registry {
            self.replay_engine
                .replay_with_upcaster(registry.as_ref(), recent_events)
                .map_err(|e| ProjectionError::BuildFailed(e.to_string()))?
        } else {
            self.replay_engine
                .replay(recent_events)
                .map_err(|e| ProjectionError::BuildFailed(e.to_string()))?
        };

        Ok(RecoveryResult::new(
            replay_result.final_state,
            replay_result.events_applied as u64,
            recent_events.first().map(|e| e.sequence).unwrap_or(0),
            recent_events.last().map(|e| e.sequence).unwrap_or(0),
            RecoveryType::FullReplay,
            start.elapsed().as_millis() as u64,
            false,
        ))
    }

    pub fn build_projection<S, E, P, I>(
        &self,
        events: I,
        projector: &P,
    ) -> Result<ProjectionResult<S>, ProjectionError>
    where
        S: Clone + Default + serde::Serialize,
        E: Clone,
        P: Projector<S, E>,
        I: IntoIterator<Item = E>,
        I::IntoIter: ExactSizeIterator,
    {
        let rebuilder = self.create_rebuilder(
            projector.schema_version().to_string().as_str(),
            projector,
            "default".to_string(),
            0,
        );

        rebuilder.rebuild_full(events)
    }

    pub fn should_create_snapshot(&self, events_processed: u64) -> bool {
        events_processed > 0
            && events_processed.is_multiple_of(self.config.snapshot_interval_events)
    }

    pub fn create_snapshot<S: serde::Serialize>(
        &self,
        projection_id: &str,
        state: &S,
        sequence: u64,
    ) -> Result<Snapshot, ProjectionError> {
        let state_bytes = serde_json::to_vec(state)
            .map_err(|e| ProjectionError::Storage(format!("serialization failed: {}", e)))?;

        let checksum = Self::compute_checksum(&state_bytes);

        let snapshot = Snapshot::new(
            projection_id.to_string(),
            self.config.max_schema_version,
            state_bytes,
            sequence,
            Self::current_timestamp_ms(),
            checksum,
        );

        if let Some(store) = &self.snapshot_store {
            store.save_snapshot(&snapshot)?;
        }

        Ok(snapshot)
    }

    pub fn load_snapshot(&self, projection_id: &str) -> Result<Option<Snapshot>, ProjectionError> {
        match &self.snapshot_store {
            Some(store) => store.load_latest_snapshot(projection_id),
            None => Ok(None),
        }
    }

    pub fn delete_snapshot(
        &self,
        projection_id: &str,
        sequence: u64,
    ) -> Result<(), ProjectionError> {
        match &self.snapshot_store {
            Some(store) => store.delete_snapshot(projection_id, sequence),
            None => Ok(()),
        }
    }

    fn compute_checksum(data: &[u8]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        hasher.finish()
    }

    fn current_timestamp_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

impl Default for EventSourcingEngine {
    fn default() -> Self {
        Self::new(1)
    }
}
