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

use super::{ProjectionError, ProjectionRecord, ProjectionState, RebuildContext, StaleReason};

// ============================================================================
// Throttle Configuration — Data Layer
// ============================================================================

#[derive(Debug, Clone, Copy)]
pub struct RebuildThrottleConfig {
    pub max_concurrent_rebuilds: usize,
    pub refill_interval_ms: u64,
    pub tokens_per_refill: usize,
}

impl Default for RebuildThrottleConfig {
    fn default() -> Self {
        Self {
            max_concurrent_rebuilds: 5,
            refill_interval_ms: 100,
            tokens_per_refill: 1,
        }
    }
}

impl RebuildThrottleConfig {
    #[must_use]
    pub const fn new(
        max_concurrent_rebuilds: usize,
        refill_interval_ms: u64,
        tokens_per_refill: usize,
    ) -> Self {
        Self {
            max_concurrent_rebuilds,
            refill_interval_ms,
            tokens_per_refill,
        }
    }
}

// ============================================================================
// Throttle State — Calc Layer
// ============================================================================

#[derive(Debug)]
pub struct RebuildThrottleState {
    available_tokens: usize,
    max_tokens: usize,
    last_refill: Instant,
    refill_interval: std::time::Duration,
    tokens_per_refill: usize,
    #[allow(dead_code)]
    active_rebuilds: AtomicUsize,
}

impl RebuildThrottleState {
    fn new(config: RebuildThrottleConfig) -> Self {
        Self {
            available_tokens: config.max_concurrent_rebuilds,
            max_tokens: config.max_concurrent_rebuilds,
            last_refill: Instant::now(),
            refill_interval: std::time::Duration::from_millis(config.refill_interval_ms),
            tokens_per_refill: config.tokens_per_refill,
            active_rebuilds: AtomicUsize::new(0),
        }
    }

    fn refill(&mut self) {
        let elapsed = self.last_refill.elapsed();
        if elapsed >= self.refill_interval {
            let intervals = (elapsed.as_millis() / self.refill_interval.as_millis()) as usize;
            let new_tokens = intervals * self.tokens_per_refill;
            self.available_tokens = (self.available_tokens + new_tokens).min(self.max_tokens);
            self.last_refill = Instant::now();
        }
    }

    fn try_acquire_slot(&mut self) -> Option<u64> {
        self.refill();
        if self.available_tokens > 0
            && self.active_rebuilds.load(Ordering::Relaxed) < self.max_tokens
        {
            self.available_tokens -= 1;
            self.active_rebuilds.fetch_add(1, Ordering::Relaxed);
            Some(0)
        } else {
            let wait_time = self.refill_interval.as_millis() as u64;
            Some(wait_time.max(10))
        }
    }

    fn release_slot(&self) {
        self.active_rebuilds.fetch_sub(1, Ordering::Relaxed);
    }

    fn is_idle(&self) -> bool {
        self.active_rebuilds.load(Ordering::Relaxed) == 0
    }

    fn active_count(&self) -> usize {
        self.active_rebuilds.load(Ordering::Relaxed)
    }
}

// ============================================================================
// Projection Engine Builder — Actions Layer
// ============================================================================

use crate::upcaster::UpcasterRegistry;

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

// ============================================================================
// Projection Engine — Actions Layer
// ============================================================================

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
