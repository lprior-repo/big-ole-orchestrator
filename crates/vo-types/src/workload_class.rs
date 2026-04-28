//! Unified WorkloadClass taxonomy for the Veloxide engine.
//!
//! This is the single canonical definition that merges the classification
//! systems from ADR-033 (dispatch priority) and ADR-013 (budget admission),
//! plus actor fairness scheduling, into one enum.
//!
//! # Dispatch Priority (ADR-033)
//!
//! Classes are ordered by dispatch priority: lower rank = higher priority.
//! - `ExactCritical` — rank 0, never starved
//! - `Live` — rank 1, never rejected in degraded mode
//! - `Standard` — rank 2, default for normal workflow execution
//! - `Recovery` — rank 3, reserved capacity for crash recovery
//! - `TimerResume` — rank 4, timer-based resume
//! - `NewInstance` — rank 5, new instance spawning
//! - `Internal` — rank 6, internal housekeeping
//! - `NonCritical` — rank 7, first rejected when degraded
//! - `Background` — rank 8, deferred in degraded mode (blob writes, projections)
//! - `UnsafeBulk` — rank 9, capped under contention

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Workload classification for admission control, dispatch priority, and
/// fairness scheduling.
///
/// Variants are ordered by priority (highest first) for admission decisions.
/// The `rank()` method returns the dispatch priority rank (lower = higher priority).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadClass {
    /// Highest priority. Never starved by lower classes.
    ExactCritical,
    /// Live work. Never rejected in any degraded mode.
    Live,
    /// Default priority for normal workflow execution.
    #[default]
    Standard,
    /// Reserved capacity for crash recovery.
    Recovery,
    /// Timer-based resume, shares budget with Recovery.
    TimerResume,
    /// New instance spawning.
    NewInstance,
    /// Internal housekeeping tasks.
    Internal,
    /// First to be rejected when degraded.
    NonCritical,
    /// Background tasks (blob writes, projections), deferred in degraded mode.
    Background,
    /// Lower priority bulk operations. Capped under contention.
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
            WorkloadClass::Live => 1,
            WorkloadClass::Standard => 2,
            WorkloadClass::Recovery => 3,
            WorkloadClass::TimerResume => 4,
            WorkloadClass::NewInstance => 5,
            WorkloadClass::Internal => 6,
            WorkloadClass::NonCritical => 7,
            WorkloadClass::Background => 8,
            WorkloadClass::UnsafeBulk => 9,
        }
    }

    /// Returns `true` if this class is never starved (always gets budget).
    #[must_use]
    pub fn never_starved(self) -> bool {
        matches!(
            self,
            WorkloadClass::ExactCritical
                | WorkloadClass::Live
                | WorkloadClass::Recovery
        )
    }

    /// Returns `true` if this class is deferred in degraded mode.
    #[must_use]
    pub fn is_deferred_in_degraded(self) -> bool {
        matches!(
            self,
            WorkloadClass::NonCritical | WorkloadClass::Background
        )
    }

    /// Returns `true` if this class is accepted in Critical degraded mode.
    ///
    /// Only `ExactCritical`, `Live`, and `Recovery` are accepted when system
    /// is critical.
    #[must_use]
    pub fn is_accepted_in_critical(self) -> bool {
        matches!(
            self,
            WorkloadClass::ExactCritical
                | WorkloadClass::Live
                | WorkloadClass::Recovery
        )
    }

    /// Returns `true` if this class is subject to contention caps.
    #[must_use]
    pub fn is_capped_under_contention(self) -> bool {
        matches!(self, WorkloadClass::UnsafeBulk)
    }

    /// Returns `true` if this class is non-critical (subject to degradation).
    ///
    /// Non-critical classes are `Standard`, `NonCritical`, `Background`,
    /// `UnsafeBulk`, `NewInstance`, `Internal`, and `TimerResume`.
    #[must_use]
    pub fn is_non_critical(self) -> bool {
        matches!(
            self,
            WorkloadClass::Standard
                | WorkloadClass::UnsafeBulk
                | WorkloadClass::NonCritical
                | WorkloadClass::Background
                | WorkloadClass::NewInstance
                | WorkloadClass::Internal
                | WorkloadClass::TimerResume
        )
    }

    /// Returns `true` if this class is protected and always admitted during
    /// degraded mode.
    #[must_use]
    pub fn is_protected(self) -> bool {
        !self.is_non_critical()
    }

    /// Parses a string into a `WorkloadClass`.
    pub fn parse(s: &str) -> Result<WorkloadClass, WorkloadClassParseError> {
        match s.to_ascii_lowercase().as_str() {
            "exact_critical" => Ok(WorkloadClass::ExactCritical),
            "live" => Ok(WorkloadClass::Live),
            "standard" => Ok(WorkloadClass::Standard),
            "recovery" => Ok(WorkloadClass::Recovery),
            "timer_resume" => Ok(WorkloadClass::TimerResume),
            "new_instance" | "newinstance" => Ok(WorkloadClass::NewInstance),
            "internal" => Ok(WorkloadClass::Internal),
            "non_critical" | "noncritical" => Ok(WorkloadClass::NonCritical),
            "background" => Ok(WorkloadClass::Background),
            "unsafe_bulk" => Ok(WorkloadClass::UnsafeBulk),
            _ => Err(WorkloadClassParseError::Unknown {
                input: s.to_string(),
            }),
        }
    }

    /// Returns the canonical snake_case name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            WorkloadClass::ExactCritical => "exact_critical",
            WorkloadClass::Live => "live",
            WorkloadClass::Standard => "standard",
            WorkloadClass::Recovery => "recovery",
            WorkloadClass::TimerResume => "timer_resume",
            WorkloadClass::NewInstance => "new_instance",
            WorkloadClass::Internal => "internal",
            WorkloadClass::NonCritical => "non_critical",
            WorkloadClass::Background => "background",
            WorkloadClass::UnsafeBulk => "unsafe_bulk",
        }
    }

    /// Returns all workload class variants ordered by priority (highest first).
    #[must_use]
    pub fn all_by_priority() -> &'static [WorkloadClass] {
        &[
            WorkloadClass::ExactCritical,
            WorkloadClass::Live,
            WorkloadClass::Standard,
            WorkloadClass::Recovery,
            WorkloadClass::TimerResume,
            WorkloadClass::NewInstance,
            WorkloadClass::Internal,
            WorkloadClass::NonCritical,
            WorkloadClass::Background,
            WorkloadClass::UnsafeBulk,
        ]
    }
}

/// All workload class variants.
pub const ALL_WORKLOAD_CLASSES: [WorkloadClass; 10] = [
    WorkloadClass::ExactCritical,
    WorkloadClass::Live,
    WorkloadClass::Standard,
    WorkloadClass::Recovery,
    WorkloadClass::TimerResume,
    WorkloadClass::NewInstance,
    WorkloadClass::Internal,
    WorkloadClass::NonCritical,
    WorkloadClass::Background,
    WorkloadClass::UnsafeBulk,
];

/// Error type for WorkloadClass parsing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorkloadClassParseError {
    /// Returned when an unknown workload class string is parsed.
    #[error(
        "unknown workload class: \"{input}\". Valid classes: exact_critical, live, standard, recovery, timer_resume, new_instance, internal, non_critical, background, unsafe_bulk"
    )]
    Unknown { input: String },
}

impl FromStr for WorkloadClass {
    type Err = WorkloadClassParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        WorkloadClass::parse(s)
    }
}

impl fmt::Display for WorkloadClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_exact_critical() {
        assert_eq!(
            WorkloadClass::parse("exact_critical"),
            Ok(WorkloadClass::ExactCritical)
        );
    }

    #[test]
    fn parse_live() {
        assert_eq!(WorkloadClass::parse("live"), Ok(WorkloadClass::Live));
    }

    #[test]
    fn parse_standard() {
        assert_eq!(
            WorkloadClass::parse("standard"),
            Ok(WorkloadClass::Standard)
        );
    }

    #[test]
    fn parse_recovery() {
        assert_eq!("recovery".parse(), Ok(WorkloadClass::Recovery));
    }

    #[test]
    fn parse_timer_resume() {
        assert_eq!(
            WorkloadClass::parse("timer_resume"),
            Ok(WorkloadClass::TimerResume)
        );
    }

    #[test]
    fn parse_new_instance() {
        assert_eq!("new_instance".parse(), Ok(WorkloadClass::NewInstance));
    }

    #[test]
    fn parse_internal() {
        assert_eq!("internal".parse(), Ok(WorkloadClass::Internal));
    }

    #[test]
    fn parse_non_critical() {
        assert_eq!(
            WorkloadClass::parse("non_critical"),
            Ok(WorkloadClass::NonCritical)
        );
    }

    #[test]
    fn parse_background() {
        assert_eq!(
            WorkloadClass::parse("background"),
            Ok(WorkloadClass::Background)
        );
    }

    #[test]
    fn parse_unsafe_bulk() {
        assert_eq!(
            WorkloadClass::parse("unsafe_bulk"),
            Ok(WorkloadClass::UnsafeBulk)
        );
    }

    #[test]
    fn parse_case_insensitive() {
        assert_eq!("Recovery".parse(), Ok(WorkloadClass::Recovery));
        assert_eq!("RECOVERY".parse(), Ok(WorkloadClass::Recovery));
        assert_eq!("New_Instance".parse(), Ok(WorkloadClass::NewInstance));
        assert_eq!("INTERNAL".parse(), Ok(WorkloadClass::Internal));
    }

    #[test]
    fn parse_newinstance_without_underscore() {
        assert_eq!("newinstance".parse(), Ok(WorkloadClass::NewInstance));
    }

    #[test]
    fn parse_rejects_unknown() {
        let result: Result<WorkloadClass, WorkloadClassParseError> = "foobar".parse();
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(
            matches!(err, WorkloadClassParseError::Unknown { ref input } if input == "foobar")
        );
        assert!(err.to_string().contains("foobar"));
        assert!(err.to_string().contains("recovery"));
    }

    #[test]
    fn parse_rejects_empty() {
        let result: Result<WorkloadClass, WorkloadClassParseError> = "".parse();
        assert!(result.is_err());
    }

    #[test]
    fn default_is_standard() {
        assert_eq!(WorkloadClass::default(), WorkloadClass::Standard);
    }

    #[test]
    fn display_format() {
        assert_eq!(WorkloadClass::ExactCritical.to_string(), "exact_critical");
        assert_eq!(WorkloadClass::Live.to_string(), "live");
        assert_eq!(WorkloadClass::Standard.to_string(), "standard");
        assert_eq!(WorkloadClass::Recovery.to_string(), "recovery");
        assert_eq!(WorkloadClass::TimerResume.to_string(), "timer_resume");
        assert_eq!(WorkloadClass::NewInstance.to_string(), "new_instance");
        assert_eq!(WorkloadClass::Internal.to_string(), "internal");
        assert_eq!(WorkloadClass::NonCritical.to_string(), "non_critical");
        assert_eq!(WorkloadClass::Background.to_string(), "background");
        assert_eq!(WorkloadClass::UnsafeBulk.to_string(), "unsafe_bulk");
    }

    #[test]
    fn roundtrip_display_from_str() {
        for class in ALL_WORKLOAD_CLASSES {
            let s = class.to_string();
            let parsed: WorkloadClass = s.parse().unwrap();
            assert_eq!(class, parsed);
        }
    }

    #[test]
    fn as_str_roundtrips() {
        for class in ALL_WORKLOAD_CLASSES {
            assert_eq!(WorkloadClass::parse(class.as_str()), Ok(class));
        }
    }

    #[test]
    fn all_classes_contains_all_variants() {
        assert_eq!(ALL_WORKLOAD_CLASSES.len(), 10);
    }

    #[test]
    fn every_workload_resolves_to_exactly_one_class() {
        let classes = [
            "exact_critical",
            "live",
            "standard",
            "recovery",
            "timer_resume",
            "new_instance",
            "internal",
            "non_critical",
            "background",
            "unsafe_bulk",
        ];
        for input in classes {
            let class: WorkloadClass = input.parse().unwrap();
            let count = ALL_WORKLOAD_CLASSES.iter().filter(|&&c| c == class).count();
            assert_eq!(
                count, 1,
                "class from '{}' mapped to exactly one variant",
                input
            );
        }
    }

    #[test]
    fn ordering_by_rank() {
        let variants = WorkloadClass::all_by_priority();
        for window in variants.windows(2) {
            assert!(
                window[0].rank() < window[1].rank(),
                "{:?} should have lower rank than {:?}",
                window[0],
                window[1]
            );
        }
    }

    #[test]
    fn never_starved_classes() {
        assert!(WorkloadClass::ExactCritical.never_starved());
        assert!(WorkloadClass::Live.never_starved());
        assert!(WorkloadClass::Recovery.never_starved());
        assert!(!WorkloadClass::Standard.never_starved());
        assert!(!WorkloadClass::TimerResume.never_starved());
        assert!(!WorkloadClass::NonCritical.never_starved());
        assert!(!WorkloadClass::Background.never_starved());
        assert!(!WorkloadClass::UnsafeBulk.never_starved());
    }

    #[test]
    fn is_deferred_in_degraded() {
        assert!(WorkloadClass::NonCritical.is_deferred_in_degraded());
        assert!(WorkloadClass::Background.is_deferred_in_degraded());
        assert!(!WorkloadClass::Live.is_deferred_in_degraded());
        assert!(!WorkloadClass::Recovery.is_deferred_in_degraded());
        assert!(!WorkloadClass::ExactCritical.is_deferred_in_degraded());
    }

    #[test]
    fn is_accepted_in_critical() {
        assert!(WorkloadClass::Live.is_accepted_in_critical());
        assert!(WorkloadClass::Recovery.is_accepted_in_critical());
        assert!(WorkloadClass::ExactCritical.is_accepted_in_critical());
        assert!(!WorkloadClass::TimerResume.is_accepted_in_critical());
        assert!(!WorkloadClass::NonCritical.is_accepted_in_critical());
        assert!(!WorkloadClass::Background.is_accepted_in_critical());
    }
}
