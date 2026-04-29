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

fn build_cancelled_lifecycle(instance_id: &str) -> Vec<EventEnvelope> {
    vec![
        make_event(instance_id, 1, workflow_started_payload("wf-1")),
        make_event(instance_id, 2, step_scheduled_payload("wf-1", "step-1")),
        make_event(instance_id, 3, cancel_requested_payload("wf-1")),
    ]
}

#[test]
fn replay_is_safe_under_concurrent_identical_streams() {
    let engine = Arc::new(ReplayEngine::new());
    let events = build_completed_lifecycle("inst-1", 1);
    let events = Arc::new(events);

    let handles: Vec<_> = (0..8)
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
        assert_eq!(result.final_state, Some(LifecycleState::Completed));
        assert_eq!(result.events_applied, 4);
    }
}

#[test]
fn replay_is_safe_under_concurrent_different_streams() {
    let engine = Arc::new(ReplayEngine::new());

    let handles: Vec<_> = (0..8)
        .map(|i| {
            let engine = Arc::clone(&engine);
            std::thread::spawn(move || {
                let instance_id = format!("inst-{}", i);
                let events = match i % 3 {
                    0 => build_completed_lifecycle(&instance_id, 1),
                    1 => build_failed_then_resumed_lifecycle(&instance_id),
                    _ => build_cancelled_lifecycle(&instance_id),
                };
                engine.replay(&events)
            })
        })
        .collect();

    let results: Vec<_> = handles
        .into_iter()
        .map(|h| {
            h.join()
                .expect("thread should not panic")
                .expect("replay should succeed")
        })
        .collect();

    assert_eq!(results.len(), 8);
    for (i, result) in results.into_iter().enumerate() {
        match i % 3 {
            0 => {
                assert_eq!(result.final_state, Some(LifecycleState::Completed));
                assert_eq!(result.events_applied, 4);
            }
            1 => {
                assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
                assert_eq!(result.events_applied, 5);
            }
            _ => {
                assert_eq!(result.final_state, Some(LifecycleState::Cancelled));
                assert_eq!(result.events_applied, 3);
            }
        }
    }
}

#[test]
fn replay_concurrent_empty_and_nonempty_streams() {
    let engine = Arc::new(ReplayEngine::new());

    let empty_engine = Arc::clone(&engine);
    let empty_handle = std::thread::spawn(move || {
        let empty: Vec<EventEnvelope> = vec![];
        empty_engine.replay(&empty)
    });

    let full_engine = Arc::clone(&engine);
    let full_handle = std::thread::spawn(move || {
        let events = build_completed_lifecycle("inst-1", 1);
        full_engine.replay(&events)
    });

    let empty_result = empty_handle
        .join()
        .expect("thread should not panic")
        .expect("empty replay should succeed");
    assert_eq!(empty_result.final_state, None);
    assert_eq!(empty_result.events_applied, 0);

    let full_result = full_handle
        .join()
        .expect("thread should not panic")
        .expect("full replay should succeed");
    assert_eq!(full_result.final_state, Some(LifecycleState::Completed));
    assert_eq!(full_result.events_applied, 4);
}

#[test]
fn replay_concurrent_determinism_under_contention() {
    let engine = Arc::new(ReplayEngine::new());
    let events = build_completed_lifecycle("inst-1", 1);
    let events = Arc::new(events);
    let expected = engine
        .replay(&events)
        .clone()
        .expect("baseline should succeed");

    let num_threads = 16;
    let results: Vec<_> = (0..num_threads)
        .map(|_| {
            let engine = Arc::clone(&engine);
            let events = Arc::clone(&events);
            std::thread::spawn(move || engine.replay(&events))
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|h| {
            h.join()
                .expect("thread should not panic")
                .expect("replay should succeed")
        })
        .collect();

    for result in &results {
        assert_eq!(
            result, &expected,
            "concurrent replay must produce identical results"
        );
    }
}

#[test]
fn replay_concurrent_with_different_sequence_starts() {
    let engine = Arc::new(ReplayEngine::new());

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let engine = Arc::clone(&engine);
            std::thread::spawn(move || {
                let start = (i as u64) * 1000 + 1;
                let events = build_completed_lifecycle("inst-common", start);
                engine.replay(&events)
            })
        })
        .collect();

    for handle in handles {
        let result = handle
            .join()
            .expect("thread should not panic")
            .expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Completed));
        assert_eq!(result.events_applied, 4);
    }
}

#[test]
fn replay_concurrent_stress_many_threads_short_sequences() {
    let engine = Arc::new(ReplayEngine::new());
    let num_threads = 32;

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let engine = Arc::clone(&engine);
            std::thread::spawn(move || {
                let instance_id = format!("inst-{}", i);
                let events = build_completed_lifecycle(&instance_id, 1);
                engine.replay(&events)
            })
        })
        .collect();

    let mut success_count = 0;
    for handle in handles {
        let result = handle
            .join()
            .expect("thread should not panic")
            .expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Completed));
        assert_eq!(result.events_applied, 4);
        success_count += 1;
    }
    assert_eq!(success_count, num_threads);
}
