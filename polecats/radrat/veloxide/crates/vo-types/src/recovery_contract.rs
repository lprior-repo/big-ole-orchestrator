//! Design-by-contract types for recovery and failover testing (ADR-041, ADR-043).
//!
//! Architecture: Data (FailoverScenario, RecoveryAssertion, RecoveryInvariant)
//!             → Calc (scenario_classification, assertion_check, invariant_verify).
//!
//! This module defines the type vocabulary for writing recovery/failover tests
//! that verify exactly-once semantics under crash conditions. All types are
//! pure data — no I/O, no runtime dependencies.
//!
//! # Categories
//!
//! - **FailoverScenario**: What went wrong (crash point, phase, severity)
//! - **RecoveryPhase**: Where in the lifecycle recovery is needed
//! - **RecoveryAssertion**: What must be true after recovery
//! - **RecoveryInvariant**: System-level properties that must always hold

use serde::{Deserialize, Serialize};

// ============================================================================
// Data Layer: Failover Scenario Types
// ============================================================================

/// Phase of the connector lifecycle where a failure occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecoveryPhase {
    /// Failure during effect preparation (before commit).
    Prepare,
    /// Failure during effect commit (ambiguous zone).
    Commit,
    /// Failure during reconciliation query.
    Reconcile,
    /// Failure during compensation/rollback.
    Compensation,
    /// Failure during timer persistence.
    TimerPersistence,
    /// Failure during signal acceptance.
    SignalAcceptance,
    /// Failure during distributed transaction coordination.
    TransactionCoordination,
}

/// Severity of a failover event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FailoverSeverity {
    /// Single operation affected, automatic retry possible.
    Transient,
    /// Operation outcome unknown, reconciliation required.
    Ambiguous,
    /// Component unavailable, failover to backup required.
    ComponentFailure,
    /// Data center or network partition, quorum-based recovery.
    Partition,
}

/// Position relative to an operation where a crash occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CrashTiming {
    /// Crash before the operation wrote any durable state.
    BeforeWrite,
    /// Crash after partial write (durable state may be inconsistent).
    PartialWrite,
    /// Crash after write committed but before acknowledgment sent.
    AfterCommit,
}

/// A specific failover scenario that a test must verify recovery from.
///
/// Each scenario is a named, reproducible failure condition that exercises
/// a specific recovery path. Scenarios are the vocabulary of crash testing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FailoverScenario {
    /// Human-readable scenario identifier (e.g., "commit-crash-before-ack").
    pub name: String,
    /// Phase of the lifecycle where the failure occurs.
    pub phase: RecoveryPhase,
    /// How severe the failure is.
    pub severity: FailoverSeverity,
    /// When the crash happens relative to the operation.
    pub timing: CrashTiming,
    /// Expected outcome after successful recovery.
    pub expected_outcome: ExpectedRecoveryOutcome,
}

/// What the system should look like after successful recovery from a scenario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExpectedRecoveryOutcome {
    /// Effect was committed — proceed as if successful.
    Committed,
    /// Effect was not committed — safe to retry.
    NotCommitted,
    /// Outcome still ambiguous after recovery — escalate.
    StillAmbiguous,
    /// Effect rolled back via compensation.
    RolledBack,
    /// Transaction completed via coordinator recovery protocol.
    TransactionResolved,
}

// ============================================================================
// Data Layer: Recovery Assertion Types
// ============================================================================

/// An assertion about system state that must hold after recovery.
///
/// Each assertion is a single checkable property. Test frameworks evaluate
/// these against actual post-recovery state.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecoveryAssertion {
    /// No effect was committed more than once (exactly-once).
    ExactlyOnceCommit { effect_id: String },
    /// No effect was lost — every submitted effect has a final state.
    NoLostEffects { expected_count: usize },
    /// Connector state machine is in a valid post-recovery state.
    ValidConnectorState {
        effect_id: String,
        expected_terminal: bool,
    },
    /// Transaction coordinator reached a terminal state.
    TransactionTerminal { transaction_id: String },
    /// Reconciliation was invoked exactly N times.
    ReconciliationCount {
        effect_id: String,
        expected_invocations: u32,
    },
    /// No duplicate effects exist in the journal.
    NoDuplicateJournalEntries { effect_id: String },
    /// Orphan detection found and recovered all orphans.
    AllOrphansRecovered { expected_orphan_count: usize },
    /// Fence token was properly incremented after recovery.
    FenceTokenAdvanced {
        effect_id: String,
        minimum_fence: u64,
    },
}

/// Result of evaluating a single recovery assertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssertionResult {
    /// Assertion passed.
    Satisfied,
    /// Assertion failed with a reason.
    Violated(RecoveryViolation),
}

/// Category of assertion violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecoveryViolation {
    /// A duplicate commit was detected.
    DuplicateCommit,
    /// An effect has no recorded outcome.
    LostEffect,
    /// State machine is stuck in a non-terminal state.
    StuckInNonTerminal,
    /// Transaction coordinator did not reach terminal state.
    TransactionAmbiguous,
    /// Reconciliation was called too many or too few times.
    ReconciliationCountMismatch,
    /// Duplicate journal entries found.
    DuplicateJournalEntry,
    /// Orphans remain after recovery sweep.
    UnrecoveredOrphans,
    /// Fence token was not advanced.
    FenceTokenStale,
}

// ============================================================================
// Data Layer: Recovery Invariant Types
// ============================================================================

/// A system-level invariant that must hold across all recovery scenarios.
///
/// Invariants are broader than assertions — they express fundamental safety
/// properties that are true before, during, and after recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecoveryInvariant {
    /// INV-R01: Terminal states never accept new transitions.
    TerminalStatesAreFinal,
    /// INV-R02: Ambiguous states always resolve via reconciliation.
    AmbiguousResolvesViaReconciliation,
    /// INV-R03: Every committed effect has exactly one receipt.
    ExactlyOneReceiptPerCommit,
    /// INV-R04: Recovery operations are idempotent.
    RecoveryIdempotency,
    /// INV-R05: Reconciliation retry count never exceeds max_retries.
    ReconciliationRetryBounded,
    /// INV-R06: No effect transitions skip the prepare phase.
    NoSkippedPrepare,
    /// INV-R07: Compensation always returns to a known state.
    CompensationCompletes,
    /// INV-R08: Transaction decisions are durable once written.
    TransactionDecisionDurable,
}

// ============================================================================
// Calc Layer: Pure Functions
// ============================================================================

impl FailoverScenario {
    /// Creates a new failover scenario with the given parameters.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        phase: RecoveryPhase,
        severity: FailoverSeverity,
        timing: CrashTiming,
        expected_outcome: ExpectedRecoveryOutcome,
    ) -> Self {
        Self {
            name: name.into(),
            phase,
            severity,
            timing,
            expected_outcome,
        }
    }

    /// Returns whether this scenario requires reconciliation to resolve.
    #[must_use]
    pub const fn requires_reconciliation(&self) -> bool {
        matches!(self.severity, FailoverSeverity::Ambiguous)
            || matches!(self.timing, CrashTiming::PartialWrite)
    }

    /// Returns whether this scenario affects the connector commit path.
    #[must_use]
    pub const fn is_connector_path(&self) -> bool {
        matches!(
            self.phase,
            RecoveryPhase::Commit | RecoveryPhase::Reconcile | RecoveryPhase::Compensation
        )
    }

    /// Returns whether this scenario represents a total component failure.
    #[must_use]
    pub const fn is_total_failure(&self) -> bool {
        matches!(
            self.severity,
            FailoverSeverity::ComponentFailure | FailoverSeverity::Partition
        )
    }
}

impl RecoveryPhase {
    /// Returns all RecoveryPhase variants.
    #[must_use]
    pub const fn all_variants() -> &'static [RecoveryPhase] {
        &[
            RecoveryPhase::Prepare,
            RecoveryPhase::Commit,
            RecoveryPhase::Reconcile,
            RecoveryPhase::Compensation,
            RecoveryPhase::TimerPersistence,
            RecoveryPhase::SignalAcceptance,
            RecoveryPhase::TransactionCoordination,
        ]
    }
}

impl FailoverSeverity {
    /// Returns all FailoverSeverity variants in escalating order.
    #[must_use]
    pub const fn all_variants() -> &'static [FailoverSeverity] {
        &[
            FailoverSeverity::Transient,
            FailoverSeverity::Ambiguous,
            FailoverSeverity::ComponentFailure,
            FailoverSeverity::Partition,
        ]
    }
}

impl CrashTiming {
    /// Returns all CrashTiming variants.
    #[must_use]
    pub const fn all_variants() -> &'static [CrashTiming] {
        &[
            CrashTiming::BeforeWrite,
            CrashTiming::PartialWrite,
            CrashTiming::AfterCommit,
        ]
    }
}

impl ExpectedRecoveryOutcome {
    /// Returns all ExpectedRecoveryOutcome variants.
    #[must_use]
    pub const fn all_variants() -> &'static [ExpectedRecoveryOutcome] {
        &[
            ExpectedRecoveryOutcome::Committed,
            ExpectedRecoveryOutcome::NotCommitted,
            ExpectedRecoveryOutcome::StillAmbiguous,
            ExpectedRecoveryOutcome::RolledBack,
            ExpectedRecoveryOutcome::TransactionResolved,
        ]
    }

    /// Returns whether this outcome represents a final, unambiguous resolution.
    #[must_use]
    pub const fn is_resolved(&self) -> bool {
        !matches!(self, ExpectedRecoveryOutcome::StillAmbiguous)
    }
}

impl RecoveryInvariant {
    /// Returns all recovery invariant variants.
    #[must_use]
    pub const fn all_variants() -> &'static [RecoveryInvariant] {
        &[
            RecoveryInvariant::TerminalStatesAreFinal,
            RecoveryInvariant::AmbiguousResolvesViaReconciliation,
            RecoveryInvariant::ExactlyOneReceiptPerCommit,
            RecoveryInvariant::RecoveryIdempotency,
            RecoveryInvariant::ReconciliationRetryBounded,
            RecoveryInvariant::NoSkippedPrepare,
            RecoveryInvariant::CompensationCompletes,
            RecoveryInvariant::TransactionDecisionDurable,
        ]
    }

    /// Returns the invariant identifier string (for logging/test output).
    #[must_use]
    pub const fn id(&self) -> &'static str {
        match self {
            RecoveryInvariant::TerminalStatesAreFinal => "INV-R01",
            RecoveryInvariant::AmbiguousResolvesViaReconciliation => "INV-R02",
            RecoveryInvariant::ExactlyOneReceiptPerCommit => "INV-R03",
            RecoveryInvariant::RecoveryIdempotency => "INV-R04",
            RecoveryInvariant::ReconciliationRetryBounded => "INV-R05",
            RecoveryInvariant::NoSkippedPrepare => "INV-R06",
            RecoveryInvariant::CompensationCompletes => "INV-R07",
            RecoveryInvariant::TransactionDecisionDurable => "INV-R08",
        }
    }
}

impl AssertionResult {
    /// Returns whether the assertion was satisfied.
    #[must_use]
    pub const fn is_satisfied(&self) -> bool {
        matches!(self, AssertionResult::Satisfied)
    }
}

/// Produces the complete matrix of failover scenarios for exhaustive testing.
///
/// Returns one scenario for every combination of phase, severity, and timing,
/// each annotated with the most appropriate expected recovery outcome.
#[must_use]
pub fn generate_scenario_matrix() -> Vec<FailoverScenario> {
    let mut scenarios = Vec::new();

    for phase in RecoveryPhase::all_variants() {
        for severity in FailoverSeverity::all_variants() {
            for timing in CrashTiming::all_variants() {
                let expected_outcome = classify_expected_outcome(*phase, *severity, *timing);
                let name = format!(
                    "{:?}-{:?}-{:?}-to-{:?}",
                    phase, severity, timing, expected_outcome
                );
                scenarios.push(FailoverScenario::new(
                    name,
                    *phase,
                    *severity,
                    *timing,
                    expected_outcome,
                ));
            }
        }
    }

    scenarios
}

/// Classifies the expected recovery outcome based on scenario parameters.
#[must_use]
pub fn classify_expected_outcome(
    phase: RecoveryPhase,
    severity: FailoverSeverity,
    timing: CrashTiming,
) -> ExpectedRecoveryOutcome {
    match (phase, severity, timing) {
        (
            RecoveryPhase::Commit,
            FailoverSeverity::Ambiguous,
            CrashTiming::PartialWrite | CrashTiming::AfterCommit,
        ) => ExpectedRecoveryOutcome::Committed,
        (RecoveryPhase::Commit, FailoverSeverity::Ambiguous, CrashTiming::BeforeWrite) => {
            ExpectedRecoveryOutcome::NotCommitted
        }
        (RecoveryPhase::Compensation, _, _) => ExpectedRecoveryOutcome::RolledBack,
        (RecoveryPhase::TransactionCoordination, _, _) => {
            ExpectedRecoveryOutcome::TransactionResolved
        }
        (_, FailoverSeverity::Transient, _) => ExpectedRecoveryOutcome::Committed,
        (RecoveryPhase::Reconcile, FailoverSeverity::Ambiguous, _) => {
            ExpectedRecoveryOutcome::StillAmbiguous
        }
        (_, _, CrashTiming::BeforeWrite) => ExpectedRecoveryOutcome::NotCommitted,
        _ => ExpectedRecoveryOutcome::Committed,
    }
}

/// Maps a recovery violation to the invariant it breaches.
#[must_use]
pub const fn violation_to_invariant(violation: RecoveryViolation) -> RecoveryInvariant {
    match violation {
        RecoveryViolation::DuplicateCommit => RecoveryInvariant::ExactlyOneReceiptPerCommit,
        RecoveryViolation::LostEffect => RecoveryInvariant::NoSkippedPrepare,
        RecoveryViolation::StuckInNonTerminal => {
            RecoveryInvariant::AmbiguousResolvesViaReconciliation
        }
        RecoveryViolation::TransactionAmbiguous => RecoveryInvariant::TransactionDecisionDurable,
        RecoveryViolation::ReconciliationCountMismatch => {
            RecoveryInvariant::ReconciliationRetryBounded
        }
        RecoveryViolation::DuplicateJournalEntry => RecoveryInvariant::ExactlyOneReceiptPerCommit,
        RecoveryViolation::UnrecoveredOrphans => RecoveryInvariant::RecoveryIdempotency,
        RecoveryViolation::FenceTokenStale => RecoveryInvariant::RecoveryIdempotency,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_matrix_covers_all_phase_severity_timing_combinations() {
        let matrix = generate_scenario_matrix();
        let expected = RecoveryPhase::all_variants().len()
            * FailoverSeverity::all_variants().len()
            * CrashTiming::all_variants().len();
        assert_eq!(matrix.len(), expected);
    }

    #[test]
    fn all_recovery_phases_have_variants() {
        assert_eq!(RecoveryPhase::all_variants().len(), 7);
    }

    #[test]
    fn all_failover_severities_have_variants() {
        assert_eq!(FailoverSeverity::all_variants().len(), 4);
    }

    #[test]
    fn all_crash_timings_have_variants() {
        assert_eq!(CrashTiming::all_variants().len(), 3);
    }

    #[test]
    fn all_expected_outcomes_have_variants() {
        assert_eq!(ExpectedRecoveryOutcome::all_variants().len(), 5);
    }

    #[test]
    fn all_invariants_have_ids() {
        for inv in RecoveryInvariant::all_variants() {
            assert!(
                inv.id().starts_with("INV-R"),
                "Invariant {:?} has unexpected id: {}",
                inv,
                inv.id()
            );
        }
    }

    #[test]
    fn all_invariants_have_unique_ids() {
        let ids: Vec<&str> = RecoveryInvariant::all_variants()
            .iter()
            .map(|i| i.id())
            .collect();
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(ids.len(), unique.len());
    }

    #[test]
    fn scenario_requires_reconciliation_when_ambiguous() {
        let scenario = FailoverScenario::new(
            "test",
            RecoveryPhase::Commit,
            FailoverSeverity::Ambiguous,
            CrashTiming::AfterCommit,
            ExpectedRecoveryOutcome::Committed,
        );
        assert!(scenario.requires_reconciliation());
    }

    #[test]
    fn scenario_no_reconciliation_when_transient() {
        let scenario = FailoverScenario::new(
            "test",
            RecoveryPhase::Commit,
            FailoverSeverity::Transient,
            CrashTiming::AfterCommit,
            ExpectedRecoveryOutcome::Committed,
        );
        assert!(!scenario.requires_reconciliation());
    }

    #[test]
    fn scenario_partial_write_requires_reconciliation() {
        let scenario = FailoverScenario::new(
            "test",
            RecoveryPhase::Commit,
            FailoverSeverity::Transient,
            CrashTiming::PartialWrite,
            ExpectedRecoveryOutcome::Committed,
        );
        assert!(scenario.requires_reconciliation());
    }

    #[test]
    fn connector_path_covers_commit_reconcile_compensation() {
        let commit = FailoverScenario::new(
            "t",
            RecoveryPhase::Commit,
            FailoverSeverity::Transient,
            CrashTiming::BeforeWrite,
            ExpectedRecoveryOutcome::Committed,
        );
        assert!(commit.is_connector_path());

        let prepare = FailoverScenario::new(
            "t",
            RecoveryPhase::Prepare,
            FailoverSeverity::Transient,
            CrashTiming::BeforeWrite,
            ExpectedRecoveryOutcome::Committed,
        );
        assert!(!prepare.is_connector_path());
    }

    #[test]
    fn total_failure_covers_component_and_partition() {
        let component = FailoverScenario::new(
            "t",
            RecoveryPhase::Commit,
            FailoverSeverity::ComponentFailure,
            CrashTiming::BeforeWrite,
            ExpectedRecoveryOutcome::Committed,
        );
        assert!(component.is_total_failure());

        let partition = FailoverScenario::new(
            "t",
            RecoveryPhase::Commit,
            FailoverSeverity::Partition,
            CrashTiming::BeforeWrite,
            ExpectedRecoveryOutcome::Committed,
        );
        assert!(partition.is_total_failure());

        let transient = FailoverScenario::new(
            "t",
            RecoveryPhase::Commit,
            FailoverSeverity::Transient,
            CrashTiming::BeforeWrite,
            ExpectedRecoveryOutcome::Committed,
        );
        assert!(!transient.is_total_failure());
    }

    #[test]
    fn expected_outcome_is_resolved_except_still_ambiguous() {
        assert!(ExpectedRecoveryOutcome::Committed.is_resolved());
        assert!(ExpectedRecoveryOutcome::NotCommitted.is_resolved());
        assert!(ExpectedRecoveryOutcome::RolledBack.is_resolved());
        assert!(ExpectedRecoveryOutcome::TransactionResolved.is_resolved());
        assert!(!ExpectedRecoveryOutcome::StillAmbiguous.is_resolved());
    }

    #[test]
    fn assertion_result_is_satisfied() {
        assert!(AssertionResult::Satisfied.is_satisfied());
        assert!(!AssertionResult::Violated(RecoveryViolation::DuplicateCommit).is_satisfied());
    }

    #[test]
    fn classify_commit_ambiguous_after_commit_yields_committed() {
        let outcome = classify_expected_outcome(
            RecoveryPhase::Commit,
            FailoverSeverity::Ambiguous,
            CrashTiming::AfterCommit,
        );
        assert_eq!(outcome, ExpectedRecoveryOutcome::Committed);
    }

    #[test]
    fn classify_commit_ambiguous_before_write_yields_not_committed() {
        let outcome = classify_expected_outcome(
            RecoveryPhase::Commit,
            FailoverSeverity::Ambiguous,
            CrashTiming::BeforeWrite,
        );
        assert_eq!(outcome, ExpectedRecoveryOutcome::NotCommitted);
    }

    #[test]
    fn classify_compensation_yields_rolled_back() {
        let outcome = classify_expected_outcome(
            RecoveryPhase::Compensation,
            FailoverSeverity::Transient,
            CrashTiming::BeforeWrite,
        );
        assert_eq!(outcome, ExpectedRecoveryOutcome::RolledBack);
    }

    #[test]
    fn classify_transaction_coordination_yields_resolved() {
        let outcome = classify_expected_outcome(
            RecoveryPhase::TransactionCoordination,
            FailoverSeverity::Ambiguous,
            CrashTiming::PartialWrite,
        );
        assert_eq!(outcome, ExpectedRecoveryOutcome::TransactionResolved);
    }

    #[test]
    fn violation_maps_to_correct_invariant() {
        assert_eq!(
            violation_to_invariant(RecoveryViolation::DuplicateCommit),
            RecoveryInvariant::ExactlyOneReceiptPerCommit
        );
        assert_eq!(
            violation_to_invariant(RecoveryViolation::StuckInNonTerminal),
            RecoveryInvariant::AmbiguousResolvesViaReconciliation
        );
        assert_eq!(
            violation_to_invariant(RecoveryViolation::TransactionAmbiguous),
            RecoveryInvariant::TransactionDecisionDurable
        );
        assert_eq!(
            violation_to_invariant(RecoveryViolation::ReconciliationCountMismatch),
            RecoveryInvariant::ReconciliationRetryBounded
        );
        assert_eq!(
            violation_to_invariant(RecoveryViolation::FenceTokenStale),
            RecoveryInvariant::RecoveryIdempotency
        );
    }

    #[test]
    fn scenario_names_are_unique_in_matrix() {
        let matrix = generate_scenario_matrix();
        let names: Vec<&str> = matrix.iter().map(|s| s.name.as_str()).collect();
        let mut unique = names.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(names.len(), unique.len());
    }

    #[test]
    fn matrix_generation_is_deterministic() {
        let matrix1 = generate_scenario_matrix();
        let matrix2 = generate_scenario_matrix();
        assert_eq!(matrix1.len(), matrix2.len());
        for (s1, s2) in matrix1.iter().zip(matrix2.iter()) {
            assert_eq!(s1.name, s2.name);
            assert_eq!(s1.phase, s2.phase);
            assert_eq!(s1.severity, s2.severity);
            assert_eq!(s1.timing, s2.timing);
            assert_eq!(s1.expected_outcome, s2.expected_outcome);
        }
    }

    #[test]
    fn classification_is_deterministic_for_all_combinations() {
        for phase in RecoveryPhase::all_variants() {
            for severity in FailoverSeverity::all_variants() {
                for timing in CrashTiming::all_variants() {
                    let outcome1 = classify_expected_outcome(*phase, *severity, *timing);
                    let outcome2 = classify_expected_outcome(*phase, *severity, *timing);
                    assert_eq!(
                        outcome1, outcome2,
                        "classify_expected_outcome not deterministic for {:?}/{:?}/{:?}",
                        phase, severity, timing
                    );
                }
            }
        }
    }

    #[test]
    fn all_violations_map_to_valid_invariants() {
        let violations = [
            RecoveryViolation::DuplicateCommit,
            RecoveryViolation::LostEffect,
            RecoveryViolation::StuckInNonTerminal,
            RecoveryViolation::TransactionAmbiguous,
            RecoveryViolation::ReconciliationCountMismatch,
            RecoveryViolation::DuplicateJournalEntry,
            RecoveryViolation::UnrecoveredOrphans,
            RecoveryViolation::FenceTokenStale,
        ];

        for violation in &violations {
            let invariant = violation_to_invariant(*violation);
            assert!(
                matches!(
                    invariant,
                    RecoveryInvariant::ExactlyOneReceiptPerCommit
                        | RecoveryInvariant::NoSkippedPrepare
                        | RecoveryInvariant::AmbiguousResolvesViaReconciliation
                        | RecoveryInvariant::TransactionDecisionDurable
                        | RecoveryInvariant::ReconciliationRetryBounded
                        | RecoveryInvariant::RecoveryIdempotency
                ),
                "Violation {:?} maps to unexpected invariant {:?}",
                violation,
                invariant
            );
        }
    }

    #[test]
    fn structural_invariants_hold_by_construction() {
        assert_eq!(RecoveryInvariant::all_variants().len(), 8);
        let structural_invariants = [
            RecoveryInvariant::TerminalStatesAreFinal,
            RecoveryInvariant::CompensationCompletes,
        ];
        assert_eq!(structural_invariants.len(), 2);

        for inv in RecoveryInvariant::all_variants() {
            if *inv == RecoveryInvariant::TerminalStatesAreFinal {
                assert_eq!(inv.id(), "INV-R01");
            }
            if *inv == RecoveryInvariant::CompensationCompletes {
                assert_eq!(inv.id(), "INV-R07");
            }
        }
    }

    #[test]
    fn all_scenarios_have_valid_expected_outcome() {
        let matrix = generate_scenario_matrix();
        for scenario in &matrix {
            let outcome =
                classify_expected_outcome(scenario.phase, scenario.severity, scenario.timing);
            assert_eq!(
                outcome, scenario.expected_outcome,
                "Scenario {} has inconsistent outcome: classified as {:?} but scenario expects {:?}",
                scenario.name, outcome, scenario.expected_outcome
            );
        }
    }

    #[test]
    fn every_phase_severity_timing_combination_classified() {
        let expected_count = RecoveryPhase::all_variants().len()
            * FailoverSeverity::all_variants().len()
            * CrashTiming::all_variants().len();
        let matrix = generate_scenario_matrix();
        assert_eq!(matrix.len(), expected_count);

        let mut covered = 0;
        for phase in RecoveryPhase::all_variants() {
            for severity in FailoverSeverity::all_variants() {
                for timing in CrashTiming::all_variants() {
                    let outcome = classify_expected_outcome(*phase, *severity, *timing);
                    assert!(
                        matches!(
                            outcome,
                            ExpectedRecoveryOutcome::Committed
                                | ExpectedRecoveryOutcome::NotCommitted
                                | ExpectedRecoveryOutcome::StillAmbiguous
                                | ExpectedRecoveryOutcome::RolledBack
                                | ExpectedRecoveryOutcome::TransactionResolved
                        ),
                        "Invalid outcome {:?} for {:?}/{:?}/{:?}",
                        outcome,
                        phase,
                        severity,
                        timing
                    );
                    covered += 1;
                }
            }
        }
        assert_eq!(covered, expected_count);
    }

    #[test]
    fn crash_timing_windows_are_mutually_exclusive() {
        let timings = CrashTiming::all_variants();
        assert_eq!(timings.len(), 3);

        for timing in timings {
            let scenario = FailoverScenario::new(
                "test",
                RecoveryPhase::Commit,
                FailoverSeverity::Ambiguous,
                *timing,
                ExpectedRecoveryOutcome::Committed,
            );
            match timing {
                CrashTiming::BeforeWrite => {
                    assert!(!matches!(
                        scenario.timing,
                        CrashTiming::PartialWrite | CrashTiming::AfterCommit
                    ));
                }
                CrashTiming::PartialWrite => {
                    assert!(!matches!(
                        scenario.timing,
                        CrashTiming::BeforeWrite | CrashTiming::AfterCommit
                    ));
                }
                CrashTiming::AfterCommit => {
                    assert!(!matches!(
                        scenario.timing,
                        CrashTiming::BeforeWrite | CrashTiming::PartialWrite
                    ));
                }
            }
        }
    }

    #[test]
    fn no_skipped_prepare_invariant_holds() {
        for phase in RecoveryPhase::all_variants() {
            for severity in FailoverSeverity::all_variants() {
                for timing in CrashTiming::all_variants() {
                    let outcome = classify_expected_outcome(*phase, *severity, *timing);
                    if phase == &RecoveryPhase::Prepare {
                        assert!(
                            matches!(outcome, ExpectedRecoveryOutcome::Committed
                                | ExpectedRecoveryOutcome::NotCommitted
                                | ExpectedRecoveryOutcome::RolledBack),
                            "Prepare phase should never yield StillAmbiguous or TransactionResolved"
                        );
    }
}
        }
    }
}
    #[test]
    fn crash_timing_windows_are_mutually_exclusive() {
        let timings = CrashTiming::all_variants();
        for i in 0..timings.len() {
            for j in (i + 1)..timings.len() {
                assert_ne!(
                    timings[i], timings[j],
                    "CrashTiming variants must be mutually exclusive: {:?} == {:?}",
                    timings[i], timings[j]
                );
            }
        }
    }

    #[test]
    fn matrix_contains_every_phase_severity_timing_combination() {
        let matrix = generate_scenario_matrix();
        for phase in RecoveryPhase::all_variants() {
            for severity in FailoverSeverity::all_variants() {
                for timing in CrashTiming::all_variants() {
                    let found = matrix.iter().any(|s| {
                        s.phase == *phase && s.severity == *severity && s.timing == *timing
                    });
                    assert!(
                        found,
                        "Matrix missing combination: {:?}-{:?}-{:?}",
                        phase, severity, timing
                    );
                }
            }
        }
    }

    #[test]
    fn matrix_is_deterministic() {
        let first = generate_scenario_matrix();
        let second = generate_scenario_matrix();
        assert_eq!(first, second);
    }

    #[test]
    fn all_violations_map_to_an_invariant() {
        let violations = [
            RecoveryViolation::DuplicateCommit,
            RecoveryViolation::LostEffect,
            RecoveryViolation::StuckInNonTerminal,
            RecoveryViolation::TransactionAmbiguous,
            RecoveryViolation::ReconciliationCountMismatch,
            RecoveryViolation::DuplicateJournalEntry,
            RecoveryViolation::UnrecoveredOrphans,
            RecoveryViolation::FenceTokenStale,
        ];
        for violation in violations {
            let inv = violation_to_invariant(violation);
            assert!(
                RecoveryInvariant::all_variants().contains(&inv),
                "Violation {:?} maps to invariant {:?} not in all_variants()",
                violation,
                inv
            );
        }
    }

    #[test]
    fn every_scenario_has_valid_phase_severity_timing() {
        let matrix = generate_scenario_matrix();
        let valid_phases: Vec<_> = RecoveryPhase::all_variants().to_vec();
        let valid_severities: Vec<_> = FailoverSeverity::all_variants().to_vec();
        let valid_timings: Vec<_> = CrashTiming::all_variants().to_vec();
        let valid_outcomes: Vec<_> = ExpectedRecoveryOutcome::all_variants().to_vec();

        for scenario in &matrix {
            assert!(
                valid_phases.contains(&scenario.phase),
                "Invalid phase {:?} in scenario {}",
                scenario.phase,
                scenario.name
            );
            assert!(
                valid_severities.contains(&scenario.severity),
                "Invalid severity {:?} in scenario {}",
                scenario.severity,
                scenario.name
            );
            assert!(
                valid_timings.contains(&scenario.timing),
                "Invalid timing {:?} in scenario {}",
                scenario.timing,
                scenario.name
            );
            assert!(
                valid_outcomes.contains(&scenario.expected_outcome),
                "Invalid outcome {:?} in scenario {}",
                scenario.expected_outcome,
                scenario.name
            );
        }
    }

    #[test]
    fn classify_outcome_always_returns_valid_variant() {
        let valid = ExpectedRecoveryOutcome::all_variants();
        for phase in RecoveryPhase::all_variants() {
            for severity in FailoverSeverity::all_variants() {
                for timing in CrashTiming::all_variants() {
                    let outcome = classify_expected_outcome(*phase, *severity, *timing);
                    assert!(
                        valid.contains(&outcome),
                        "classify_expected_outcome({:?}, {:?}, {:?}) = {:?} not in valid outcomes",
                        phase,
                        severity,
                        timing,
                        outcome
                    );
                }
            }
        }
    }

    #[test]
    fn compensation_always_yields_rolled_back() {
        for severity in FailoverSeverity::all_variants() {
            for timing in CrashTiming::all_variants() {
                let outcome =
                    classify_expected_outcome(RecoveryPhase::Compensation, *severity, *timing);
                assert_eq!(
                    outcome,
                    ExpectedRecoveryOutcome::RolledBack,
                    "Compensation+{:?}+{:?} should be RolledBack, got {:?}",
                    severity,
                    timing,
                    outcome
                );
            }
        }
    }

    #[test]
    fn transaction_coordination_always_yields_resolved() {
        for severity in FailoverSeverity::all_variants() {
            for timing in CrashTiming::all_variants() {
                let outcome = classify_expected_outcome(
                    RecoveryPhase::TransactionCoordination,
                    *severity,
                    *timing,
                );
                assert_eq!(
                    outcome,
                    ExpectedRecoveryOutcome::TransactionResolved,
                    "TransactionCoordination+{:?}+{:?} should be TransactionResolved, got {:?}",
                    severity,
                    timing,
                    outcome
                );
            }
        }
    }

    #[test]
    fn reconcile_ambiguous_yields_still_ambiguous() {
        for timing in CrashTiming::all_variants() {
            let outcome = classify_expected_outcome(
                RecoveryPhase::Reconcile,
                FailoverSeverity::Ambiguous,
                *timing,
            );
            assert_eq!(
                outcome,
                ExpectedRecoveryOutcome::StillAmbiguous,
                "Reconcile+Ambiguous+{:?} should be StillAmbiguous, got {:?}",
                timing,
                outcome
            );
        }
    }

    #[test]
    fn transient_always_yields_committed() {
        for phase in RecoveryPhase::all_variants() {
            if *phase == RecoveryPhase::Compensation
                || *phase == RecoveryPhase::TransactionCoordination
            {
                continue;
            }
            for timing in CrashTiming::all_variants() {
                let outcome =
                    classify_expected_outcome(*phase, FailoverSeverity::Transient, *timing);
                assert_eq!(
                    outcome,
                    ExpectedRecoveryOutcome::Committed,
                    "{:?}+Transient+{:?} should be Committed, got {:?}",
                    phase,
                    timing,
                    outcome
                );
            }
        }
    }

    #[test]
    fn invariant_count_matches_documentation() {
        assert_eq!(
            RecoveryInvariant::all_variants().len(),
            8,
            "INV-R01 through INV-R08 = 8 invariants"
        );
    }

    #[test]
    fn violation_count_covers_all_documented_categories() {
        let all: Vec<RecoveryViolation> = vec![
            RecoveryViolation::DuplicateCommit,
            RecoveryViolation::LostEffect,
            RecoveryViolation::StuckInNonTerminal,
            RecoveryViolation::TransactionAmbiguous,
            RecoveryViolation::ReconciliationCountMismatch,
            RecoveryViolation::DuplicateJournalEntry,
            RecoveryViolation::UnrecoveredOrphans,
            RecoveryViolation::FenceTokenStale,
        ];
        assert_eq!(all.len(), 8, "Expected 8 RecoveryViolation variants");
    }

    #[test]
    fn no_scenario_has_empty_name() {
        let matrix = generate_scenario_matrix();
        for scenario in &matrix {
            assert!(
                !scenario.name.is_empty(),
                "Scenario with phase={:?}, severity={:?}, timing={:?} has empty name",
                scenario.phase,
                scenario.severity,
                scenario.timing
            );
        }
    }
}
