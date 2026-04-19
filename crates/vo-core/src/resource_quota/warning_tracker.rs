//! Thread-safe quota warning state tracking.
//!
//! Provides atomic gauges for monitoring soft limit warning state per resource,
//! following the admission/metrics pattern in the codebase.

use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use super::types::{ResourceKind, SoftLimitWarning};

/// Per-resource warning state tracked atomically.
#[derive(Debug, Default)]
struct ResourceWarningState {
    /// Whether the soft limit is currently exceeded.
    active: AtomicBool,
    /// Timestamp of the most recent warning emission (unix epoch seconds).
    last_warned: AtomicU64,
}

impl ResourceWarningState {
    fn new() -> Self {
        Self::default()
    }

    fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    fn set_active(&self, val: bool) {
        self.active.store(val, Ordering::SeqCst);
    }

    fn last_warned(&self) -> u64 {
        self.last_warned.load(Ordering::SeqCst)
    }

    fn mark_warned(&self) {
        self.last_warned.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            Ordering::SeqCst,
        );
    }
}

/// Thread-safe tracker for quota soft limit warnings.
///
/// Tracks per-resource warning state with rate limiting to prevent
/// warning storms when usage hovers near the soft limit threshold.
pub struct QuotaWarningTracker {
    // Intentionally not Clone — the tracker holds mutable state (atomic counters,
    // recent buffer mutex) that should not be duplicated.
    cpu: ResourceWarningState,
    memory: ResourceWarningState,
    disk: ResourceWarningState,
    /// Minimum seconds between repeated warnings for the same resource.
    min_interval_secs: AtomicU64,
    /// Recent warnings for inspection (bounded ring buffer).
    recent: Mutex<Vec<(ResourceKind, Instant)>>,
    max_recent: usize,
}

impl QuotaWarningTracker {
    /// Creates a new tracker with default 60-second warning interval.
    #[must_use]
    pub fn new() -> Self {
        Self::with_interval(60)
    }

    /// Creates a new tracker with a custom minimum warning interval in seconds.
    #[must_use]
    pub fn with_interval(min_interval_secs: u64) -> Self {
        Self {
            cpu: ResourceWarningState::new(),
            memory: ResourceWarningState::new(),
            disk: ResourceWarningState::new(),
            min_interval_secs: AtomicU64::new(min_interval_secs),
            recent: Mutex::new(Vec::with_capacity(16)),
            max_recent: 16,
        }
    }

    fn state_for(&self, resource: ResourceKind) -> &ResourceWarningState {
        match resource {
            ResourceKind::Cpu => &self.cpu,
            ResourceKind::Memory => &self.memory,
            ResourceKind::Disk => &self.disk,
        }
    }

    /// Records a soft limit warning, respecting the rate limit.
    ///
    /// Returns `true` if the warning was actually emitted (not rate-limited).
    pub fn record_warning(&self, warning: &SoftLimitWarning) -> bool {
        let state = self.state_for(warning.resource);
        state.set_active(true);

        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let min_interval = self.min_interval_secs.load(Ordering::SeqCst);
        let last = state.last_warned();

        if now_secs.saturating_sub(last) < min_interval {
            return false;
        }

        state.mark_warned();
        if let Ok(mut recent) = self.recent.lock() {
            if recent.len() >= self.max_recent {
                recent.remove(0);
            }
            recent.push((warning.resource, Instant::now()));
        }
        true
    }

    /// Clears the warning state for a resource (e.g., when usage drops below threshold).
    pub fn clear_warning(&self, resource: ResourceKind) {
        self.state_for(resource).set_active(false);
    }

    /// Returns whether any resource currently has an active soft limit warning.
    #[must_use]
    pub fn any_active(&self) -> bool {
        self.cpu.is_active() || self.memory.is_active() || self.disk.is_active()
    }

    /// Returns whether a specific resource has an active warning.
    #[must_use]
    pub fn is_active(&self, resource: ResourceKind) -> bool {
        self.state_for(resource).is_active()
    }

    /// Returns the number of recent warnings in the ring buffer.
    #[must_use]
    pub fn recent_count(&self) -> usize {
        self.recent.lock().map(|r| r.len()).unwrap_or(0)
    }

    /// Sets the minimum interval between repeated warnings.
    pub fn set_interval(&self, secs: u64) {
        self.min_interval_secs.store(secs, Ordering::SeqCst);
    }

    /// Returns the current minimum warning interval in seconds.
    #[must_use]
    pub fn interval(&self) -> u64 {
        self.min_interval_secs.load(Ordering::SeqCst)
    }
}

impl Default for QuotaWarningTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource_quota::types::ResourceKind;

    fn make_warning(resource: ResourceKind) -> SoftLimitWarning {
        SoftLimitWarning::new(resource, "test-ns", 85, 80, 100)
    }

    #[test]
    fn tracker_new_creates_with_defaults() {
        let tracker = QuotaWarningTracker::new();
        assert!(!tracker.any_active());
        assert_eq!(tracker.interval(), 60);
    }

    #[test]
    fn tracker_with_interval_sets_custom_interval() {
        let tracker = QuotaWarningTracker::with_interval(30);
        assert_eq!(tracker.interval(), 30);
    }

    #[test]
    fn record_warning_activates_state() {
        let tracker = QuotaWarningTracker::with_interval(0);
        let warning = make_warning(ResourceKind::Cpu);
        tracker.record_warning(&warning);
        assert!(tracker.is_active(ResourceKind::Cpu));
        assert!(tracker.any_active());
    }

    #[test]
    fn clear_warning_deactivates_state() {
        let tracker = QuotaWarningTracker::with_interval(0);
        let warning = make_warning(ResourceKind::Memory);
        tracker.record_warning(&warning);
        assert!(tracker.is_active(ResourceKind::Memory));
        tracker.clear_warning(ResourceKind::Memory);
        assert!(!tracker.is_active(ResourceKind::Memory));
    }

    #[test]
    fn record_warning_returns_true_when_not_rate_limited() {
        let tracker = QuotaWarningTracker::with_interval(0);
        let warning = make_warning(ResourceKind::Disk);
        assert!(tracker.record_warning(&warning));
    }

    #[test]
    fn record_warning_rate_limits_subsequent_calls() {
        let tracker = QuotaWarningTracker::with_interval(3600);
        let warning = make_warning(ResourceKind::Cpu);
        assert!(tracker.record_warning(&warning));
        assert!(!tracker.record_warning(&warning));
    }

    #[test]
    fn record_warning_tracks_recent_count() {
        let tracker = QuotaWarningTracker::with_interval(0);
        assert_eq!(tracker.recent_count(), 0);
        tracker.record_warning(&make_warning(ResourceKind::Cpu));
        assert_eq!(tracker.recent_count(), 1);
        tracker.record_warning(&make_warning(ResourceKind::Memory));
        assert_eq!(tracker.recent_count(), 2);
    }

    #[test]
    fn recent_buffer_evicts_oldest_when_full() {
        let tracker = QuotaWarningTracker::with_interval(0);
        let resources = [
            ResourceKind::Cpu,
            ResourceKind::Memory,
            ResourceKind::Disk,
        ];
        for _ in 0..20 {
            for &r in &resources {
                tracker.record_warning(&make_warning(r));
            }
        }
        assert_eq!(tracker.recent_count(), 16);
    }

    #[test]
    fn set_interval_updates_interval() {
        let tracker = QuotaWarningTracker::new();
        assert_eq!(tracker.interval(), 60);
        tracker.set_interval(120);
        assert_eq!(tracker.interval(), 120);
    }

    #[test]
    fn each_resource_tracks_independently() {
        let tracker = QuotaWarningTracker::with_interval(0);
        tracker.record_warning(&make_warning(ResourceKind::Cpu));
        assert!(tracker.is_active(ResourceKind::Cpu));
        assert!(!tracker.is_active(ResourceKind::Memory));
        assert!(!tracker.is_active(ResourceKind::Disk));
    }
}
