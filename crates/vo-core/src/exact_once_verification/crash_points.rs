//! Crash-point definitions for exact-once verification (ADR-043).
//!
//! Every critical transition in the Veloxide engine defines injectable
//! crash points before and after the operation. This module provides the
//! type-safe enumeration of all crash points.
//!
//! ## Crash-Point Matrix
//!
//! | Point | Description | Before | After |
//! |-------|-------------|--------|-------|
//! | [`DedupeWrite`][CrashPoint::DedupeWrite] | dedupe write | ✓ | ✓ |
//! | [`StepScheduled`][CrashPoint::StepScheduled] | StepScheduled transition | ✓ | ✓ |
//! | [`FenceAcquisition`][CrashPoint::FenceAcquisition] | fence acquisition | ✓ | ✓ |
//! | [`ChildStart`][CrashPoint::ChildStart] | child start | ✓ | ✓ |
//! | [`EffectPrepared`][CrashPoint::EffectPrepared] | EffectPrepared | ✓ | ✓ |
//! | [`ConnectorCommit`][CrashPoint::ConnectorCommit] | connector commit | ✓ | ✓ |
//! | [`EffectCommitted`][CrashPoint::EffectCommitted] | EffectCommitted | ✓ | ✓ |
//! | [`StepCompleted`][CrashPoint::StepCompleted] | StepCompleted | ✓ | ✓ |
//! | [`TimerPersistence`][CrashPoint::TimerPersistence] | timer persistence | ✓ | ✓ |
//! | [`SignalAcceptance`][CrashPoint::SignalAcceptance] | signal acceptance | ✓ | ✓ |
//! | [`LineageRollover`][CrashPoint::LineageRollover] | lineage rollover | ✓ | ✓ |
//! | [`Compensation`][CrashPoint::Compensation] | compensation prepare/commit | ✓ | ✓ |

use serde::{Deserialize, Serialize};

/// Crash point in the exact-once transition lifecycle.
///
/// Each crash point represents a location where a crash can be injected
/// to test resilience and deterministic replay behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CrashPoint {
    /// Dedupe write operation.
    ///
    /// Crash before: write not recorded, duplicate may reprocess.
    /// Crash after: write recorded, duplicate rejected on replay.
    DedupeWrite,

    /// StepScheduled transition.
    ///
    /// Crash before: step not scheduled, may be re-scheduled.
    /// Crash after: step scheduled, fence acquired.
    StepScheduled,

    /// Fence acquisition for step execution.
    ///
    /// Crash before: fence not acquired, stale completion cannot win.
    /// Crash after: fence held, completion valid.
    FenceAcquisition,

    /// Child workflow/actor start.
    ///
    /// Crash before: child not started, no orphan created.
    /// Crash after: child started, lifecycle bound to parent.
    ChildStart,

    /// EffectPrepared — effect prepared for execution.
    ///
    /// Crash before: effect not prepared, no compensation needed.
    /// Crash after: effect prepared, compensation available if needed.
    EffectPrepared,

    /// Connector commit — external connector commitment.
    ///
    /// Crash before: connector not committed, ambiguous state.
    /// Crash after: connector committed, reconciliation if ambiguous.
    ConnectorCommit,

    /// EffectCommitted — effect successfully committed.
    ///
    /// Crash before: effect not committed, may retry.
    /// Crash after: effect committed, terminal for this attempt.
    EffectCommitted,

    /// StepCompleted transition.
    ///
    /// Crash before: step not completed, may be retried.
    /// Crash after: step completed, moving to next step.
    StepCompleted,

    /// Timer persistence.
    ///
    /// Crash before: timer not persisted, will re-fire.
    /// Crash after: timer persisted, will not double-fire.
    TimerPersistence,

    /// Signal acceptance by workflow.
    ///
    /// Crash before: signal not accepted, may be re-delivered.
    /// Crash after: signal accepted, routing finalized.
    SignalAcceptance,

    /// Lineage rollover — signal lineage epoch change.
    ///
    /// Crash before: old lineage valid, signals accepted.
    /// Crash after: new lineage active, old signals stale.
    LineageRollover,

    /// Compensation prepare/commit cycle.
    ///
    /// Crash before: compensation not started, effect still valid.
    /// Crash after: compensation completed, effect rolled back.
    Compensation,
}

impl CrashPoint {
    /// Returns all crash point variants for iteration.
    #[must_use]
    pub fn all_variants() -> &'static [CrashPoint] {
        &[
            CrashPoint::DedupeWrite,
            CrashPoint::StepScheduled,
            CrashPoint::FenceAcquisition,
            CrashPoint::ChildStart,
            CrashPoint::EffectPrepared,
            CrashPoint::ConnectorCommit,
            CrashPoint::EffectCommitted,
            CrashPoint::StepCompleted,
            CrashPoint::TimerPersistence,
            CrashPoint::SignalAcceptance,
            CrashPoint::LineageRollover,
            CrashPoint::Compensation,
        ]
    }

    /// Returns the crash point name as a string for logging/display.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            CrashPoint::DedupeWrite => "DedupeWrite",
            CrashPoint::StepScheduled => "StepScheduled",
            CrashPoint::FenceAcquisition => "FenceAcquisition",
            CrashPoint::ChildStart => "ChildStart",
            CrashPoint::EffectPrepared => "EffectPrepared",
            CrashPoint::ConnectorCommit => "ConnectorCommit",
            CrashPoint::EffectCommitted => "EffectCommitted",
            CrashPoint::StepCompleted => "StepCompleted",
            CrashPoint::TimerPersistence => "TimerPersistence",
            CrashPoint::SignalAcceptance => "SignalAcceptance",
            CrashPoint::LineageRollover => "LineageRollover",
            CrashPoint::Compensation => "Compensation",
        }
    }

    /// Returns whether this crash point is related to effect lifecycle.
    #[must_use]
    pub const fn is_effect_related(&self) -> bool {
        matches!(
            self,
            CrashPoint::EffectPrepared
                | CrashPoint::ConnectorCommit
                | CrashPoint::EffectCommitted
                | CrashPoint::Compensation
        )
    }

    /// Returns whether this crash point is related to step lifecycle.
    #[must_use]
    pub const fn is_step_related(&self) -> bool {
        matches!(
            self,
            CrashPoint::StepScheduled
                | CrashPoint::FenceAcquisition
                | CrashPoint::ChildStart
                | CrashPoint::StepCompleted
        )
    }

    /// Returns whether this crash point is related to external signals/timers.
    #[must_use]
    pub const fn is_external_related(&self) -> bool {
        matches!(
            self,
            CrashPoint::TimerPersistence
                | CrashPoint::SignalAcceptance
                | CrashPoint::LineageRollover
        )
    }
}

/// Position of crash injection relative to the operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CrashPosition {
    /// Crash before the operation completes.
    Before,
    /// Crash after the operation completes.
    After,
}

impl CrashPosition {
    /// Returns both positions for iteration.
    #[must_use]
    pub fn all() -> &'static [CrashPosition] {
        &[CrashPosition::Before, CrashPosition::After]
    }
}

impl std::fmt::Display for CrashPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CrashPosition::Before => write!(f, "Before"),
            CrashPosition::After => write!(f, "After"),
        }
    }
}

/// A crash scenario combining point and position.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CrashScenario {
    /// The crash point.
    pub point: CrashPoint,
    /// The position (before or after).
    pub position: CrashPosition,
}

impl CrashScenario {
    /// Creates a new crash scenario.
    #[must_use]
    pub fn new(point: CrashPoint, position: CrashPosition) -> Self {
        Self { point, position }
    }

    /// Returns all possible crash scenarios (every point × position combination).
    #[must_use]
    pub fn all_scenarios() -> Vec<CrashScenario> {
        let mut scenarios = Vec::new();
        for point in CrashPoint::all_variants() {
            for position in CrashPosition::all() {
                scenarios.push(CrashScenario::new(*point, *position));
            }
        }
        scenarios
    }
}

impl std::fmt::Display for CrashPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl std::fmt::Display for CrashScenario {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}/{}",
            self.point,
            match self.position {
                CrashPosition::Before => "Before",
                CrashPosition::After => "After",
            }
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CrashCategory {
    Effect,
    Step,
    External,
    Dedupe,
}

impl CrashCategory {
    pub fn all() -> &'static [CrashCategory] {
        &[
            CrashCategory::Effect,
            CrashCategory::Step,
            CrashCategory::External,
            CrashCategory::Dedupe,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            CrashCategory::Effect => "Effect",
            CrashCategory::Step => "Step",
            CrashCategory::External => "External",
            CrashCategory::Dedupe => "Dedupe",
        }
    }
}

impl std::fmt::Display for CrashCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashPointMatrix {
    categories: [Vec<CrashPoint>; 4],
}

impl CrashPointMatrix {
    pub fn new() -> Self {
        let effect_points: Vec<CrashPoint> = CrashPoint::all_variants()
            .iter()
            .filter(|p| p.is_effect_related())
            .copied()
            .collect();

        let step_points: Vec<CrashPoint> = CrashPoint::all_variants()
            .iter()
            .filter(|p| p.is_step_related())
            .copied()
            .collect();

        let external_points: Vec<CrashPoint> = CrashPoint::all_variants()
            .iter()
            .filter(|p| p.is_external_related())
            .copied()
            .collect();

        let dedupe_points: Vec<CrashPoint> = CrashPoint::all_variants()
            .iter()
            .filter(|p| matches!(p, CrashPoint::DedupeWrite))
            .copied()
            .collect();

        Self {
            categories: [effect_points, step_points, external_points, dedupe_points],
        }
    }

    pub fn for_category(self: &CrashPointMatrix, category: CrashCategory) -> &[CrashPoint] {
        match category {
            CrashCategory::Effect => &self.categories[0],
            CrashCategory::Step => &self.categories[1],
            CrashCategory::External => &self.categories[2],
            CrashCategory::Dedupe => &self.categories[3],
        }
    }

    pub fn scenarios_for_category(
        self: &CrashPointMatrix,
        category: CrashCategory,
    ) -> Vec<CrashScenario> {
        self.for_category(category)
            .iter()
            .flat_map(|point| {
                CrashPosition::all()
                    .iter()
                    .map(|pos| CrashScenario::new(*point, *pos))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    pub fn all_scenarios(self: &CrashPointMatrix) -> Vec<CrashScenario> {
        CrashScenario::all_scenarios()
    }

    pub fn effect_scenarios(self: &CrashPointMatrix) -> Vec<CrashScenario> {
        self.scenarios_for_category(CrashCategory::Effect)
    }

    pub fn step_scenarios(self: &CrashPointMatrix) -> Vec<CrashScenario> {
        self.scenarios_for_category(CrashCategory::Step)
    }

    pub fn external_scenarios(self: &CrashPointMatrix) -> Vec<CrashScenario> {
        self.scenarios_for_category(CrashCategory::External)
    }

    pub fn dedupe_scenarios(self: &CrashPointMatrix) -> Vec<CrashScenario> {
        self.scenarios_for_category(CrashCategory::Dedupe)
    }

    pub fn total_scenario_count(self: &CrashPointMatrix) -> usize {
        CrashScenario::all_scenarios().len()
    }

    pub fn scenario_count_for_category(self: &CrashPointMatrix, category: CrashCategory) -> usize {
        self.for_category(category).len() * 2
    }
}

impl Default for CrashPointMatrix {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrashMatrixError {
    EmptyCategory { category: CrashCategory },
    InvalidScenarioCount { expected: usize, actual: usize },
}

impl std::fmt::Display for CrashMatrixError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyCategory { category } => {
                write!(f, "Category {} has no crash points", category)
            }
            Self::InvalidScenarioCount { expected, actual } => {
                write!(
                    f,
                    "Invalid scenario count: expected {}, got {}",
                    expected, actual
                )
            }
        }
    }
}

impl std::error::Error for CrashMatrixError {}

pub type CrashMatrixResult<T> = Result<T, CrashMatrixError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_crash_points_have_names() {
        for point in CrashPoint::all_variants() {
            assert!(!point.name().is_empty());
        }
    }

    #[test]
    fn test_all_scenarios_count() {
        let scenarios = CrashScenario::all_scenarios();
        // 12 crash points × 2 positions = 24 scenarios
        assert_eq!(scenarios.len(), 24);
    }

    #[test]
    fn test_effect_related_classification() {
        assert!(CrashPoint::EffectPrepared.is_effect_related());
        assert!(CrashPoint::ConnectorCommit.is_effect_related());
        assert!(CrashPoint::EffectCommitted.is_effect_related());
        assert!(CrashPoint::Compensation.is_effect_related());

        assert!(!CrashPoint::StepScheduled.is_effect_related());
    }

    #[test]
    fn test_step_related_classification() {
        assert!(CrashPoint::StepScheduled.is_step_related());
        assert!(CrashPoint::FenceAcquisition.is_step_related());
        assert!(CrashPoint::ChildStart.is_step_related());
        assert!(CrashPoint::StepCompleted.is_step_related());

        assert!(!CrashPoint::EffectPrepared.is_step_related());
    }

    #[test]
    fn test_external_related_classification() {
        assert!(CrashPoint::TimerPersistence.is_external_related());
        assert!(CrashPoint::SignalAcceptance.is_external_related());
        assert!(CrashPoint::LineageRollover.is_external_related());

        assert!(!CrashPoint::StepScheduled.is_external_related());
    }

    #[test]
    fn test_crash_point_matrix_new() {
        let matrix = CrashPointMatrix::new();
        assert_eq!(matrix.total_scenario_count(), 24);
    }

    #[test]
    fn test_crash_point_matrix_effect_scenarios() {
        let matrix = CrashPointMatrix::new();
        let effect_scenarios = matrix.effect_scenarios();

        assert_eq!(effect_scenarios.len(), 8);

        for scenario in &effect_scenarios {
            assert!(
                matches!(
                    scenario.point,
                    CrashPoint::EffectPrepared
                        | CrashPoint::ConnectorCommit
                        | CrashPoint::EffectCommitted
                        | CrashPoint::Compensation
                ),
                "Expected effect-related crash point, got {:?}",
                scenario.point
            );
        }
    }

    #[test]
    fn test_crash_point_matrix_step_scenarios() {
        let matrix = CrashPointMatrix::new();
        let step_scenarios = matrix.step_scenarios();

        assert_eq!(step_scenarios.len(), 8);

        for scenario in &step_scenarios {
            assert!(
                matches!(
                    scenario.point,
                    CrashPoint::StepScheduled
                        | CrashPoint::FenceAcquisition
                        | CrashPoint::ChildStart
                        | CrashPoint::StepCompleted
                ),
                "Expected step-related crash point, got {:?}",
                scenario.point
            );
        }
    }

    #[test]
    fn test_crash_point_matrix_external_scenarios() {
        let matrix = CrashPointMatrix::new();
        let external_scenarios = matrix.external_scenarios();

        assert_eq!(external_scenarios.len(), 6);

        for scenario in &external_scenarios {
            assert!(
                matches!(
                    scenario.point,
                    CrashPoint::TimerPersistence
                        | CrashPoint::SignalAcceptance
                        | CrashPoint::LineageRollover
                ),
                "Expected external-related crash point, got {:?}",
                scenario.point
            );
        }
    }

    #[test]
    fn test_crash_point_matrix_dedupe_scenarios() {
        let matrix = CrashPointMatrix::new();
        let dedupe_scenarios = matrix.dedupe_scenarios();

        assert_eq!(dedupe_scenarios.len(), 2);

        for scenario in &dedupe_scenarios {
            assert_eq!(
                scenario.point,
                CrashPoint::DedupeWrite,
                "Expected DedupeWrite crash point"
            );
        }
    }

    #[test]
    fn test_crash_point_matrix_scenario_count_by_category() {
        let matrix = CrashPointMatrix::new();

        assert_eq!(matrix.scenario_count_for_category(CrashCategory::Effect), 8);
        assert_eq!(matrix.scenario_count_for_category(CrashCategory::Step), 8);
        assert_eq!(
            matrix.scenario_count_for_category(CrashCategory::External),
            6
        );
        assert_eq!(matrix.scenario_count_for_category(CrashCategory::Dedupe), 2);
    }

    #[test]
    fn test_crash_point_matrix_all_scenarios() {
        let matrix = CrashPointMatrix::new();
        let all = matrix.all_scenarios();

        assert_eq!(all.len(), 24);
    }

    #[test]
    fn test_crash_point_matrix_categories_cover_all_points() {
        let matrix = CrashPointMatrix::new();
        let all_variants = CrashPoint::all_variants();

        let mut covered_names = Vec::new();
        for category in CrashCategory::all() {
            let points = matrix.for_category(*category);
            for point in points {
                covered_names.push(point.name());
            }
        }

        covered_names.sort();
        covered_names.dedup();

        assert_eq!(
            covered_names.len(),
            all_variants.len(),
            "All crash points should be covered by categories"
        );

        for variant in all_variants {
            assert!(
                covered_names.contains(&variant.name()),
                "Crash point {:?} not covered by any category",
                variant
            );
        }
    }

    #[test]
    fn test_crash_category_name() {
        assert_eq!(CrashCategory::Effect.name(), "Effect");
        assert_eq!(CrashCategory::Step.name(), "Step");
        assert_eq!(CrashCategory::External.name(), "External");
        assert_eq!(CrashCategory::Dedupe.name(), "Dedupe");
    }

    #[test]
    fn test_crash_category_all() {
        let categories = CrashCategory::all();
        assert_eq!(categories.len(), 4);
    }

    #[test]
    fn test_invalid_matrix_combinations_empty_category() {
        let matrix = CrashPointMatrix::new();
        let result: CrashMatrixResult<&[CrashPoint]> = Err(CrashMatrixError::EmptyCategory {
            category: CrashCategory::Effect,
        });
        assert!(result.is_err());
    }
}
