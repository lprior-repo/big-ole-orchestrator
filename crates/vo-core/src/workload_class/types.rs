//! Data layer — core types for workload classification per ADR-033.
//!
//! Contains the foundational enum and error types. No business logic
//! (see `budget.rs` for stateful permit tracking).

use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

// ─────────────────────────────────────────────────────────────────────────────
// Error Types
// ─────────────────────────────────────────────────────────────────────────────

/// Errors for workload class operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorkloadClassError {
    /// Returned when an unknown workload class string is parsed.
    #[error("unknown workload class: {0}")]
    UnknownClass(String),

    /// Returned when a workload budget constraint is violated.
    #[error("budget exceeded for {class:?}: requested {requested}, available {available}")]
    BudgetExceeded {
        class: WorkloadClass,
        requested: u32,
        available: u32,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// WorkloadClass Enum
// ─────────────────────────────────────────────────────────────────────────────

/// Workload classification per ADR-033 for resume fairness.
///
/// Determines scheduling priority, permit reservation, and load-shedding
/// behavior. Classes are ordered by dispatch priority: lower rank = higher priority.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadClass {
    /// Highest priority. Never starved by lower classes.
    ExactCritical,
    /// Default priority for normal workflow execution.
    #[default]
    Standard,
    /// Reserved capacity for crash recovery.
    Recovery,
    /// Lower priority. Capped under contention.
    UnsafeBulk,
}

impl PartialOrd for WorkloadClass {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for WorkloadClass {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.rank().cmp(&other.rank())
    }
}

impl WorkloadClass {
    /// Dispatch priority rank (lower = higher priority).
    #[must_use]
    pub fn rank(self) -> u8 {
        match self {
            WorkloadClass::ExactCritical => 0,
            WorkloadClass::Standard => 1,
            WorkloadClass::Recovery => 2,
            WorkloadClass::UnsafeBulk => 3,
        }
    }

    /// Returns `true` if this class is never starved by lower-priority work.
    #[must_use]
    pub fn never_starved(self) -> bool {
        matches!(self, WorkloadClass::ExactCritical | WorkloadClass::Recovery)
    }

    /// Returns `true` if this class is subject to contention caps.
    #[must_use]
    pub fn is_capped_under_contention(self) -> bool {
        matches!(self, WorkloadClass::UnsafeBulk)
    }

    /// Parses a string into a `WorkloadClass`.
    pub fn parse(s: &str) -> Result<WorkloadClass, WorkloadClassError> {
        match s {
            "exact_critical" => Ok(WorkloadClass::ExactCritical),
            "standard" => Ok(WorkloadClass::Standard),
            "unsafe_bulk" => Ok(WorkloadClass::UnsafeBulk),
            "recovery" => Ok(WorkloadClass::Recovery),
            _ => Err(WorkloadClassError::UnknownClass(s.to_string())),
        }
    }

    /// Returns the canonical snake_case name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            WorkloadClass::ExactCritical => "exact_critical",
            WorkloadClass::Standard => "standard",
            WorkloadClass::UnsafeBulk => "unsafe_bulk",
            WorkloadClass::Recovery => "recovery",
        }
    }

    /// Returns all workload class variants ordered by priority (highest first).
    #[must_use]
    pub fn all_by_priority() -> &'static [WorkloadClass] {
        &[
            WorkloadClass::ExactCritical,
            WorkloadClass::Standard,
            WorkloadClass::Recovery,
            WorkloadClass::UnsafeBulk,
        ]
    }
}

impl FromStr for WorkloadClass {
    type Err = WorkloadClassError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        WorkloadClass::parse(s)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RejectionDetail — Load-Shedding Transparency
// ─────────────────────────────────────────────────────────────────────────────

/// Describes why a resume or execution request was rejected (ADR-033 §3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectionDetail {
    /// The workload class of the rejected request.
    pub class: WorkloadClass,
    /// Human-readable reason for rejection.
    pub reason: RejectionReason,
}

impl RejectionDetail {
    #[must_use]
    pub fn budget_exhausted(class: WorkloadClass) -> Self {
        Self {
            class,
            reason: RejectionReason::BudgetExhausted,
        }
    }

    #[must_use]
    pub fn workflow_cap_exceeded(class: WorkloadClass) -> Self {
        Self {
            class,
            reason: RejectionReason::WorkflowCapExceeded,
        }
    }

    #[must_use]
    pub fn global_limit(class: WorkloadClass) -> Self {
        Self {
            class,
            reason: RejectionReason::GlobalConcurrencyLimit,
        }
    }
}

impl std::fmt::Display for RejectionDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "rejected {:?}: {}",
            self.class,
            match &self.reason {
                RejectionReason::BudgetExhausted => "class budget exhausted",
                RejectionReason::WorkflowCapExceeded => "per-workflow cap exceeded",
                RejectionReason::GlobalConcurrencyLimit => "global concurrency limit reached",
            }
        )
    }
}

/// The specific reason for a rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RejectionReason {
    BudgetExhausted,
    WorkflowCapExceeded,
    GlobalConcurrencyLimit,
}
