use std::sync::Arc;

use super::{EventSourcingConfig, EventSourcingEngine, SnapshotStore};
use crate::replay::projection::ProjectionEngine;
use crate::replay::ReplayEngine;
use crate::upcaster::UpcasterRegistry;

pub struct EventSourcingEngineBuilder {
    pub(super) config: EventSourcingConfig,
    pub(super) upcaster_registry: Option<Box<dyn UpcasterRegistry>>,
    pub(super) snapshot_store: Option<Arc<dyn SnapshotStore>>,
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
