//! Workload class taxonomy for budget-based admission control per ADR-013.
//!
//! This module implements the coupling between write pressure indicators and
//! degraded mode admission. When storage health degrades, non-critical workload
//! classes are restricted while critical classes (Live, Recovery) preserve budgets.
//!
//! # Workload Classes
//!
//! - **Live**: Highest priority, receives reserved budget, never rejected in degraded mode
//! - **Recovery**: Reserved budget for crash recovery, cannot starve Live
//! - **TimerResume**: Shares budget with Recovery
//! - **NonCritical**: First to be rejected in degraded mode
//! - **Background**: Deferred in degraded mode (blob writes, projections)
//!
//! # Degraded Mode State Machine
//!
//! - **Normal**: All classes accepted
//! - **Degraded**: NonCritical and Background restricted
//! - **Critical**: Only Live and Recovery accepted

use serde::{Deserialize, Serialize};

use super::types::PressureIndicator;

// ─────────────────────────────────────────────────────────────────────────────
// WorkloadClass Enum
// ─────────────────────────────────────────────────────────────────────────────

/// Workload classification for budget-based admission per ADR-013.
///
/// Variants are ordered by priority (highest first) for admission decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadClass {
    /// Highest priority. Never rejected in any degraded mode.
    Live,
    /// Reserved capacity for crash recovery.
    Recovery,
    /// Timer-based resume, shares budget with Recovery.
    TimerResume,
    /// First to be rejected when degraded.
    NonCritical,
    /// Background tasks (blob writes, projections), deferred in degraded mode.
    Background,
}

impl WorkloadClass {
    /// Returns `true` if this class is never starved (always gets budget).
    #[must_use]
    pub fn never_starved(self) -> bool {
        matches!(self, WorkloadClass::Live | WorkloadClass::Recovery)
    }

    /// Returns `true` if this class is deferred in degraded mode.
    #[must_use]
    pub fn is_deferred_in_degraded(self) -> bool {
        matches!(self, WorkloadClass::NonCritical | WorkloadClass::Background)
    }

    /// Returns `true` if this class is accepted in Critical degraded mode.
    ///
    /// Only Live and Recovery are accepted when system is critical.
    #[must_use]
    pub fn is_accepted_in_critical(self) -> bool {
        matches!(self, WorkloadClass::Live | WorkloadClass::Recovery)
    }

    /// Returns all variants ordered by priority (highest first).
    #[must_use]
    pub fn all_by_priority() -> &'static [WorkloadClass] {
        &[
            WorkloadClass::Live,
            WorkloadClass::Recovery,
            WorkloadClass::TimerResume,
            WorkloadClass::NonCritical,
            WorkloadClass::Background,
        ]
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DegradedMode Enum
// ─────────────────────────────────────────────────────────────────────────────

/// Degraded mode state machine per ADR-013.
///
/// Represents the system's resilience state based on storage health indicators.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "triggers", rename_all = "snake_case")]
pub enum DegradedMode {
    /// Normal operation — all workload classes accepted.
    Normal,
    /// Pressure detected — non-critical classes restricted.
    Degraded {
        /// Which pressure indicators triggered degraded mode.
        triggers: Vec<PressureIndicator>,
    },
    /// Critical pressure — only Live and Recovery accepted.
    Critical {
        /// Which pressure indicators triggered critical mode.
        triggers: Vec<PressureIndicator>,
    },
}

impl DegradedMode {
    /// Returns `true` if this mode is Normal.
    #[must_use]
    pub fn is_normal(self) -> bool {
        matches!(self, DegradedMode::Normal)
    }

    /// Returns `true` if this mode is Degraded or Critical.
    #[must_use]
    pub fn is_degraded(self) -> bool {
        matches!(
            self,
            DegradedMode::Degraded { .. } | DegradedMode::Critical { .. }
        )
    }

    /// Returns the triggers that caused this degraded mode.
    #[must_use]
    pub fn triggers(self) -> Vec<PressureIndicator> {
        match self {
            DegradedMode::Normal => Vec::new(),
            DegradedMode::Degraded { triggers } | DegradedMode::Critical { triggers } => triggers,
        }
    }
}
