//! Graceful shutdown propagation through the actor hierarchy.

// =============================================================================
// Graceful Shutdown Propagation
// =============================================================================

/// Result of a shutdown propagation operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShutdownResult {
    /// All children shut down successfully.
    Success,
    /// Some children are still running.
    ChildrenRunning { pending: usize },
    /// Shutdown timed out.
    Timeout { remaining: usize },
}

/// Controls graceful shutdown propagation through the actor hierarchy.
#[derive(Debug)]
pub struct ShutdownPropagator {
    graceful_timeout: std::time::Duration,
    force_kill_timeout: std::time::Duration,
}

impl ShutdownPropagator {
    /// Creates a new propagator with the given timeouts.
    #[must_use]
    pub fn new(
        graceful_timeout: std::time::Duration,
        force_kill_timeout: std::time::Duration,
    ) -> Self {
        Self {
            graceful_timeout,
            force_kill_timeout,
        }
    }

    /// Default propagator with 30s graceful, 10s force kill.
    #[must_use]
    pub fn default_propagator() -> Self {
        Self {
            graceful_timeout: std::time::Duration::from_secs(30),
            force_kill_timeout: std::time::Duration::from_secs(10),
        }
    }

    /// Returns the graceful shutdown timeout.
    #[must_use]
    pub const fn graceful_timeout(&self) -> std::time::Duration {
        self.graceful_timeout
    }

    /// Returns the force kill timeout.
    #[must_use]
    pub const fn force_kill_timeout(&self) -> std::time::Duration {
        self.force_kill_timeout
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_propagator() {
        let propagator = ShutdownPropagator::default_propagator();
        assert_eq!(
            propagator.graceful_timeout(),
            std::time::Duration::from_secs(30)
        );
        assert_eq!(
            propagator.force_kill_timeout(),
            std::time::Duration::from_secs(10)
        );
    }
}
