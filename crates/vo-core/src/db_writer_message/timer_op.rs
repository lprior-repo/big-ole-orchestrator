//! Timer operation for atomic timer management.

use serde::{Deserialize, Serialize};
use vo_types::{FireAtMs, TimerId};

/// Timer operation for atomic timer management.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimerOp {
    /// Upsert (insert or update) a timer.
    Upsert {
        timer_id: TimerId,
        fire_at: FireAtMs,
    },
    /// Delete a timer.
    Delete { timer_id: TimerId },
}
