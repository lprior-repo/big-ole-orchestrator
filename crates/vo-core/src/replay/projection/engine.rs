//! Projection engine — throttle, builder, and engine core.
//!
//! - `RebuildThrottleConfig` — configuration data
//! - `RebuildThrottleState` — token-bucket state machine (Calc)
//! - `ProjectionEngineBuilder` — builder pattern (Actions)
//! - `ProjectionEngine` — coordinates rebuilds and manages lifecycle (Actions)
//! - `RebuildContext` — tracks individual rebuild operations

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use super::throttle::{RebuildThrottleConfig, RebuildThrottleState};
use super::{ProjectionError, ProjectionRecord, ProjectionState, RebuildContext, StaleReason};
use crate::upcaster::UpcasterRegistry;

// =====================================================================

pub struct ProjectionEngineBuilder {
    max_supported_version: u8,
    throttle_config: RebuildThrottleConfig,
    upcaster_registry: Option<Box<dyn UpcasterRegistry>>,
}

impl ProjectionEngineBuilder {
    #[must_use]
    pub fn new(max_supported_version: u8) -> Self {
        Self {
            max_supported_version,
            throttle_config: RebuildThrottleConfig::default(),
            upcaster_registry: None,
        }
    }

    #[must_use]
    pub fn throttle_config(mut self, config: RebuildThrottleConfig) -> Self {
        self.throttle_config = config;
        self
    }

    #[must_use]
    pub fn upcaster_registry(mut self, registry: Box<dyn UpcasterRegistry>) -> Self {
        self.upcaster_registry = Some(registry);
        self
    }

    #[must_use]
    pub fn build(self) -> ProjectionEngine {
        ProjectionEngine {
            upcaster_registry: self.upcaster_registry,
            max_supported_version: self.max_supported_version,
            throttle: RebuildThrottleState::new(self.throttle_config),
            throttle_config: self.throttle_config,
            active_rebuilds: Arc::new(HashMap::new()),
            rebuild_in_progress: AtomicBool::new(false),
        }
    }
}

// =====================================================================

pub struct ProjectionEngine {
    upcaster_registry: Option<Box<dyn UpcasterRegistry>>,
    max_supported_version: u8,
    throttle: RebuildThrottleState,
    throttle_config: RebuildThrottleConfig,
    #[allow(dead_code)]
    active_rebuilds: Arc<HashMap<String, Arc<RebuildContext>>>,
    #[allow(dead_code)]
    rebuild_in_progress: AtomicBool,
}

impl ProjectionEngine {
    pub fn builder(max_supported_version: u8) -> ProjectionEngineBuilder {
        ProjectionEngineBuilder::new(max_supported_version)
    }

    pub fn new(max_supported_version: u8) -> Self {
        Self::builder(max_supported_version).build()
    }

    pub fn with_upcaster(mut self, registry: Box<dyn UpcasterRegistry>) -> Self {
        self.upcaster_registry = Some(registry);
        self
    }

    #[must_use]
    pub const fn max_supported_version(&self) -> u8 {
        self.max_supported_version
    }

    pub fn throttle_config(&self) -> RebuildThrottleConfig {
        self.throttle_config
    }

    pub fn try_acquire_rebuild_slot(
        &mut self,
        _projection_id: &str,
    ) -> Result<(), ProjectionError> {
        match self.throttle.try_acquire_slot() {
            Some(wait_ms) if wait_ms > 0 => Err(ProjectionError::ThrottleExceeded(wait_ms)),
            Some(_) | None => {
                self.rebuild_in_progress.store(true, Ordering::Relaxed);
                Ok(())
            }
        }
    }

    pub fn release_rebuild_slot(&self) {
        self.throttle.release_slot();
        self.rebuild_in_progress.store(false, Ordering::Relaxed);
    }

    pub fn active_rebuild_count(&self) -> usize {
        self.throttle.active_count()
    }

    pub fn is_idle(&self) -> bool {
        self.throttle.is_idle()
    }

    pub fn is_rebuild_in_progress(&self) -> bool {
        self.rebuild_in_progress.load(Ordering::Relaxed)
    }

    pub fn upcaster_registry(&self) -> Option<&dyn UpcasterRegistry> {
        self.upcaster_registry.as_deref()
    }

    pub fn detect_staleness(
        &self,
        record: &ProjectionRecord,
        current_sequence: u64,
    ) -> Option<StaleReason> {
        if record.schema_version != self.max_supported_version {
            return Some(StaleReason::SchemaVersionMismatch {
                expected: self.max_supported_version,
                actual: record.schema_version,
            });
        }

        let (_, end_seq) = record.sequence_range;
        if end_seq < current_sequence {
            return Some(StaleReason::SequenceGapDetected {
                gap_at: end_seq + 1,
            });
        }

        None
    }
}
