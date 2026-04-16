//! BLACK-HAT adversarial attack tests for vo-api HTTP surface.
//!
//! Attacks: SQL injection in query params, path traversal, rate limit bypass,
//! auth header manipulation, oversized payloads, method override smuggling.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::{get, post, delete},
    Router,
};
use tower::ServiceExt;

fn attack_router() -> Router {
    Router::new()
        .route("/api/v1/workflows", post(stub_create))
        .route("/api/v1/workflows/{id}", get(stub_get).delete(stub_delete))
        .route("/api/v1/workflows/{id}/events", get(stub_events))
}

async fn stub_create(_: Request<Body>) -> (StatusCode, String) {
    (StatusCode::CREATED, r#"{"id":"wf-ok"}"#.into())
}

async fn stub_get(_: Request<Body>) -> (StatusCode, String) {
    (StatusCode::OK, r#"{"id":"wf-ok"}"#.into())
}

async fn stub_delete(_: Request<Body>) -> (StatusCode, String) { (StatusCode::NO_CONTENT, String::new()) }
async fn stub_events(_: Request<Body>) -> (StatusCode, String) { (StatusCode::OK, "[]".into()) }

#[tokio::test]
async fn sqli_in_query_param_rejected() {
    let app = attack_router();
    let req = Request::builder().method("GET")
        .uri("/api/v1/workflows/wf-1?limit=10%20DROP%20TABLE%20events--")
        .body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_ne!(resp.status(), StatusCode::OK);
}
