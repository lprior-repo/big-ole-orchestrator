use vo_types::{
    classify_expected_outcome, generate_scenario_matrix, violation_to_invariant, AssertionResult,
    CrashTiming, ExpectedRecoveryOutcome, FailoverScenario, FailoverSeverity, RecoveryInvariant,
    RecoveryPhase, RecoveryViolation,
};

#[test]
fn test_valid_scenario_produces_expected_assertion() {
    let scenario = FailoverScenario::new(
        "commit-ambiguous-after-commit",
        RecoveryPhase::Commit,
        FailoverSeverity::Ambiguous,
        CrashTiming::AfterCommit,
        ExpectedRecoveryOutcome::Committed,
    );

    let outcome = classify_expected_outcome(scenario.phase, scenario.severity, scenario.timing);
    assert_eq!(
        outcome, ExpectedRecoveryOutcome::Committed,
        "Commit phase with Ambiguous severity and AfterCommit timing should yield Committed outcome"
    );

    let scenario2 = FailoverScenario::new(
        "commit-ambiguous-before-write",
        RecoveryPhase::Commit,
        FailoverSeverity::Ambiguous,
        CrashTiming::BeforeWrite,
        ExpectedRecoveryOutcome::NotCommitted,
    );

    let outcome2 = classify_expected_outcome(scenario2.phase, scenario2.severity, scenario2.timing);
    assert_eq!(
        outcome2, ExpectedRecoveryOutcome::NotCommitted,
        "Commit phase with Ambiguous severity and BeforeWrite timing should yield NotCommitted outcome"
    );

    let scenario3 = FailoverScenario::new(
        "compensation-transient-before-write",
        RecoveryPhase::Compensation,
        FailoverSeverity::Transient,
        CrashTiming::BeforeWrite,
        ExpectedRecoveryOutcome::RolledBack,
    );

    let outcome3 = classify_expected_outcome(scenario3.phase, scenario3.severity, scenario3.timing);
    assert_eq!(
        outcome3,
        ExpectedRecoveryOutcome::RolledBack,
        "Compensation phase should always yield RolledBack outcome regardless of severity/timing"
    );

    let scenario4 = FailoverScenario::new(
        "transaction-coordination-ambiguous-partial",
        RecoveryPhase::TransactionCoordination,
        FailoverSeverity::Ambiguous,
        CrashTiming::PartialWrite,
        ExpectedRecoveryOutcome::TransactionResolved,
    );

    let outcome4 = classify_expected_outcome(scenario4.phase, scenario4.severity, scenario4.timing);
    assert_eq!(
        outcome4,
        ExpectedRecoveryOutcome::TransactionResolved,
        "TransactionCoordination phase should always yield TransactionResolved outcome"
    );
}

#[test]
fn test_matrix_generates_all_combinations() {
    let matrix = generate_scenario_matrix();

    let phase_count = RecoveryPhase::all_variants().len();
    let severity_count = FailoverSeverity::all_variants().len();
    let timing_count = CrashTiming::all_variants().len();
    let expected_count = phase_count * severity_count * timing_count;

    assert_eq!(
        matrix.len(),
        expected_count,
        "Matrix should contain all {} x {} x {} = {} combinations, but got {}",
        phase_count,
        severity_count,
        timing_count,
        expected_count,
        matrix.len()
    );

    let names: Vec<&str> = matrix.iter().map(|s| s.name.as_str()).collect();
    let mut sorted_names = names.clone();
    sorted_names.sort();
    sorted_names.dedup();
    assert_eq!(
        names.len(),
        sorted_names.len(),
        "All scenario names should be unique, but found duplicates"
    );
}

#[test]
fn test_unknown_failure_mode_handled_gracefully() {
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
                    "All known combinations should return a valid outcome, but {:?}/{:?}/{:?} returned unexpected outcome",
                    phase,
                    severity,
                    timing
                );
            }
        }
    }
}

#[test]
fn test_malformed_scenario_returns_error() {
    let valid_scenario = FailoverScenario::new(
        "valid-commit",
        RecoveryPhase::Commit,
        FailoverSeverity::Ambiguous,
        CrashTiming::AfterCommit,
        ExpectedRecoveryOutcome::Committed,
    );
    assert_eq!(
        valid_scenario.phase,
        RecoveryPhase::Commit,
        "Valid scenario should be created successfully"
    );

    let scenario_matrix = generate_scenario_matrix();
    for scenario in scenario_matrix {
        let outcome = classify_expected_outcome(scenario.phase, scenario.severity, scenario.timing);
        assert!(
            scenario.expected_outcome == outcome,
            "Scenario '{}' has expected_outcome {:?} but classify_expected_outcome returned {:?}",
            scenario.name,
            scenario.expected_outcome,
            outcome
        );
    }
}

#[test]
fn test_all_failure_modes_have_invariant_mapping() {
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
                RecoveryInvariant::TerminalStatesAreFinal
                    | RecoveryInvariant::AmbiguousResolvesViaReconciliation
                    | RecoveryInvariant::ExactlyOneReceiptPerCommit
                    | RecoveryInvariant::RecoveryIdempotency
                    | RecoveryInvariant::ReconciliationRetryBounded
                    | RecoveryInvariant::NoSkippedPrepare
                    | RecoveryInvariant::CompensationCompletes
                    | RecoveryInvariant::TransactionDecisionDurable
            ),
            "All violations should map to a valid invariant, but {:?} mapped to {:?}",
            violation,
            invariant
        );
    }
}

#[test]
fn test_invariant_ids_are_deterministic() {
    for invariant in RecoveryInvariant::all_variants() {
        let id1 = invariant.id();
        let id2 = invariant.id();
        assert_eq!(
            id1, id2,
            "Invariant id() should be deterministic, but {:?} returned '{}' and '{}'",
            invariant, id1, id2
        );
        assert!(
            id1.starts_with("INV-R"),
            "Invariant id should start with 'INV-R', but {:?} returned '{}'",
            invariant,
            id1
        );
    }
}

#[test]
fn test_crash_timing_windows_are_mutually_exclusive() {
    let timings = CrashTiming::all_variants();
    assert_eq!(
        timings.len(),
        3,
        "CrashTiming should have exactly 3 variants"
    );

    for timing in timings {
        match timing {
            CrashTiming::BeforeWrite => {
                let scenario = FailoverScenario::new(
                    "test-before-write",
                    RecoveryPhase::Commit,
                    FailoverSeverity::Transient,
                    CrashTiming::BeforeWrite,
                    ExpectedRecoveryOutcome::NotCommitted,
                );
                assert!(!scenario.requires_reconciliation());
            }
            CrashTiming::PartialWrite => {
                let scenario = FailoverScenario::new(
                    "test-partial-write",
                    RecoveryPhase::Commit,
                    FailoverSeverity::Transient,
                    CrashTiming::PartialWrite,
                    ExpectedRecoveryOutcome::Committed,
                );
                assert!(scenario.requires_reconciliation());
            }
            CrashTiming::AfterCommit => {
                let scenario = FailoverScenario::new(
                    "test-after-commit",
                    RecoveryPhase::Commit,
                    FailoverSeverity::Transient,
                    CrashTiming::AfterCommit,
                    ExpectedRecoveryOutcome::Committed,
                );
                assert!(!scenario.requires_reconciliation());
            }
        }
    }
}

#[test]
fn test_assertion_result_classification() {
    let satisfied = AssertionResult::Satisfied;
    assert!(
        satisfied.is_satisfied(),
        "Satisfied result should return true for is_satisfied()"
    );

    let violated = AssertionResult::Violated(RecoveryViolation::DuplicateCommit);
    assert!(
        !violated.is_satisfied(),
        "Violated result should return false for is_satisfied()"
    );

    let violated_lost = AssertionResult::Violated(RecoveryViolation::LostEffect);
    assert!(
        !violated_lost.is_satisfied(),
        "Violated result for LostEffect should return false for is_satisfied()"
    );
}

#[test]
fn test_expected_outcome_resolved_classification() {
    assert!(
        ExpectedRecoveryOutcome::Committed.is_resolved(),
        "Committed should be resolved"
    );
    assert!(
        ExpectedRecoveryOutcome::NotCommitted.is_resolved(),
        "NotCommitted should be resolved"
    );
    assert!(
        ExpectedRecoveryOutcome::RolledBack.is_resolved(),
        "RolledBack should be resolved"
    );
    assert!(
        ExpectedRecoveryOutcome::TransactionResolved.is_resolved(),
        "TransactionResolved should be resolved"
    );
    assert!(
        !ExpectedRecoveryOutcome::StillAmbiguous.is_resolved(),
        "StillAmbiguous should NOT be resolved"
    );
}
