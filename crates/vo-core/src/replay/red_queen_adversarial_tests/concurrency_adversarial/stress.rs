use super::super::*;
use std::sync::Arc;
use vo_types::events::EventEnvelope;
use vo_types::state::LifecycleState;

fn build_completed_lifecycle(instance_id: &str, start_seq: u64) -> Vec<EventEnvelope> {
    vec![
        make_event(instance_id, start_seq, workflow_started_payload("wf-1")),
        make_event(
            instance_id,
            start_seq + 1,
            step_scheduled_payload("wf-1", "step-1"),
        ),
        make_event(
            instance_id,
            start_seq + 2,
            step_started_payload("wf-1", "step-1"),
        ),
        make_event(
            instance_id,
            start_seq + 3,
            step_completed_payload("wf-1", "step-1"),
        ),
    ]
}

fn build_failed_then_resumed_lifecycle(instance_id: &str) -> Vec<EventEnvelope> {
    vec![
        make_event(instance_id, 1, workflow_started_payload("wf-1")),
        make_event(instance_id, 2, step_scheduled_payload("wf-1", "step-1")),
        make_event(instance_id, 3, step_started_payload("wf-1", "step-1")),
        make_event(instance_id, 4, step_failed_payload("wf-1", "step-1")),
        make_event(instance_id, 5, instance_resumed_payload("wf-1")),
    ]
}

#[test]
fn replay_concurrent_large_sequences_independent_results() {
    let engine = Arc::new(ReplayEngine::new());

    let build_large = |instance_id: &str, num_steps: usize| -> Vec<EventEnvelope> {
        let mut events = Vec::new();
        events.push(make_event(instance_id, 1, workflow_started_payload("wf-1")));
        let mut seq = 2u64;
        for step in 0..num_steps {
            let step_id = format!("step-{}", step);
            events.push(make_event(
                instance_id,
                seq,
                step_scheduled_payload("wf-1", &step_id),
            ));
            events.push(make_event(
                instance_id,
                seq + 1,
                step_started_payload("wf-1", &step_id),
            ));
            events.push(make_event(
                instance_id,
                seq + 2,
                step_failed_payload("wf-1", &step_id),
            ));
            events.push(make_event(
                instance_id,
                seq + 3,
                instance_resumed_payload("wf-1"),
            ));
            seq += 4;
        }
        events
    };

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let engine = Arc::clone(&engine);
            std::thread::spawn(move || {
                let instance_id = format!("inst-{}", i);
                let events = build_large(&instance_id, 125);
                engine.replay(&events)
            })
        })
        .collect();

    for (i, handle) in handles.into_iter().enumerate() {
        let result = handle
            .join()
            .expect("thread should not panic")
            .expect("replay should succeed");
        assert_eq!(
            result.events_applied, 501,
            "thread {} should apply all events",
            i
        );
    }
}

#[test]
fn replay_concurrent_error_streams_dont_crash_others() {
    let engine = Arc::new(ReplayEngine::new());

    let good_handle = {
        let engine = Arc::clone(&engine);
        std::thread::spawn(move || {
            let events = build_completed_lifecycle("inst-good", 1);
            engine.replay(&events)
        })
    };

    let bad_handle = {
        let engine = Arc::clone(&engine);
        std::thread::spawn(move || {
            let events = [
                make_event("inst-bad", 1, workflow_started_payload("wf-1")),
                make_event("inst-bad", 3, step_scheduled_payload("wf-1", "step-1")),
            ];
            engine.replay(&events)
        })
    };

    let good_result = good_handle
        .join()
        .expect("thread should not panic")
        .expect("good stream should succeed");
    assert_eq!(good_result.final_state, Some(LifecycleState::Completed));

    let bad_result = bad_handle.join().expect("thread should not panic");
    assert!(bad_result.is_err(), "bad stream should fail");
    assert!(matches!(
        bad_result.unwrap_err(),
        ReplayError::SequenceGap { .. }
    ));
}

#[test]
fn replay_concurrent_with_instance_mismatch_errors() {
    let engine = Arc::new(ReplayEngine::new());

    let handles: Vec<_> = (0..8)
        .map(|i| {
            let engine = Arc::clone(&engine);
            std::thread::spawn(move || {
                if i % 2 == 0 {
                    let events = build_completed_lifecycle(&format!("inst-{}", i), 1);
                    engine.replay(&events)
                } else {
                    let events = [
                        make_event("inst-mismatch-a", 1, workflow_started_payload("wf-1")),
                        make_event(
                            "inst-mismatch-b",
                            2,
                            step_scheduled_payload("wf-1", "step-1"),
                        ),
                    ];
                    engine.replay(&events)
                }
            })
        })
        .collect();

    for (i, handle) in handles.into_iter().enumerate() {
        let result = handle.join().expect("thread should not panic");
        if i % 2 == 0 {
            assert!(result.is_ok(), "even thread {} should succeed", i);
        } else {
            assert!(
                result.is_err(),
                "odd thread {} should fail with mismatch",
                i
            );
        }
    }
}

#[test]
fn replay_concurrent_with_payload_decode_errors() {
    let engine = Arc::new(ReplayEngine::new());

    let good_handle = {
        let engine = Arc::clone(&engine);
        std::thread::spawn(move || {
            let events = build_completed_lifecycle("inst-good", 1);
            engine.replay(&events)
        })
    };

    let corrupt_handle = {
        let engine = Arc::clone(&engine);
        std::thread::spawn(move || {
            let events = [
                make_event("inst-corrupt", 1, workflow_started_payload("wf-1")),
                make_event(
                    "inst-corrupt",
                    2,
                    serde_json::json!({"type": "GarbageType", "workflow_id": "wf-1", "version": 1}),
                ),
            ];
            engine.replay(&events)
        })
    };

    let good_result = good_handle
        .join()
        .expect("thread should not panic")
        .expect("good should succeed");
    assert_eq!(good_result.final_state, Some(LifecycleState::Completed));

    let corrupt_result = corrupt_handle.join().expect("thread should not panic");
    assert!(corrupt_result.is_err());
}

#[test]
fn replay_concurrent_interleaved_failure_recovery_cycles() {
    let engine = Arc::new(ReplayEngine::new());

    let build_multi_recovery = |instance_id: &str, cycles: usize| -> Vec<EventEnvelope> {
        let mut events = Vec::new();
        events.push(make_event(instance_id, 1, workflow_started_payload("wf-1")));
        let mut seq = 2u64;
        for _ in 0..cycles {
            events.push(make_event(
                instance_id,
                seq,
                step_scheduled_payload("wf-1", "step-1"),
            ));
            events.push(make_event(
                instance_id,
                seq + 1,
                step_started_payload("wf-1", "step-1"),
            ));
            events.push(make_event(
                instance_id,
                seq + 2,
                step_failed_payload("wf-1", "step-1"),
            ));
            events.push(make_event(
                instance_id,
                seq + 3,
                instance_resumed_payload("wf-1"),
            ));
            seq += 4;
        }
        events
    };

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let engine = Arc::clone(&engine);
            std::thread::spawn(move || {
                let instance_id = format!("inst-{}", i);
                let events = build_multi_recovery(&instance_id, 5);
                engine.replay(&events)
            })
        })
        .collect();

    for handle in handles {
        let result = handle
            .join()
            .expect("thread should not panic")
            .expect("multi-recovery replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
        assert_eq!(result.events_applied, 1 + 5 * 4);
    }
}

#[test]
fn replay_shared_engine_produces_deterministic_results_across_threads() {
    let engine = Arc::new(ReplayEngine::new());
    let events = build_failed_then_resumed_lifecycle("inst-1");
    let events = Arc::new(events);

    let num_trials = 10;
    let mut all_results = Vec::new();

    for _ in 0..num_trials {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let engine = Arc::clone(&engine);
                let events = Arc::clone(&events);
                std::thread::spawn(move || engine.replay(&events))
            })
            .collect();

        for handle in handles {
            let result = handle
                .join()
                .expect("thread should not panic")
                .expect("replay should succeed");
            all_results.push(result);
        }
    }

    let first = &all_results[0];
    for result in &all_results {
        assert_eq!(
            result, first,
            "all concurrent replays must produce identical results"
        );
    }
}

#[test]
fn replay_concurrent_with_continued_as_new_interleaved() {
    let engine = Arc::new(ReplayEngine::new());

    let h1 = {
        let engine = Arc::clone(&engine);
        std::thread::spawn(move || {
            let events = [
                make_event("inst-a", 1, workflow_started_payload("wf-1")),
                make_event("inst-a", 2, continued_as_new_payload("wf-1")),
                make_event("inst-a", 3, step_scheduled_payload("wf-1", "step-1")),
                make_event("inst-a", 4, step_started_payload("wf-1", "step-1")),
                make_event("inst-a", 5, step_completed_payload("wf-1", "step-1")),
            ];
            engine.replay(&events)
        })
    };

    let h2 = {
        let engine = Arc::clone(&engine);
        std::thread::spawn(move || {
            let events = build_completed_lifecycle("inst-b", 1);
            engine.replay(&events)
        })
    };

    let r1 = h1
        .join()
        .expect("thread should not panic")
        .expect("replay should succeed");
    assert_eq!(r1.final_state, Some(LifecycleState::Completed));
    assert_eq!(r1.events_applied, 5);

    let r2 = h2
        .join()
        .expect("thread should not panic")
        .expect("replay should succeed");
    assert_eq!(r2.final_state, Some(LifecycleState::Completed));
    assert_eq!(r2.events_applied, 4);
}

#[test]
fn replay_concurrent_does_not_observably_share_state() {
    let engine = Arc::new(ReplayEngine::new());

    let build_failure_recovery = |instance_id: &str, cycles: usize| -> Vec<EventEnvelope> {
        let mut events = Vec::new();
        events.push(make_event(instance_id, 1, workflow_started_payload("wf-1")));
        let mut seq = 2u64;
        for _ in 0..cycles {
            events.push(make_event(
                instance_id,
                seq,
                step_scheduled_payload("wf-1", "step-1"),
            ));
            events.push(make_event(
                instance_id,
                seq + 1,
                step_started_payload("wf-1", "step-1"),
            ));
            events.push(make_event(
                instance_id,
                seq + 2,
                step_failed_payload("wf-1", "step-1"),
            ));
            events.push(make_event(
                instance_id,
                seq + 3,
                instance_resumed_payload("wf-1"),
            ));
            seq += 4;
        }
        events
    };

    let h1 = {
        let engine = Arc::clone(&engine);
        std::thread::spawn(move || {
            let events = build_failure_recovery("inst-short", 2);
            engine.replay(&events)
        })
    };

    let h2 = {
        let engine = Arc::clone(&engine);
        std::thread::spawn(move || {
            let events = build_failure_recovery("inst-long", 50);
            engine.replay(&events)
        })
    };

    let r1 = h1
        .join()
        .expect("thread should not panic")
        .expect("short replay should succeed");
    assert_eq!(r1.events_applied, 9);
    assert_eq!(r1.final_state, Some(LifecycleState::RunningDecision));

    let r2 = h2
        .join()
        .expect("thread should not panic")
        .expect("long replay should succeed");
    assert_eq!(r2.events_applied, 201);
    assert_eq!(r2.final_state, Some(LifecycleState::RunningDecision));
}
