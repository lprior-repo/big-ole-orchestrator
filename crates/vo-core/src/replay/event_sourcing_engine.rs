//! Event sourcing engine (ESE).
//!
//! Unified event sourcing engine that combines replay, projection, and
//! snapshot-based recovery for complete state reconstruction.
//!
//! ## Architecture
//!
//! ```ignore
//! EventSourcingEngine
//!   ├── ReplayEngine      — stateless event replay
//!   ├── ProjectionEngine  — projection management
//!   └── SnapshotManager   — snapshot-based recovery
//! ```
//!
//! ## Usage
//!
//! ```ignore
//! let engine = EventSourcingEngine::builder()
//!     .max_schema_version(5)
//!     .build();
//!
//! // Full replay from genesis
//! let state = engine.reconstruct_state(events).await?;
//!
//! // Snapshot-accelerated recovery
//! let state = engine.recover_with_snapshot(snapshot, recent_events).await?;
//!
//! // Build projection from events
//! let projection = engine.build_projection(events, projector).await?;
//! ```

use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::replay::projection::{
    ProjectionEngine, ProjectionError, ProjectionRebuilder, ProjectionResult, Projector,
    RebuildThrottleConfig,
};
use crate::replay::ReplayEngine;
use crate::snapshot_compat::{check_snapshot_compat, SnapshotCompat};
use crate::upcaster::UpcasterRegistry;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub projection_id: String,
    pub schema_version: u8,
    pub state_bytes: Vec<u8>,
    pub sequence: u64,
    pub created_at: u64,
    pub checksum: u64,
}

impl Snapshot {
    #[must_use]
    pub fn new(
        projection_id: String,
        schema_version: u8,
        state_bytes: Vec<u8>,
        sequence: u64,
        created_at: u64,
        checksum: u64,
    ) -> Self {
        Self {
            projection_id,
            schema_version,
            state_bytes,
            sequence,
            created_at,
            checksum,
        }
    }

    pub fn is_compatible_with(&self, engine_version: u8) -> bool {
        matches!(
            check_snapshot_compat(self.schema_version as u16, engine_version as u16),
            SnapshotCompat::Compatible
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EventSourcingConfig {
    pub max_schema_version: u8,
    pub throttle_config: RebuildThrottleConfig,
    pub snapshot_interval_events: u64,
}

impl Default for EventSourcingConfig {
    fn default() -> Self {
        Self {
            max_schema_version: 1,
            throttle_config: RebuildThrottleConfig::default(),
            snapshot_interval_events: 1000,
        }
    }
}

impl EventSourcingConfig {
    #[must_use]
    pub fn new(max_schema_version: u8, snapshot_interval_events: u64) -> Self {
        Self {
            max_schema_version,
            throttle_config: RebuildThrottleConfig::default(),
            snapshot_interval_events,
        }
    }

    #[must_use]
    pub const fn with_throttle(mut self, config: RebuildThrottleConfig) -> Self {
        self.throttle_config = config;
        self
    }
}

#[derive(Debug, Clone)]
pub struct RecoveryResult<S = ()> {
    pub state: S,
    pub events_applied: u64,
    pub starting_sequence: u64,
    pub ending_sequence: u64,
    pub recovery_type: RecoveryType,
    pub duration_ms: u64,
    pub snapshot_used: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryType {
    FullReplay,
    SnapshotAccelerated,
    Incremental,
}

impl RecoveryResult {
    #[must_use]
    pub fn unit(events_applied: u64, starting_sequence: u64, ending_sequence: u64) -> Self {
        Self {
            state: (),
            events_applied,
            starting_sequence,
            ending_sequence,
            recovery_type: RecoveryType::FullReplay,
            duration_ms: 0,
            snapshot_used: false,
        }
    }
}

impl<S> RecoveryResult<S> {
    #[must_use]
    pub fn new(
        state: S,
        events_applied: u64,
        starting_sequence: u64,
        ending_sequence: u64,
        recovery_type: RecoveryType,
        duration_ms: u64,
        snapshot_used: bool,
    ) -> Self {
        Self {
            state,
            events_applied,
            starting_sequence,
            ending_sequence,
            recovery_type,
            duration_ms,
            snapshot_used,
        }
    }
}

pub trait SnapshotStore: Send + Sync {
    fn save_snapshot(&self, snapshot: &Snapshot) -> Result<(), ProjectionError>;
    fn load_latest_snapshot(
        &self,
        projection_id: &str,
    ) -> Result<Option<Snapshot>, ProjectionError>;
    fn delete_snapshot(&self, projection_id: &str, sequence: u64) -> Result<(), ProjectionError>;
}

pub trait EventStore: Send + Sync {
    fn fetch_events(
        &self,
        projection_id: &str,
        from_sequence: u64,
        to_sequence: u64,
    ) -> Result<Vec<vo_types::events::EventEnvelope>, ProjectionError>;
}

#[derive(Debug)]
pub struct InMemorySnapshotStore {
    snapshots: std::sync::Mutex<std::collections::HashMap<String, Snapshot>>,
}

impl InMemorySnapshotStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            snapshots: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

impl Default for InMemorySnapshotStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotStore for InMemorySnapshotStore {
    fn save_snapshot(&self, snapshot: &Snapshot) -> Result<(), ProjectionError> {
        self.snapshots
            .lock()
            .map_err(|_| ProjectionError::Storage("lock poisoned".to_string()))?
            .insert(snapshot.projection_id.clone(), snapshot.clone());
        Ok(())
    }

    fn load_latest_snapshot(
        &self,
        projection_id: &str,
    ) -> Result<Option<Snapshot>, ProjectionError> {
        Ok(self
            .snapshots
            .lock()
            .map_err(|_| ProjectionError::Storage("lock poisoned".to_string()))?
            .get(projection_id)
            .cloned())
    }

    fn delete_snapshot(&self, projection_id: &str, _sequence: u64) -> Result<(), ProjectionError> {
        self.snapshots
            .lock()
            .map_err(|_| ProjectionError::Storage("lock poisoned".to_string()))?
            .remove(projection_id);
        Ok(())
    }
}

pub struct EventSourcingEngineBuilder {
    config: EventSourcingConfig,
    upcaster_registry: Option<Box<dyn UpcasterRegistry>>,
    snapshot_store: Option<Arc<dyn SnapshotStore>>,
}

impl EventSourcingEngineBuilder {
    #[must_use]
    pub fn new(max_schema_version: u8) -> Self {
        Self {
            config: EventSourcingConfig {
                max_schema_version,
                ..Default::default()
            },
            upcaster_registry: None,
            snapshot_store: None,
        }
    }

    #[must_use]
    pub fn config(mut self, config: EventSourcingConfig) -> Self {
        self.config = config;
        self
    }

    #[must_use]
    pub fn upcaster_registry(mut self, registry: Box<dyn UpcasterRegistry>) -> Self {
        self.upcaster_registry = Some(registry);
        self
    }

    #[must_use]
    pub fn snapshot_store(mut self, store: Arc<dyn SnapshotStore>) -> Self {
        self.snapshot_store = Some(store);
        self
    }

    #[must_use]
    pub fn build(self) -> EventSourcingEngine {
        let projection_engine = ProjectionEngine::builder(self.config.max_schema_version)
            .throttle_config(self.config.throttle_config)
            .build();

        EventSourcingEngine {
            config: self.config,
            replay_engine: ReplayEngine::new(),
            projection_engine,
            upcaster_registry: self.upcaster_registry,
            snapshot_store: self.snapshot_store,
        }
    }
}

pub struct EventSourcingEngine {
    config: EventSourcingConfig,
    replay_engine: ReplayEngine,
    projection_engine: ProjectionEngine,
    upcaster_registry: Option<Box<dyn UpcasterRegistry>>,
    snapshot_store: Option<Arc<dyn SnapshotStore>>,
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

        let recovery_type = if events.is_empty() {
            RecoveryType::FullReplay
        } else {
            RecoveryType::FullReplay
        };

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

            let events_to_replay = if recent_events.is_empty() {
                vec![]
            } else {
                recent_events.to_vec()
            };

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

            let replay_result = self
                .replay_engine
                .replay(&events_to_replay)
                .map_err(|e| ProjectionError::BuildFailed(e.to_string()))?;

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

        let replay_result = self
            .replay_engine
            .replay(recent_events)
            .map_err(|e| ProjectionError::BuildFailed(e.to_string()))?;

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
        events_processed > 0 && events_processed % self.config.snapshot_interval_events == 0
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_creation_and_compat() {
        let snapshot = Snapshot::new("test-proj".to_string(), 1, vec![1, 2, 3], 100, 1000, 12345);

        assert_eq!(snapshot.sequence, 100);
        assert!(snapshot.is_compatible_with(1));
        assert!(!snapshot.is_compatible_with(2));
    }

    #[test]
    fn event_sourcing_config_default() {
        let config = EventSourcingConfig::default();
        assert_eq!(config.max_schema_version, 1);
        assert_eq!(config.snapshot_interval_events, 1000);
    }

    #[test]
    fn event_sourcing_config_custom() {
        let config =
            EventSourcingConfig::new(5, 500).with_throttle(RebuildThrottleConfig::new(3, 200, 2));
        assert_eq!(config.max_schema_version, 5);
        assert_eq!(config.snapshot_interval_events, 500);
        assert_eq!(config.throttle_config.max_concurrent_rebuilds, 3);
    }

    #[test]
    fn in_memory_snapshot_store() {
        let store = InMemorySnapshotStore::new();
        let snapshot = Snapshot::new("test".to_string(), 1, vec![1, 2, 3], 100, 1000, 12345);

        store.save_snapshot(&snapshot).unwrap();
        let loaded = store.load_latest_snapshot("test").unwrap().unwrap();
        assert_eq!(loaded.sequence, 100);

        store.delete_snapshot("test", 100).unwrap();
        let loaded = store.load_latest_snapshot("test").unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn should_create_snapshot() {
        let engine = EventSourcingEngine::builder(1)
            .config(EventSourcingConfig {
                snapshot_interval_events: 100,
                ..Default::default()
            })
            .build();

        assert!(!engine.should_create_snapshot(0));
        assert!(!engine.should_create_snapshot(50));
        assert!(engine.should_create_snapshot(100));
        assert!(!engine.should_create_snapshot(150));
        assert!(engine.should_create_snapshot(200));
    }

    #[test]
    fn engine_with_snapshot_store() {
        let store = Arc::new(InMemorySnapshotStore::new());
        let engine = EventSourcingEngine::builder(1)
            .snapshot_store(store.clone())
            .build();

        let snapshot = engine
            .create_snapshot::<String>("test", &"hello".to_string(), 50)
            .unwrap();
        assert_eq!(snapshot.sequence, 50);

        let loaded = engine.load_snapshot("test").unwrap().unwrap();
        assert_eq!(loaded.sequence, 50);
    }

    #[test]
    fn recovery_type_variants() {
        assert_eq!(RecoveryType::FullReplay, RecoveryType::FullReplay);
        assert_eq!(
            RecoveryType::SnapshotAccelerated,
            RecoveryType::SnapshotAccelerated
        );
    }
}
