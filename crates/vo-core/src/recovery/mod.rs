//! Recovery queue throttling and orphan detection (ADR-043).
//!
//! This module provides:
//! - [`RecoveryThrottle`]: A bounded channel that enforces ingestion rate limits
//! - [`OrphanDetector`]: Periodic sweep mechanism to detect orphan processes
//! - [`RecoveryQueue`]: Throttled queue producer for orphan recovery
//!
//! ## Requirements
//!
//! 1. THE SYSTEM SHALL sweep for orphan processes periodically
//! 2. WHEN orphan is detected, THE SYSTEM SHALL queue it for recovery via throttled queue
//! 3. IF recovery queue is full, THE SYSTEM SHALL NOT enqueue more orphans

pub mod sweep;
pub mod throttle;

use serde::{Deserialize, Serialize};
use std::time::Duration;

pub use sweep::{OrphanDetector, OrphanQuery};
pub use throttle::{RecoveryThrottle, RecoveryThrottleConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrphanProcess {
    pub instance_id: String,
    pub lineage_id: String,
    pub failed_at: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryItem {
    pub orphan: OrphanProcess,
    pub enqueued_at: Duration,
}

#[derive(Debug, thiserror::Error)]
pub enum RecoveryError {
    #[error("recovery queue is full: cannot enqueue more orphans (throttle engaged)")]
    QueueFull,

    #[error("sweep channel closed unexpectedly")]
    SweepChannelClosed,

    #[error("orphan detection query failed: {0}")]
    SweepQueryFailed(String),
}

pub type RecoveryResult<T> = Result<T, RecoveryError>;

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn recovery_throttle_respects_capacity_limit() {
        let config = RecoveryThrottleConfig {
            capacity: 2,
            refill_rate: 1,
            refill_period: Duration::from_secs(1),
        };
        let mut throttle = RecoveryThrottle::new(config);

        let item = RecoveryItem {
            orphan: OrphanProcess {
                instance_id: "test-1".to_string(),
                lineage_id: "lineage-1".to_string(),
                failed_at: Duration::from_secs(0),
            },
            enqueued_at: Duration::from_secs(0),
        };

        let result1 = throttle.enqueue(item.clone()).await;
        assert!(result1.is_ok(), "First enqueue should succeed");

        let result2 = throttle.enqueue(item.clone()).await;
        assert!(result2.is_ok(), "Second enqueue should succeed");

        let result3 = throttle.enqueue(item.clone()).await;
        assert!(
            matches!(result3, Err(RecoveryError::QueueFull)),
            "Third enqueue should fail with QueueFull"
        );
    }

    #[tokio::test]
    async fn recovery_throttle_refills_over_time() {
        let config = RecoveryThrottleConfig {
            capacity: 1,
            refill_rate: 1,
            refill_period: Duration::from_millis(100),
        };
        let mut throttle = RecoveryThrottle::new(config);

        let item = RecoveryItem {
            orphan: OrphanProcess {
                instance_id: "test-1".to_string(),
                lineage_id: "lineage-1".to_string(),
                failed_at: Duration::from_secs(0),
            },
            enqueued_at: Duration::from_secs(0),
        };

        let result1 = throttle.enqueue(item.clone()).await;
        assert!(result1.is_ok());

        let result2 = throttle.enqueue(item.clone()).await;
        assert!(matches!(result2, Err(RecoveryError::QueueFull)));

        throttle.advance_time(Duration::from_millis(150));

        let result3 = throttle.enqueue(item.clone()).await;
        assert!(result3.is_ok(), "After refill, enqueue should succeed");
    }
}