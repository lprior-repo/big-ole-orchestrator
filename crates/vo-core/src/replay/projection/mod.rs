//! Projection engine types (ADR-037).
//!
//! Types for the event sourcing projection engine — transforms immutable
//! event sequences into materialized read models (projections).
//!
//! ## Architecture
//!
//! - `ProjectionEngine` — coordinates projection rebuilds and manages lifecycle
//! - `ProjectionRebuilder` — handles full rebuild from event log
//! - `ProjectionStateManager` — tracks state transitions and detects staleness
//! - `RebuildThrottle` — token-bucket throttle for concurrent rebuild limiting
//!
//! ## Usage
//!
//! ```ignore
//! let engine = ProjectionEngine::builder()
//!     .max_supported_version(5)
//!     .max_concurrent_rebuilds(5)
//!     .build();
//!
//! let rebuilder = engine.create_rebuilder(projection_id, schema_version);
//! let result = rebuilder.rebuild_full(events)?;
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::upcaster::UpcasterRegistry;

pub mod error;

pub use error::{
    ProjectionError, ProjectionStateError, ProjectionVersionError, ReplayError, StorageError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionRecord {
    pub projection_id: String,
    pub schema_version: u8,
    pub state_bytes: Vec<u8>,
    pub sequence_range: (u64, u64),
    pub checksum: u64,
    pub created_at: u64,
    pub updated_at: u64,
}

impl ProjectionRecord {
    pub fn new(
        projection_id: String,
        schema_version: u8,
        state_bytes: Vec<u8>,
        sequence_range: (u64, u64),
        checksum: u64,
        created_at: u64,
        updated_at: u64,
    ) -> Self {
        Self {
            projection_id,
            schema_version,
            state_bytes,
            sequence_range,
            checksum,
            created_at,
            updated_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionResult<S> {
    pub state: S,
    pub events_applied: u64,
    pub starting_sequence: u64,
    pub ending_sequence: u64,
    pub duration_ms: u64,
    pub schema_version: u8,
}

impl<S> ProjectionResult<S> {
    pub fn new(
        state: S,
        events_applied: u64,
        starting_sequence: u64,
        ending_sequence: u64,
        duration_ms: u64,
        schema_version: u8,
    ) -> Self {
        Self {
            state,
            events_applied,
            starting_sequence,
            ending_sequence,
            duration_ms,
            schema_version,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectionState {
    Building,
    Ready,
    Stale {
        detected_at: u64,
        reason: StaleReason,
    },
    Rebuilding {
        progress: u32,
        from_sequence: u64,
    },
    Failed {
        reason: String,
        attempted_at: u64,
    },
}

impl ProjectionState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, ProjectionState::Failed { .. })
    }

    pub fn is_stale(&self) -> bool {
        matches!(
            self,
            ProjectionState::Stale { .. } | ProjectionState::Rebuilding { .. }
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StaleReason {
    SchemaVersionMismatch { expected: u8, actual: u8 },
    SequenceGapDetected { gap_at: u64 },
    CorruptionDetected,
    ManualInvalidation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectionEvent {
    ProjectionStarted {
        projection_id: String,
        from_sequence: u64,
    },
    ProjectionProgress {
        projection_id: String,
        percent: u32,
        at_sequence: u64,
    },
    ProjectionCompleted {
        projection_id: String,
        events_applied: u64,
    },
    ProjectionStale {
        projection_id: String,
        reason: StaleReason,
    },
    ProjectionRebuildStarted {
        projection_id: String,
        reason: StaleReason,
    },
    ProjectionRebuildFailed {
        projection_id: String,
        error: String,
    },
}

pub trait Projector<S, E>
where
    S: Clone + Default + serde::Serialize,
{
    type Error: Into<ProjectionError>;

    fn project(&self, state: S, event: &E) -> Result<S, Self::Error>;

    fn initial_state() -> S;

    fn schema_version(&self) -> u8;
}

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
struct RebuildThrottleState {
    available_tokens: usize,
    max_tokens: usize,
    last_refill: Instant,
    refill_interval: Duration,
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
            refill_interval: Duration::from_millis(config.refill_interval_ms),
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

// ============================================================================
// Rebuild Context — Tracks individual rebuild operations
// ============================================================================

#[derive(Debug)]
pub struct RebuildContext {
    pub projection_id: String,
    pub started_at: Instant,
    pub from_sequence: u64,
    pub events_total: AtomicU64,
    pub events_processed: AtomicU64,
    pub progress_percent: AtomicU32,
    pub cancelled: AtomicBool,
}

impl RebuildContext {
    fn new(projection_id: String, from_sequence: u64) -> Self {
        Self {
            projection_id,
            started_at: Instant::now(),
            from_sequence,
            events_total: AtomicU64::new(0),
            events_processed: AtomicU64::new(0),
            progress_percent: AtomicU32::new(0),
            cancelled: AtomicBool::new(false),
        }
    }

    pub fn set_total_events(&self, total: u64) {
        self.events_total.store(total, Ordering::Relaxed);
    }

    pub fn update_progress(&self, processed: u64) {
        self.events_processed.store(processed, Ordering::Relaxed);
        let total = self.events_total.load(Ordering::Relaxed);
        if total > 0 {
            let percent = ((processed as f64 / total as f64) * 100.0) as u32;
            self.progress_percent
                .store(percent.min(100), Ordering::Relaxed);
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.started_at.elapsed().as_millis() as u64
    }
}

// ============================================================================
// Projection Rebuilder — Handles full rebuild from event log
// ============================================================================

use std::marker::PhantomData;

pub struct ProjectionRebuilder<'a, S, E, P>
where
    S: Clone + Default + serde::Serialize,
    E: Clone,
    P: Projector<S, E>,
{
    #[allow(dead_code)]
    engine: &'a ProjectionEngine,
    projector: &'a P,
    context: Arc<RebuildContext>,
    _phantom: PhantomData<(S, E)>,
}

impl<'a, S, E, P> ProjectionRebuilder<'a, S, E, P>
where
    S: Clone + Default + serde::Serialize,
    E: Clone,
    P: Projector<S, E>,
{
    pub fn new(
        engine: &'a ProjectionEngine,
        projector: &'a P,
        projection_id: String,
        from_sequence: u64,
    ) -> Self {
        Self {
            engine,
            projector,
            context: Arc::new(RebuildContext::new(projection_id, from_sequence)),
            _phantom: PhantomData,
        }
    }

    pub fn context(&self) -> &Arc<RebuildContext> {
        &self.context
    }

    pub fn rebuild_full<I>(&self, events: I) -> Result<ProjectionResult<S>, ProjectionError>
    where
        I: IntoIterator<Item = E>,
        I::IntoIter: ExactSizeIterator,
    {
        let start = Instant::now();
        let events_iter = events.into_iter();
        let total_events = events_iter.len() as u64;
        self.context.set_total_events(total_events);

        let mut state = S::default();
        let mut processed: u64 = 0;
        let start_seq = self.context.from_sequence;

        for event in events_iter {
            if self.context.is_cancelled() {
                return Err(ProjectionError::BuildFailed(
                    "rebuild cancelled".to_string(),
                ));
            }

            state = self
                .projector
                .project(state, &event)
                .map_err(|e| ProjectionError::BuildFailed(e.into().to_string()))?;

            processed += 1;
            self.context.update_progress(processed);
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let end_seq = start_seq.saturating_add(processed);

        Ok(ProjectionResult::new(
            state,
            processed,
            start_seq,
            end_seq,
            duration_ms,
            self.projector.schema_version(),
        ))
    }
}

// ============================================================================
// Projection State Manager — Tracks and transitions projection states
// ============================================================================

#[derive(Debug, Clone)]
pub struct ProjectionStateManager {
    states: Arc<std::sync::Mutex<HashMap<String, ProjectionState>>>,
}

impl ProjectionStateManager {
    pub fn new() -> Self {
        Self {
            states: Arc::new(std::sync::Mutex::new(HashMap::new())),
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
            | (Some(ProjectionState::Building), ProjectionState::Failed { .. })
            | (Some(ProjectionState::Ready), ProjectionState::Stale { .. })
            | (Some(ProjectionState::Ready), ProjectionState::Rebuilding { .. })
            | (Some(ProjectionState::Ready), ProjectionState::Failed { .. })
            | (Some(ProjectionState::Stale { .. }), ProjectionState::Rebuilding { .. })
            | (Some(ProjectionState::Stale { .. }), ProjectionState::Failed { .. })
            | (Some(ProjectionState::Rebuilding { .. }), ProjectionState::Ready)
            | (Some(ProjectionState::Rebuilding { .. }), ProjectionState::Failed { .. })
            | (Some(ProjectionState::Failed { .. }), ProjectionState::Rebuilding { .. })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_state_is_terminal() {
        assert!(!ProjectionState::Building.is_terminal());
        assert!(!ProjectionState::Ready.is_terminal());
        assert!(!ProjectionState::Stale {
            detected_at: 0,
            reason: StaleReason::ManualInvalidation
        }
        .is_terminal());
        assert!(!ProjectionState::Rebuilding {
            progress: 50,
            from_sequence: 1
        }
        .is_terminal());
        assert!(ProjectionState::Failed {
            reason: "test".to_string(),
            attempted_at: 100
        }
        .is_terminal());
    }

    #[test]
    fn projection_state_is_stale() {
        assert!(!ProjectionState::Building.is_stale());
        assert!(!ProjectionState::Ready.is_stale());
        assert!(ProjectionState::Stale {
            detected_at: 0,
            reason: StaleReason::ManualInvalidation
        }
        .is_stale());
        assert!(ProjectionState::Rebuilding {
            progress: 50,
            from_sequence: 1
        }
        .is_stale());
        assert!(!ProjectionState::Failed {
            reason: "test".to_string(),
            attempted_at: 100
        }
        .is_stale());
    }

    #[test]
    fn stale_reason_variants() {
        use StaleReason::*;
        let reasons = vec![
            SchemaVersionMismatch {
                expected: 1,
                actual: 0,
            },
            SequenceGapDetected { gap_at: 100 },
            CorruptionDetected,
            ManualInvalidation,
        ];
        for reason in reasons {
            let debug = format!("{:?}", reason);
            assert!(!debug.is_empty());
        }
    }

    #[test]
    fn projection_record_construction() {
        let record = ProjectionRecord::new(
            "test-projection".to_string(),
            1,
            vec![1, 2, 3],
            (1, 100),
            12345,
            1000,
            2000,
        );
        assert_eq!(record.projection_id, "test-projection");
        assert_eq!(record.schema_version, 1);
        assert_eq!(record.state_bytes, vec![1, 2, 3]);
        assert_eq!(record.sequence_range, (1, 100));
        assert_eq!(record.checksum, 12345);
        assert_eq!(record.created_at, 1000);
        assert_eq!(record.updated_at, 2000);
    }

    #[test]
    fn projection_result_construction() {
        let result: ProjectionResult<String> =
            ProjectionResult::new("final state".to_string(), 50, 1, 50, 100, 1);
        assert_eq!(result.state, "final state");
        assert_eq!(result.events_applied, 50);
        assert_eq!(result.starting_sequence, 1);
        assert_eq!(result.ending_sequence, 50);
        assert_eq!(result.duration_ms, 100);
        assert_eq!(result.schema_version, 1);
    }

    #[test]
    fn rebuild_throttle_config_default() {
        let config = RebuildThrottleConfig::default();
        assert_eq!(config.max_concurrent_rebuilds, 5);
        assert_eq!(config.refill_interval_ms, 100);
        assert_eq!(config.tokens_per_refill, 1);
    }

    #[test]
    fn rebuild_throttle_config_custom() {
        let config = RebuildThrottleConfig::new(10, 50, 2);
        assert_eq!(config.max_concurrent_rebuilds, 10);
        assert_eq!(config.refill_interval_ms, 50);
        assert_eq!(config.tokens_per_refill, 2);
    }

    #[test]
    fn rebuild_context_progress() {
        let ctx = RebuildContext::new("test".to_string(), 0);
        ctx.set_total_events(100);
        assert_eq!(ctx.events_total.load(Ordering::Relaxed), 100);

        ctx.update_progress(50);
        assert_eq!(ctx.events_processed.load(Ordering::Relaxed), 50);
        assert_eq!(ctx.progress_percent.load(Ordering::Relaxed), 50);

        ctx.update_progress(100);
        assert_eq!(ctx.progress_percent.load(Ordering::Relaxed), 100);
    }

    #[test]
    fn rebuild_context_cancel() {
        let ctx = RebuildContext::new("test".to_string(), 0);
        assert!(!ctx.is_cancelled());

        ctx.cancel();
        assert!(ctx.is_cancelled());
    }

    #[test]
    fn rebuild_context_elapsed() {
        let ctx = RebuildContext::new("test".to_string(), 0);
        std::thread::sleep(Duration::from_millis(10));
        assert!(ctx.elapsed_ms() >= 10);
    }

    #[test]
    fn projection_state_manager_transitions() {
        let mgr = ProjectionStateManager::new();

        assert!(mgr.transition_to("p1", ProjectionState::Building).is_ok());
        assert!(mgr.transition_to("p1", ProjectionState::Ready).is_ok());
        assert!(mgr
            .transition_to(
                "p1",
                ProjectionState::Stale {
                    detected_at: 100,
                    reason: StaleReason::ManualInvalidation
                }
            )
            .is_ok());
        assert!(mgr
            .transition_to(
                "p1",
                ProjectionState::Rebuilding {
                    progress: 0,
                    from_sequence: 101
                }
            )
            .is_ok());
        assert!(mgr.transition_to("p1", ProjectionState::Ready).is_ok());
        assert!(mgr.is_ready("p1"));

        assert!(mgr.transition_to("p2", ProjectionState::Building).is_ok());
        assert!(mgr
            .transition_to(
                "p2",
                ProjectionState::Failed {
                    reason: "test".to_string(),
                    attempted_at: 100
                }
            )
            .is_ok());
        assert!(mgr.is_failed("p2"));
    }

    #[test]
    fn projection_state_manager_invalid_transition() {
        let mgr = ProjectionStateManager::new();

        assert!(mgr.transition_to("p1", ProjectionState::Building).is_ok());
        let result = mgr.transition_to("p1", ProjectionState::Ready);
        assert!(result.is_ok());

        let result = mgr.transition_to("p1", ProjectionState::Building);
        assert!(result.is_err());
    }

    #[test]
    fn projection_error_is_retryable() {
        use ProjectionError::*;
        assert!(ThrottleExceeded(100).is_retryable());
        assert!(ConcurrencyConflict("test".to_string()).is_retryable());
        assert!(Storage("test".to_string()).is_retryable());
        assert!(!ProjectionNotFound("test".to_string()).is_retryable());
    }

    #[test]
    fn projection_engine_builder() {
        let engine = ProjectionEngine::builder(5)
            .throttle_config(RebuildThrottleConfig::new(3, 200, 2))
            .build();

        assert_eq!(engine.throttle_config().max_concurrent_rebuilds, 3);
        assert_eq!(engine.throttle_config().refill_interval_ms, 200);
        assert_eq!(engine.throttle_config().tokens_per_refill, 2);
        assert_eq!(engine.max_supported_version(), 5);
        assert!(engine.is_idle());
    }

    #[test]
    fn projection_engine_detect_staleness_version() {
        let engine = ProjectionEngine::new(5);

        let record = ProjectionRecord::new("test".to_string(), 3, vec![], (1, 100), 0, 0, 0);

        let stale = engine.detect_staleness(&record, 100);
        assert!(matches!(
            stale,
            Some(StaleReason::SchemaVersionMismatch {
                expected: 5,
                actual: 3
            })
        ));
    }

    #[test]
    fn projection_engine_detect_staleness_sequence() {
        let engine = ProjectionEngine::new(5);

        let record = ProjectionRecord::new("test".to_string(), 5, vec![], (1, 100), 0, 0, 0);

        let stale = engine.detect_staleness(&record, 150);
        assert!(matches!(
            stale,
            Some(StaleReason::SequenceGapDetected { gap_at: 101 })
        ));
    }

    #[test]
    fn projection_engine_no_staleness() {
        let engine = ProjectionEngine::new(5);

        let record = ProjectionRecord::new("test".to_string(), 5, vec![], (1, 100), 0, 0, 0);

        let stale = engine.detect_staleness(&record, 100);
        assert!(stale.is_none());
    }
}
