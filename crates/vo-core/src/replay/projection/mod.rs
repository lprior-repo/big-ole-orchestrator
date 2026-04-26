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

pub mod context;
pub mod engine;
pub mod error;
pub mod rebuilder;
pub mod state_manager;
pub mod throttle;
pub mod types;

pub use engine::{ProjectionEngine, ProjectionEngineBuilder};
pub use error::{
    ProjectionError, ProjectionStateError, ProjectionVersionError, ReplayError, StorageError,
};
pub use rebuilder::ProjectionRebuilder;
pub use context::RebuildContext;
pub use state_manager::ProjectionStateManager;
pub use throttle::RebuildThrottleConfig;
pub use types::{
    ProjectionEvent, ProjectionRecord, ProjectionResult, ProjectionState, Projector, StaleReason,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

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
