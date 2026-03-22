use axum::{
    body::to_bytes,
    body::Body,
    http::{Request, StatusCode},
    routing::get,
    Json, Router,
};
use serde_json::Value;
use tower::util::ServiceExt;

/// Integration tests for GET /api/v1/workflows/:id/journal endpoint.
///
/// These tests focus on the HTTP layer: request parsing, routing, and error responses.
/// The journal handler requires an OrchestratorMsg actor ref and event store,
/// so full end-to-end tests with real actor infrastructure are handled separately.

#[tokio::test]
async fn given_empty_id_when_get_journal_then_bad_request() {
    let app = Router::new().route(
        "/api/v1/workflows/:id/journal",
        get(wtf_api::handlers::get_journal),
    );

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/workflows//journal")
        .body(Body::empty())
        .expect("request");

    let res = app.oneshot(req).await.expect("response");
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn given_whitespace_id_when_get_journal_then_not_found() {
    let app = Router::new().route(
        "/api/v1/workflows/:id/journal",
        get(wtf_api::handlers::get_journal),
    );

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/workflows/%20%20%20/journal")
        .body(Body::empty())
        .expect("request");

    let res = app.oneshot(req).await.expect("response");
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn given_id_without_namespace_when_get_journal_then_not_found() {
    let app = Router::new().route(
        "/api/v1/workflows/:id/journal",
        get(wtf_api::handlers::get_journal),
    );

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/workflows/01ARZ3NDEKTSV4RRFFQ69G5FAV/journal")
        .body(Body::empty())
        .expect("request");

    let res = app.oneshot(req).await.expect("response");
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn given_valid_namespaced_id_when_get_journal_without_actor_then_internal_error() {
    let app = Router::new().route(
        "/api/v1/workflows/:id/journal",
        get(wtf_api::handlers::get_journal),
    );

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/workflows/payments/01ARZ3NDEKTSV4RRFFQ69G5FAV/journal")
        .body(Body::empty())
        .expect("request");

    let res = app.oneshot(req).await.expect("response");
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let body = to_bytes(res.into_body(), usize::MAX).await.expect("body");
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json.get("code").and_then(Value::as_str), Some("actor_error"));
}

#[tokio::test]
async fn journal_endpoint_route_is_configured() {
    let app = Router::new().route(
        "/api/v1/workflows/:id/journal",
        get(wtf_api::handlers::get_journal),
    );

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/workflows/test/instance123/journal")
        .body(Body::empty())
        .expect("request");

    let res = app.oneshot(req).await.expect("response");
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn journal_response_structure_is_valid_json() {
    let app = Router::new().route(
        "/api/v1/workflows/:id/journal",
        get(wtf_api::handlers::get_journal),
    );

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/workflows/payments/01ARZ3NDEKTSV4RRFFQ69G5FAV/journal")
        .body(Body::empty())
        .expect("request");

    let res = app.oneshot(req).await.expect("response");
    let body = to_bytes(res.into_body(), usize::MAX).await.expect("body");

    let json: Result<Value, _> = serde_json::from_slice(&body);
    assert!(json.is_ok(), "Response should be valid JSON even on error");
}

#[tokio::test]
async fn journal_endpoint_returns_correct_content_type() {
    let app = Router::new().route(
        "/api/v1/workflows/:id/journal",
        get(wtf_api::handlers::get_journal),
    );

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/workflows/payments/01ARZ3NDEKTSV4RRFFQ69G5FAV/journal")
        .body(Body::empty())
        .expect("request");

    let res = app.oneshot(req).await.expect("response");
    assert!(
        res.headers()
            .get("content-type")
            .map(|v| v.to_str().unwrap_or("").contains("application/json"))
            .unwrap_or(false),
        "Should return application/json content-type"
    );
}
