use crate::replay::projection::throttle::RebuildThrottleConfig;

#[derive(Debug, Clone, Copy)]
pub struct EventSourcingConfig {
    pub max_schema_version: u8,
    pub throttle_config: RebuildThrottleConfig,
    pub snapshot_interval_events: u64,
}

impl Default for EventSourcingConfig {
    fn default() -> Self {
        Self {
            max_schema_version: 1,
            throttle_config: RebuildThrottleConfig::default(),
            snapshot_interval_events: 1000,
        }
    }
}

impl EventSourcingConfig {
    #[must_use]
    pub fn new(max_schema_version: u8, snapshot_interval_events: u64) -> Self {
        Self {
            max_schema_version,
            throttle_config: RebuildThrottleConfig::default(),
            snapshot_interval_events,
        }
    }

    #[must_use]
    pub const fn with_throttle(mut self, config: RebuildThrottleConfig) -> Self {
        self.throttle_config = config;
        self
    }
}
