//! Memory leak detection types and invariants.
//!
//! This module defines the design-by-contract types for memory leak detection
//! in the vo-executor state management system.
//!
//! # Key Concepts
//!
//! - [`MemorySnapshot`]: Point-in-time capture of memory state
//! - [`LeakIndicator`]: Result of comparing two snapshots for leak detection
//! - [`LeakThreshold`]: Bounds defining acceptable vs unacceptable accumulation
//! - [`MemoryLeakDetector`]: Tool for comparing snapshots and reporting leaks
//!
//! # Invariants
//!
//! 1. **Baseline Invariant**: After all work completes, state count should return to baseline
//! 2. **Bounded Growth Invariant**: State count growth should be proportional to concurrent work
//! 3. **Error Cleanup Invariant**: Error map should not grow unbounded
//!
//! # Usage
//!
//! ```ignore
//! use vo_types::memory_leak::{MemorySnapshot, LeakIndicator, LeakThreshold};
//!
//! // Capture baseline
//! let baseline = MemorySnapshot::capture();
//!
//! // Run workloads...
//!
//! // Check for leaks
//! let indicator = LeakIndicator::detect(&baseline, &current, &threshold);
//! ```

use std::time::{Duration, Instant};

/// Captures the current state of memory in the executor.
///
/// This is a point-in-time snapshot that records:
/// - Number of entries in the state map
/// - Number of entries in the error map
/// - Timestamp of capture
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySnapshot {
    /// Number of entries in the state map.
    pub state_count: usize,
    /// Number of entries in the error map.
    pub error_count: usize,
    /// Wall clock time when captured.
    pub captured_at: Instant,
}

impl MemorySnapshot {
    /// Returns the time elapsed since this snapshot was captured.
    pub fn age(&self) -> Duration {
        self.captured_at.elapsed()
    }

    /// Returns the total entries across both maps.
    pub fn total_entries(&self) -> usize {
        self.state_count.saturating_add(self.error_count)
    }
}

/// Defines thresholds for determining if memory accumulation constitutes a leak.
///
/// A leak is detected when:
/// - Growth exceeds `max_state_growth` AND
/// - The growth persists beyond `cleanup_grace_period`
#[derive(Debug, Clone, PartialEq)]
pub struct LeakThreshold {
    /// Maximum acceptable growth in state map entries.
    pub max_state_growth: usize,
    /// Maximum acceptable growth in error map entries.
    pub max_error_growth: usize,
    /// Time allowed for cleanup after work completes.
    pub cleanup_grace_period: Duration,
    /// Minimum ratio of state_count to active jobs to flag as leak.
    /// E.g., 1.0 means one stale state per active job is suspicious.
    pub stale_state_ratio_threshold: f64,
}

impl Default for LeakThreshold {
    fn default() -> Self {
        Self {
            max_state_growth: 100,
            max_error_growth: 50,
            cleanup_grace_period: Duration::from_secs(5),
            stale_state_ratio_threshold: 1.0,
        }
    }
}

impl LeakThreshold {
    /// Creates a strict threshold for high-sensitivity leak detection.
    pub fn strict() -> Self {
        Self {
            max_state_growth: 10,
            max_error_growth: 5,
            cleanup_grace_period: Duration::from_millis(500),
            stale_state_ratio_threshold: 0.5,
        }
    }

    /// Creates a lenient threshold for tolerating transient accumulation.
    pub fn permissive() -> Self {
        Self {
            max_state_growth: 500,
            max_error_growth: 200,
            cleanup_grace_period: Duration::from_secs(30),
            stale_state_ratio_threshold: 2.0,
        }
    }
}

/// Indicates whether a memory leak has been detected by comparing two snapshots.
#[derive(Debug, Clone, PartialEq)]
pub enum LeakIndicator {
    /// No leak detected - memory usage is within acceptable bounds.
    NoLeak,
    /// Potential leak detected - growth exceeds threshold.
    LeakDetected {
        /// Number of states added since baseline.
        states_added: usize,
        /// Number of errors added since baseline.
        errors_added: usize,
        /// The threshold that was exceeded.
        threshold: LeakThreshold,
    },
    /// Stale states detected - states exist but no corresponding active jobs.
    StaleStates {
        /// Number of stale states.
        count: usize,
        /// Ratio of stale states to active jobs.
        ratio: f64,
    },
}

impl LeakIndicator {
    /// Detects a leak by comparing a baseline snapshot to a current snapshot.
    pub fn detect(
        baseline: &MemorySnapshot,
        current: &MemorySnapshot,
        threshold: &LeakThreshold,
    ) -> Self {
        let states_added = current.state_count.saturating_sub(baseline.state_count);
        let errors_added = current.error_count.saturating_sub(baseline.error_count);

        if states_added > threshold.max_state_growth {
            return LeakIndicator::LeakDetected {
                states_added,
                errors_added,
                threshold: threshold.clone(),
            };
        }

        if errors_added > threshold.max_error_growth {
            return LeakIndicator::LeakDetected {
                states_added,
                errors_added,
                threshold: threshold.clone(),
            };
        }

        LeakIndicator::NoLeak
    }

    /// Returns true if any leak was detected.
    pub fn is_leak(&self) -> bool {
        matches!(
            self,
            LeakIndicator::LeakDetected { .. } | LeakIndicator::StaleStates { .. }
        )
    }
}

/// Memory leak detector for comparing snapshots and generating reports.
///
/// # Example
///
/// ```ignore
/// use vo_types::memory_leak::{MemoryLeakDetector, LeakThreshold};
///
/// let detector = MemoryLeakDetector::new(LeakThreshold::default());
/// detector.set_baseline(MemorySnapshot::capture());
///
/// // Run workloads...
///
/// let report = detector.check_leak(&current_snapshot);
/// assert!(!report.has_leak());
/// ```
#[derive(Debug, Clone)]
pub struct MemoryLeakDetector {
    threshold: LeakThreshold,
    baseline: Option<MemorySnapshot>,
}

impl MemoryLeakDetector {
    /// Creates a new detector with the given threshold.
    pub fn new(threshold: LeakThreshold) -> Self {
        Self {
            threshold,
            baseline: None,
        }
    }

    /// Sets the baseline snapshot for comparison.
    pub fn set_baseline(&mut self, baseline: MemorySnapshot) {
        self.baseline = Some(baseline);
    }

    /// Returns a reference to the current baseline if set.
    pub fn baseline(&self) -> Option<&MemorySnapshot> {
        self.baseline.as_ref()
    }

    /// Checks for leaks against the stored baseline.
    ///
    /// Returns `None` if no baseline has been set.
    pub fn check_leak(&self, current: &MemorySnapshot) -> Option<LeakIndicator> {
        self.baseline
            .as_ref()
            .map(|baseline| LeakIndicator::detect(baseline, current, &self.threshold))
    }

    /// Resets the baseline to None.
    pub fn reset(&mut self) {
        self.baseline = None;
    }
}

/// Represents the expected cleanup behavior after work completes.
///
/// This is part of the contract: after all work finishes,
/// memory should be cleaned up within the grace period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupExpectation {
    /// Expected time for state map to be cleaned.
    pub state_cleanup_time: Duration,
    /// Expected time for error map to be cleaned.
    pub error_cleanup_time: Duration,
    /// Whether cleanup is required or best-effort.
    pub is_required: bool,
}

impl Default for CleanupExpectation {
    fn default() -> Self {
        Self {
            state_cleanup_time: Duration::from_secs(5),
            error_cleanup_time: Duration::from_secs(10),
            is_required: true,
        }
    }
}

/// Invariant: After N executions complete, state_count should return to baseline.
///
/// This is the PRIMARY invariant for memory leak detection.
/// A leak is defined as state accumulation that persists after work completes.
///
/// # Formal Statement
///
/// For any workload with baseline snapshot B and final snapshot F:
/// - If all jobs have completed (no active work remaining)
/// - And cleanup grace period T has elapsed
/// - Then |F.state_count - B.state_count| <= threshold
///
/// # Implications
///
/// - States must be explicitly cleaned up after execution completes
/// - Transient states during execution are acceptable
/// - Persistent accumulation is a bug (not a feature)
#[derive(Debug, Clone)]
pub struct StateCleanupInvariant;

impl StateCleanupInvariant {
    /// Checks if the invariant holds between baseline and current snapshots.
    pub fn check(
        baseline: &MemorySnapshot,
        current: &MemorySnapshot,
        threshold: &LeakThreshold,
    ) -> bool {
        let states_added = current.state_count.saturating_sub(baseline.state_count);
        states_added <= threshold.max_state_growth
    }
}

/// Invariant: Error map should not grow unbounded.
///
/// Even under failure conditions, the error map should be bounded.
/// This prevents memory exhaustion from repeated failures.
///
/// # Formal Statement
///
/// For any sequence of N operations with errors:
/// - Final error_count <= baseline.error_count + max_error_growth
/// OR
/// - Old errors have been explicitly cleaned up
#[derive(Debug, Clone)]
pub struct ErrorBoundedInvariant;

impl ErrorBoundedInvariant {
    /// Checks if error growth is bounded.
    pub fn check(
        baseline: &MemorySnapshot,
        current: &MemorySnapshot,
        threshold: &LeakThreshold,
    ) -> bool {
        let errors_added = current.error_count.saturating_sub(baseline.error_count);
        errors_added <= threshold.max_error_growth
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(state: usize, error: usize) -> MemorySnapshot {
        MemorySnapshot {
            state_count: state,
            error_count: error,
            captured_at: Instant::now(),
        }
    }

    #[test]
    fn no_leak_when_within_threshold() {
        let baseline = snapshot(10, 5);
        let current = snapshot(50, 20);
        let threshold = LeakThreshold::default();

        let indicator = LeakIndicator::detect(&baseline, &current, &threshold);
        assert!(matches!(indicator, LeakIndicator::NoLeak));
    }

    #[test]
    fn leak_detected_when_state_growth_exceeds_threshold() {
        let baseline = snapshot(10, 5);
        let current = snapshot(200, 20); // states_added = 190 > max_state_growth (100)
        let threshold = LeakThreshold::default();

        let indicator = LeakIndicator::detect(&baseline, &current, &threshold);
        assert!(matches!(
            indicator,
            LeakIndicator::LeakDetected {
                states_added: 190,
                ..
            }
        ));
    }

    #[test]
    fn leak_detected_when_error_growth_exceeds_threshold() {
        let baseline = snapshot(10, 5);
        let current = snapshot(50, 100); // errors_added = 95 > max_error_growth (50)
        let threshold = LeakThreshold::default();

        let indicator = LeakIndicator::detect(&baseline, &current, &threshold);
        assert!(matches!(
            indicator,
            LeakIndicator::LeakDetected {
                errors_added: 95,
                ..
            }
        ));
    }

    #[test]
    fn state_cleanup_invariant_holds_when_within_bounds() {
        let baseline = snapshot(10, 5);
        let current = snapshot(100, 20);
        let threshold = LeakThreshold::default();

        assert!(StateCleanupInvariant::check(
            &baseline, &current, &threshold
        ));
    }

    #[test]
    fn state_cleanup_invariant_fails_when_exceeds_bounds() {
        let baseline = snapshot(10, 5);
        let current = snapshot(200, 20); // 190 > 100 threshold
        let threshold = LeakThreshold::default();

        assert!(!StateCleanupInvariant::check(
            &baseline, &current, &threshold
        ));
    }

    #[test]
    fn error_bounded_invariant_holds_when_within_bounds() {
        let baseline = snapshot(10, 5);
        let current = snapshot(50, 50);
        let threshold = LeakThreshold::default();

        assert!(ErrorBoundedInvariant::check(
            &baseline, &current, &threshold
        ));
    }

    #[test]
    fn error_bounded_invariant_fails_when_exceeds_bounds() {
        let baseline = snapshot(10, 5);
        let current = snapshot(50, 100);
        let threshold = LeakThreshold::default();

        assert!(!ErrorBoundedInvariant::check(
            &baseline, &current, &threshold
        ));
    }

    #[test]
    fn leak_indicator_is_leak_for_leak_detected() {
        let baseline = snapshot(10, 5);
        let current = snapshot(200, 20);
        let threshold = LeakThreshold::default();

        let indicator = LeakIndicator::detect(&baseline, &current, &threshold);
        assert!(indicator.is_leak());
    }

    #[test]
    fn leak_indicator_is_not_leak_for_no_leak() {
        let baseline = snapshot(10, 5);
        let current = snapshot(50, 20);
        let threshold = LeakThreshold::default();

        let indicator = LeakIndicator::detect(&baseline, &current, &threshold);
        assert!(!indicator.is_leak());
    }

    #[test]
    fn memory_snapshot_total_entries() {
        let snap = snapshot(100, 50);
        assert_eq!(snap.total_entries(), 150);
    }

    #[test]
    fn memory_leak_detector_baseline_flow() {
        let mut detector = MemoryLeakDetector::new(LeakThreshold::default());
        assert!(detector.baseline().is_none());

        let baseline = snapshot(10, 5);
        detector.set_baseline(baseline.clone());
        assert_eq!(detector.baseline(), Some(&baseline));

        let current = snapshot(50, 20);
        let result = detector.check_leak(&current);
        assert!(result.is_some());
        assert!(!result.unwrap().is_leak());

        detector.reset();
        assert!(detector.baseline().is_none());
    }

    #[test]
    fn leak_threshold_variants() {
        let default = LeakThreshold::default();
        let strict = LeakThreshold::strict();
        let permissive = LeakThreshold::permissive();

        assert!(strict.max_state_growth < default.max_state_growth);
        assert!(permissive.max_state_growth > default.max_state_growth);
        assert!(strict.stale_state_ratio_threshold < default.stale_state_ratio_threshold);
        assert!(permissive.stale_state_ratio_threshold > default.stale_state_ratio_threshold);
    }
}
