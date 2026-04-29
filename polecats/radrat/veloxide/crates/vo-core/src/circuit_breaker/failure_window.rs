//! Sliding failure window for tracking unique-hash failures per workflow.

use std::time::{Duration, Instant};

use vo_types::BinaryHash;

use crate::circuit_breaker::FailureRecord;

/// Sliding window of failure records for a single workflow.
/// Invariant: entries are ordered by `failed_at` ascending.
/// Invariant: no two entries share the same `BinaryHash`.
#[derive(Debug, Clone)]
pub struct FailureWindow {
    /// Unique-hash failure records within the window.
    records: Vec<FailureRecord>,
}

impl Default for FailureWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl FailureWindow {
    /// Create a new empty failure window.
    #[must_use]
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Returns the number of records in the window.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns true if the window has no records.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Returns a reference to the records slice.
    #[must_use]
    pub fn records(&self) -> &[FailureRecord] {
        &self.records
    }
}

/// Add or update a failure record in the sliding window.
///
/// # Invariants enforced
/// - INV-004: duplicate hashes update timestamp, not count
/// - INV-007: expired entries evicted before insertion
///
/// # Returns
/// Number of unique hashes in the window after insertion.
pub fn record_failure_in_window(
    window: &mut FailureWindow,
    hash: BinaryHash,
    failed_at: Instant,
    window_duration: Duration,
) -> usize {
    // INV-007: evict expired entries first
    window.records.retain(|r| {
        let elapsed = failed_at.duration_since(r.failed_at);
        elapsed <= window_duration
    });

    // INV-004: check for duplicate hash
    if let Some(existing) = window.records.iter_mut().find(|r| r.hash == hash) {
        // Duplicate: update timestamp only, no count change
        existing.failed_at = failed_at;
        // Re-sort to maintain ordering invariant
        window.records.sort_by_key(|r| r.failed_at);
    } else {
        // Novel hash: insert in sorted position
        let record = FailureRecord { hash, failed_at };
        let pos = window.records.partition_point(|r| r.failed_at <= failed_at);
        window.records.insert(pos, record);
    }

    window.records.len()
}

/// Count unique hashes in the failure window, evicting expired entries.
///
/// Pure read with side-effect of eviction.
pub fn unique_failures_in_window(
    window: &mut FailureWindow,
    now: Instant,
    window_duration: Duration,
) -> usize {
    // INV-007: evict expired entries
    window.records.retain(|r| {
        let elapsed = now.duration_since(r.failed_at);
        elapsed <= window_duration
    });

    window.records.len()
}

#[cfg(test)]
#[path = "failure_window_tests.rs"]
mod tests;
