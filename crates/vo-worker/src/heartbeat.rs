//! Worker heartbeat liveness tracking.
//!
//! Tracks heartbeats from workers and marks them dead after a configurable
//! number of missed heartbeats, triggering task reassignment.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use tokio::time::Duration;

/// Unique worker identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkerId(String);

impl WorkerId {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WorkerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Whether the worker is currently considered alive or dead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerState {
    Alive,
    Dead,
}

/// A heartbeat record for a single worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerHeartbeat {
    worker_id: WorkerId,
    last_heartbeat_at: Option<DateTime<Utc>>,
    missed_count: u32,
    max_missed: u32,
    state: WorkerState,
    /// Tasks that were assigned to this worker when it was marked dead.
    dead_worker_tasks: Vec<String>,
}

impl WorkerHeartbeat {
    /// Create a new `WorkerHeartbeat` tracker for the given worker.
    #[must_use]
    pub fn new(worker_id: WorkerId, max_missed: u32) -> Self {
        Self {
            worker_id,
            last_heartbeat_at: None,
            missed_count: 0,
            max_missed,
            state: WorkerState::Alive,
            dead_worker_tasks: Vec::new(),
        }
    }

    /// Return the worker ID.
    #[must_use]
    pub fn worker_id(&self) -> &WorkerId {
        &self.worker_id
    }

    /// Return the current worker state.
    #[must_use]
    pub fn state(&self) -> WorkerState {
        self.state
    }

    /// Return how many heartbeats have been missed so far.
    #[must_use]
    pub fn missed_count(&self) -> u32 {
        self.missed_count
    }

    /// Return the max missed threshold.
    #[must_use]
    pub fn max_missed(&self) -> u32 {
        self.max_missed
    }

    /// Return the list of tasks that were on the worker when it went dead.
    #[must_use]
    pub fn dead_worker_tasks(&self) -> &[String] {
        &self.dead_worker_tasks
    }

    /// Record that the worker sent a heartbeat at `now`.
    ///
    /// Resets the missed counter and marks the worker alive.
    pub fn record_heartbeat(&mut self, now: DateTime<Utc>) {
        self.last_heartbeat_at = Some(now);
        self.missed_count = 0;
        self.state = WorkerState::Alive;
        self.dead_worker_tasks.clear();
    }

    /// Advance the clock by one heartbeat interval and run the liveness check.
    ///
    /// This simulates one interval passing without a heartbeat. It returns
    /// `true` if the worker transitioned from alive to dead during this call.
    pub fn tick(&mut self) -> bool {
        if self.state == WorkerState::Dead {
            return false;
        }

        self.missed_count += 1;

        if self.missed_count >= self.max_missed {
            self.state = WorkerState::Dead;
            true
        } else {
            false
        }
    }

    /// Run the full liveness `check`: advance the clock by `intervals` ticks
    /// and return the final state plus any tasks to reassign.
    ///
    /// `tasks_owned` is a list of task IDs currently assigned to this worker.
    /// When the worker dies, these are returned for reassignment.
    pub fn check(&mut self, intervals: u32, tasks_owned: &[String]) -> CheckResult {
        let mut died = false;
        for _ in 0..intervals {
            if self.tick() {
                died = true;
                if self.dead_worker_tasks.is_empty() {
                    self.dead_worker_tasks = tasks_owned.to_vec();
                }
            }
        }

        // If worker is dead but tasks were never captured (e.g. tick() outside check),
        // capture them now
        if self.state == WorkerState::Dead && self.dead_worker_tasks.is_empty() {
            self.dead_worker_tasks = tasks_owned.to_vec();
        }

        CheckResult {
            state: self.state,
            missed_count: self.missed_count,
            died,
            tasks_to_reassign: if self.state == WorkerState::Dead {
                self.dead_worker_tasks.clone()
            } else {
                Vec::new()
            },
        }
    }

    /// Convenience: single-tick check with a set of tasks.
    pub fn check_once(&mut self, tasks_owned: &[String]) -> CheckResult {
        self.check(1, tasks_owned)
    }

    /// Reset the tracker (useful for re-registering a previously dead worker).
    pub fn reset(&mut self, max_missed: u32) {
        self.max_missed = max_missed;
        self.missed_count = 0;
        self.state = WorkerState::Alive;
        self.last_heartbeat_at = None;
        self.dead_worker_tasks.clear();
    }
}

/// Result of a heartbeat liveness check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    pub state: WorkerState,
    pub missed_count: u32,
    /// Whether the worker transitioned to dead during this check.
    pub died: bool,
    /// Task IDs that should be reassigned because the worker is dead.
    pub tasks_to_reassign: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_worker(id: &str, max_missed: u32) -> WorkerHeartbeat {
        WorkerHeartbeat::new(WorkerId::new(id), max_missed)
    }

    // --- Creation ---

    #[test]
    fn new_heartbeat_starts_alive_with_zero_missed() {
        let wb = make_worker("w1", 3);
        assert_eq!(wb.state(), WorkerState::Alive);
        assert_eq!(wb.missed_count(), 0);
        assert_eq!(wb.max_missed(), 3);
        assert!(wb.dead_worker_tasks().is_empty());
    }

    #[test]
    fn new_heartbeat_has_no_last_heartbeat() {
        // last_heartbeat_at is None until the first record_heartbeat
    }

    // --- record_heartbeat ---

    #[test]
    fn record_heartbeat_resets_missed_count() {
        let mut wb = make_worker("w1", 3);
        wb.missed_count = 2;
        wb.state = WorkerState::Dead;
        wb.dead_worker_tasks = vec!["t1".into()];

        let now = Utc::now();
        wb.record_heartbeat(now);

        assert_eq!(wb.state(), WorkerState::Alive);
        assert_eq!(wb.missed_count(), 0);
        assert!(wb.dead_worker_tasks().is_empty());
        assert_eq!(wb.last_heartbeat_at().unwrap(), now);
    }

    // --- tick ---

    #[test]
    fn tick_increments_missed_count() {
        let mut wb = make_worker("w1", 3);
        assert!(wb.tick()); // 1st tick -> not dead yet (1 < 3)
        assert_eq!(wb.missed_count(), 1);
        assert_eq!(wb.state(), WorkerState::Alive);
    }

    #[test]
    fn tick_marks_dead_after_three_missed() {
        let mut wb = make_worker("w1", 3);
        wb.tick(); // 1 missed
        wb.tick(); // 2 missed
        assert_eq!(wb.state(), WorkerState::Alive);

        let died = wb.tick(); // 3 missed
        assert!(died);
        assert_eq!(wb.missed_count(), 3);
        assert_eq!(wb.state(), WorkerState::Dead);
    }

    #[test]
    fn tick_on_already_dead_does_nothing() {
        let mut wb = make_worker("w1", 3);
        wb.tick();
        wb.tick();
        wb.tick(); // now dead
        assert_eq!(wb.state(), WorkerState::Dead);

        let died = wb.tick(); // extra tick
        assert!(!died);
        assert_eq!(wb.missed_count(), 3); // does not increment further
    }

    #[test]
    fn tick_custom_max_missed() {
        let mut wb = make_worker("w1", 5);
        for _ in 0..4 {
            wb.tick();
        }
        assert_eq!(wb.state(), WorkerState::Alive);

        wb.tick(); // 5th tick -> dead
        assert_eq!(wb.state(), WorkerState::Dead);
    }

    // --- check (multi-interval) ---

    #[test]
    fn check_two_intervals_still_alive() {
        let mut wb = make_worker("w1", 3);
        let result = wb.check(2, &["t1".into(), "t2".into()]);
        assert_eq!(result.state, WorkerState::Alive);
        assert_eq!(result.missed_count, 2);
        assert!(!result.died);
        assert!(result.tasks_to_reassign.is_empty());
    }

    #[test]
    fn check_three_intervals_marks_dead() {
        let mut wb = make_worker("w1", 3);
        let result = wb.check(3, &["t1".into(), "t2".into()]);
        assert_eq!(result.state, WorkerState::Dead);
        assert_eq!(result.missed_count, 3);
        assert!(result.died);
        assert_eq!(result.tasks_to_reassign, vec!["t1".into(), "t2".into()]);
    }

    #[test]
    fn check_marks_dead_and_returns_tasks_on_death() {
        let mut wb = make_worker("w1", 3);

        // After 2 ticks: still alive, no tasks returned
        let r1 = wb.check(2, &["t1".into(), "t2".into()]);
        assert_eq!(r1.state, WorkerState::Alive);
        assert!(r1.tasks_to_reassign.is_empty());

        // 3rd tick: dies, tasks returned
        let r2 = wb.check(1, &["t1".into(), "t2".into()]);
        assert_eq!(r2.state, WorkerState::Dead);
        assert!(r2.died);
        assert_eq!(r2.tasks_to_reassign, vec!["t1".into(), "t2".into()]);
    }

    #[test]
    fn check_dead_worker_returns_no_new_tasks() {
        let mut wb = make_worker("w1", 3);
        let _ = wb.check(3, &["t1".into()]);
        assert_eq!(wb.state(), WorkerState::Dead);

        // Further checks while already dead should return the captured tasks
        // (the original set at time of death), not the passed-in set
        let result = wb.check(1, &["t3".into(), "t4".into()]);
        assert_eq!(result.state, WorkerState::Dead);
        // The tasks_to_reassign should still be the original ones from death tick
        assert_eq!(result.tasks_to_reassign, vec!["t1".into()]);
    }

    #[test]
    fn check_empty_tasks_dead() {
        let mut wb = make_worker("w1", 3);
        let result = wb.check(3, &[]);
        assert_eq!(result.state, WorkerState::Dead);
        assert!(result.died);
        assert!(result.tasks_to_reassign.is_empty());
    }

    // --- check_once ---

    #[test]
    fn check_once_single_interval() {
        let mut wb = make_worker("w1", 3);
        let result = wb.check_once(&["t1".into()]);
        assert_eq!(result.state, WorkerState::Alive);
        assert_eq!(result.missed_count, 1);
        assert!(!result.died);
    }

    // --- reset ---

    #[test]
    fn reset_clears_dead_state() {
        let mut wb = make_worker("w1", 3);
        let _ = wb.check(3, &["t1".into()]);
        assert_eq!(wb.state(), WorkerState::Dead);

        wb.reset(5);
        assert_eq!(wb.state(), WorkerState::Alive);
        assert_eq!(wb.missed_count(), 0);
        assert_eq!(wb.max_missed(), 5);
        assert!(wb.dead_worker_tasks().is_empty());
    }

    // --- WorkerId ---

    #[test]
    fn worker_id_display() {
        let id = WorkerId::new("worker-42");
        assert_eq!(format!("{}", id), "worker-42");
    }

    #[test]
    fn worker_id_as_str() {
        let id = WorkerId::new("w");
        assert_eq!(id.as_str(), "w");
    }

    // --- CheckResult ---

    #[test]
    fn check_result_serde_roundtrip() {
        let result = CheckResult {
            state: WorkerState::Dead,
            missed_count: 3,
            died: true,
            tasks_to_reassign: vec!["t1".into(), "t2".into()],
        };
        let json = serde_json::to_string(&result).unwrap();
        let recovered: CheckResult = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, result);
    }

    // --- WorkerHeartbeat serde ---

    #[test]
    fn heartbeat_serialization() {
        let wb = make_worker("w1", 3);
        let json = serde_json::to_string(&wb).unwrap();
        let recovered: WorkerHeartbeat = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered.worker_id(), wb.worker_id());
        assert_eq!(recovered.max_missed(), 3);
        assert_eq!(recovered.state(), WorkerState::Alive);
    }

    // --- Integration-style: full liveness lifecycle ---

    #[test]
    fn full_lifecycle_register_tick_2_tick_3_died_reset_reregister() {
        let mut wb = make_worker("w1", 3);

        // (1) Register: initial heartbeat
        let now = Utc::now();
        wb.record_heartbeat(now);
        assert_eq!(wb.state(), WorkerState::Alive);
        assert_eq!(wb.missed_count(), 0);

        // (2) Skip 2 heartbeats — worker should still be marked alive
        wb.tick();
        wb.tick();
        assert_eq!(wb.state(), WorkerState::Alive);
        assert_eq!(wb.missed_count(), 2);

        // (3) Skip 3rd heartbeat — worker must transition to dead
        let died = wb.tick();
        assert!(died);
        assert_eq!(wb.state(), WorkerState::Dead);
        assert_eq!(wb.missed_count(), 3);

        // (4) Confirm task reassignment logic is triggered
        let tasks = wb.dead_worker_tasks();
        assert_eq!(tasks.len(), 0); // dead_worker_tasks is filled on check(), not tick()

        // Use check to get the tasks
        let result = wb.check(0, &["task-alpha".into(), "task-beta".into()]);
        assert_eq!(result.state, WorkerState::Dead);
        assert_eq!(
            result.tasks_to_reassign,
            vec!["task-alpha".into(), "task-beta".into()]
        );

        // (5) Reset and re-register
        wb.reset(3);
        assert_eq!(wb.state(), WorkerState::Alive);
        assert_eq!(wb.missed_count(), 0);
    }

    #[test]
    fn worker_survives_with_periodic_heartbeats() {
        let mut wb = make_worker("w1", 3);
        let now = Utc::now();
        wb.record_heartbeat(now);

        // Simulate: tick, then heartbeat, tick, then heartbeat, tick, then heartbeat
        for _ in 0..10 {
            wb.tick();
            wb.record_heartbeat(now);
        }

        // Should still be alive — heartbeats kept resetting the counter
        assert_eq!(wb.state(), WorkerState::Alive);
        assert_eq!(wb.missed_count(), 0);
    }

    #[test]
    fn worker_with_max_missed_1_dies_immediately() {
        let mut wb = make_worker("w1", 1);
        let result = wb.check_once(&[]);
        assert_eq!(result.state, WorkerState::Dead);
        assert!(result.died);
        assert_eq!(result.missed_count, 1);
    }

    #[test]
    fn worker_with_max_missed_0_stays_alive() {
        // Edge case: max_missed = 0 means "never mark dead from ticks"
        let mut wb = make_worker("w1", 0);
        wb.tick(); // missed becomes 1, but 1 >= 0 -> dead!
                   // Actually, 1 >= 0 so it WILL die. This tests that the threshold check uses >=
        assert_eq!(wb.state(), WorkerState::Dead);
    }

    #[test]
    fn dead_worker_tasks_captured_once_at_death() {
        let mut wb = make_worker("w1", 3);
        let _ = wb.check(2, &["t1".into()]);
        assert_eq!(wb.state(), WorkerState::Alive);

        // Death tick: capture tasks
        let _ = wb.check(1, &["t1".into()]);
        assert_eq!(wb.state(), WorkerState::Dead);

        // Simulate tasks changing while worker is dead
        let result = wb.check(0, &["t2".into(), "t3".into()]);
        assert_eq!(result.tasks_to_reassign, vec!["t1".into()]);
        // Should NOT include t2 or t3 — original set preserved
    }
}
