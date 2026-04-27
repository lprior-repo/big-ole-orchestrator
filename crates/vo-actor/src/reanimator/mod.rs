//! Reanimator Loop - Timer-based workflow resumption service.
//!
//! Per ADR-005, the Reanimator Loop is a single background tokio task that:
//! - Every 1 second, performs a range scan on timers partition from 0 to current_timestamp
//! - For every timer key found, atomically records TimerFired and deletes the wake-up key
//! - Enqueues resume work for instance_id under fairness budget rules

mod error;
mod loop_core;
pub mod mock;
#[cfg(test)]
mod mock_tests;
#[cfg(test)]
mod recovery_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_types;
#[cfg(test)]
mod timing_attack_tests;
#[cfg(test)]
mod integration_tests;
pub mod traits;
pub mod types;
#[cfg(kani)]
mod verification;

pub use error::{ReanimatorError, ReanimatorErrorClass};
pub use loop_core::{ReanimatorHandle, ReanimatorLoop};
pub use mock::{MockTimerStorage, MockWorkQueue};
pub use traits::{PendingTimer, TimerStorage, WorkQueue};
pub use types::{
    calculate_batch_size, calculate_scan_result, check_resume_budget, filter_timers_by_fairness,
    validate_timer_record, FairnessBudget, ReanimatorConfig, ReanimatorState, TimerRecord,
    TimerScanResult,
};