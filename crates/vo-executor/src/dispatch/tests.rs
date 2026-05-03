use super::*;
use std::collections::BTreeMap;

fn test_fd4_success() -> Fd4Envelope {
    Fd4Envelope {
        version: 1,
        instance_id: "inst-1".to_string(),
        node_id: "node-1".to_string(),
        result: TaskResult::Success {
            output: serde_json::Value::String("ok".to_string()),
        },
    }
}

fn test_fd4_effect_intent() -> Fd4Envelope {
    Fd4Envelope {
        version: 1,
        instance_id: "inst-1".to_string(),
        node_id: "node-1".to_string(),
        result: TaskResult::EffectIntent {
            intent: serde_json::json!({
                "effect_kind": "http_call",
                "params": {"url": "https://example.com"},
                "connector_id": "stripe"
            }),
        },
    }
}

fn test_fd4_failure() -> Fd4Envelope {
    Fd4Envelope {
        version: 1,
        instance_id: "inst-1".to_string(),
        node_id: "node-1".to_string(),
        result: TaskResult::Failure {
            error: vo_ipc::TaskError {
                code: "ERR".to_string(),
                message: "boom".to_string(),
                details: None,
            },
        },
    }
}

#[test]
fn interpret_pure_success() {
    let env = test_fd4_success();
    let result = interpret_result(NodeKind::Pure, &env).unwrap();
    assert_eq!(
        result,
        StepResult::Success {
            output: "\"ok\"".to_string()
        }
    );
}

#[test]
fn interpret_unsafe_success() {
    let env = test_fd4_success();
    let result = interpret_result(NodeKind::Unsafe, &env).unwrap();
    assert!(result.is_success());
}

#[test]
fn interpret_pure_failure() {
    let env = test_fd4_failure();
    let result = interpret_result(NodeKind::Pure, &env).unwrap();
    assert!(!result.is_success());
}

#[test]
fn interpret_unsafe_failure() {
    let env = test_fd4_failure();
    let result = interpret_result(NodeKind::Unsafe, &env).unwrap();
    assert!(!result.is_success());
}

#[test]
fn interpret_managed_effect_intent() {
    let env = test_fd4_effect_intent();
    let result = interpret_result(NodeKind::ManagedEffect, &env).unwrap();
    match result {
        StepResult::EffectIntent {
            effect_kind,
            connector_id,
            ..
        } => {
            assert_eq!(effect_kind, "http_call");
            assert_eq!(connector_id, "stripe");
        }
        other => panic!("expected EffectIntent, got {:?}", other),
    }
}

#[test]
fn interpret_managed_effect_failure() {
    let env = test_fd4_failure();
    let result = interpret_result(NodeKind::ManagedEffect, &env).unwrap();
    assert!(!result.is_success());
}

#[test]
fn interpret_pure_rejects_effect_intent() {
    let env = test_fd4_effect_intent();
    let result = interpret_result(NodeKind::Pure, &env);
    assert!(matches!(
        result,
        Err(ExecuteNodeError::DispatchMismatch { .. })
    ));
}

#[test]
fn interpret_unsafe_rejects_effect_intent() {
    let env = test_fd4_effect_intent();
    let result = interpret_result(NodeKind::Unsafe, &env);
    assert!(matches!(
        result,
        Err(ExecuteNodeError::DispatchMismatch { .. })
    ));
}

#[test]
fn interpret_managed_effect_rejects_success() {
    let env = test_fd4_success();
    let result = interpret_result(NodeKind::ManagedEffect, &env);
    assert!(matches!(
        result,
        Err(ExecuteNodeError::DispatchMismatch { .. })
    ));
}

#[tokio::test]
async fn dispatch_wait_returns_deferred() {
    let result = dispatch_node(
        NodeKind::Wait,
        std::path::Path::new("/bin/true"),
        1000,
        "inst-1",
        "node-1",
        serde_json::Value::Null,
        BTreeMap::new(),
        BTreeMap::new(),
    )
    .await
    .unwrap();
    assert_eq!(
        result.step_result,
        StepResult::Success {
            output: "wait_deferred".to_string()
        }
    );
}

#[tokio::test]
async fn dispatch_signal_returns_emitted() {
    let result = dispatch_node(
        NodeKind::Signal,
        std::path::Path::new("/bin/true"),
        1000,
        "inst-1",
        "node-1",
        serde_json::Value::Null,
        BTreeMap::new(),
        BTreeMap::new(),
    )
    .await
    .unwrap();
    assert_eq!(
        result.step_result,
        StepResult::Success {
            output: "signal_emitted".to_string()
        }
    );
}

#[test]
fn step_result_effect_intent_serde_roundtrip() {
    let r = StepResult::EffectIntent {
        effect_kind: "http_call".to_string(),
        params: "{\"url\":\"https://example.com\"}".to_string(),
        connector_id: "stripe".to_string(),
    };
    let json = serde_json::to_string(&r).unwrap();
    let back: StepResult = serde_json::from_str(&json).unwrap();
    assert_eq!(r, back);
}

#[test]
fn effect_intent_value_serde_roundtrip() {
    let env = serde_json::json!({
        "effect_kind": "sql_query",
        "params": {"query": "SELECT 1"},
        "connector_id": "postgres"
    });
    let json = serde_json::to_string(&env).unwrap();
    let back: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(env, back);
}

#[test]
fn task_result_effect_intent_serde_roundtrip() {
    let tr = TaskResult::EffectIntent {
        intent: serde_json::json!({
            "effect_kind": "blob_write",
            "params": {"key": "test"},
            "connector_id": "s3"
        }),
    };
    let json = serde_json::to_string(&tr).unwrap();
    let back: TaskResult = serde_json::from_str(&json).unwrap();
    assert_eq!(tr, back);
}
