//! Recovery queue throttling and orphan detection (ADR-032).
//!
//! This module provides:
//! - [`ThrottledRecoveryChannel`]: A bounded channel that limits orphan recovery rate
//! - [`OrphanDetector`]: Sweeps for orphan processes and enqueues them for recovery
//!
//! ## Key Invariants
//!
//! 1. Recovery queue ingestion rate never exceeds configured throttle
//! 2. If recovery queue is full, no more orphans are enqueued (throttling guarantee)
//! 3. Orphans are identified and queued safely

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tokio::time::{interval, MissedTickBehavior};
use tracing::{debug, error, info, warn};

use vo_types::InstanceId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryChannelConfig {
    pub queue_capacity: usize,
    pub max_orphan_batch_size: u32,
    pub sweep_interval: Duration,
}

impl Default for RecoveryChannelConfig {
    fn default() -> Self {
        Self {
            queue_capacity: 1000,
            max_orphan_batch_size: 10,
            sweep_interval: Duration::from_secs(30),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryQueueStatus {
    Ready,
    Full,
    Closed,
}

impl RecoveryQueueStatus {
    #[must_use]
    pub const fn can_enqueue(self) -> bool {
        matches!(self, Self::Ready)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrphanRecord {
    pub instance_id: InstanceId,
    pub detected_at_ms: u64,
    pub reason: OrphanReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrphanReason {
    StalePendingTimer,
    IncompleteEffect,
    InterruptedWorkflow,
}

impl std::fmt::Display for OrphanReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StalePendingTimer => write!(f, "stale_pending_timer"),
            Self::IncompleteEffect => write!(f, "incomplete_effect"),
            Self::InterruptedWorkflow => write!(f, "interrupted_workflow"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryItem {
    Orphan(OrphanRecord),
    TimerRecovery {
        instance_id: InstanceId,
        fire_at_ms: u64,
    },
}

#[derive(Debug, Clone)]
pub struct ThrottledRecoveryChannel {
    sender: mpsc::Sender<RecoveryItem>,
    receiver: Arcstd::sync::Mutex<Option<mpsc::Receiver<RecoveryItem>>>,
    status_sender: watch::Sender<RecoveryQueueStatus>,
    config: RecoveryChannelConfig,
}

impl ThrottledRecoveryChannel {
    pub fn new(config: RecoveryChannelConfig) -> Self {
        let (sender, receiver) = mpsc::channel(config.queue_capacity);
        let (status_sender, _) = watch::channel(RecoveryQueueStatus::Ready);

        Self {
            sender,
            receiver: Arc::new(std::sync::Mutex::new(Some(receiver))),
            status_sender,
            config,
        }
    }

    pub fn with_receiver(
        sender: mpsc::Sender<RecoveryItem>,
        receiver: mpsc::Receiver<RecoveryItem>,
        config: RecoveryChannelConfig,
    ) -> Self {
        let (status_sender, _) = watch::channel(RecoveryQueueStatus::Ready);

        Self {
            sender,
            receiver: Arc::new(std::sync::Mutex::new(Some(receiver))),
            status_sender,
            config,
        }
    }

    #[must_use]
    pub fn current_status(&self) -> RecoveryQueueStatus {
        *self.status_sender.borrow()
    }

    #[must_use]
    pub fn try_enqueue_orphan(&self, orphan: OrphanRecord) -> Result<(), RecoveryError> {
        if !self.current_status().can_enqueue() {
            return Err(RecoveryError::QueueFull {
                instance_id: orphan.instance_id.clone(),
            });
        }

        self.sender
            .try_send(RecoveryItem::Orphan(orphan))
            .map_err(|e| match e {
                mpsc::error::TrySendError::Full(_) => RecoveryError::QueueFull {
                    instance_id: match e {
                        mpsc::error::TrySendError::Full(RecoveryItem::Orphan(o)) => o.instance_id,
                        _ => InstanceId::default(),
                    },
                },
                mpsc::error::TrySendError::Closed(_) => RecoveryError::ChannelClosed,
            })?;

        Ok(())
    }

    #[must_use]
    pub fn try_enqueue_timer_recovery(
        &self,
        instance_id: InstanceId,
        fire_at_ms: u64,
    ) -> Result<(), RecoveryError> {
        if !self.current_status().can_enqueue() {
            return Err(RecoveryError::QueueFull { instance_id });
        }

        self.sender
            .try_send(RecoveryItem::TimerRecovery { instance_id, fire_at_ms })
            .map_err(|e| match e {
                mpsc::error::TrySendError::Full(_) => RecoveryError::QueueFull { instance_id },
                mpsc::error::TrySendError::Closed(_) => RecoveryError::ChannelClosed,
            })?;

        Ok(())
    }

    pub async fn enqueue_orphan(&self, orphan: OrphanRecord) -> Result<(), RecoveryError> {
        if !self.current_status().can_enqueue() {
            return Err(RecoveryError::QueueFull {
                instance_id: orphan.instance_id.clone(),
            });
        }

        self.sender
            .send(RecoveryItem::Orphan(orphan))
            .await
            .map_err(|_| RecoveryError::ChannelClosed)?;

        Ok(())
    }

    pub async fn enqueue_timer_recovery(
        &self,
        instance_id: InstanceId,
        fire_at_ms: u64,
    ) -> Result<(), RecoveryError> {
        if !self.current_status().can_enqueue() {
            return Err(RecoveryError::QueueFull { instance_id });
        }

        self.sender
            .send(RecoveryItem::TimerRecovery { instance_id, fire_at_ms })
            .await
            .map_err(|_| RecoveryError::ChannelClosed)?;

        Ok(())
    }

    #[must_use]
    pub fn is_full(&self) -> bool {
        !self.current_status().can_enqueue()
    }

    #[must_use]
    pub fn config(&self) -> &RecoveryChannelConfig {
        &self.config
    }

    pub fn take_receiver(&self) -> Option<mpsc::Receiver<RecoveryItem>> {
        self.receiver.lock().ok().and_then(|mut g| g.take())
    }

    fn update_status(&self, status: RecoveryQueueStatus) {
        let _ = self.status_sender.send(status);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryError {
    QueueFull { instance_id: InstanceId },
    ChannelClosed,
    OrphanDetectionFailed { reason: String },
    SweepTimeout { elapsed: Duration },
}

impl std::fmt::Display for RecoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QueueFull { instance_id } => {
                write!(f, "Recovery queue full, cannot enqueue orphan {}", instance_id)
            }
            Self::ChannelClosed => write!(f, "Recovery channel closed"),
            Self::OrphanDetectionFailed { reason } => {
                write!(f, "Orphan detection failed: {}", reason)
            }
            Self::SweepTimeout { elapsed } => {
                write!(f, "Orphan sweep timed out after {:?}", elapsed)
            }
        }
    }
}

impl std::error::Error for RecoveryError {}

impl RecoveryError {
    #[must_use]
    pub const fn is_throttling(&self) -> bool {
        matches!(self, Self::QueueFull { .. })
    }
}

pub trait OrphanDetector: Send + Sync {
    fn detect_orphans(&self) -> impl std::future::Future<Output = Result<Vec<OrphanRecord>, RecoveryError>> + Send;
    fn is_orphan_candidate(&self, instance_id: &InstanceId) -> impl std::future::Future<Output = Result<bool, RecoveryError>> + Send;
}

pub struct OrphanSweepState {
    pub orphans_detected: u64,
    pub orphans_enqueued: u64,
    pub orphans_rejected: u64,
    pub last_sweep_at_ms: u64,
}

impl Default for OrphanSweepState {
    fn default() -> Self {
        Self {
            orphans_detected: 0,
            orphans_enqueued: 0,
            orphans_rejected: 0,
            last_sweep_at_ms: 0,
        }
    }
}

impl OrphanSweepState {
    pub fn record_detection(&mut self, count: u64) {
        self.orphans_detected += count;
    }

    pub fn record_enqueued(&mut self, count: u64) {
        self.orphans_enqueued += count;
    }

    pub fn record_rejected(&mut self, count: u64) {
        self.orphans_rejected += count;
    }

    #[must_use]
    pub fn rejection_rate(&self) -> f64 {
        if self.orphans_detected == 0 {
            return 0.0;
        }
        self.orphans_rejected as f64 / self.orphans_detected as f64
    }
}

pub struct RecoverySweeper {
    config: RecoveryChannelConfig,
    channel: Arc<ThrottledRecoveryChannel>,
    state: std::sync::Mutex<OrphanSweepState>,
}

impl RecoverySweeper {
    pub fn new(
        config: RecoveryChannelConfig,
        channel: Arc<ThrottledRecoveryChannel>,
    ) -> Self {
        Self {
            config,
            channel,
            state: std::sync::Mutex::new(OrphanSweepState::default()),
        }
    }

    pub fn channel(&self) -> &Arc<ThrottledRecoveryChannel> {
        &self.channel
    }

    pub fn state(&self) -> std::sync::MutexGuard<'_, OrphanSweepState> {
        self.state.lock().unwrap()
    }

    pub async fn run_sweep<D>(&self, detector: &D) -> Result<(), RecoveryError>
    where
        D: OrphanDetector,
    {
        let orphans = detector.detect_orphans().await?;

        if orphans.is_empty() {
            debug!("No orphans detected during sweep");
            return Ok(());
        }

        let mut state = self.state();
        state.record_detection(orphans.len() as u64);

        let mut enqueued = 0u64;
        let mut rejected = 0u64;

        for orphan in orphans.iter().take(self.config.max_orphan_batch_size as usize) {
            match self.channel.try_enqueue_orphan(orphan.clone()) {
                Ok(()) => enqueued += 1,
                Err(e) if e.is_throttling() => {
                    rejected += 1;
                    warn!(
                        instance_id = %orphan.instance_id,
                        reason = %orphan.reason,
                        "Orphan rejected due to throttling"
                    );
                }
                Err(e) => {
                    error!(
                        instance_id = %orphan.instance_id,
                        error = %e,
                        "Failed to enqueue orphan"
                    );
                }
            }
        }

        state.record_enqueued(enqueued);
        state.record_rejected(rejected);

        if rejected > 0 {
            info!(
                enqueued,
                rejected,
                total = orphans.len(),
                "Orphan sweep completed with throttling"
            );
        } else {
            debug!(
                enqueued,
                total = orphans.len(),
                "Orphan sweep completed"
            );
        }

        Ok(())
    }

    pub async fn run_periodic_sweep<D>(&self, detector: Arc<D>) -> Result<(), RecoveryError>
    where
        D: OrphanDetector,
    {
        let mut sweep_interval = interval(self.config.sweep_interval);
        sweep_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            sweep_interval.tick().await;

            if let Err(e) = self.run_sweep(detector.as_ref()).await {
                if e.is_throttling() {
                    debug!("Sweep skipped due to throttling");
                } else {
                    error!(error = %e, "Orphan sweep failed");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throttled_channel_new_channel_has_ready_status() {
        let config = RecoveryChannelConfig::default();
        let channel = ThrottledRecoveryChannel::new(config);

        assert_eq!(channel.current_status(), RecoveryQueueStatus::Ready);
        assert!(!channel.is_full());
    }

    #[test]
    fn throttled_channel_try_enqueue_orphan_succeeds_when_not_full() {
        let config = RecoveryChannelConfig {
            queue_capacity: 10,
            max_orphan_batch_size: 5,
            sweep_interval: Duration::from_secs(1),
        };
        let channel = ThrottledRecoveryChannel::new(config);

        let orphan = OrphanRecord {
            instance_id: InstanceId::new_v4(),
            detected_at_ms: 1000,
            reason: OrphanReason::StalePendingTimer,
        };

        assert!(channel.try_enqueue_orphan(orphan).is_ok());
    }

    #[test]
    fn throttled_channel_respects_queue_capacity() {
        let config = RecoveryChannelConfig {
            queue_capacity: 2,
            max_orphan_batch_size: 10,
            sweep_interval: Duration::from_secs(1),
        };
        let channel = ThrottledRecoveryChannel::new(config);

        let orphan1 = OrphanRecord {
            instance_id: InstanceId::new_v4(),
            detected_at_ms: 1000,
            reason: OrphanReason::StalePendingTimer,
        };
        let orphan2 = OrphanRecord {
            instance_id: InstanceId::new_v4(),
            detected_at_ms: 1001,
            reason: OrphanReason::IncompleteEffect,
        };
        let orphan3 = OrphanRecord {
            instance_id: InstanceId::new_v4(),
            detected_at_ms: 1002,
            reason: OrphanReason::InterruptedWorkflow,
        };

        assert!(channel.try_enqueue_orphan(orphan1).is_ok());
        assert!(channel.try_enqueue_orphan(orphan2).is_ok());
        let result = channel.try_enqueue_orphan(orphan3);
        assert!(result.is_err());
        assert!(result.unwrap_err().is_throttling());
    }

    #[test]
    fn throttled_channel_timer_recovery_also_throttled() {
        let config = RecoveryChannelConfig {
            queue_capacity: 1,
            max_orphan_batch_size: 10,
            sweep_interval: Duration::from_secs(1),
        };
        let channel = ThrottledRecoveryChannel::new(config);

        let instance_id = InstanceId::new_v4();
        assert!(channel
            .try_enqueue_timer_recovery(instance_id.clone(), 1000)
            .is_ok());

        let result = channel.try_enqueue_timer_recovery(InstanceId::new_v4(), 1001);
        assert!(result.is_err());
        assert!(result.unwrap_err().is_throttling());
    }

    #[test]
    fn recovery_queue_status_can_enqueue() {
        assert!(RecoveryQueueStatus::Ready.can_enqueue());
        assert!(!RecoveryQueueStatus::Full.can_enqueue());
        assert!(!RecoveryQueueStatus::Closed.can_enqueue());
    }

    #[test]
    fn orphan_sweep_state_tracks_metrics() {
        let mut state = OrphanSweepState::default();

        state.record_detection(10);
        assert_eq!(state.orphans_detected, 10);

        state.record_enqueued(7);
        assert_eq!(state.orphans_enqueued, 7);

        state.record_rejected(3);
        assert_eq!(state.orphans_rejected, 3);

        assert!((state.rejection_rate() - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn orphan_sweep_state_zero_detection_gives_zero_rejection_rate() {
        let state = OrphanSweepState::default();
        assert_eq!(state.rejection_rate(), 0.0);
    }

    #[tokio::test]
    async fn recovery_error_display() {
        let err = RecoveryError::QueueFull {
            instance_id: InstanceId::new_v4(),
        };
        assert!(err.to_string().contains("Queue full"));

        let err = RecoveryError::ChannelClosed;
        assert!(err.to_string().contains("closed"));

        let err = RecoveryError::OrphanDetectionFailed {
            reason: "test".to_string(),
        };
        assert!(err.to_string().contains("Orphan detection failed"));
    }

    #[tokio::test]
    async fn throttled_channel_async_enqueue() {
        let config = RecoveryChannelConfig {
            queue_capacity: 1,
            max_orphan_batch_size: 10,
            sweep_interval: Duration::from_secs(1),
        };
        let channel = ThrottledRecoveryChannel::new(config);

        let orphan = OrphanRecord {
            instance_id: InstanceId::new_v4(),
            detected_at_ms: 1000,
            reason: OrphanReason::StalePendingTimer,
        };

        assert!(channel.enqueue_orphan(orphan).await.is_ok());
    }

    #[test]
    fn orphan_reason_display() {
        assert_eq!(OrphanReason::StalePendingTimer.to_string(), "stale_pending_timer");
        assert_eq!(OrphanReason::IncompleteEffect.to_string(), "incomplete_effect");
        assert_eq!(
            OrphanReason::InterruptedWorkflow.to_string(),
            "interrupted_workflow"
        );
    }
}
