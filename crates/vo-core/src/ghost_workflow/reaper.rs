//! ReaperConfig — configuration for the background reaper loop

use std::time::Duration;

/// Configuration for the ghost workflow reaper background task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReaperConfig {
    sweep_interval: Duration,
}

impl ReaperConfig {
    #[must_use]
    pub fn new(sweep_interval: Duration) -> Self {
        Self { sweep_interval }
    }

    #[must_use]
    pub fn sweep_interval(&self) -> Duration {
        self.sweep_interval
    }
}

impl Default for ReaperConfig {
    fn default() -> Self {
        Self {
            sweep_interval: Duration::from_secs(60),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn reaper_config_default_is_60_seconds() {
        let config = ReaperConfig::default();
        assert_eq!(config.sweep_interval(), Duration::from_secs(60));
    }
}
