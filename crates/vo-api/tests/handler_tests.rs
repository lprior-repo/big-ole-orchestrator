//! Integration tests for vo-api HTTP handlers.
//!
//! Tests: events, workflow, signal, workflow_lifecycle, workflow_start, helpers.
//! Uses stub handlers with tower::ServiceExt::oneshot (same pattern as qa_api.rs).

use axum::{
    body::Body,
    extract::Path,
    http::{header, Request, StatusCode},
    routing::{get, post},
    Extension, Json, Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;

fn err(status: StatusCode, code: &str, msg: &str) -> (StatusCode, Json<Value>) {
    (status, Json(json!({"error": code, "message": msg})))
}

// ---------------------------------------------------------------------------
// Stub handlers that replicate validation logic from real handlers
// ---------------------------------------------------------------------------

async fn stub_get_events(Path(id): Path<String>) -> (StatusCode, Json<Value>) {
    if id.split('/').count() != 2 {
        return err(
            StatusCode::BAD_REQUEST,
            "invalid_id",
            "id must be namespace/instance_id",
        );
    }
    err(
        StatusCode::NOT_IMPLEMENTED,
        "not_implemented",
        "event log streaming: see bead vo-k0ck",
    )
}

async fn stub_start_workflow(req: Request<Body>) -> (StatusCode, Json<Value>) {
    let ct = req
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap_or(""))
        .unwrap_or("");
    if !ct.contains("application/json") {
        return err(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "invalid_content_type",
            "expected application/json",
        );
    }
    let bytes = axum::body::to_bytes(req.into_body(), 1 << 20)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);

    if body
        .get("dedupe_key")
        .and_then(|v| v.as_str())
        .map_or(true, |k| k.is_empty())
    {
        return err(
            StatusCode::BAD_REQUEST,
            "missing_dedupe_key",
            "dedupe_key is required for exact workflow ingress (ADR-028)",
        );
    }

    let namespace = body
        .get("namespace")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if namespace.is_empty() {
        return err(
            StatusCode::BAD_REQUEST,
            "invalid_namespace",
            "namespace contains illegal characters",
        );
    }

    let paradigm = body
        .get("paradigm")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !["fsm", "dag", "procedural"].contains(&paradigm) {
        return err(
            StatusCode::BAD_REQUEST,
            "invalid_paradigm",
            "paradigm must be 'fsm', 'dag', or 'procedural'",
        );
    }

    if body.get("workflow_type").and_then(|v| v.as_str()).is_none() {
        return err(
            StatusCode::BAD_REQUEST,
            "invalid_input",
            "missing required field: workflow_type",
        );
    }

    (
        StatusCode::CREATED,
        Json(json!({
            "instance_id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "namespace": body["namespace"],
            "workflow_type": body["workflow_type"],
        })),
    )
}

async fn stub_get_workflow(Path(id): Path<String>) -> (StatusCode, Json<Value>) {
    let parts: Vec<&str> = id.split('/').collect();
    if parts.len() != 2 {
        return err(
            StatusCode::BAD_REQUEST,
            "invalid_id",
            "id must be <namespace>/<instance_id>",
        );
    }
    (
        StatusCode::OK,
        Json(json!({
            "instance_id": id,
            "namespace": parts[0],
            "workflow_type": "checkout",
            "paradigm": "fsm",
            "phase": "live",
            "events_applied": 10,
        })),
    )
}

async fn stub_list_workflows() -> (StatusCode, Json<Value>) {
    (
        StatusCode::OK,
        Json(json!([
            {
                "instance_id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
                "namespace": "payments",
                "workflow_type": "checkout",
                "paradigm": "fsm",
                "phase": "live",
                "events_applied": 5
            }
        ])),
    )
}

async fn stub_terminate_workflow(Path(id): Path<String>) -> StatusCode {
    let parts: Vec<&str> = id.split('/').collect();
    if parts.len() != 2 {
        return StatusCode::BAD_REQUEST;
    }
    StatusCode::NO_CONTENT
}

async fn stub_unquarantine_workflow(Path(id): Path<String>) -> (StatusCode, Json<Value>) {
    let parts: Vec<&str> = id.split('/').collect();
    if parts.len() != 2 {
        return err(
            StatusCode::BAD_REQUEST,
            "invalid_id",
            "id must be <namespace>/<instance_id>",
        );
    }
    err(
        StatusCode::NOT_IMPLEMENTED,
        "not_implemented",
        "circuit breaker state injection required (see bead ve-jfj5)",
    )
}

async fn stub_get_workflow_status(Path(id): Path<String>) -> (StatusCode, Json<Value>) {
    let parts: Vec<&str> = id.split('/').collect();
    if parts.len() != 2 {
        return err(
            StatusCode::BAD_REQUEST,
            "invalid_id",
            "id must be <namespace>/<instance_id>",
        );
    }
    (
        StatusCode::OK,
        Json(json!({
            "instance_id": id,
            "namespace": parts[0],
            "workflow_type": "checkout",
            "paradigm": "fsm",
            "phase": "live",
            "events_applied": 10,
            "registration_status": null,
            "is_quarantined": false,
        })),
    )
}

async fn stub_send_signal(Path(id): Path<String>, req: Request<Body>) -> (StatusCode, Json<Value>) {
    let parts: Vec<&str> = id.split('/').collect();
    if parts.len() != 2 {
        return err(
            StatusCode::BAD_REQUEST,
            "invalid_id",
            "id must be <namespace>/<instance_id>",
        );
    }
    let bytes = axum::body::to_bytes(req.into_body(), 1 << 20)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);

    if body
        .get("signal_name")
        .and_then(|v| v.as_str())
        .map_or(true, |s| s.is_empty())
    {
        return err(
            StatusCode::BAD_REQUEST,
            "invalid_signal",
            "signal_name is required",
        );
    }

    (StatusCode::ACCEPTED, Json(json!({"status": "accepted"})))
}

async fn stub_compensate_workflow(Path(id): Path<String>) -> (StatusCode, Json<Value>) {
    let parts: Vec<&str> = id.split('/').collect();
    if parts.len() != 2 {
        return err(
            StatusCode::BAD_REQUEST,
            "invalid_id",
            "id must be <namespace>/<instance_id>",
        );
    }
    (
        StatusCode::ACCEPTED,
        Json(json!({
            "instance_id": id,
            "status": "compensation_initiated",
        })),
    )
}

// ---------------------------------------------------------------------------
// App factory
// ---------------------------------------------------------------------------

fn app() -> Router {
    Router::new()
        .route("/api/v1/workflows", post(stub_start_workflow).get(stub_list_workflows))
        .route(
            "/api/v1/workflows/{id}",
            get(stub_get_workflow).delete(stub_terminate_workflow),
        )
        .route(
            "/api/v1/workflows/{id}/events",
            get(stub_get_events),
        )
        .route(
            "/api/v1/workflows/{id}/status",
            get(stub_get_workflow_status),
        )
        .route(
            "/api/v1/workflows/{id}/unquarantine",
            post(stub_unquarantine_workflow),
        )
        .route(
            "/api/v1/workflows/{id}/signals",
            post(stub_send_signal),
        )
        .route(
            "/api/v1/workflows/{id}/compensate",
            post(stub_compensate_workflow),
        )
}

async fn send(req: Request<Body>) -> (StatusCode, String) {
    let resp = app().oneshot(req).await.unwrap();
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, String::from_utf8_lossy(&body).into_owned())
}

fn assert_err(body: &str, code: &str) {
    let v: Value = serde_json::from_str(body).expect("error body must be valid JSON");
    assert_eq!(v["error"], code, "expected error code '{code}', got: {v}");
    assert!(v.get("message").is_some(), "must have 'message' field");
}

// ===========================================================================
// events handler tests
// ===========================================================================

mod events {
    use super::*;

    #[tokio::test]
    async fn get_events_501_not_implemented() {
        let req = Request::builder()
            .uri("/api/v1/workflows/payments%2F01ARZ3NDEKTSV4RRFFQ69G5FAV/events")
            .body(Body::empty())
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert_err(&body, "not_implemented");
    }

    #[tokio::test]
    async fn get_events_400_invalid_id_no_slash() {
        let req = Request::builder()
            .uri("/api/v1/workflows/noslash/events")
            .body(Body::empty())
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_err(&body, "invalid_id");
    }

    #[tokio::test]
    async fn get_events_400_empty_id() {
        let req = Request::builder()
            .uri("/api/v1/workflows//events")
            .body(Body::empty())
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_err(&body, "invalid_id");
    }
}

// ===========================================================================
// workflow handler tests
// ===========================================================================

mod workflow {
    use super::*;

    #[tokio::test]
    async fn start_workflow_201_success() {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"namespace":"payments","workflow_type":"checkout","paradigm":"fsm","input":{},"dedupe_key":"dk-1"}"#,
            ))
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::CREATED);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert!(v.get("instance_id").is_some());
        assert_eq!(v["namespace"], "payments");
        assert_eq!(v["workflow_type"], "checkout");
    }

    #[tokio::test]
    async fn start_workflow_400_missing_dedupe_key() {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"namespace":"payments","workflow_type":"checkout","paradigm":"fsm","input":{}}"#,
            ))
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_err(&body, "missing_dedupe_key");
    }

    #[tokio::test]
    async fn start_workflow_400_empty_dedupe_key() {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"namespace":"payments","workflow_type":"checkout","paradigm":"fsm","input":{},"dedupe_key":""}"#,
            ))
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_err(&body, "missing_dedupe_key");
    }

    #[tokio::test]
    async fn start_workflow_400_invalid_paradigm() {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"namespace":"payments","workflow_type":"checkout","paradigm":"quantum","input":{},"dedupe_key":"dk-1"}"#,
            ))
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_err(&body, "invalid_paradigm");
    }

    #[tokio::test]
    async fn start_workflow_400_missing_workflow_type() {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"namespace":"payments","paradigm":"fsm","input":{},"dedupe_key":"dk-1"}"#,
            ))
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_err(&body, "invalid_input");
    }

    #[tokio::test]
    async fn start_workflow_rejects_missing_content_type() {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows")
            .body(Body::from("{}"))
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_err(&body, "invalid_content_type");
    }

    #[tokio::test]
    async fn get_workflow_200_valid_id() {
        let req = Request::builder()
            .uri("/api/v1/workflows/payments%2F01ARZ3NDEKTSV4RRFFQ69G5FAV")
            .body(Body::empty())
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::OK);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["namespace"], "payments");
        assert_eq!(v["paradigm"], "fsm");
        assert_eq!(v["phase"], "live");
        assert_eq!(v["events_applied"], 10);
    }

    #[tokio::test]
    async fn get_workflow_400_invalid_id_no_slash() {
        let req = Request::builder()
            .uri("/api/v1/workflows/noslash")
            .body(Body::empty())
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_err(&body, "invalid_id");
    }

    #[tokio::test]
    async fn list_workflows_200_with_items() {
        let req = Request::builder()
            .uri("/api/v1/workflows")
            .body(Body::empty())
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::OK);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert!(v.is_array());
        assert_eq!(v.as_array().unwrap().len(), 1);
        assert_eq!(v[0]["namespace"], "payments");
    }

    #[tokio::test]
    async fn terminate_workflow_204_valid_id() {
        let req = Request::builder()
            .method("DELETE")
            .uri("/api/v1/workflows/ns%2F01ARZ3NDEKTSV4RRFFQ69G5FAV")
            .body(Body::empty())
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn terminate_workflow_400_invalid_id() {
        let req = Request::builder()
            .method("DELETE")
            .uri("/api/v1/workflows/noslash")
            .body(Body::empty())
            .unwrap();
        let (status, _) = send(req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn unquarantine_workflow_501_not_implemented() {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows/ns%2F01ARZ3NDEKTSV4RRFFQ69G5FAV/unquarantine")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"operator":"admin"}"#))
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert_err(&body, "not_implemented");
    }

    #[tokio::test]
    async fn unquarantine_workflow_400_invalid_id() {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows/noslash/unquarantine")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"operator":"admin"}"#))
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_err(&body, "invalid_id");
    }

    #[tokio::test]
    async fn get_workflow_status_200_valid_id() {
        let req = Request::builder()
            .uri("/api/v1/workflows/ns%2F01ARZ3NDEKTSV4RRFFQ69G5FAV/status")
            .body(Body::empty())
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::OK);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["is_quarantined"], false);
        assert!(v.get("registration_status").is_some());
    }

    #[tokio::test]
    async fn get_workflow_status_400_invalid_id() {
        let req = Request::builder()
            .uri("/api/v1/workflows/noslash/status")
            .body(Body::empty())
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_err(&body, "invalid_id");
    }
}

// ===========================================================================
// signal handler tests
// ===========================================================================

mod signal {
    use super::*;

    #[tokio::test]
    async fn send_signal_202_valid_request() {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows/ns%2F01ARZ3NDEKTSV4RRFFQ69G5FAV/signals")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"signal_name":"approve","payload":{"approved":true}}"#,
            ))
            .unwrap();
        let (status, _) = send(req).await;
        assert_eq!(status, StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn send_signal_400_invalid_id() {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows/noslash/signals")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"signal_name":"approve","payload":{}}"#,
            ))
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_err(&body, "invalid_id");
    }

    #[tokio::test]
    async fn send_signal_400_missing_signal_name() {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows/ns%2F01ARZ3NDEKTSV4RRFFQ69G5FAV/signals")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"payload":{}}"#))
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_err(&body, "invalid_signal");
    }

    #[tokio::test]
    async fn send_signal_400_empty_signal_name() {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows/ns%2F01ARZ3NDEKTSV4RRFFQ69G5FAV/signals")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"signal_name":"","payload":{}}"#))
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_err(&body, "invalid_signal");
    }
}

// ===========================================================================
// workflow_lifecycle handler tests
// ===========================================================================

mod workflow_lifecycle {
    use super::*;

    #[tokio::test]
    async fn terminate_204_valid_id() {
        let req = Request::builder()
            .method("DELETE")
            .uri("/api/v1/workflows/ns%2F01ARZ3NDEKTSV4RRFFQ69G5FAV")
            .body(Body::empty())
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn terminate_400_no_slash() {
        let req = Request::builder()
            .method("DELETE")
            .uri("/api/v1/workflows/invalid")
            .body(Body::empty())
            .unwrap();
        let (status, _) = send(req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn compensate_202_valid_id() {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows/ns%2F01ARZ3NDEKTSV4RRFFQ69G5FAV/compensate")
            .body(Body::empty())
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::ACCEPTED);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["status"], "compensation_initiated");
    }

    #[tokio::test]
    async fn compensate_400_invalid_id() {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows/noslash/compensate")
            .body(Body::empty())
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_err(&body, "invalid_id");
    }

    #[tokio::test]
    async fn unquarantine_501_not_implemented() {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows/ns%2F01ARZ3NDEKTSV4RRFFQ69G5FAV/unquarantine")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"operator":"admin"}"#))
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert_err(&body, "not_implemented");
    }

    #[tokio::test]
    async fn unquarantine_400_invalid_id() {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows/noslash/unquarantine")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"operator":"admin"}"#))
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_err(&body, "invalid_id");
    }
}

// ===========================================================================
// workflow_start handler tests
// ===========================================================================

mod workflow_start {
    use super::*;

    #[tokio::test]
    async fn start_201_all_valid_paradigms() {
        for paradigm in ["fsm", "dag", "procedural"] {
            let req = Request::builder()
                .method("POST")
                .uri("/api/v1/workflows")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(format!(
                    r#"{{"namespace":"ns","workflow_type":"wf","paradigm":"{paradigm}","input":{{}},"dedupe_key":"dk-{paradigm}"}}"#
                )))
                .unwrap();
            let (status, body) = send(req).await;
            assert_eq!(status, StatusCode::CREATED, "paradigm: {paradigm}");
            let v: Value = serde_json::from_str(&body).unwrap();
            assert!(v.get("instance_id").is_some(), "paradigm: {paradigm}");
        }
    }

    #[tokio::test]
    async fn start_400_dedupe_key_absent() {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"namespace":"ns","workflow_type":"wf","paradigm":"fsm","input":{}}"#,
            ))
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_err(&body, "missing_dedupe_key");
    }

    #[tokio::test]
    async fn start_400_dedupe_key_empty_string() {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"namespace":"ns","workflow_type":"wf","paradigm":"fsm","input":{},"dedupe_key":""}"#,
            ))
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_err(&body, "missing_dedupe_key");
    }

    #[tokio::test]
    async fn start_400_invalid_paradigm() {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"namespace":"ns","workflow_type":"wf","paradigm":"bogus","input":{},"dedupe_key":"dk-1"}"#,
            ))
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_err(&body, "invalid_paradigm");
    }

    #[tokio::test]
    async fn start_400_no_content_type() {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows")
            .body(Body::from("{}"))
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_err(&body, "invalid_content_type");
    }

    #[tokio::test]
    async fn start_400_empty_namespace() {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"namespace":"","workflow_type":"wf","paradigm":"fsm","input":{},"dedupe_key":"dk-1"}"#,
            ))
            .unwrap();
        let (status, body) = send(req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_err(&body, "invalid_namespace");
    }

    // ======================================================================
    // BDD: production orchestrator receives StartWorkflow (tw-4y6h.2.4)
    // ======================================================================

    use ractor::Actor;
    use std::sync::Arc;
    use vo_actor::OrchestratorMsg;
    use vo_core::admission::{PressureGuardResult, WriterPressureGuard};
    use vo_storage::dedupe_partition::InMemoryDedupeStore;

    struct AlwaysAdmitPressureGuard;

    impl WriterPressureGuard for AlwaysAdmitPressureGuard {
        fn check(&self) -> PressureGuardResult {
            PressureGuardResult::Admitted
        }
    }

    struct CapturingOrchestrator;

    impl Actor for CapturingOrchestrator {
        type Msg = OrchestratorMsg;
        type State = ();
        type Arguments = ();

        async fn pre_start(
            &self,
            _myself: ractor::ActorRef<Self::Msg>,
            _args: Self::Arguments,
        ) -> Result<Self::State, ractor::ActorProcessingErr> {
            Ok(())
        }

        async fn handle(
            &self,
            _myself: ractor::ActorRef<Self::Msg>,
            message: Self::Msg,
            _state: &mut Self::State,
        ) -> Result<(), ractor::ActorProcessingErr> {
            if let OrchestratorMsg::StartWorkflow { reply, .. } = message {
                let _ = reply.send(Ok(()));
            }
            Ok(())
        }
    }

    fn production_app(
        master_ref: ractor::ActorRef<OrchestratorMsg>,
        dedupe_store: Arc<dyn vo_storage::dedupe_partition::DedupeStore>,
    ) -> Router {
        Router::new()
            .route(
                "/api/v1/workflows",
                post(vo_api::handlers::start_workflow),
            )
            .layer(Extension(master_ref))
            .layer(Extension(Arc::new(AlwaysAdmitPressureGuard) as Arc<dyn WriterPressureGuard>))
            .layer(Extension(dedupe_store))
    }

    #[tokio::test]
    async fn given_start_request_when_handler_runs_then_production_orchestrator_receives_start() {
        let (master_ref, _handle) = ractor::Actor::spawn(
            Some("test-prod-orchestrator".to_string()),
            CapturingOrchestrator,
            (),
        )
        .await
        .expect("spawn production orchestrator");

        let dedupe_store: Arc<dyn vo_storage::dedupe_partition::DedupeStore> =
            Arc::new(InMemoryDedupeStore::new());

        let app = production_app(master_ref, dedupe_store);

        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"namespace":"test-ns","workflow_type":"test-wf","paradigm":"fsm","input":{"key":"val"},"dedupe_key":"dk-prod-42","command_envelope":{"version":1,"command_id":"cmd-prod-42","correlation_id":"corr-prod-42","causation_id":"cause-prod-42","issuer":"api_client","issued_at":1700000000}}"#,
            ))
            .unwrap();

        let resp = app.oneshot(req).await.expect("oneshot");
        let status = resp.status();
        let (_parts, body_stream) = resp.into_parts();
        let body_bytes = axum::body::to_bytes(body_stream, 1 << 20)
            .await
            .expect("read body");
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap_or_default();
        assert_eq!(status, StatusCode::CREATED, "response body: {body_str}");

        let body: Value = serde_json::from_slice(&body_bytes).expect("parse json");
        assert_eq!(body["namespace"], "test-ns");
        assert_eq!(body["workflow_type"], "test-wf");
        assert!(body.get("instance_id").is_some());

        let _ = _handle;
    }

    // ======================================================================
    // BDD: command_envelope required on workflow start (tw-4y6h.17.1)
    // ======================================================================

    #[tokio::test]
    async fn given_workflow_start_request_when_admitted_then_command_envelope_is_required() {
        let (master_ref, _handle) = ractor::Actor::spawn(
            Some("test-envelope-required-orchestrator".to_string()),
            CapturingOrchestrator,
            (),
        )
        .await
        .expect("spawn orchestrator");

        let dedupe_store: Arc<dyn vo_storage::dedupe_partition::DedupeStore> =
            Arc::new(InMemoryDedupeStore::new());

        let app = production_app(master_ref, Arc::clone(&dedupe_store));

        // Given: a workflow start request without command_envelope
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"namespace":"test-ns","workflow_type":"test-wf","paradigm":"fsm","input":{"key":"val"},"dedupe_key":"dk-no-envelope"}"#,
            ))
            .unwrap();

        // When: start_workflow validates admission
        let resp = app.oneshot(req).await.expect("oneshot");
        let status = resp.status();
        let (_parts, body_stream) = resp.into_parts();
        let body_bytes = axum::body::to_bytes(body_stream, 1 << 20)
            .await
            .expect("read body");
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap_or_default();

        // Then: the request is rejected with missing_command_envelope
        assert_eq!(status, StatusCode::BAD_REQUEST, "response body: {body_str}");

        let body: Value = serde_json::from_slice(&body_bytes).expect("parse json");
        assert_eq!(body["error"], "missing_command_envelope");
        assert!(
            body["message"]
                .as_str()
                .unwrap_or("")
                .contains("command_envelope"),
            "error message should mention command_envelope"
        );

        let _ = _handle;
    }

    #[tokio::test]
    async fn given_workflow_start_request_when_admitted_then_command_envelope_with_all_fields_succeeds() {
        let (master_ref, _handle) = ractor::Actor::spawn(
            Some("test-envelope-valid-orchestrator".to_string()),
            CapturingOrchestrator,
            (),
        )
        .await
        .expect("spawn orchestrator");

        let dedupe_store: Arc<dyn vo_storage::dedupe_partition::DedupeStore> =
            Arc::new(InMemoryDedupeStore::new());

        let app = production_app(master_ref, Arc::clone(&dedupe_store));

        // Given: a workflow start request with a valid command_envelope containing
        // command_id, correlation_id, causation_id, issuer, and issued_at
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"namespace":"test-ns","workflow_type":"test-wf","paradigm":"fsm","input":{"key":"val"},"dedupe_key":"dk-valid-envelope","command_envelope":{"version":1,"command_id":"cmd-valid-001","correlation_id":"corr-valid-001","causation_id":"cause-valid-001","issuer":"api_client","issued_at":1700000000}}"#,
            ))
            .unwrap();

        // When: start_workflow validates admission
        let resp = app.oneshot(req).await.expect("oneshot");
        let status = resp.status();
        let (_parts, body_stream) = resp.into_parts();
        let body_bytes = axum::body::to_bytes(body_stream, 1 << 20)
            .await
            .expect("read body");
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap_or_default();

        // Then: the request succeeds (201) — envelope was valid
        assert_eq!(status, StatusCode::CREATED, "response body: {body_str}");

        let body: Value = serde_json::from_slice(&body_bytes).expect("parse json");
        assert_eq!(body["namespace"], "test-ns");
        assert!(body.get("instance_id").is_some());

        let _ = _handle;
    }

    #[tokio::test]
    async fn given_workflow_start_request_when_envelope_missing_field_then_rejected() {
        let (master_ref, _handle) = ractor::Actor::spawn(
            Some("test-envelope-missing-field-orchestrator".to_string()),
            CapturingOrchestrator,
            (),
        )
        .await
        .expect("spawn orchestrator");

        let dedupe_store: Arc<dyn vo_storage::dedupe_partition::DedupeStore> =
            Arc::new(InMemoryDedupeStore::new());

        let app = production_app(master_ref, Arc::clone(&dedupe_store));

        // Given: a workflow start request with an envelope missing command_id
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"namespace":"test-ns","workflow_type":"test-wf","paradigm":"fsm","input":{},"dedupe_key":"dk-missing-field","command_envelope":{"version":1,"correlation_id":"corr-001","causation_id":"cause-001","issuer":"api_client","issued_at":1700000000}}"#,
            ))
            .unwrap();

        // When: start_workflow validates admission
        let resp = app.oneshot(req).await.expect("oneshot");
        let status = resp.status();
        let (_parts, body_stream) = resp.into_parts();
        let body_bytes = axum::body::to_bytes(body_stream, 1 << 20)
            .await
            .expect("read body");
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap_or_default();

        // Then: the request is rejected with invalid_command_envelope
        assert_eq!(status, StatusCode::BAD_REQUEST, "response body: {body_str}");

        let body: Value = serde_json::from_slice(&body_bytes).expect("parse json");
        assert_eq!(body["error"], "invalid_command_envelope");

        let _ = _handle;
    }

    // ======================================================================
    // BDD: exact workflow start without dedupe key → rejected (tw-4y6h.5.1)
    // ======================================================================

    #[tokio::test]
    async fn given_exact_start_without_dedupe_key_when_started_then_request_is_rejected() {
        let (master_ref, _handle) = ractor::Actor::spawn(
            Some("test-dedupe-reject-orchestrator".to_string()),
            CapturingOrchestrator,
            (),
        )
        .await
        .expect("spawn orchestrator");

        let dedupe_store: Arc<dyn vo_storage::dedupe_partition::DedupeStore> =
            Arc::new(InMemoryDedupeStore::new());

        let app = production_app(master_ref, Arc::clone(&dedupe_store));

        // Given: an exact workflow start request with no dedupe_key
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"namespace":"test-ns","workflow_type":"exact-wf","paradigm":"fsm","input":{"key":"val"}}"#,
            ))
            .unwrap();

        // When: start_workflow validates admission
        let resp = app.oneshot(req).await.expect("oneshot");
        let status = resp.status();
        let (_parts, body_stream) = resp.into_parts();
        let body_bytes = axum::body::to_bytes(body_stream, 1 << 20)
            .await
            .expect("read body");
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap_or_default();

        // Then: the request fails with a structured missing-dedupe error
        assert_eq!(status, StatusCode::BAD_REQUEST, "response body: {body_str}");

        let body: Value = serde_json::from_slice(&body_bytes).expect("parse json");
        assert_eq!(body["error"], "missing_dedupe_key");
        assert!(body["message"].as_str().unwrap_or("").contains("dedupe_key"));

        // And: no durable records are written to the dedupe store
        let contains_any = dedupe_store
            .contains(&vo_types::DedupeKey::parse("any-key").unwrap())
            .expect("dedupe store query");
        assert!(!contains_any, "dedupe store must be empty — no records written");

        let _ = _handle;
    }
}

// ===========================================================================
// helpers unit tests (re-exported via vo_api::handlers::helpers)
// ===========================================================================

mod helpers {
    use vo_api::handlers::helpers::*;

    #[test]
    fn split_path_id_valid() {
        let result = split_path_id("payments/01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert!(result.is_some());
        let (ns, id) = result.unwrap();
        assert_eq!(ns, "payments");
        assert_eq!(id.as_str(), "01ARZ3NDEKTSV4RRFFQ69G5FAV");
    }

    #[test]
    fn split_path_id_with_hyphenated_namespace() {
        let result = split_path_id("my-namespace/01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert!(result.is_some());
        let (ns, _) = result.unwrap();
        assert_eq!(ns, "my-namespace");
    }

    #[test]
    fn split_path_id_no_slash() {
        assert!(split_path_id("noslash").is_none());
    }

    #[test]
    fn split_path_id_empty() {
        assert!(split_path_id("").is_none());
    }

    #[test]
    fn split_path_id_only_slash() {
        assert!(split_path_id("/").is_none());
    }

    #[test]
    fn split_path_id_empty_namespace() {
        let result = split_path_id("/01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert!(result.is_some());
        let (ns, _) = result.unwrap();
        assert_eq!(ns, "");
    }

    #[test]
    fn parse_paradigm_fsm() {
        assert!(parse_paradigm("fsm").is_some());
    }

    #[test]
    fn parse_paradigm_dag() {
        assert!(parse_paradigm("dag").is_some());
    }

    #[test]
    fn parse_paradigm_procedural() {
        assert!(parse_paradigm("procedural").is_some());
    }

    #[test]
    fn parse_paradigm_invalid() {
        assert!(parse_paradigm("bogus").is_none());
    }

    #[test]
    fn parse_paradigm_empty() {
        assert!(parse_paradigm("").is_none());
    }

    #[test]
    fn parse_paradigm_case_sensitive() {
        assert!(parse_paradigm("FSM").is_none());
        assert!(parse_paradigm("Fsm").is_none());
    }

    #[test]
    fn paradigm_to_str_fsm() {
        use vo_actor::WorkflowParadigm;
        assert_eq!(paradigm_to_str(WorkflowParadigm::Fsm), "fsm");
    }

    #[test]
    fn paradigm_to_str_dag() {
        use vo_actor::WorkflowParadigm;
        assert_eq!(paradigm_to_str(WorkflowParadigm::Dag), "dag");
    }

    #[test]
    fn paradigm_to_str_procedural() {
        use vo_actor::WorkflowParadigm;
        assert_eq!(paradigm_to_str(WorkflowParadigm::Procedural), "procedural");
    }

    #[test]
    fn phase_to_str_replay() {
        use vo_actor::InstancePhaseView;
        assert_eq!(phase_to_str(InstancePhaseView::Replay), "replay");
    }

    #[test]
    fn phase_to_str_live() {
        use vo_actor::InstancePhaseView;
        assert_eq!(phase_to_str(InstancePhaseView::Live), "live");
    }

    #[test]
    fn paradigm_roundtrip() {
        use vo_actor::WorkflowParadigm;
        for p in [WorkflowParadigm::Fsm, WorkflowParadigm::Dag, WorkflowParadigm::Procedural] {
            let s = paradigm_to_str(p.clone());
            let back = parse_paradigm(s).unwrap();
            assert_eq!(back, p);
        }
    }

    #[test]
    fn phase_roundtrip() {
        use vo_actor::InstancePhaseView;
        for p in [InstancePhaseView::Replay, InstancePhaseView::Live] {
            let s = phase_to_str(p.clone());
            let back = match s {
                "replay" => InstancePhaseView::Replay,
                "live" => InstancePhaseView::Live,
                _ => panic!("unexpected phase: {s}"),
            };
            assert_eq!(back, p);
        }
    }
}

// ===========================================================================
// cross-cutting: error envelope shape
// ===========================================================================

mod error_envelope {
    use super::*;

    #[tokio::test]
    async fn all_error_responses_have_json_content_type() {
        let endpoints = vec![
            ("/api/v1/workflows/bad/events", "GET"),
            ("/api/v1/workflows/bad", "GET"),
            ("/api/v1/workflows/bad/status", "GET"),
            ("/api/v1/workflows/bad/signals", "POST"),
            ("/api/v1/workflows/bad/unquarantine", "POST"),
            ("/api/v1/workflows/bad/compensate", "POST"),
        ];
        for (uri, method) in endpoints {
            let mut builder = Request::builder().method(method).uri(uri);
            if method == "POST" {
                builder = builder.header(header::CONTENT_TYPE, "application/json");
            }
            let req = builder.body(Body::empty()).unwrap();
            let resp = app().oneshot(req).await.unwrap();
            let ct = resp
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            assert!(
                ct.contains("application/json"),
                "URI {uri}: expected json content-type, got: {ct}"
            );
        }
    }

    #[tokio::test]
    async fn error_bodies_are_valid_json() {
        let endpoints = vec![
            "/api/v1/workflows/bad/events",
            "/api/v1/workflows/bad",
            "/api/v1/workflows/bad/status",
        ];
        for uri in endpoints {
            let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
            let (status, body) = send(req).await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
            let v: Value = serde_json::from_str(&body)
                .unwrap_or_else(|e| panic!("URI {uri}: body not valid JSON: {e}\nbody: {body}"));
            assert!(v.get("error").is_some(), "URI {uri}: missing 'error' field");
            assert!(v.get("message").is_some(), "URI {uri}: missing 'message' field");
        }
    }
}

// ===========================================================================
// method enforcement
// ===========================================================================

mod method_enforcement {
    use super::*;

    #[tokio::test]
    async fn get_on_post_only_returns_405() {
        let req = Request::builder()
            .uri("/api/v1/workflows/ns%2Finst/compensate")
            .body(Body::empty())
            .unwrap();
        let resp = app().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn post_on_get_only_returns_405() {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/workflows/ns%2Finst/events")
            .body(Body::empty())
            .unwrap();
        let resp = app().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn put_on_any_route_returns_405() {
        let req = Request::builder()
            .method("PUT")
            .uri("/api/v1/workflows")
            .body(Body::from("{}"))
            .unwrap();
        let resp = app().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    }
}
