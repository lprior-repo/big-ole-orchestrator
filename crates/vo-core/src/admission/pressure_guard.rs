//! Writer pressure guard for HTTP ingress load shedding.
//!
//! Per ADR-006 and ADR-015, when the DbWriter mailbox reaches 80% capacity,
//! the HTTP API must return 429 Too Many Requests with a `Retry-After` header.
//! This module provides the [`WriterPressureGuard`] trait and a concrete
//! [`WatchdogPressureGuard`] that reads from the storage watchdog's
//! `watch::Receiver<StorageHealth>`.

use crate::storage_watchdog::types::{StorageHealth, StorageWatchdogConfig};

/// Default DbWriter mailbox capacity (ADR-015: 10,000 messages).
const DB_WRITER_MAILBOX_CAPACITY: u64 = 10_000;

/// Shed threshold as a fraction of capacity (80%).
const SHED_FRACTION: f64 = 0.80;

/// Result of a pressure check against the writer guard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PressureGuardResult {
    /// Admission granted — writer has capacity.
    Admitted,
    /// Admission rejected — writer under pressure, caller should retry.
    Shed {
        /// Suggested `Retry-After` value in seconds.
        retry_after_secs: u64,
        /// Human-readable reason for the shed.
        reason: String,
    },
}

/// Trait for checking whether the DbWriter is under too much pressure to
/// admit new workflow starts.
pub trait WriterPressureGuard: Send + Sync {
    /// Check current writer pressure and return an admission decision.
    fn check(&self) -> PressureGuardResult;
}

/// Pressure guard backed by the storage watchdog's `watch::Receiver<StorageHealth>`.
///
/// This reads the latest [`StorageHealth`] and maps writer-queue-depth pressure
/// to a shed decision using the 80% mailbox-capacity threshold from ADR-015.
///
/// When the `StorageHealth` is `Degraded` or `Critical` and the indicators
/// include [`PressureIndicator::WriterQueueDepth`], ingress is shed.
#[derive(Debug)]
pub struct WatchdogPressureGuard {
    health_receiver: tokio::sync::watch::Receiver<StorageHealth>,
    config: StorageWatchdogConfig,
}

impl WatchdogPressureGuard {
    /// Create a new guard from the watchdog's health receiver.
    ///
    /// The `config` is used to determine the shed threshold. The effective
    /// shed level is `config.writer_queue_depth_threshold * 0.8`, capped at
    /// `DB_WRITER_MAILBOX_CAPACITY * 0.8`.
    #[must_use]
    pub fn new(
        health_receiver: tokio::sync::watch::Receiver<StorageHealth>,
        config: StorageWatchdogConfig,
    ) -> Self {
        Self {
            health_receiver,
            config,
        }
    }

    /// Create a guard that always admits (for testing / graceful degradation).
    #[must_use]
    pub fn permissive() -> Self {
        let (_, rx) = tokio::sync::watch::channel(StorageHealth::Healthy);
        Self {
            health_receiver: rx,
            config: StorageWatchdogConfig {
                writer_queue_depth_threshold: u64::MAX,
                ..StorageWatchdogConfig::default()
            },
        }
    }

    fn shed_threshold(&self) -> u64 {
        let watchdog_threshold = self.config.writer_queue_depth_threshold;
        let mailbox_shed = (DB_WRITER_MAILBOX_CAPACITY as f64 * SHED_FRACTION) as u64;
        std::cmp::min(watchdog_threshold, mailbox_shed)
    }
}

impl WriterPressureGuard for WatchdogPressureGuard {
    fn check(&self) -> PressureGuardResult {
        let health = self.health_receiver.borrow();

        match &*health {
            StorageHealth::Healthy => PressureGuardResult::Admitted,
            StorageHealth::Degraded { indicators }
            | StorageHealth::Critical {
                indicators,
                writer_stalled: _,
            } => {
                let has_writer_pressure = indicators.iter().any(|i| {
                    matches!(
                        i,
                        crate::admission::types::PressureIndicator::WriterQueueDepth
                    )
                });

                if has_writer_pressure {
                    PressureGuardResult::Shed {
                        retry_after_secs: 5,
                        reason: format!(
                            "writer queue at shed threshold (>{}/{} capacity)",
                            self.shed_threshold(),
                            DB_WRITER_MAILBOX_CAPACITY
                        ),
                    }
                } else {
                    PressureGuardResult::Admitted
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_guard(health: StorageHealth, threshold: u64) -> WatchdogPressureGuard {
        let (tx, rx) = tokio::sync::watch::channel(health);
        let _ = tx; // keep sender alive
        let config = StorageWatchdogConfig {
            writer_queue_depth_threshold: threshold,
            ..StorageWatchdogConfig::default()
        };
        WatchdogPressureGuard::new(rx, config)
    }

    #[test]
    fn healthy_always_admits() {
        let guard = make_guard(StorageHealth::Healthy, 500);
        assert_eq!(guard.check(), PressureGuardResult::Admitted);
    }

    #[test]
    fn degraded_without_writer_pressure_admits() {
        let guard = make_guard(
            StorageHealth::Degraded {
                indicators: vec![crate::admission::types::PressureIndicator::CompactionStall],
            },
            500,
        );
        assert_eq!(guard.check(), PressureGuardResult::Admitted);
    }

    #[test]
    fn degraded_with_writer_pressure_sheds() {
        let guard = make_guard(
            StorageHealth::Degraded {
                indicators: vec![crate::admission::types::PressureIndicator::WriterQueueDepth],
            },
            500,
        );
        match guard.check() {
            PressureGuardResult::Shed {
                retry_after_secs,
                reason,
            } => {
                assert_eq!(retry_after_secs, 5);
                assert!(reason.contains("writer queue at shed threshold"));
            }
            other => panic!("expected Shed, got {other:?}"),
        }
    }

    #[test]
    fn critical_with_writer_pressure_sheds() {
        let guard = make_guard(
            StorageHealth::Critical {
                indicators: vec![
                    crate::admission::types::PressureIndicator::WriterQueueDepth,
                    crate::admission::types::PressureIndicator::BatchCommitLatency,
                    crate::admission::types::PressureIndicator::StorageStall,
                ],
                writer_stalled: false,
            },
            500,
        );
        match guard.check() {
            PressureGuardResult::Shed {
                retry_after_secs, ..
            } => {
                assert_eq!(retry_after_secs, 5);
            }
            other => panic!("expected Shed, got {other:?}"),
        }
    }

    #[test]
    fn critical_without_writer_pressure_admits() {
        let guard = make_guard(
            StorageHealth::Critical {
                indicators: vec![
                    crate::admission::types::PressureIndicator::CompactionStall,
                    crate::admission::types::PressureIndicator::StorageStall,
                    crate::admission::types::PressureIndicator::BatchCommitLatency,
                ],
                writer_stalled: true,
            },
            500,
        );
        assert_eq!(guard.check(), PressureGuardResult::Admitted);
    }

    #[test]
    fn permissive_guard_always_admits() {
        let guard = WatchdogPressureGuard::permissive();
        assert_eq!(guard.check(), PressureGuardResult::Admitted);
    }

    #[test]
    fn shed_threshold_caps_at_mailbox_capacity() {
        let (tx, rx) = tokio::sync::watch::channel(StorageHealth::Healthy);
        let _ = tx;
        let guard = WatchdogPressureGuard::new(
            rx,
            StorageWatchdogConfig {
                writer_queue_depth_threshold: 100_000,
                ..StorageWatchdogConfig::default()
            },
        );
        let expected = (DB_WRITER_MAILBOX_CAPACITY as f64 * SHED_FRACTION) as u64;
        assert_eq!(guard.shed_threshold(), expected);
    }
}
