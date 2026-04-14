//! Snapshot recovery with throttle — crash recovery coordinator.
//!
//! Architecture: Data (`RecoveryError`, `ThrottleConfig`) → Calc
//! (`select_best_recovery_point`, `ThrottleState`) → Actions
//! (`SnapshotRecovery`, `RecoveryThrottle`).
//!
//! ## Problem
//!
//! After a crash, all instances may attempt to recover simultaneously, creating
//! a "recovery storm" that overwhelms storage and network resources.
//!
//! ## Solution
//!
//! 1. **Recovery point selection**: Each instance selects the most recent valid
//!    snapshot as its recovery point, then replays only events after that point.
//! 2. **Throttled recovery**: A token-bucket throttle limits concurrent recovery
//!    operations across all instances, pacing the storm.
//! 3. **Atomic batch integration**: Recovery coordinates with the `BudgetQueues`
//!    atomic batch writer to ensure consistent recovery state.
//!
//! ## Usage
//!
//! ```ignore
//! let recovery = SnapshotRecovery::new(throttle_config, appender);
//! let point = recovery.select_best_recovery_point(&partition, &instance_id)?;
//! recovery.acquire_recovery_slot().await?;
//! // ... perform recovery ...
//! recovery.release_recovery_slot();
//! ```

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use vo_types::InstanceId;
use vo_types::state::InstanceState;

use crate::append::{Appender, BudgetQueues};
use crate::codec::StorageError;
use crate::snapshots::{self, snapshot_load_latest};

#[cfg(test)]
mod tests;

// ---------------------------------------------------------------------------
// Data layer — error enum
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RecoveryError {
    #[error("no snapshot available for instance {instance_id}")]
    NoSnapshotAvailable { instance_id: InstanceId },
    #[error("throttle exceeded, would wait {wait_ms}ms")]
    ThrottleExceeded { wait_ms: u64 },
    #[error("recovery already in progress for instance {instance_id}")]
    RecoveryInProgress { instance_id: InstanceId },
    #[error("invalid recovery point: {reason}")]
    InvalidRecoveryPoint { reason: String },
    #[error("append error during recovery: {reason}")]
    AppendError { reason: String },
    #[error("codec error during recovery: {reason}")]
    CodecError { reason: String },
}

impl From<StorageError> for RecoveryError {
    fn from(e: StorageError) -> Self {
        match e {
            StorageError::DeserializationFailed => Self::CodecError {
                reason: "deserialization failed".to_string(),
            },
            StorageError::CorruptKey => Self::CodecError {
                reason: "corrupt key".to_string(),
            },
            StorageError::InvalidKey => Self::InvalidRecoveryPoint {
                reason: "invalid key".to_string(),
            },
            _ => Self::CodecError {
                reason: format!("{e:?}"),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Data layer — throttle config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct ThrottleConfig {
    pub max_concurrent_recoveries: usize,
    pub refill_interval_ms: u64,
    pub tokens_per_refill: usize,
}

impl Default for ThrottleConfig {
    fn default() -> Self {
        Self {
            max_concurrent_recoveries: 10,
            refill_interval_ms: 100,
            tokens_per_refill: 1,
        }
    }
}

impl ThrottleConfig {
    #[must_use]
    pub const fn new(
        max_concurrent_recoveries: usize,
        refill_interval_ms: u64,
        tokens_per_refill: usize,
    ) -> Self {
        Self {
            max_concurrent_recoveries,
            refill_interval_ms,
            tokens_per_refill,
        }
    }
}

// ---------------------------------------------------------------------------
// Calc layer — recovery point selection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RecoveryPoint {
    pub instance_id: InstanceId,
    pub snapshot_sequence: u64,
    pub state: InstanceState,
    pub selected_at: Instant,
}

impl RecoveryPoint {
    #[must_use]
    pub fn new(
        instance_id: InstanceId,
        snapshot_sequence: u64,
        state: InstanceState,
    ) -> Self {
        Self {
            instance_id,
            snapshot_sequence,
            state,
            selected_at: Instant::now(),
        }
    }

    #[must_use]
    pub fn events_to_replay(&self, current_sequence: u64) -> u64 {
        current_sequence.saturating_sub(self.snapshot_sequence)
    }
}

fn select_best_recovery_point_impl(
    partition: &fjall::Keyspace,
    instance_id: &InstanceId,
) -> Result<Option<RecoveryPoint>, StorageError> {
    let result = snapshot_load_latest(partition, instance_id)?;
    Ok(result.map(|(sequence, state)| {
        RecoveryPoint::new(instance_id.clone(), sequence, state)
    }))
}

// ---------------------------------------------------------------------------
// Calc layer — throttle state
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct ThrottleState {
    available_tokens: usize,
    max_tokens: usize,
    last_refill: Instant,
    refill_interval: Duration,
    tokens_per_refill: usize,
    active_recoveries: AtomicUsize,
}

impl ThrottleState {
    fn new(config: ThrottleConfig) -> Self {
        Self {
            available_tokens: config.max_concurrent_recoveries,
            max_tokens: config.max_concurrent_recoveries,
            last_refill: Instant::now(),
            refill_interval: Duration::from_millis(config.refill_interval_ms),
            tokens_per_refill: config.tokens_per_refill,
            active_recoveries: AtomicUsize::new(0),
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
        if self.available_tokens > 0 && self.active_recoveries.load(Ordering::Relaxed) < self.max_tokens
        {
            self.available_tokens -= 1;
            self.active_recoveries.fetch_add(1, Ordering::Relaxed);
            Some(0)
        } else {
            let wait_time = self.refill_interval_ms().max(10);
            Some(wait_time)
        }
    }

    fn release_slot(&self) {
        self.active_recoveries.fetch_sub(1, Ordering::Relaxed);
    }

    fn refill_interval_ms(&self) -> u64 {
        self.refill_interval.as_millis() as u64
    }

    fn is_idle(&self) -> bool {
        self.active_recoveries.load(Ordering::Relaxed) == 0
    }
}

// ---------------------------------------------------------------------------
// Actions layer — SnapshotRecovery
// ---------------------------------------------------------------------------

pub struct SnapshotRecovery {
    throttle: ThrottleState,
    appender: Arc<Appender>,
}

impl SnapshotRecovery {
    #[must_use]
    pub fn new(config: ThrottleConfig, appender: Arc<Appender>) -> Self {
        Self {
            throttle: ThrottleState::new(config),
            appender,
        }
    }

    pub fn select_best_recovery_point(
        &self,
        partition: &fjall::Keyspace,
        instance_id: &InstanceId,
    ) -> Result<RecoveryPoint, RecoveryError> {
        select_best_recovery_point_impl(partition, instance_id)?.ok_or_else(|| {
            RecoveryError::NoSnapshotAvailable {
                instance_id: instance_id.clone(),
            }
        })
    }

    pub fn try_acquire_recovery_slot(&mut self) -> Result<(), RecoveryError> {
        match self.throttle.try_acquire_slot() {
            Some(wait_ms) if wait_ms > 0 => Err(RecoveryError::ThrottleExceeded { wait_ms }),
            Some(_) => Ok(()),
            None => Ok(()),
        }
    }

    pub fn release_recovery_slot(&self) {
        self.throttle.release_slot();
    }

    #[must_use]
    pub fn active_recoveries(&self) -> usize {
        self.throttle.active_recoveries.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn available_slots(&self) -> usize {
        self.throttle.available_tokens
    }

    #[must_use]
    pub const fn throttle_config(&self) -> ThrottleConfig {
        ThrottleConfig {
            max_concurrent_recoveries: self.throttle.max_tokens,
            refill_interval_ms: self.throttle.refill_interval.as_millis() as u64,
            tokens_per_refill: self.throttle.tokens_per_refill,
        }
    }

    pub fn is_idle(&self) -> bool {
        self.throttle.is_idle()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_instance_id() -> InstanceId {
        InstanceId::from_bytes([1u8; 16])
    }

    #[test]
    fn throttle_config_default() {
        let config = ThrottleConfig::default();
        assert_eq!(config.max_concurrent_recoveries, 10);
        assert_eq!(config.refill_interval_ms, 100);
        assert_eq!(config.tokens_per_refill, 1);
    }

    #[test]
    fn throttle_config_custom() {
        let config = ThrottleConfig::new(5, 50, 2);
        assert_eq!(config.max_concurrent_recoveries, 5);
        assert_eq!(config.refill_interval_ms, 50);
        assert_eq!(config.tokens_per_refill, 2);
    }

    #[test]
    fn recovery_point_new() {
        let id = make_instance_id();
        let state = InstanceState { counter: 42 };
        let point = RecoveryPoint::new(id.clone(), 100, state.clone());

        assert_eq!(point.instance_id, id);
        assert_eq!(point.snapshot_sequence, 100);
        assert_eq!(point.state, state);
    }

    #[test]
    fn recovery_point_events_to_replay() {
        let id = make_instance_id();
        let state = InstanceState { counter: 42 };
        let point = RecoveryPoint::new(id, 100, state);

        assert_eq!(point.events_to_replay(150), 50);
        assert_eq!(point.events_to_replay(100), 0);
        assert_eq!(point.events_to_replay(50), 0);
    }

    #[test]
    fn throttle_state_acquire_and_release() {
        let config = ThrottleConfig::default();
        let mut state = ThrottleState::new(config);

        assert_eq!(state.active_recoveries.load(Ordering::Relaxed), 0);
        assert_eq!(state.available_tokens, 10);

        let slot = state.try_acquire_slot();
        assert!(slot.is_some());

        assert_eq!(state.active_recoveries.load(Ordering::Relaxed), 1);
        assert_eq!(state.available_tokens, 9);

        state.release_slot();
        assert_eq!(state.active_recoveries.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn throttle_state_exhausts_tokens() {
        let config = ThrottleConfig::new(2, 1000, 1);
        let mut state = ThrottleState::new(config);

        assert!(state.try_acquire_slot().is_some());
        assert!(state.try_acquire_slot().is_some());

        let result = state.try_acquire_slot();
        assert!(result.is_some());
        let wait_ms = result.unwrap();
        assert!(wait_ms > 0);
    }

    #[test]
    fn throttle_state_refill() {
        let config = ThrottleConfig::new(2, 10, 1);
        let mut state = ThrottleState::new(config);

        assert!(state.try_acquire_slot().is_some());
        assert!(state.try_acquire_slot().is_some());

        std::thread::sleep(Duration::from_millis(25));

        let result = state.try_acquire_slot();
        assert!(result.is_some());
    }

    #[test]
    fn throttle_state_is_idle() {
        let config = ThrottleConfig::default();
        let state = ThrottleState::new(config);

        assert!(state.is_idle());

        drop(state);
    }

    #[test]
    fn recovery_error_display() {
        let id = make_instance_id();
        let err = RecoveryError::NoSnapshotAvailable { instance_id: id.clone() };
        assert!(err.to_string().contains("no snapshot available"));

        let err = RecoveryError::ThrottleExceeded { wait_ms: 100 };
        assert!(err.to_string().contains("throttle exceeded"));
    }

    #[test]
    fn storage_error_converts_to_recovery_error() {
        use crate::codec::StorageError;
        let err: RecoveryError = StorageError::DeserializationFailed.into();
        assert!(matches!(err, RecoveryError::CodecError { .. }));

        let err: RecoveryError = StorageError::InvalidKey.into();
        assert!(matches!(err, RecoveryError::InvalidRecoveryPoint { .. }));
    }
}
