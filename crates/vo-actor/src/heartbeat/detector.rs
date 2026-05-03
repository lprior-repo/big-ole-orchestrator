//! Failure detection logic for actor health monitoring.
//!
//! Per ADR-012: Tracks consecutive health check failures per actor and
//! determines when failure thresholds are exceeded.

use tokio::time::Instant;

/// Actor tracking state for heartbeat monitoring.
#[derive(Debug, Clone)]
pub struct ActorHealthState {
    pub(crate) consecutive_failures: u32,
    last_check: Option<Instant>,
    last_healthy: Option<Instant>,
}

impl ActorHealthState {
    /// Creates a new health state with zero failures.
    #[must_use]
    pub fn new() -> Self {
        Self {
            consecutive_failures: 0,
            last_check: None,
            last_healthy: None,
        }
    }

    /// Records a health check failure.
    pub fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        self.last_check = Some(Instant::now());
    }

    /// Records a healthy health check result.
    pub fn record_healthy(&mut self) {
        self.consecutive_failures = 0;
        self.last_check = Some(Instant::now());
        self.last_healthy = Some(Instant::now());
    }
}

impl Default for ActorHealthState {
    fn default() -> Self {
        Self::new()
    }
}
