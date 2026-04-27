mod memory_pressure_aggressive {
    use super::*;

    fn make_very_large_payload(size_bytes: usize) -> serde_json::Value {
        let large_field = "x".repeat(size_bytes);
        serde_json::json!({
            "type": "WorkflowStarted",
            "workflow_id": "wf-1",
            "binary_hash": "sha256abc",
            "workflow_version_hash": "wvhash123",
            "dedupe_key_hash": null,
            "version": 1,
            "massive_data": large_field
        })
    }

    fn make_large_wide_payload(num_fields: usize, field_size: usize) -> serde_json::Value {
        let mut obj = serde_json::Map::new();
        obj.insert(
            "type".to_string(),
            serde_json::Value::String("WorkflowStarted".to_string()),
        );
        obj.insert(
            "workflow_id".to_string(),
            serde_json::Value::String("wf-1".to_string()),
        );
        obj.insert(
            "binary_hash".to_string(),
            serde_json::Value::String("sha256abc".to_string()),
        );
        obj.insert(
            "workflow_version_hash".to_string(),
            serde_json::Value::String("wvhash123".to_string()),
        );
        obj.insert("dedupe_key_hash".to_string(), serde_json::Value::Null);
        obj.insert("version".to_string(), serde_json::Value::Number(1.into()));

        for i in 0..num_fields {
            obj.insert(
                format!("field_{}", i),
                serde_json::Value::String("x".repeat(field_size)),
            );
        }
        serde_json::Value::Object(obj)
    }

    #[test]
    fn replay_handles_50mb_single_payload() {
        let engine = ReplayEngine::new();
        let events = [make_event("inst-1", 1, make_very_large_payload(50_000_000))];
        let result = engine.replay(&events).expect("50MB payload should not OOM");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
    }

    #[test]
    fn replay_handles_50_large_payloads_1mb_each() {
        let engine = ReplayEngine::new();
        let events = (1..=50)
            .map(|i| make_event("inst-1", i, make_very_large_payload(1_000_000)))
            .collect::<Vec<_>>();
        let result = engine
            .replay(&events)
            .expect("50 x 1MB payloads should not OOM");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
        assert_eq!(result.events_applied, 50);
    }

    #[test]
    fn replay_handles_wide_payload_10000_fields() {
        let engine = ReplayEngine::new();
        let events = [make_event("inst-1", 1, make_large_wide_payload(10000, 100))];
        let result = engine
            .replay(&events)
            .expect("10000 fields should not blow up");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
    }

    #[test]
    fn replay_handles_sequence_of_wide_payloads() {
        let engine = ReplayEngine::new();
        let events = (1..=20)
            .map(|i| make_event("inst-1", i, make_large_wide_payload(1000, 1000)))
            .collect::<Vec<_>>();
        let result = engine
            .replay(&events)
            .expect("20 x 1000-field payloads should not OOM");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
        assert_eq!(result.events_applied, 20);
    }

    #[test]
    fn replay_handles_mixed_large_and_small_payloads() {
        let engine = ReplayEngine::new();
        let mut events = Vec::new();
        events.push(make_event("inst-1", 1, workflow_started_payload("wf-1")));

        for i in 2..=20 {
            let payload = if i % 3 == 0 {
                make_very_large_payload(5_000_000)
            } else if i % 3 == 1 {
                step_scheduled_payload("wf-1", &format!("step-{}", i))
            } else {
                serde_json::json!({
                    "type": "StepScheduled",
                    "workflow_id": "wf-1",
                    "step_id": format!("step-{}", i),
                    "attempt": 1,
                    "fence": 1,
                    "execution_id": format!("exec-{}", i),
                    "version": 1,
                    "extra_data": "x".repeat(1_000_000)
                })
            };
            events.push(make_event("inst-1", i, payload));
        }

        let result = engine
            .replay(&events)
            .expect("mixed large/small should not OOM");
        assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
    }
}

