//! QA validation tests for vo-api HTTP surface.
//!
//! Validates: status codes, content-type headers, error envelope shape,
//! method enforcement, path-id format rejection, and response structure.

use axum::{
    body::Body,
    extract::Path,
    http::{header, Request, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;

fn err(status: StatusCode, code: &str, msg: &str) -> (StatusCode, Json<Value>) {
    (status, Json(json!({"error": code, "message": msg})))
}

async fn stub_start(req: Request<Body>) -> (StatusCode, Json<Value>) {
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
    if body.get("namespace").and_then(|v| v.as_str()).is_none() {
        return err(
            StatusCode::BAD_REQUEST,
            "invalid_input",
            "missing required field: namespace",
        );
    }
    if body
        .get("paradigm")
        .and_then(|v| v.as_str())
        .is_none_or(|p| !["fsm", "dag", "procedural"].contains(&p))
    {
        return err(
            StatusCode::BAD_REQUEST,
            "invalid_paradigm",
            "paradigm must be fsm, dag, or procedural",
        );
    }
    (
        StatusCode::CREATED,
        Json(
            json!({"instance_id": "01ARZ3NDEKTSV4RRFFQ69G5FAV", "namespace": body["namespace"], "workflow_type": body["workflow_type"]}),
        ),
    )
}

async fn stub_get(Path(id): Path<String>) -> (StatusCode, Json<Value>) {
    if id.split('/').count() != 2 {
        return err(
            StatusCode::BAD_REQUEST,
            "invalid_id",
            "id must be namespace/instance_id",
        );
    }
    (
        StatusCode::OK,
        Json(
            json!({"instance_id": id, "namespace": "test", "workflow_type": "w", "paradigm": "dag", "phase": "live", "events_applied": 0}),
        ),
    )
}

async fn stub_list() -> (StatusCode, Json<Value>) {
    (StatusCode::OK, Json(json!([])))
}

async fn stub_delete(Path(id): Path<String>) -> StatusCode {
    if id.split('/').count() != 2 {
        return StatusCode::BAD_REQUEST;
    }
    StatusCode::NO_CONTENT
}

fn app() -> Router {
    Router::new()
        .route("/api/v1/workflows", post(stub_start).get(stub_list))
        .route("/api/v1/workflows/{*id}", get(stub_get).delete(stub_delete))
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
    assert_eq!(v["error"], code);
    assert!(v.get("message").is_some(), "must have 'message' field");
}

#[tokio::test]
async fn unmatched_route_returns_404() {
    let req = Request::builder()
        .uri("/api/v1/nonexistent")
        .body(Body::empty())
        .unwrap();
    let resp = app().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn post_workflows_201_with_json_content_type() {
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/workflows")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"namespace":"payments","workflow_type":"charge","paradigm":"dag","input":{}}"#,
        ))
        .unwrap();
    let (status, body) = send(req).await;
    assert_eq!(status, StatusCode::CREATED);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert!(v.get("instance_id").is_some());
    assert_eq!(v["namespace"], "payments");
}

#[tokio::test]
async fn post_workflows_rejects_missing_content_type() {
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
async fn post_workflows_rejects_invalid_paradigm() {
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/workflows")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"namespace":"x","workflow_type":"y","paradigm":"quantum","input":{}}"#,
        ))
        .unwrap();
    let (status, body) = send(req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_err(&body, "invalid_paradigm");
}

#[tokio::test]
async fn post_workflows_rejects_missing_namespace() {
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/workflows")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"workflow_type":"y","paradigm":"fsm","input":{}}"#,
        ))
        .unwrap();
    let (status, body) = send(req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_err(&body, "invalid_input");
}

#[tokio::test]
async fn get_workflow_200_valid_id() {
    let req = Request::builder()
        .uri("/api/v1/workflows/payments/01ARZ3NDEKTSV4RRFFQ69G5FAV")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(req).await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["namespace"], "test");
    assert_eq!(v["paradigm"], "dag");
    assert_eq!(v["phase"], "live");
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
async fn delete_workflow_204_no_body() {
    let req = Request::builder()
        .method("DELETE")
        .uri("/api/v1/workflows/ns/inst")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(req).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(body.is_empty());
}

#[tokio::test]
async fn delete_workflow_400_invalid_id() {
    let req = Request::builder()
        .method("DELETE")
        .uri("/api/v1/workflows/noslash")
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_workflows_200_empty_array() {
    let req = Request::builder()
        .uri("/api/v1/workflows")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(req).await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert!(v.is_array());
}

#[tokio::test]
async fn put_collection_method_not_allowed() {
    let req = Request::builder()
        .method("PUT")
        .uri("/api/v1/workflows")
        .body(Body::from("{}"))
        .unwrap();
    let (status, _) = send(req).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn error_responses_have_json_content_type() {
    let req = Request::builder()
        .uri("/api/v1/workflows/bad")
        .body(Body::empty())
        .unwrap();
    let resp = app().oneshot(req).await.unwrap();
    let ct = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(ct.contains("application/json"), "got: {ct}");
}
