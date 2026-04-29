mod exponential_blowup_scenarios {
    use super::*;

    #[test]
    fn replay_handles_deeply_nested_json_payload() {
        let engine = ReplayEngine::new();

        fn build_nested_json(depth: usize) -> serde_json::Value {
            if depth == 0 {
                serde_json::json!({"base": "value"})
            } else {
                serde_json::json!({
                    "nested": build_nested_json(depth - 1)
                })
            }
        }

        let deep_payload = build_nested_json(100);
        let json = serde_json::json!({
            "type": "WorkflowStarted",
            "workflow_id": "wf-1",
            "binary_hash": "sha256abc",
            "workflow_version_hash": "wvhash123",
            "dedupe_key_hash": null,
            "version": 1,
            "deep_data": deep_payload
        });

        let events = [make_event("inst-1", 1, json)];
        let result = engine
            .replay(&events)
            .expect("deeply nested should not blow up");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
    }

    #[test]
    fn replay_handles_wide_json_payload() {
        let engine = ReplayEngine::new();

        let mut wide_obj = serde_json::Map::new();
        wide_obj.insert(
            "type".to_string(),
            serde_json::Value::String("WorkflowStarted".to_string()),
        );
        wide_obj.insert(
            "workflow_id".to_string(),
            serde_json::Value::String("wf-1".to_string()),
        );
        wide_obj.insert(
            "binary_hash".to_string(),
            serde_json::Value::String("sha256abc".to_string()),
        );
        wide_obj.insert(
            "workflow_version_hash".to_string(),
            serde_json::Value::String("wvhash123".to_string()),
        );
        wide_obj.insert("dedupe_key_hash".to_string(), serde_json::Value::Null);
        wide_obj.insert("version".to_string(), serde_json::Value::Number(1.into()));

        for i in 0..1000 {
            wide_obj.insert(
                format!("field_{}", i),
                serde_json::Value::String(format!("value_{}", i)),
            );
        }

        let json = serde_json::Value::Object(wide_obj);
        let events = [make_event("inst-1", 1, json)];
        let result = engine
            .replay(&events)
            .expect("wide payload should not blow up");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
    }

    #[test]
    fn replay_handles_large_event_sequence_linear_time() {
        let engine = ReplayEngine::new();
        let mut events = Vec::new();

        events.push(make_event("inst-1", 1, workflow_started_payload("wf-1")));

        for i in 2..=1000 {
            let step_num = (i - 2) % 4;
            let payload = match step_num {
                0 => step_scheduled_payload("wf-1", &format!("step-{}", i)),
                1 => step_started_payload("wf-1", &format!("step-{}", i)),
                2 => step_completed_payload("wf-1", &format!("step-{}", i)),
                _ => step_scheduled_payload("wf-1", &format!("step-{}", i + 1)),
            };
            events.push(make_event("inst-1", i, payload));
        }

        let result = engine.replay(&events).expect("1000 events should replay");
        assert_eq!(result.events_applied, 1000);
    }

    #[test]
    fn replay_detects_sequence_gap_in_large_sequence() {
        let engine = ReplayEngine::new();
        let mut events = Vec::new();

        events.push(make_event("inst-1", 1, workflow_started_payload("wf-1")));

        for i in 2..=100 {
            if i == 50 {
                events.push(make_event(
                    "inst-1",
                    52,
                    step_scheduled_payload("wf-1", "step-50"),
                ));
            } else if i < 50 {
                events.push(make_event(
                    "inst-1",
                    i,
                    step_scheduled_payload("wf-1", &format!("step-{}", i)),
                ));
            } else {
                events.push(make_event(
                    "inst-1",
                    i + 1,
                    step_scheduled_payload("wf-1", &format!("step-{}", i)),
                ));
            }
        }

        let err = engine
            .replay(&events)
            .expect_err("should detect gap at 50->52");
        assert!(matches!(
            err,
            ReplayError::SequenceGap {
                expected: 51,
                actual: 52,
                at_index: 49
            }
        ));
    }
}

