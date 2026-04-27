use super::*;
use vo_types::state::LifecycleState;

#[test]
fn replay_handles_exponential_nesting_2_to_the_10() {
    let engine = ReplayEngine::new();

    fn build_exponential(depth: usize) -> serde_json::Value {
        if depth == 0 {
            serde_json::json!({"base": "value"})
        } else {
            let left = build_exponential(depth - 1);
            let right = build_exponential(depth - 1);
            serde_json::json!({
                "left": left,
                "right": right
            })
        }
    }

    let nested = build_exponential(10);
    let json = serde_json::json!({
        "type": "WorkflowStarted",
        "workflow_id": "wf-1",
        "binary_hash": "sha256abc",
        "workflow_version_hash": "wvhash123",
        "dedupe_key_hash": null,
        "version": 1,
        "data": nested
    });

    let events = [make_event("inst-1", 1, json)];
    let result = engine
        .replay(&events)
        .expect("2^10 nesting should not blow up");
    assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
}

#[test]
fn replay_handles_exponential_nesting_2_to_the_12() {
    let engine = ReplayEngine::new();

    fn build_exponential(depth: usize) -> serde_json::Value {
        if depth == 0 {
            serde_json::json!({"base": "value"})
        } else {
            let left = build_exponential(depth - 1);
            let right = build_exponential(depth - 1);
            serde_json::json!({
                "left": left,
                "right": right
            })
        }
    }

    let nested = build_exponential(12);
    let json = serde_json::json!({
        "type": "WorkflowStarted",
        "workflow_id": "wf-1",
        "binary_hash": "sha256abc",
        "workflow_version_hash": "wvhash123",
        "dedupe_key_hash": null,
        "version": 1,
        "data": nested
    });

    let events = [make_event("inst-1", 1, json)];
    let result = engine
        .replay(&events)
        .expect("2^12 nesting should not blow up");
    assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
}

#[test]
fn replay_handles_array_of_deeply_nested_objects() {
    let engine = ReplayEngine::new();

    fn build_nested(depth: usize) -> serde_json::Value {
        if depth == 0 {
            serde_json::json!({"leaf": "value"})
        } else {
            serde_json::json!({
                "nested": build_nested(depth - 1)
            })
        }
    }

    let arr = (0..50).map(|_| build_nested(20)).collect::<Vec<_>>();

    let json = serde_json::json!({
        "type": "WorkflowStarted",
        "workflow_id": "wf-1",
        "binary_hash": "sha256abc",
        "workflow_version_hash": "wvhash123",
        "dedupe_key_hash": null,
        "version": 1,
        "array_data": arr
    });

    let events = [make_event("inst-1", 1, json)];
    let result = engine
        .replay(&events)
        .expect("50 x depth-20 nested should not blow up");
    assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
}

#[test]
fn replay_handles_mutual_recursion_payload() {
    let engine = ReplayEngine::new();

    fn build_mutual_recursion(depth: usize) -> serde_json::Value {
        if depth == 0 {
            serde_json::json!({"terminates": "value"})
        } else {
            serde_json::json!({
                "type_a": {
                    "next_b": build_mutual_recursion(depth - 1)
                },
                "type_b": {
                    "next_a": build_mutual_recursion(depth - 1)
                }
            })
        }
    }

    let nested = build_mutual_recursion(10);
    let json = serde_json::json!({
        "type": "WorkflowStarted",
        "workflow_id": "wf-1",
        "binary_hash": "sha256abc",
        "workflow_version_hash": "wvhash123",
        "dedupe_key_hash": null,
        "version": 1,
        "mutual": nested
    });

    let events = [make_event("inst-1", 1, json)];
    let result = engine
        .replay(&events)
        .expect("mutual recursion depth 10 should not blow up");
    assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
}

#[test]
fn replay_handles_wide_and_deep_combination() {
    let engine = ReplayEngine::new();

    fn build_wide_deep(width: usize, depth: usize) -> serde_json::Value {
        if depth == 0 {
            serde_json::json!({"leaf": "value"})
        } else {
            let mut obj = serde_json::Map::new();
            for i in 0..width {
                obj.insert(format!("field_{}", i), build_wide_deep(width, depth - 1));
            }
            serde_json::Value::Object(obj)
        }
    }

    let nested = build_wide_deep(5, 6);
    let json = serde_json::json!({
        "type": "WorkflowStarted",
        "workflow_id": "wf-1",
        "binary_hash": "sha256abc",
        "workflow_version_hash": "wvhash123",
        "dedupe_key_hash": null,
        "version": 1,
        "wide_deep": nested
    });

    let events = [make_event("inst-1", 1, json)];
    let result = engine
        .replay(&events)
        .expect("5^6 wide and deep should not blow up");
    assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
}
