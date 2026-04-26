use std::sync::Mutex;
use std::time::Instant;

#[derive(Debug, Default)]
pub struct CommitLatencyTracker {
    state: Mutex<CommitLatencyState>,
}

#[derive(Debug, Default)]
struct CommitLatencyState {
    last_commit_at: Option<Instant>,
    sample_count: u64,
    total_latency_ms: u128,
}

impl CommitLatencyTracker {
    pub fn record_commit(&self, latency_ms: u64) {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.last_commit_at = Some(Instant::now());
        state.sample_count += 1;
        state.total_latency_ms += u128::from(latency_ms);
    }

    #[must_use]
    pub fn time_since_last_commit(&self) -> Option<std::time::Duration> {
        let state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.last_commit_at.map(|instant| instant.elapsed())
    }

    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn average_latency_ms(&self) -> Option<u64> {
        let state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if state.sample_count == 0 {
            return None;
        }
        Some(
            u64::try_from(state.total_latency_ms / u128::from(state.sample_count))
                .unwrap_or(u64::MAX),
        )
    }

    #[must_use]
    pub fn sample_count(&self) -> u64 {
        let state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.sample_count
    }
}
