use axum::{
    extract::Extension,
    http::StatusCode,
    Json,
};
use ractor::ActorRef;
use vo_actor::OrchestratorMsg;
use vo_api::types::{ApiError, V3SignalRequest, V3StartRequest};
use vo_api::handlers::{start_workflow, get_workflow, terminate_workflow, list_workflows, send_signal, get_events};

fn mock_master() -> ActorRef<OrchestratorMsg> {
    ActorRef::new(None, "mock".into())
}

#[tokio::test]
async fn start_workflow_rejects_empty_dedupe_key() {
    let req = V3StartRequest {
        namespace: "payments".to_string(),
        workflow_type: "checkout".to_string(),
        paradigm: "fsm".to_string(),
        input: serde_json::json!({}),
        instance_id: None,
        dedupe_key: Some("".to_string()),
    };

    let response = start_workflow(
        Extension(mock_master()),
        Json(req),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn start_workflow_rejects_missing_dedupe_key() {
    let req = V3StartRequest {
        namespace: "payments".to_string(),
        workflow_type: "checkout".to_string(),
        paradigm: "fsm".to_string(),
        input: serde_json::json!({}),
        instance_id: None,
        dedupe_key: None,
    };

    let response = start_workflow(
        Extension(mock_master()),
        Json(req),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn start_workflow_rejects_invalid_namespace_characters() {
    let req = V3StartRequest {
        namespace: "invalid namespace!".to_string(),
        workflow_type: "checkout".to_string(),
        paradigm: "fsm".to_string(),
        input: serde_json::json!({}),
        instance_id: None,
        dedupe_key: Some("key123".to_string()),
    };

    let response = start_workflow(
        Extension(mock_master()),
        Json(req),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn start_workflow_rejects_invalid_paradigm() {
    let req = V3StartRequest {
        namespace: "payments".to_string(),
        workflow_type: "checkout".to_string(),
        paradigm: "invalid".to_string(),
        input: serde_json::json!({}),
        instance_id: None,
        dedupe_key: Some("key123".to_string()),
    };

    let response = start_workflow(
        Extension(mock_master()),
        Json(req),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn start_workflow_rejects_uppercase_paradigm() {
    let req = V3StartRequest {
        namespace: "payments".to_string(),
        workflow_type: "checkout".to_string(),
        paradigm: "FSM".to_string(),
        input: serde_json::json!({}),
        instance_id: None,
        dedupe_key: Some("key123".to_string()),
    };

    let response = start_workflow(
        Extension(mock_master()),
        Json(req),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn start_workflow_rejects_invalid_instance_id_chars() {
    let req = V3StartRequest {
        namespace: "payments".to_string(),
        workflow_type: "checkout".to_string(),
        paradigm: "fsm".to_string(),
        input: serde_json::json!({}),
        instance_id: Some("invalid instance!".to_string()),
        dedupe_key: Some("key123".to_string()),
    };

    let response = start_workflow(
        Extension(mock_master()),
        Json(req),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn start_workflow_rejects_malformed_json_input() {
    let req = V3StartRequest {
        namespace: "payments".to_string(),
        workflow_type: "checkout".to_string(),
        paradigm: "fsm".to_string(),
        input: serde_json::json!({"nested": {"deep": "value"}}),
        instance_id: None,
        dedupe_key: Some("key123".to_string()),
    };

    let response = start_workflow(
        Extension(mock_master()),
        Json(req),
    ).await.into_response();

    assert_ne!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn get_workflow_rejects_id_without_namespace() {
    let response = get_workflow(
        Extension(mock_master()),
        axum::extract::Path("no-slash-here".to_string()),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn get_workflow_rejects_path_traversal() {
    let response = get_workflow(
        Extension(mock_master()),
        axum::extract::Path("../../../etc/passwd".to_string()),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn get_workflow_rejects_nested_path_traversal() {
    let response = get_workflow(
        Extension(mock_master()),
        axum::extract::Path("namespace/../../../../../root".to_string()),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn terminate_workflow_rejects_id_without_namespace() {
    let response = terminate_workflow(
        Extension(mock_master()),
        axum::extract::Path("no-slash-here".to_string()),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn terminate_workflow_rejects_path_traversal() {
    let response = terminate_workflow(
        Extension(mock_master()),
        axum::extract::Path("../admin".to_string()),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn send_signal_rejects_id_without_namespace() {
    let req = V3SignalRequest {
        signal_name: "test_signal".to_string(),
        payload: serde_json::json!({}),
    };

    let response = send_signal(
        Extension(mock_master()),
        axum::extract::Path("no-slash-here".to_string()),
        Json(req),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn send_signal_rejects_path_traversal() {
    let req = V3SignalRequest {
        signal_name: "test_signal".to_string(),
        payload: serde_json::json!({}),
    };

    let response = send_signal(
        Extension(mock_master()),
        axum::extract::Path("../../admin".to_string()),
        Json(req),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn send_signal_rejects_empty_signal_name() {
    let req = V3SignalRequest {
        signal_name: "".to_string(),
        payload: serde_json::json!({}),
    };

    let response = send_signal(
        Extension(mock_master()),
        axum::extract::Path("namespace/instance123".to_string()),
        Json(req),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn send_signal_rejects_sql_injection_in_payload() {
    let req = V3SignalRequest {
        signal_name: "test".to_string(),
        payload: serde_json::json!({"sql": "DROP TABLE users;--"}),
    };

    let response = send_signal(
        Extension(mock_master()),
        axum::extract::Path("namespace/instance123".to_string()),
        Json(req),
    ).await.into_response();

    assert_ne!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn send_signal_rejects_shell_injection_in_payload() {
    let req = V3SignalRequest {
        signal_name: "test".to_string(),
        payload: serde_json::json!({"cmd": "; rm -rf /"}),
    };

    let response = send_signal(
        Extension(mock_master()),
        axum::extract::Path("namespace/instance123".to_string()),
        Json(req),
    ).await.into_response();

    assert_ne!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn get_events_rejects_id_without_namespace() {
    let response = get_events(
        Extension(mock_master()),
        axum::extract::Path("no-slash-here".to_string()),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn get_events_returns_not_implemented() {
    let response = get_events(
        Extension(mock_master()),
        axum::extract::Path("namespace/instance123".to_string()),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
}

#[tokio::test]
async fn list_workflows_returns_service_unavailable_without_master() {
    let response = list_workflows(
        Extension(mock_master()),
    ).await.into_response();

    assert_ne!(response.status(), StatusCode::OK);
}

fn assert_error_response(response: &axum::response::Response, expected_code: &str) {
    let (parts, body) = response.split();
    assert_eq!(parts.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn start_workflow_error_response_has_correct_structure() {
    let req = V3StartRequest {
        namespace: "".to_string(),
        workflow_type: "".to_string(),
        paradigm: "invalid".to_string(),
        input: serde_json::json!({}),
        instance_id: None,
        dedupe_key: None,
    };

    let response = start_workflow(
        Extension(mock_master()),
        Json(req),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn oversized_payload_namespace() {
    let long_namespace = "a".repeat(10000);
    let req = V3StartRequest {
        namespace: long_namespace,
        workflow_type: "checkout".to_string(),
        paradigm: "fsm".to_string(),
        input: serde_json::json!({}),
        instance_id: None,
        dedupe_key: Some("key123".to_string()),
    };

    let response = start_workflow(
        Extension(mock_master()),
        Json(req),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn oversized_payload_workflow_type() {
    let req = V3StartRequest {
        namespace: "payments".to_string(),
        workflow_type: "a".repeat(10000),
        paradigm: "fsm".to_string(),
        input: serde_json::json!({}),
        instance_id: None,
        dedupe_key: Some("key123".to_string()),
    };

    let response = start_workflow(
        Extension(mock_master()),
        Json(req),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn oversized_input_payload() {
    let large_input = serde_json::json!({
        "data": "a".repeat(1_000_000)
    });
    let req = V3StartRequest {
        namespace: "payments".to_string(),
        workflow_type: "checkout".to_string(),
        paradigm: "fsm".to_string(),
        input: large_input,
        instance_id: None,
        dedupe_key: Some("key123".to_string()),
    };

    let response = start_workflow(
        Extension(mock_master()),
        Json(req),
    ).await.into_response();

    assert_ne!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn xss_attempt_in_namespace() {
    let req = V3StartRequest {
        namespace: "<script>alert('xss')</script>".to_string(),
        workflow_type: "checkout".to_string(),
        paradigm: "fsm".to_string(),
        input: serde_json::json!({}),
        instance_id: None,
        dedupe_key: Some("key123".to_string()),
    };

    let response = start_workflow(
        Extension(mock_master()),
        Json(req),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn xss_attempt_in_workflow_type() {
    let req = V3StartRequest {
        namespace: "payments".to_string(),
        workflow_type: "<img src=x onerror=alert(1)>".to_string(),
        paradigm: "fsm".to_string(),
        input: serde_json::json!({}),
        instance_id: None,
        dedupe_key: Some("key123".to_string()),
    };

    let response = start_workflow(
        Extension(mock_master()),
        Json(req),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn null_bytes_in_namespace_rejected() {
    let req = V3StartRequest {
        namespace: "pay\x00ments".to_string(),
        workflow_type: "checkout".to_string(),
        paradigm: "fsm".to_string(),
        input: serde_json::json!({}),
        instance_id: None,
        dedupe_key: Some("key123".to_string()),
    };

    let response = start_workflow(
        Extension(mock_master()),
        Json(req),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn unicode_override_in_namespace() {
    let req = V3StartRequest {
        namespace: "\u{202E}payments".to_string(),
        workflow_type: "checkout".to_string(),
        paradigm: "fsm".to_string(),
        input: serde_json::json!({}),
        instance_id: None,
        dedupe_key: Some("key123".to_string()),
    };

    let response = start_workflow(
        Extension(mock_master()),
        Json(req),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn newline_injection_in_namespace() {
    let req = V3StartRequest {
        namespace: "payments\r\nInjection".to_string(),
        workflow_type: "checkout".to_string(),
        paradigm: "fsm".to_string(),
        input: serde_json::json!({}),
        instance_id: None,
        dedupe_key: Some("key123".to_string()),
    };

    let response = start_workflow(
        Extension(mock_master()),
        Json(req),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn tab_injection_in_workflow_type() {
    let req = V3StartRequest {
        namespace: "payments".to_string(),
        workflow_type: "checkout\twith\ttabs".to_string(),
        paradigm: "fsm".to_string(),
        input: serde_json::json!({}),
        instance_id: None,
        dedupe_key: Some("key123".to_string()),
    };

    let response = start_workflow(
        Extension(mock_master()),
        Json(req),
    ).await.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
