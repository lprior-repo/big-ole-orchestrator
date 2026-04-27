//! Sliding failure window for tracking unique-hash failures per workflow.
//!
//! This module implements the sliding time window used by Layer 2 of the
//! circuit breaker to track unique binary hash failures. When the number of
//! distinct hashes within the window reaches the failure threshold, the
//! workflow is automatically quarantined.
//!
//! # Algorithm
//!
//! ```text
//!  FailureWindow {
//!      records: Vec<FailureRecord>  // Ordered by failed_at ascending
//!  }
//!
//!  record_failure(window, hash, failed_at, window_duration)
//!    │
//!    ├─ Evict expired entries (INV-007):
//!    │   records.retain(|r| (failed_at - r.failed_at) ≤ window_duration)
//!    │
//!    ├─ Check for duplicate hash (INV-004):
//!    │   ├─ Hash exists: update timestamp, re-sort
//!    │   └─ Hash new: insert in sorted position
//!    │
//!    └─ Return unique count (records.len())
//!
//!  unique_failures_in_window(window, now, window_duration)
//!    │
//!    ├─ Evict expired entries
//!    └─ Return remaining count
//! ```
//!
//! # Invariants
//!
//! | ID | Description |
//! |----|-------------|
//! | INV-004 | Duplicate hashes update timestamp only, not count. |
//! | INV-007 | Expired entries are evicted before insertion and before counting. |
//!
//! The `records` vector is always maintained in ascending order by `failed_at`,
//! which enables efficient partition-point insertion.
//!
//! # Examples
//!
//! ```
//! use vo_core::circuit_breaker::{FailureWindow, failure_window::record_failure_in_window};
//! use std::time::{Duration, Instant};
//! use vo_types::BinaryHash;
//!
//! let mut window = FailureWindow::new();
//! let hash1 = BinaryHash::parse("aaa").unwrap();
//! let hash2 = BinaryHash::parse("bbb").unwrap();
//! let now = Instant::now();
//!
//! // Record two unique failures
//! assert_eq!(record_failure_in_window(&mut window, hash1.clone(), now, Duration::from_secs(600)), 1);
//! assert_eq!(record_failure_in_window(&mut window, hash2.clone(), now, Duration::from_secs(600)), 2);
//!
//! // Record hash1 again — count stays at 2 (INV-004)
//! assert_eq!(record_failure_in_window(&mut window, hash1, now + Duration::from_secs(10), Duration::from_secs(600)), 2);
//! ```

use std::time::{Duration, Instant};

use vo_types::BinaryHash;

use crate::circuit_breaker::FailureRecord;

/// Sliding window of failure records for a single workflow.
///
/// This struct maintains an ordered list of [`FailureRecord`] entries, each
/// representing a unique binary hash that failed for a workflow. The window
/// slides continuously: records older than the configured duration are evicted.
///
/// # Invariants
///
//! - INV-004: No two entries share the same `BinaryHash`. Duplicate hashes
//!   update the timestamp of the existing entry.
//! - INV-007: Entries are evicted when `now - failed_at > window_duration`.
//! - The `records` vector is always sorted by `failed_at` in ascending order.
//!
/// # Examples
///
//! ```
//! use vo_core::circuit_breaker::FailureWindow;
//!
//! let window = FailureWindow::new();
//! assert!(window.is_empty());
//! assert_eq!(window.len(), 0);
//! ```
#[derive(Debug, Clone)]
pub struct FailureWindow {
    /// Unique-hash failure records within the window.
    ///
    /// This vector is maintained in ascending order by `failed_at`.
    /// No two entries share the same `BinaryHash` (INV-004).
    records: Vec<FailureRecord>,
}

impl Default for FailureWindow {
    /// Returns an empty failure window.
    ///
    /// Equivalent to [`FailureWindow::new()`].
    fn default() -> Self {
        Self::new()
    }
}

impl FailureWindow {
    /// Create a new empty failure window.
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::circuit_breaker::FailureWindow;
    ///
    /// let window = FailureWindow::new();
    /// assert!(window.is_empty());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Returns the number of records in the window.
    ///
    /// This is the count of unique binary hashes currently within the sliding
    /// window. It is used by [`record_failure()`][crate::circuit_breaker::record_failure]
    /// to determine if the quarantine threshold has been reached.
    ///
    /// # Note
    ///
    /// This does not perform eviction. Expired entries are evicted during
    /// the next call to [`record_failure_in_window()`] or
    /// [`unique_failures_in_window()`].
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::circuit_breaker::FailureWindow;
    ///
    /// let mut window = FailureWindow::new();
    /// assert_eq!(window.len(), 0);
    /// window.records.push(vo_core::circuit_breaker::FailureRecord {
    ///     hash: vo_types::BinaryHash::parse("aaa").unwrap(),
    ///     failed_at: std::time::Instant::now(),
    /// });
    /// assert_eq!(window.len(), 1);
    /// ```
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns true if the window has no records.
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::circuit_breaker::FailureWindow;
    ///
    /// let window = FailureWindow::new();
    /// assert!(window.is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Returns a reference to the records slice.
    ///
    /// The records are sorted by `failed_at` in ascending order. This allows
    /// callers to inspect the failure history for debugging or metrics.
    ///
    /// # Note
    ///
    /// This does not perform eviction. Use [`record_failure_in_window()`] or
    /// [`unique_failures_in_window()`] for evicted views.
    #[must_use]
    pub fn records(&self) -> &[FailureRecord] {
        &self.records
    }
}

/// Add or update a failure record in the sliding window.
///
/// This is the primary function for recording failures. It performs three
/// operations in order:
///
/// 1. **Evict expired entries** (INV-007): Removes all records older than
///    `window_duration` from `failed_at`.
/// 2. **Handle duplicate hashes** (INV-004): If the hash already exists,
///    update its timestamp. Otherwise, insert a new record.
/// 3. **Count unique hashes**: Returns the total number of unique hashes
///    remaining in the window after insertion.
///
/// # Invariants Enforced
///
/// - **INV-004**: If the hash already exists in the window, only the timestamp
///   is updated. The unique count does not increase.
/// - **INV-007**: All expired entries are evicted before the duplicate check
///   and insertion, ensuring the count reflects only entries within the window.
///
/// # Arguments
///
/// * `window` — The mutable failure window to update.
/// * `hash` — The binary hash of the failed build.
/// * `failed_at` — The instant at which the failure was observed.
/// * `window_duration` — The sliding window duration. Records older than
///   this from `failed_at` are evicted.
///
/// # Returns
///
/// The number of unique hashes in the window after insertion (and eviction).
/// This is the count used by [`record_failure()`][crate::circuit_breaker::record_failure]
/// to determine if the quarantine threshold has been breached.
///
/// # Examples
///
//! ```
//! use vo_core::circuit_breaker::{FailureWindow, failure_window::record_failure_in_window};
//! use std::time::{Duration, Instant};
//! use vo_types::BinaryHash;
//!
//! let mut window = FailureWindow::new();
//! let hash1 = BinaryHash::parse("aaa").unwrap();
//! let hash2 = BinaryHash::parse("bbb").unwrap();
//! let now = Instant::now();
//!
//! // First unique hash → count = 1
//! assert_eq!(record_failure_in_window(&mut window, hash1.clone(), now, Duration::from_secs(600)), 1);
//!
//! // Second unique hash → count = 2
//! assert_eq!(record_failure_in_window(&mut window, hash2.clone(), now, Duration::from_secs(600)), 2);
//!
//! // Duplicate hash → count stays 2 (INV-004: timestamp updated, no count increase)
//! assert_eq!(record_failure_in_window(&mut window, hash1, now + Duration::from_secs(5), Duration::from_secs(600)), 2);
//! ```
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
/// This is a read-only operation (with side-effect of eviction) that returns
/// the count of unique hashes currently within the sliding window. It is used
/// by [`CircuitBreakerState::get_failure_count()`] for safe access.
///
/// # Side Effects
///
/// Expired entries (older than `window_duration` from `now`) are evicted
/// from the window (INV-007). This ensures the count is accurate at the
/// current time.
///
/// # Arguments
///
/// * `window` — The mutable failure window.
/// * `now` — The current instant, used for expiration.
/// * `window_duration` — The sliding window duration.
///
/// # Returns
///
/// The number of unique hashes remaining after eviction.
///
/// # Examples
///
//! ```
//! use vo_core::circuit_breaker::{FailureWindow, failure_window::unique_failures_in_window};
//! use std::time::{Duration, Instant};
//! use vo_types::BinaryHash;
//!
//! let mut window = FailureWindow::new();
//! let hash = BinaryHash::parse("aaa").unwrap();
//! let now = Instant::now();
//!
//! // Record a failure
//! vo_core::circuit_breaker::failure_window::record_failure_in_window(
//!     &mut window, hash.clone(), now, Duration::from_secs(600),
//! );
//! assert_eq!(unique_failures_in_window(&mut window, now, Duration::from_secs(600)), 1);
//!
//! // Expire the entry (go far into the future)
//! let far_future = now + Duration::from_secs(700);
//! assert_eq!(unique_failures_in_window(&mut window, far_future, Duration::from_secs(600)), 0);
//! ```
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
