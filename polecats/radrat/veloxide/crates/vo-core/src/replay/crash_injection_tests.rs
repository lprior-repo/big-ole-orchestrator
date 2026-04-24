//! Crash injection tests for exact-once verification (ADR-043).
//!
//! These tests verify the crash injection infrastructure:
//! - Crash point matrix (24 scenarios: 12 points × 2 positions)
//! - VerificationHarness crash detection
//! - RecoveryAssertion determinism verification
//!
//! ## Crash-Point Matrix
//!
//! | Point | Description |
//! |-------|-------------|
//! | DedupeWrite | dedupe write |
//! | StepScheduled | StepScheduled transition |
//! | FenceAcquisition | fence acquisition |
//! | ChildStart | child start |
//! | EffectPrepared | EffectPrepared |
//! | ConnectorCommit | connector commit |
//! | EffectCommitted | EffectCommitted |
//! | StepCompleted | StepCompleted |
//! | TimerPersistence | timer persistence |
//! | SignalAcceptance | signal acceptance |
//! | LineageRollover | lineage rollover |
//! | Compensation | compensation prepare/commit |

use crate::exact_once_verification::assertions::{
    assert_fence_token_ordering, assert_no_duplicate_effects, RecoveryAssertion, RecoveryContext,
};
use crate::exact_once_verification::crash_points::{CrashPoint, CrashPosition, CrashScenario};
use crate::exact_once_verification::harness::VerificationHarness;
use crate::replay::engine::ReplayEngine;
use crate::replay::test_helpers::*;
use vo_types::events::EventEnvelope;
use vo_types::state::LifecycleState;

#[cfg(test)]
mod harness_tests {
    use super::*;

    #[test]
    fn verification_harness_no_crash_by_default() {
        let harness = VerificationHarness::new();
        for point in CrashPoint::all_variants() {
            assert!(!harness.should_crash(*point));
            assert!(!harness.should_crash_at(*point, CrashPosition::Before));
            assert!(!harness.should_crash_at(*point, CrashPosition::After));
        }
    }

    #[test]
    fn verification_harness_crash_at_specific_point() {
        let harness = VerificationHarness::with_crash_scenario(
            CrashPoint::StepScheduled,
            CrashPosition::Before,
        );

        assert!(harness.should_crash(CrashPoint::StepScheduled));
        assert!(harness.should_crash_at(CrashPoint::StepScheduled, CrashPosition::Before));
        assert!(!harness.should_crash_at(CrashPoint::StepScheduled, CrashPosition::After));
        assert!(!harness.should_crash(CrashPoint::DedupeWrite));
    }

    #[test]
    fn verification_harness_crash_at_same_point_different_position() {
        let harness_before = VerificationHarness::with_crash_scenario(
            CrashPoint::EffectPrepared,
            CrashPosition::Before,
        );
        let harness_after = VerificationHarness::with_crash_scenario(
            CrashPoint::EffectPrepared,
            CrashPosition::After,
        );

        assert!(harness_before.should_crash_at(CrashPoint::EffectPrepared, CrashPosition::Before));
        assert!(!harness_before.should_crash_at(CrashPoint::EffectPrepared, CrashPosition::After));

        assert!(harness_after.should_crash_at(CrashPoint::EffectPrepared, CrashPosition::After));
        assert!(!harness_after.should_crash_at(CrashPoint::EffectPrepared, CrashPosition::Before));
    }
}

#[cfg(test)]
mod crash_point_matrix_tests {
    use super::*;

    fn workflow_completion_sequence() -> Vec<EventEnvelope> {
        vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        ]
    }

    fn verify_all_crash_points_determinism(events: &[EventEnvelope]) {
        let ctx = RecoveryContext::new();
        let results = ctx.verify_all_crash_points(events, events.len());

        assert_eq!(results.len(), 24, "Should have 24 crash scenarios");

        for (i, result) in results.iter().enumerate() {
            assert!(
                result.is_ok(),
                "Crash scenario {} failed: {:?}",
                i,
                result.as_ref().err()
            );
        }
    }

    #[test]
    fn all_crash_points_for_completion_sequence() {
        let events = workflow_completion_sequence();
        verify_all_crash_points_determinism(&events);
    }

    #[test]
    fn all_crash_points_for_failure_sequence() {
        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
        ];
        verify_all_crash_points_determinism(&events);
    }

    #[test]
    fn all_crash_points_for_timer_sequence() {
        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, timer_set_payload("wf-1", "timer-1")),
            make_event("inst-1", 5, timer_fired_payload("wf-1", "timer-1")),
            make_event("inst-1", 6, step_completed_payload("wf-1", "step-1")),
        ];
        verify_all_crash_points_determinism(&events);
    }

    #[test]
    fn all_crash_points_for_cancellation_sequence() {
        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, cancel_requested_payload("wf-1")),
        ];
        verify_all_crash_points_determinism(&events);
    }

    #[test]
    fn all_crash_points_for_resume_sequence() {
        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, workflow_failed_payload("wf-1")),
            make_event("inst-1", 3, instance_resumed_payload("wf-1")),
            make_event("inst-1", 4, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 5, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 6, step_completed_payload("wf-1", "step-1")),
        ];
        verify_all_crash_points_determinism(&events);
    }
}

#[cfg(test)]
mod recovery_assertion_tests {
    use super::*;

    #[test]
    fn recovery_assertion_new() {
        let scenario = CrashScenario::new(CrashPoint::DedupeWrite, CrashPosition::Before);
        let assertion = RecoveryAssertion::new(scenario);

        assert_eq!(assertion.crash_point(), CrashPoint::DedupeWrite);
        assert!(assertion.pre_crash_state().is_none());
        assert!(assertion.post_crash_state().is_none());
    }

    #[test]
    fn recovery_assertion_with_pre_crash() {
        let engine = ReplayEngine::new();
        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        ];

        let scenario = CrashScenario::new(CrashPoint::DedupeWrite, CrashPosition::Before);
        let assertion = RecoveryAssertion::new(scenario)
            .with_pre_crash(&events, &engine)
            .expect("Pre-crash replay should succeed");

        assert!(assertion.pre_crash_state().is_some());
        assert_eq!(assertion.events_applied_pre(), 2);
    }

    #[test]
    fn recovery_assertion_deterministic_same_events() {
        let engine = ReplayEngine::new();
        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        ];

        let scenario = CrashScenario::new(CrashPoint::StepScheduled, CrashPosition::Before);
        let assertion = RecoveryAssertion::new(scenario)
            .with_pre_crash(&events, &engine)
            .expect("pre-crash should succeed")
            .with_post_crash(&events, &engine)
            .expect("post-crash should succeed");

        assert!(assertion.assert_deterministic().is_ok());
    }

    #[test]
    fn recovery_assertion_detects_state_mismatch() {
        let engine = ReplayEngine::new();
        let events_pre = vec![make_event("inst-1", 1, workflow_started_payload("wf-1"))];
        let events_post = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        ];

        let scenario = CrashScenario::new(CrashPoint::DedupeWrite, CrashPosition::Before);
        let assertion = RecoveryAssertion::new(scenario)
            .with_pre_crash(&events_pre, &engine)
            .expect("pre-crash should succeed")
            .with_post_crash(&events_post, &engine)
            .expect("post-crash should succeed");

        // Different event counts should cause deterministic assertion to fail
        let result = assertion.assert_deterministic();
        assert!(result.is_err());
    }

    #[test]
    fn recovery_assertion_asserts_legal_state() {
        let engine = ReplayEngine::new();
        let events = vec![make_event("inst-1", 1, workflow_started_payload("wf-1"))];

        let scenario = CrashScenario::new(CrashPoint::DedupeWrite, CrashPosition::Before);
        let assertion = RecoveryAssertion::new(scenario)
            .with_pre_crash(&events, &engine)
            .expect("pre-crash should succeed")
            .with_post_crash(&events, &engine)
            .expect("post-crash should succeed");

        // RunningDecision is legal, so this should pass
        assert!(assertion.assert_legal_state().is_ok());
    }

    #[test]
    fn recovery_assertion_assert_no_regression() {
        let engine = ReplayEngine::new();
        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        ];

        let scenario = CrashScenario::new(CrashPoint::DedupeWrite, CrashPosition::Before);
        let assertion = RecoveryAssertion::new(scenario)
            .with_pre_crash(&events, &engine)
            .expect("pre-crash should succeed")
            .with_post_crash(&events, &engine)
            .expect("post-crash should succeed");

        assert!(assertion.assert_no_regression().is_ok());
    }

    #[test]
    fn recovery_assertion_detects_event_regression() {
        let engine = ReplayEngine::new();
        let events_pre = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        ];
        let events_post = vec![make_event("inst-1", 1, workflow_started_payload("wf-1"))];

        let scenario = CrashScenario::new(CrashPoint::DedupeWrite, CrashPosition::Before);
        let assertion = RecoveryAssertion::new(scenario)
            .with_pre_crash(&events_pre, &engine)
            .expect("pre-crash should succeed")
            .with_post_crash(&events_post, &engine)
            .expect("post-crash should succeed");

        // Post has fewer events, should fail no_regression
        assert!(assertion.assert_no_regression().is_err());
    }
}

#[cfg(test)]
mod invariant_verification_tests {
    use super::*;

    #[test]
    fn no_duplicate_effects_in_sequence() {
        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        ];

        assert!(assert_no_duplicate_effects(&events).is_ok());
    }

    #[test]
    fn no_duplicate_effects_detects_duplicates() {
        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        ];

        assert!(assert_no_duplicate_effects(&events).is_err());
    }

    #[test]
    fn fence_token_ordering_valid() {
        let acquisitions = vec![(1, "token-1".to_string()), (3, "token-2".to_string())];
        let completions = vec![(2, "token-1".to_string()), (4, "token-2".to_string())];

        assert!(assert_fence_token_ordering(&acquisitions, &completions).is_ok());
    }

    #[test]
    fn fence_token_ordering_rejects_stale() {
        let acquisitions = vec![(3, "token-1".to_string())];
        let completions = vec![(2, "token-1".to_string())];

        let result = assert_fence_token_ordering(&acquisitions, &completions);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Stale fence completion"));
    }
}

#[cfg(test)]
mod crash_scenario_exhaustiveness_tests {
    use super::*;

    #[test]
    fn all_crash_points_count() {
        assert_eq!(CrashPoint::all_variants().len(), 12);
    }

    #[test]
    fn all_positions_count() {
        assert_eq!(CrashPosition::all().len(), 2);
    }

    #[test]
    fn all_scenarios_count() {
        let scenarios = CrashScenario::all_scenarios();
        assert_eq!(scenarios.len(), 24);
    }

    #[test]
    fn each_crash_point_has_both_positions() {
        for point in CrashPoint::all_variants() {
            let before = CrashScenario::new(*point, CrashPosition::Before);
            let after = CrashScenario::new(*point, CrashPosition::After);
            assert_ne!(
                before,
                after,
                "{} should have distinct Before/After",
                point.name()
            );
        }
    }

    #[test]
    fn crash_point_classifications() {
        // Effect-related
        assert!(CrashPoint::EffectPrepared.is_effect_related());
        assert!(CrashPoint::ConnectorCommit.is_effect_related());
        assert!(CrashPoint::EffectCommitted.is_effect_related());
        assert!(CrashPoint::Compensation.is_effect_related());
        assert!(!CrashPoint::StepScheduled.is_effect_related());

        // Step-related
        assert!(CrashPoint::StepScheduled.is_step_related());
        assert!(CrashPoint::FenceAcquisition.is_step_related());
        assert!(CrashPoint::ChildStart.is_step_related());
        assert!(CrashPoint::StepCompleted.is_step_related());
        assert!(!CrashPoint::EffectPrepared.is_step_related());

        // External-related
        assert!(CrashPoint::TimerPersistence.is_external_related());
        assert!(CrashPoint::SignalAcceptance.is_external_related());
        assert!(CrashPoint::LineageRollover.is_external_related());
        assert!(!CrashPoint::StepScheduled.is_external_related());
    }

    #[test]
    fn crash_point_names() {
        assert_eq!(CrashPoint::DedupeWrite.name(), "DedupeWrite");
        assert_eq!(CrashPoint::StepScheduled.name(), "StepScheduled");
        assert_eq!(CrashPoint::FenceAcquisition.name(), "FenceAcquisition");
        assert_eq!(CrashPoint::ChildStart.name(), "ChildStart");
        assert_eq!(CrashPoint::EffectPrepared.name(), "EffectPrepared");
        assert_eq!(CrashPoint::ConnectorCommit.name(), "ConnectorCommit");
        assert_eq!(CrashPoint::EffectCommitted.name(), "EffectCommitted");
        assert_eq!(CrashPoint::StepCompleted.name(), "StepCompleted");
        assert_eq!(CrashPoint::TimerPersistence.name(), "TimerPersistence");
        assert_eq!(CrashPoint::SignalAcceptance.name(), "SignalAcceptance");
        assert_eq!(CrashPoint::LineageRollover.name(), "LineageRollover");
        assert_eq!(CrashPoint::Compensation.name(), "Compensation");
    }

    #[test]
    fn crash_scenario_display() {
        let scenario = CrashScenario::new(CrashPoint::DedupeWrite, CrashPosition::Before);
        assert_eq!(format!("{}", scenario), "DedupeWrite/Before");

        let scenario2 = CrashScenario::new(CrashPoint::StepCompleted, CrashPosition::After);
        assert_eq!(format!("{}", scenario2), "StepCompleted/After");
    }
}

#[cfg(test)]
mod recovery_context_tests {
    use super::*;

    #[test]
    fn recovery_context_new() {
        let ctx = RecoveryContext::new();
        assert!(!ctx.harness().should_crash(CrashPoint::DedupeWrite));
    }

    #[test]
    fn recovery_context_replay() {
        let ctx = RecoveryContext::new();
        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        ];

        let result = ctx.replay(&events);
        assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
        assert_eq!(result.events_applied, 2);
    }
}
