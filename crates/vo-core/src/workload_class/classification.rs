//! Classification layer — workload class taxonomy per ADR-033.
//!
//! Defines the four workload classes that govern resume scheduling and
//! execution permit allocation:
//! - `ExactCritical` — never starved, highest dispatch priority
//! - `Standard` — normal workflow execution
//! - `UnsafeBulk` — lower priority, capped under contention
//! - `Recovery` — reserved capacity for crash recovery

use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorkloadClassError {
    #[error("unknown workload class: {0}")]
    UnknownClass(String),

    #[error("budget exceeded for {class:?}: requested {requested}, available {available}")]
    BudgetExceeded {
        class: WorkloadClass,
        requested: u32,
        available: u32,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadClass {
    ExactCritical,
    #[default]
    Standard,
    Recovery,
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
    #[must_use]
    pub fn rank(self) -> u8 {
        match self {
            WorkloadClass::ExactCritical => 0,
            WorkloadClass::Standard => 1,
            WorkloadClass::Recovery => 2,
            WorkloadClass::UnsafeBulk => 3,
        }
    }

    #[must_use]
    pub fn never_starved(self) -> bool {
        matches!(self, WorkloadClass::ExactCritical | WorkloadClass::Recovery)
    }

    #[must_use]
    pub fn is_capped_under_contention(self) -> bool {
        matches!(self, WorkloadClass::UnsafeBulk)
    }

    pub fn parse(s: &str) -> Result<WorkloadClass, WorkloadClassError> {
        match s {
            "exact_critical" => Ok(WorkloadClass::ExactCritical),
            "standard" => Ok(WorkloadClass::Standard),
            "unsafe_bulk" => Ok(WorkloadClass::UnsafeBulk),
            "recovery" => Ok(WorkloadClass::Recovery),
            _ => Err(WorkloadClassError::UnknownClass(s.to_string())),
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            WorkloadClass::ExactCritical => "exact_critical",
            WorkloadClass::Standard => "standard",
            WorkloadClass::UnsafeBulk => "unsafe_bulk",
            WorkloadClass::Recovery => "recovery",
        }
    }

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
