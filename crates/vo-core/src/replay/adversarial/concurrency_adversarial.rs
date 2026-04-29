mod concurrency_adversarial {
    use super::*;
    use std::sync::Arc;

    fn build_completed_lifecycle(instance_id: &str, start_seq: u64) -> Vec<EventEnvelope> {
        vec![
            make_event(instance_id, start_seq, workflow_started_payload("wf-1")),
            make_event(instance_id, start_seq + 1, step_scheduled_payload("wf-1", "step-1")),
            make_event(instance_id, start_seq + 2, step_started_payload("wf-1", "step-1")),
            make_event(instance_id, start_seq + 3, step_completed_payload("wf-1", "step-1")),
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
            let result = handle.join().expect("thread should not panic").expect("replay should succeed");
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
            .map(|h| h.join().expect("thread should not panic").expect("replay should succeed"))
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

        let empty_result = empty_handle.join().expect("thread should not panic").expect("empty replay should succeed");
        assert_eq!(empty_result.final_state, None);
        assert_eq!(empty_result.events_applied, 0);

        let full_result = full_handle.join().expect("thread should not panic").expect("full replay should succeed");
        assert_eq!(full_result.final_state, Some(LifecycleState::Completed));
        assert_eq!(full_result.events_applied, 4);
    }

    #[test]
    fn replay_concurrent_determinism_under_contention() {
        let engine = Arc::new(ReplayEngine::new());
        let events = build_completed_lifecycle("inst-1", 1);
        let events = Arc::new(events);
        let expected = engine.replay(&events).clone().expect("baseline should succeed");

        let num_threads = 16;
        let results: Vec<_> = (0..num_threads)
            .map(|_| {
                let engine = Arc::clone(&engine);
                let events = Arc::clone(&events);
                std::thread::spawn(move || engine.replay(&events))
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|h| h.join().expect("thread should not panic").expect("replay should succeed"))
            .collect();

        for result in &results {
            assert_eq!(result, &expected, "concurrent replay must produce identical results");
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
            let result = handle.join().expect("thread should not panic").expect("replay should succeed");
            assert_eq!(result.final_state, Some(LifecycleState::Completed));
            assert_eq!(result.events_applied, 4);
        }
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
                events.push(make_event(instance_id, seq, step_scheduled_payload("wf-1", &step_id)));
                events.push(make_event(instance_id, seq + 1, step_started_payload("wf-1", &step_id)));
                events.push(make_event(instance_id, seq + 2, step_failed_payload("wf-1", &step_id)));
                events.push(make_event(instance_id, seq + 3, instance_resumed_payload("wf-1")));
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
            let result = handle.join().expect("thread should not panic").expect("replay should succeed");
            assert_eq!(result.events_applied, 501, "thread {} should apply all events", i);
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

        let good_result = good_handle.join().expect("thread should not panic").expect("good stream should succeed");
        assert_eq!(good_result.final_state, Some(LifecycleState::Completed));

        let bad_result = bad_handle.join().expect("thread should not panic");
        assert!(bad_result.is_err(), "bad stream should fail");
        assert!(matches!(bad_result.unwrap_err(), ReplayError::SequenceGap { .. }));
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
                            make_event("inst-mismatch-b", 2, step_scheduled_payload("wf-1", "step-1")),
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
                assert!(result.is_err(), "odd thread {} should fail with mismatch", i);
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

        let good_result = good_handle.join().expect("thread should not panic").expect("good should succeed");
        assert_eq!(good_result.final_state, Some(LifecycleState::Completed));

        let corrupt_result = corrupt_handle.join().expect("thread should not panic");
        assert!(corrupt_result.is_err());
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
            let result = handle.join().expect("thread should not panic").expect("replay should succeed");
            assert_eq!(result.final_state, Some(LifecycleState::Completed));
            assert_eq!(result.events_applied, 4);
            success_count += 1;
        }
        assert_eq!(success_count, num_threads);
    }

    #[test]
    fn replay_concurrent_interleaved_failure_recovery_cycles() {
        let engine = Arc::new(ReplayEngine::new());

        let build_multi_recovery = |instance_id: &str, cycles: usize| -> Vec<EventEnvelope> {
            let mut events = Vec::new();
            events.push(make_event(instance_id, 1, workflow_started_payload("wf-1")));
            let mut seq = 2u64;
            for _ in 0..cycles {
                events.push(make_event(instance_id, seq, step_scheduled_payload("wf-1", "step-1")));
                events.push(make_event(instance_id, seq + 1, step_started_payload("wf-1", "step-1")));
                events.push(make_event(instance_id, seq + 2, step_failed_payload("wf-1", "step-1")));
                events.push(make_event(instance_id, seq + 3, instance_resumed_payload("wf-1")));
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
            let result = handle.join().expect("thread should not panic").expect("multi-recovery replay should succeed");
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
                let result = handle.join().expect("thread should not panic").expect("replay should succeed");
                all_results.push(result);
            }
        }

        let first = &all_results[0];
        for result in &all_results {
            assert_eq!(result, first, "all concurrent replays must produce identical results");
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

        let r1 = h1.join().expect("thread should not panic").expect("replay should succeed");
        assert_eq!(r1.final_state, Some(LifecycleState::Completed));
        assert_eq!(r1.events_applied, 5);

        let r2 = h2.join().expect("thread should not panic").expect("replay should succeed");
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
                events.push(make_event(instance_id, seq, step_scheduled_payload("wf-1", "step-1")));
                events.push(make_event(instance_id, seq + 1, step_started_payload("wf-1", "step-1")));
                events.push(make_event(instance_id, seq + 2, step_failed_payload("wf-1", "step-1")));
                events.push(make_event(instance_id, seq + 3, instance_resumed_payload("wf-1")));
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

        let r1 = h1.join().expect("thread should not panic").expect("short replay should succeed");
        assert_eq!(r1.events_applied, 9);
        assert_eq!(r1.final_state, Some(LifecycleState::RunningDecision));

        let r2 = h2.join().expect("thread should not panic").expect("long replay should succeed");
        assert_eq!(r2.events_applied, 201);
        assert_eq!(r2.final_state, Some(LifecycleState::RunningDecision));
    }
}
