//! Timer lifecycle management for workflow completion and cancellation.
//!
//! Per ADR-005, this module implements:
//! - Timer cancellation when workflow reaches terminal state
//! - Timer persistence across crashes (via reanimator recovery)
//!
//! Architecture: Data → Calc → Actions

use std::sync::Arc;

use vo_types::{InstanceId, TimestampMs};

use crate::reanimator::{ReanimatorError, TimerRecord, TimerStorage};

/// Errors from timer lifecycle operations.
<<<<<<< HEAD
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TimerLifecycleError {
    #[error("Storage error: {0}")]
    StorageError(String),
    #[error("Instance not found: {0}")]
    InstanceNotFound(InstanceId),
    #[error("Timer not found: {instance_id} at {fire_at_ms}")]
=======
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimerLifecycleError {
    /// Storage error while performing lifecycle operation.
    StorageError(String),
    /// Instance not found.
    InstanceNotFound(InstanceId),
    /// Timer not found.
>>>>>>> origin/polecat/synth-mnw6kj8v
    TimerNotFound {
        instance_id: InstanceId,
        fire_at_ms: TimestampMs,
    },
}

<<<<<<< HEAD
=======
impl std::fmt::Display for TimerLifecycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StorageError(s) => write!(f, "Storage error: {s}"),
            Self::InstanceNotFound(id) => write!(f, "Instance not found: {id}"),
            Self::TimerNotFound { instance_id, fire_at_ms } => {
                write!(f, "Timer not found: {instance_id} at {fire_at_ms}")
            }
        }
    }
}

impl std::error::Error for TimerLifecycleError {}

>>>>>>> origin/polecat/synth-mnw6kj8v
impl From<ReanimatorError> for TimerLifecycleError {
    fn from(err: ReanimatorError) -> Self {
        match err {
            ReanimatorError::StorageError(s) => Self::StorageError(s),
            ReanimatorError::InstanceNotFound(id) => Self::InstanceNotFound(id),
            _ => Self::StorageError(err.to_string()),
        }
    }
}

/// Result type for timer lifecycle operations.
pub type TimerLifecycleResult<T> = Result<T, TimerLifecycleError>;

/// Cancels all pending timers for an instance when it reaches a terminal state.
///
/// Per INV-3: When a workflow reaches Completed, Failed, or Cancelled state,
/// all pending timers must be cancelled to prevent orphan timers from firing
/// for non-existent workflow instances.
///
/// # Arguments
/// * `storage` - The timer storage backend
/// * `instance_id` - The instance whose timers should be cancelled
///
/// # Returns
/// * `Ok(cancelled_count)` - Number of timers that were cancelled
///
/// # Errors
/// * `TimerLifecycleError::StorageError` - If storage operations fail
pub async fn cancel_timers_for_instance<S>(
    storage: &Arc<S>,
    instance_id: &InstanceId,
) -> TimerLifecycleResult<u32>
where
    S: TimerStorage + 'static,
{
<<<<<<< HEAD
    let cancelled_count = storage
        .delete_all_timers_for_instance(instance_id)
        .await
        .map_err(|e| TimerLifecycleError::StorageError(e.to_string()))?;
=======
    let now = TimestampMs::now();
    let zero = TimestampMs::try_from(0u64).expect("0 is valid");
    let mut cancelled_count = 0u32;

    loop {
        let timers = storage
            .scan_due_timers(zero, now, 100)
            .await
            .map_err(|e| TimerLifecycleError::StorageError(e.to_string()))?;

        let instance_timers: Vec<TimerRecord> = timers
            .into_iter()
            .filter(|t| t.instance_id == *instance_id)
            .collect();

        if instance_timers.is_empty() {
            break;
        }

        for timer in instance_timers {
            match storage
                .delete_timer(&timer.instance_id, timer.fire_at_ms)
                .await
            {
                Ok(()) => {
                    cancelled_count += 1;
                    tracing::debug!(
                        instance_id = %timer.instance_id,
                        fire_at_ms = %timer.fire_at_ms,
                        "Cancelled timer on workflow completion"
                    );
                }
                Err(ReanimatorError::InstanceNotFound(_)) => {
                    tracing::debug!(
                        instance_id = %instance_id,
                        "Instance not found during timer cancellation"
                    );
                    break;
                }
                Err(e) => {
                    tracing::warn!(
                        instance_id = %timer.instance_id,
                        error = %e,
                        "Failed to cancel timer"
                    );
                }
            }
        }

        if cancelled_count < 100 {
            break;
        }
    }
>>>>>>> origin/polecat/synth-mnw6kj8v

    tracing::info!(
        instance_id = %instance_id,
        cancelled_count = %cancelled_count,
        "Cancelled all timers for instance"
    );

    Ok(cancelled_count)
}

/// Scans for all pending timers for a specific instance.
///
/// This is used during hibernation recovery to verify and replay
/// any timers that were in-flight when a workflow was hibernated.
///
/// # Arguments
/// * `storage` - The timer storage backend
/// * `instance_id` - The instance to scan timers for
/// * `max_results` - Maximum number of timers to return
///
/// # Returns
/// * `Ok(timers)` - List of pending timers for the instance
pub async fn scan_instance_timers<S>(
    storage: &Arc<S>,
    instance_id: &InstanceId,
    max_results: u32,
) -> TimerLifecycleResult<Vec<TimerRecord>>
where
    S: TimerStorage + 'static,
{
    let now = TimestampMs::now();
    let zero = TimestampMs::try_from(0u64).expect("0 is valid");

    let all_timers = storage
        .scan_due_timers(zero, now, max_results)
        .await
        .map_err(|e| TimerLifecycleError::StorageError(e.to_string()))?;

    let instance_timers: Vec<TimerRecord> = all_timers
        .into_iter()
        .filter(|t| t.instance_id == *instance_id)
        .collect();

    Ok(instance_timers)
}

/// Checks if an instance has any pending timers.
///
/// # Arguments
/// * `storage` - The timer storage backend
/// * `instance_id` - The instance to check
///
/// # Returns
/// * `Ok(true)` - If the instance has pending timers
/// * `Ok(false)` - If the instance has no pending timers
pub async fn has_pending_timers<S>(
    storage: &Arc<S>,
    instance_id: &InstanceId,
) -> TimerLifecycleResult<bool>
where
    S: TimerStorage + 'static,
{
    let timers = scan_instance_timers(storage, instance_id, 1).await?;
    Ok(!timers.is_empty())
}

/// Validates that a timer can be safely cancelled.
///
/// Returns an error if the timer is already completed or doesn't exist.
/// This is a pure calculation function.
///
/// # Arguments
/// * `timer` - The timer to validate
/// * `instance_id` - The instance the timer belongs to
///
/// # Returns
/// * `Ok(())` - If the timer can be cancelled
/// * `Err(TimerLifecycleError::TimerNotFound)` - If the timer doesn't exist
pub fn validate_timer_for_cancellation(
    timer: &TimerRecord,
    instance_id: &InstanceId,
) -> TimerLifecycleResult<()> {
    if timer.instance_id != *instance_id {
        return Err(TimerLifecycleError::TimerNotFound {
            instance_id: instance_id.clone(),
            fire_at_ms: timer.fire_at_ms,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(test)]
    use crate::reanimator::MockTimerStorage;
    use vo_types::TimestampMs;

    fn create_instance_id() -> InstanceId {
        InstanceId::from_bytes([1; 16])
    }

    fn create_timer_record(instance_id: InstanceId, fire_at_ms: u64) -> TimerRecord {
        TimerRecord::new(
            instance_id,
            TimestampMs::try_from(fire_at_ms).expect("valid"),
            Some(vo_types::TimerId::from_bytes([2; 16])),
            TimestampMs::try_from(fire_at_ms - 1000).expect("valid"),
        )
    }

    #[tokio::test]
    async fn cancel_timers_for_instance_cancels_all_timers() {
        let instance_id = create_instance_id();
        let storage = Arc::new(MockTimerStorage::empty());

        storage
            .add_timer(create_timer_record(instance_id.clone(), 5000))
            .await;
        storage
            .add_timer(create_timer_record(instance_id.clone(), 6000))
            .await;

        let other_instance = InstanceId::from_bytes([9; 16]);
        storage
            .add_timer(create_timer_record(other_instance.clone(), 5500))
            .await;

        let count = cancel_timers_for_instance(&storage, &instance_id)
            .await
            .expect("cancel should succeed");

        assert_eq!(count, 2);
        assert!(!has_pending_timers(&storage, &instance_id)
            .await
            .expect("check should succeed"));
        assert!(has_pending_timers(&storage, &other_instance)
            .await
            .expect("check should succeed"));
    }

    #[tokio::test]
    async fn cancel_timers_for_instance_returns_zero_when_no_timers() {
        let instance_id = create_instance_id();
        let storage = Arc::new(MockTimerStorage::empty());

        let count = cancel_timers_for_instance(&storage, &instance_id)
            .await
            .expect("cancel should succeed");

        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn scan_instance_timers_returns_only_matching_instance() {
        let instance_id = create_instance_id();
        let storage = Arc::new(MockTimerStorage::empty());

        storage
            .add_timer(create_timer_record(instance_id.clone(), 5000))
            .await;

        let other_instance = InstanceId::from_bytes([9; 16]);
        storage
            .add_timer(create_timer_record(other_instance.clone(), 6000))
            .await;

        let timers = scan_instance_timers(&storage, &instance_id, 100)
            .await
            .expect("scan should succeed");

        assert_eq!(timers.len(), 1);
        assert_eq!(timers[0].instance_id, instance_id);
    }

    #[tokio::test]
    async fn has_pending_timers_returns_true_when_timers_exist() {
        let instance_id = create_instance_id();
        let storage = Arc::new(MockTimerStorage::empty());

        storage
            .add_timer(create_timer_record(instance_id.clone(), 5000))
            .await;

        let has = has_pending_timers(&storage, &instance_id)
            .await
            .expect("check should succeed");

        assert!(has);
    }

    #[tokio::test]
    async fn has_pending_timers_returns_false_when_no_timers() {
        let instance_id = create_instance_id();
        let storage = Arc::new(MockTimerStorage::empty());

        let has = has_pending_timers(&storage, &instance_id)
            .await
            .expect("check should succeed");

        assert!(!has);
    }

    #[test]
    fn validate_timer_for_cancellation_accepts_matching_instance() {
        let instance_id = create_instance_id();
        let timer = create_timer_record(instance_id.clone(), 5000);

        let result = validate_timer_for_cancellation(&timer, &instance_id);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_timer_for_cancellation_rejects_different_instance() {
        let instance_id = create_instance_id();
        let other_instance = InstanceId::from_bytes([9; 16]);
        let timer = create_timer_record(instance_id.clone(), 5000);

        let result = validate_timer_for_cancellation(&timer, &other_instance);
        assert!(matches!(
            result,
            Err(TimerLifecycleError::TimerNotFound { .. })
        ));
    }
}
