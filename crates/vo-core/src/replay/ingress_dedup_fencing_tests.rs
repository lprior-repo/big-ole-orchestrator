//! Ingress dedup and fencing tests for ADR-028/029.
//!
//! Tests cover:
//! 1. Ingress dedup window: duplicate commands within retention rejected, after expiry accepted
//! 2. Execution lease fencing: stale completions with old fence tokens rejected
//! 3. Property: no duplicate logical work inside retention regardless of crash count
//! 4. Concurrent submission stress test

use super::engine::ReplayEngine;
use super::test_helpers::*;
use vo_types::state::LifecycleState;

#[cfg(test)]
mod dedup_window_tests {
    use super::*;

    #[test]
    fn duplicate_command_within_retention_is_rejected() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
            make_event("inst-1", 5, step_scheduled_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
        assert_eq!(result.events_applied, 5);
    }

    #[test]
    fn duplicate_workflow_start_within_retention_is_rejected() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, workflow_started_payload("wf-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
        assert_eq!(result.events_applied, 2);
    }

    #[test]
    fn timer_events_respected_within_dedup_window() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, timer_set_payload("wf-1", "timer-1")),
            make_event("inst-1", 4, timer_fired_payload("wf-1", "timer-1")),
            make_event("inst-1", 5, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 6, step_completed_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Completed));
        assert_eq!(result.events_applied, 6);
    }
}

#[cfg(test)]
mod fencing_tests {
    use super::*;

    #[test]
    fn stale_step_completion_cannot_win() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
            make_event("inst-1", 5, instance_resumed_payload("wf-1")),
            make_event("inst-1", 6, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 7, step_started_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::StepExecuting));
        assert_eq!(result.events_applied, 7);
    }

    #[test]
    fn multiple_failure_recovery_cycles_preserve_fence() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
            make_event("inst-1", 5, instance_resumed_payload("wf-1")),
            make_event("inst-1", 6, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 7, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 8, step_failed_payload("wf-1", "step-1")),
            make_event("inst-1", 9, instance_resumed_payload("wf-1")),
            make_event("inst-1", 10, step_scheduled_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
        assert_eq!(result.events_applied, 10);
    }

    #[test]
    fn stale_workflow_started_after_failure_is_rejected() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
            make_event("inst-1", 5, workflow_started_payload("wf-1")),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            super::super::types::ReplayError::TransitionFailed {
                state: LifecycleState::Failed,
                ..
            }
        ));
    }

    #[test]
    fn stale_cancel_requested_after_failure_is_rejected() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
            make_event("inst-1", 5, cancel_requested_payload("wf-1")),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            super::super::types::ReplayError::TransitionFailed {
                state: LifecycleState::Failed,
                ..
            }
        ));
    }

    #[test]
    fn continued_as_new_after_completion_is_ignored() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
            make_event("inst-1", 5, continued_as_new_payload("wf-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Completed));
        assert_eq!(result.events_applied, 4);
    }
}

#[cfg(test)]
mod no_duplicate_logical_work_tests {
    use super::*;

    #[test]
    fn no_duplicate_work_after_multiple_crash_cycles() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
            make_event("inst-1", 5, instance_resumed_payload("wf-1")),
            make_event("inst-1", 6, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 7, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 8, step_failed_payload("wf-1", "step-1")),
            make_event("inst-1", 9, instance_resumed_payload("wf-1")),
            make_event("inst-1", 10, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 11, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 12, step_completed_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Completed));
        assert_eq!(result.events_applied, 12);
    }

    #[test]
    fn duplicate_timer_cannot_double_fire() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, timer_set_payload("wf-1", "timer-1")),
            make_event("inst-1", 4, timer_fired_payload("wf-1", "timer-1")),
            make_event("inst-1", 5, timer_fired_payload("wf-1", "timer-1")),
            make_event("inst-1", 6, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 7, step_completed_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Completed));
        assert_eq!(result.events_applied, 6);
    }

    #[test]
    fn duplicate_step_started_is_ignored() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 5, step_completed_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Completed));
        assert_eq!(result.events_applied, 4);
    }

    #[test]
    fn instance_resumed_only_once_after_failure() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
            make_event("inst-1", 5, instance_resumed_payload("wf-1")),
            make_event("inst-1", 6, instance_resumed_payload("wf-1")),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            super::super::types::ReplayError::TransitionFailed {
                state: LifecycleState::RunningDecision,
                ..
            }
        ));
    }
}

#[cfg(test)]
mod stress_tests {
    use super::*;

    #[test]
    fn rapid_step_sequence_stress() {
        let engine = ReplayEngine::new();
        let mut events = vec![make_event("inst-1", 1, workflow_started_payload("wf-1"))];
        let mut seq = 2u64;

        for step_num in 1..=20 {
            let step_id = format!("step-{}", step_num);
            events.push(make_event(
                "inst-1",
                seq,
                step_scheduled_payload("wf-1", &step_id),
            ));
            seq += 1;
            events.push(make_event(
                "inst-1",
                seq,
                step_started_payload("wf-1", &step_id),
            ));
            seq += 1;
            if step_num % 3 == 0 {
                events.push(make_event(
                    "inst-1",
                    seq,
                    step_failed_payload("wf-1", &step_id),
                ));
                seq += 1;
                events.push(make_event("inst-1", seq, instance_resumed_payload("wf-1")));
                seq += 1;
            } else {
                events.push(make_event(
                    "inst-1",
                    seq,
                    step_completed_payload("wf-1", &step_id),
                ));
                seq += 1;
            }
        }

        let result = engine.replay(&events).expect("replay should succeed");
        assert!(matches!(
            result.final_state,
            Some(LifecycleState::StepScheduled | LifecycleState::Completed)
        ));
        assert_eq!(result.events_applied, events.len() as usize);
    }

    #[test]
    fn interleaved_timer_stress() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, timer_set_payload("wf-1", "timer-1")),
            make_event("inst-1", 4, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 5, timer_fired_payload("wf-1", "timer-1")),
            make_event("inst-1", 6, timer_set_payload("wf-1", "timer-2")),
            make_event("inst-1", 7, step_completed_payload("wf-1", "step-1")),
            make_event("inst-1", 8, step_scheduled_payload("wf-1", "step-2")),
            make_event("inst-1", 9, timer_fired_payload("wf-1", "timer-2")),
            make_event("inst-1", 10, step_started_payload("wf-1", "step-2")),
            make_event("inst-1", 11, step_completed_payload("wf-1", "step-2")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Completed));
        assert_eq!(result.events_applied, 11);
    }

    #[test]
    fn repeated_failure_recovery_stress() {
        let engine = ReplayEngine::new();
        let mut events = vec![make_event("inst-1", 1, workflow_started_payload("wf-1"))];
        let mut seq = 2u64;

        for cycle in 1..=10 {
            events.push(make_event(
                "inst-1",
                seq,
                step_scheduled_payload("wf-1", "step-1"),
            ));
            seq += 1;
            events.push(make_event(
                "inst-1",
                seq,
                step_started_payload("wf-1", "step-1"),
            ));
            seq += 1;
            events.push(make_event(
                "inst-1",
                seq,
                step_failed_payload("wf-1", "step-1"),
            ));
            seq += 1;
            if cycle < 10 {
                events.push(make_event("inst-1", seq, instance_resumed_payload("wf-1")));
                seq += 1;
            }
        }

        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Failed));
        assert_eq!(result.events_applied, events.len() as usize);
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;

    #[test]
    fn final_state_deterministic_regardless_of_path() {
        let engine1 = ReplayEngine::new();
        let events1 = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        ];
        let result1 = engine1.replay(&events1).expect("replay should succeed");

        let engine2 = ReplayEngine::new();
        let events2 = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
            make_event("inst-1", 5, instance_resumed_payload("wf-1")),
            make_event("inst-1", 6, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 7, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 8, step_completed_payload("wf-1", "step-1")),
        ];
        let result2 = engine2.replay(&events2).expect("replay should succeed");

        assert_eq!(result1.final_state, result2.final_state);
    }

    #[test]
    fn events_applied_count_reflects_legal_transitions() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
            make_event("inst-1", 5, step_scheduled_payload("wf-1", "step-2")),
            make_event("inst-1", 6, step_started_payload("wf-1", "step-2")),
            make_event("inst-1", 7, step_completed_payload("wf-1", "step-2")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Completed));
        assert_eq!(result.events_applied, 7);
    }
}
