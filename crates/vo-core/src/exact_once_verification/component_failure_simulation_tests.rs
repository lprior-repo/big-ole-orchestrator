//! Component failure simulation tests for vo-core.
//!
//! This module tests the system's behavior when components fail during execution,
//! verifying that data integrity is maintained across various failure scenarios.
//!
//! ## Test Coverage
//!
//! 1. **Step Component Failures**: Step scheduling, execution, and completion failures
//! 2. **Timer Component Failures**: Timer scheduling and firing failures  
//! 3. **Signal Component Failures**: Signal acceptance and routing failures
//! 4. **Child Workflow Failures**: Child workflow start failures
//! 5. **Effect Component Failures**: Effect preparation, commitment, and compensation failures
//! 6. **Data Integrity**: Verifies no data loss or corruption after failures

use vo_types::events::EventEnvelope;
use vo_types::state::LifecycleState;

use crate::exact_once_verification::assertions::{RecoveryAssertion, RecoveryContext};
use crate::exact_once_verification::crash_points::{CrashPoint, CrashPosition, CrashScenario};
use crate::exact_once_verification::harness::VerificationHarness;
use crate::replay::test_helpers::*;
use crate::replay::ReplayEngine;

#[cfg(test)]
mod step_component_failure_tests {
    use super::*;

    fn workflow_with_step_sequence() -> Vec<EventEnvelope> {
        vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        ]
    }

    fn workflow_with_failing_step() -> Vec<EventEnvelope> {
        vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
        ]
    }

    #[test]
    fn step_failure_maintains_deterministic_state() {
        let events = workflow_with_failing_step();
        let engine = ReplayEngine::new();

        let result = engine.replay(&events);
        assert!(result.is_ok());
        assert!(result.unwrap().final_state.is_some());
    }

    #[test]
    fn step_scheduling_failure_recovery() {
        let events_before_failure = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        ];

        let engine = ReplayEngine::new();
        let result = engine.replay(&events_before_failure);
        assert!(result.is_ok());

        let state = result.unwrap().final_state;
        assert!(state.is_some());
    }

    #[test]
    fn step_completion_crash_before_effect_prepared() {
        let scenario = CrashScenario::new(CrashPoint::EffectPrepared, CrashPosition::Before);
        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        ];

        let ctx = RecoveryContext::new();
        let result = ctx.verify_at_point(scenario, &events, &events);
        assert!(
            result.is_ok(),
            "Should recover deterministically: {:?}",
            result.err()
        );
    }

    #[test]
    fn step_failure_at_fence_acquisition() {
        let scenario = CrashScenario::new(CrashPoint::FenceAcquisition, CrashPosition::Before);
        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        ];

        let ctx = RecoveryContext::new();
        let result = ctx.verify_at_point(scenario, &events, &events);
        assert!(
            result.is_ok(),
            "Fence acquisition failure should be recoverable: {:?}",
            result.err()
        );
    }
}

#[cfg(test)]
mod timer_component_failure_tests {
    use super::*;

    fn workflow_with_timer_sequence() -> Vec<EventEnvelope> {
        vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, timer_set_payload("wf-1", "timer-1")),
            make_event("inst-1", 4, timer_fired_payload("wf-1", "timer-1")),
            make_event("inst-1", 5, step_completed_payload("wf-1", "step-1")),
        ]
    }

    #[test]
    fn timer_persistence_failure_recovery() {
        let scenario = CrashScenario::new(CrashPoint::TimerPersistence, CrashPosition::Before);
        let events = workflow_with_timer_sequence();

        let ctx = RecoveryContext::new();
        let result = ctx.verify_at_point(scenario, &events, &events);
        assert!(
            result.is_ok(),
            "Timer persistence failure should be recoverable"
        );
    }

    #[test]
    fn timer_fired_before_step_completion() {
        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, timer_set_payload("wf-1", "timer-1")),
            make_event("inst-1", 4, timer_fired_payload("wf-1", "timer-1")),
        ];

        let engine = ReplayEngine::new();
        let result = engine.replay(&events);
        assert!(result.is_ok());
    }

    #[test]
    fn timer_crash_before_persistence() {
        let events_before_crash = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, timer_set_payload("wf-1", "timer-1")),
        ];

        let scenario = CrashScenario::new(CrashPoint::TimerPersistence, CrashPosition::Before);
        let ctx = RecoveryContext::new();
        let result = ctx.verify_at_point(scenario, &events_before_crash, &events_before_crash);
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod signal_component_failure_tests {
    use super::*;

    fn workflow_with_signal_sequence() -> Vec<EventEnvelope> {
        vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        ]
    }

    #[test]
    fn signal_acceptance_failure_recovery() {
        let scenario = CrashScenario::new(CrashPoint::SignalAcceptance, CrashPosition::Before);
        let events = workflow_with_signal_sequence();

        let ctx = RecoveryContext::new();
        let result = ctx.verify_at_point(scenario, &events, &events);
        assert!(
            result.is_ok(),
            "Signal acceptance failure should be recoverable"
        );
    }

    #[test]
    fn signal_routing_after_lineage_rollover() {
        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, continued_as_new_payload("wf-1")),
            make_event("inst-1", 3, workflow_started_payload("wf-1")),
        ];

        let engine = ReplayEngine::new();
        let result = engine.replay(&events);
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod effect_component_failure_tests {
    use super::*;

    fn workflow_with_effect_sequence() -> Vec<EventEnvelope> {
        vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event(
                "inst-1",
                4,
                effect_prepared_payload("wf-1", "step-1", "effect-1"),
            ),
            make_event(
                "inst-1",
                5,
                effect_committed_payload("wf-1", "step-1", "effect-1"),
            ),
            make_event("inst-1", 6, step_completed_payload("wf-1", "step-1")),
        ]
    }

    #[test]
    fn effect_prepared_failure_recovery() {
        let scenario = CrashScenario::new(CrashPoint::EffectPrepared, CrashPosition::After);
        let events = workflow_with_effect_sequence();

        let ctx = RecoveryContext::new();
        let result = ctx.verify_at_point(scenario, &events, &events);
        assert!(
            result.is_ok(),
            "Effect prepared failure should be recoverable"
        );
    }

    #[test]
    fn connector_commit_failure_recovery() {
        let scenario = CrashScenario::new(CrashPoint::ConnectorCommit, CrashPosition::Before);
        let events = workflow_with_effect_sequence();

        let ctx = RecoveryContext::new();
        let result = ctx.verify_at_point(scenario, &events, &events);
        assert!(
            result.is_ok(),
            "Connector commit failure should be recoverable"
        );
    }

    #[test]
    fn effect_committed_failure_recovery() {
        let scenario = CrashScenario::new(CrashPoint::EffectCommitted, CrashPosition::After);
        let events = workflow_with_effect_sequence();

        let ctx = RecoveryContext::new();
        let result = ctx.verify_at_point(scenario, &events, &events);
        assert!(
            result.is_ok(),
            "Effect committed failure should be recoverable"
        );
    }

    #[test]
    fn compensation_after_effect_failure() {
        let events_with_compensation = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event(
                "inst-1",
                4,
                effect_prepared_payload("wf-1", "step-1", "effect-1"),
            ),
        ];

        let scenario = CrashScenario::new(CrashPoint::Compensation, CrashPosition::Before);
        let ctx = RecoveryContext::new();
        let result = ctx.verify_at_point(
            scenario,
            &events_with_compensation,
            &events_with_compensation,
        );
        assert!(result.is_ok(), "Compensation failure should be recoverable");
    }
}

#[cfg(test)]
mod dedupe_component_failure_tests {
    use super::*;

    #[test]
    fn dedupe_write_failure_recovery() {
        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        ];

        let scenario = CrashScenario::new(CrashPoint::DedupeWrite, CrashPosition::Before);
        let ctx = RecoveryContext::new();
        let result = ctx.verify_at_point(scenario, &events, &events);
        assert!(result.is_ok(), "Dedupe write failure should be recoverable");
    }

    #[test]
    fn duplicate_event_detection_integrity() {
        let events_with_dup = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        ];

        let assertion = crate::exact_once_verification::assertions::assert_no_duplicate_effects(
            &events_with_dup,
        );
        assert!(assertion.is_err(), "Duplicate events should be detected");
    }
}

#[cfg(test)]
mod lineage_rollover_failure_tests {
    use super::*;

    #[test]
    fn lineage_rollover_failure_recovery() {
        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, continued_as_new_payload("wf-1")),
            make_event("inst-1", 3, workflow_started_payload("wf-1")),
            make_event("inst-1", 4, step_scheduled_payload("wf-1", "step-1")),
        ];

        let scenario = CrashScenario::new(CrashPoint::LineageRollover, CrashPosition::After);
        let ctx = RecoveryContext::new();
        let result = ctx.verify_at_point(scenario, &events, &events);
        assert!(
            result.is_ok(),
            "Lineage rollover failure should be recoverable"
        );
    }

    #[test]
    fn signal_routing_preserved_across_rollover() {
        let harness = VerificationHarness::new();
        let events_pre = vec![make_event("inst-1", 1, workflow_started_payload("wf-1"))];
        let rollover_event = make_event("inst-1", 2, continued_as_new_payload("wf-1"));
        let events_post = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, continued_as_new_payload("wf-1")),
            make_event("inst-1", 3, step_scheduled_payload("wf-1", "step-1")),
        ];

        let result = harness.verify_lineage_rollover_deterministic(
            &events_pre,
            &rollover_event,
            &events_post[1..],
        );
        assert!(result, "Lineage rollover should preserve signal routing");
    }
}

#[cfg(test)]
mod child_workflow_failure_tests {
    use super::*;

    #[test]
    fn child_start_failure_recovery() {
        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "parent-step")),
            make_event("inst-1", 3, step_started_payload("wf-1", "parent-step")),
        ];

        let scenario = CrashScenario::new(CrashPoint::ChildStart, CrashPosition::Before);
        let ctx = RecoveryContext::new();
        let result = ctx.verify_at_point(scenario, &events, &events);
        assert!(result.is_ok(), "Child start failure should be recoverable");
    }

    #[test]
    fn child_start_after_crash_recovery() {
        let events_before_crash = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "parent-step")),
        ];

        let events_after_crash = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "parent-step")),
            make_event("inst-1", 3, step_started_payload("wf-1", "parent-step")),
        ];

        let scenario = CrashScenario::new(CrashPoint::ChildStart, CrashPosition::After);
        let ctx = RecoveryContext::new();
        let result = ctx.verify_at_point(scenario, &events_before_crash, &events_after_crash);
        assert!(
            result.is_ok(),
            "Child start after crash should be recoverable"
        );
    }
}

#[cfg(test)]
mod data_integrity_verification_tests {
    use super::*;

    #[test]
    fn no_data_loss_after_step_failure() {
        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
            make_event("inst-1", 5, workflow_failed_payload("wf-1")),
        ];

        let engine = ReplayEngine::new();
        let result = engine.replay(&events);

        assert!(
            result.is_ok(),
            "Replay should succeed even with step failure"
        );
        assert_eq!(
            result.unwrap().events_applied,
            5,
            "All events should be applied"
        );
    }

    #[test]
    fn no_data_loss_after_timer_failure() {
        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, timer_set_payload("wf-1", "timer-1")),
        ];

        let engine = ReplayEngine::new();
        let result = engine.replay(&events);

        assert!(
            result.is_ok(),
            "Replay should succeed even with timer failure"
        );
    }

    #[test]
    fn state_legal_after_all_crash_points() {
        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        ];

        let ctx = RecoveryContext::new();
        let results = ctx.verify_all_crash_points(&events, events.len());

        for (i, result) in results.iter().enumerate() {
            assert!(
                result.is_ok(),
                "Crash scenario {} failed: {:?}",
                i,
                result.as_ref().err()
            );
        }
    }
}

#[cfg(test)]
mod integration_failure_scenario_tests {
    use super::*;

    #[test]
    fn full_workflow_lifecycle_with_failures() {
        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, timer_set_payload("wf-1", "timer-1")),
            make_event("inst-1", 5, timer_fired_payload("wf-1", "timer-1")),
            make_event(
                "inst-1",
                6,
                effect_prepared_payload("wf-1", "step-1", "effect-1"),
            ),
            make_event(
                "inst-1",
                7,
                effect_committed_payload("wf-1", "step-1", "effect-1"),
            ),
            make_event("inst-1", 8, step_completed_payload("wf-1", "step-1")),
        ];

        let engine = ReplayEngine::new();
        let result = engine.replay(&events);

        assert!(result.is_ok(), "Full workflow should replay successfully");
        assert_eq!(result.unwrap().events_applied, 8);
    }

    #[test]
    fn recovery_after_multiple_component_failures() {
        let instance1 = make_instance_id(1);
        let instance2 = make_instance_id(2);

        let events1 = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        ];

        let events2 = vec![
            make_event("inst-2", 1, workflow_started_payload("wf-2")),
            make_event("inst-2", 2, step_scheduled_payload("wf-2", "step-1")),
        ];

        let engine = ReplayEngine::new();

        let result1 = engine.replay(&events1);
        let result2 = engine.replay(&events2);

        assert!(result1.is_ok());
        assert!(result2.is_ok());
    }

    #[test]
    fn crash_at_step_scheduled_maintains_integrity() {
        let events_before_crash = vec![make_event("inst-1", 1, workflow_started_payload("wf-1"))];

        let events_after_crash = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        ];

        let scenario = CrashScenario::new(CrashPoint::StepScheduled, CrashPosition::Before);
        let ctx = RecoveryContext::new();

        let result = ctx.verify_at_point(scenario, &events_before_crash, &events_after_crash);
        assert!(result.is_ok(), "StepScheduled crash should be recoverable");
    }
}

fn make_instance_id(seed: u8) -> vo_types::InstanceId {
    vo_types::InstanceId::from_bytes([seed; 16])
}
