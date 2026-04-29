//! Re-export canonical WorkloadClass from vo_types.
//!
//! The unified `WorkloadClass` contains all variants across ADR-033 (dispatch
//! priority), ADR-013 (budget admission), and actor fairness scheduling.

pub use vo_types::workload_class::WorkloadClass;

/// The 4 dispatch-priority classes used by ADR-033 budget tracking, in rank order.
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
    pub fn parse(s: &str) -> Result<WorkloadClass, super::error::WorkloadClassError> {
        match s {
            "exact_critical" => Ok(WorkloadClass::ExactCritical),
            "standard" => Ok(WorkloadClass::Standard),
            "unsafe_bulk" => Ok(WorkloadClass::UnsafeBulk),
            "recovery" => Ok(WorkloadClass::Recovery),
            _ => Err(super::error::WorkloadClassError::UnknownClass(
                s.to_string(),
            )),
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
    type Err = super::error::WorkloadClassError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        WorkloadClass::parse(s)
    }
}

impl WorkloadClass {
    /// Returns `true` if this class is non-critical (subject to degradation).
    ///
    /// Non-critical classes are `Standard` and `UnsafeBulk`.
    #[must_use]
    pub fn is_non_critical(self) -> bool {
        matches!(self, WorkloadClass::Standard | WorkloadClass::UnsafeBulk)
    }

    /// Returns `true` if this class is protected and always admitted during degraded mode.
    #[must_use]
    pub fn is_protected(self) -> bool {
        !self.is_non_critical()
    }
}
