//! Stub module for TimerRecord type.
//! Placeholder for future timer record implementation.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerRecord {
    pub fire_at_ms: u64,
}

impl TimerRecord {
    #[must_use]
    pub fn new(fire_at_ms: u64) -> Self {
        Self { fire_at_ms }
    }
}
