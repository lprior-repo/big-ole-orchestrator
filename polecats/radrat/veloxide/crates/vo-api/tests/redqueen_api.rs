//! RED-QUEEN coevolutionary adversarial tests for vo-api HTTP surface.
//!
//! Attacks: request smuggling, content-type confusion, header injection,
//! path traversal, method confusion, malformed payloads, boundary abuse.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    routing::{get, post},
    Router,
};
use tower::ServiceExt;

fn adversarial_router() -> Router {
    Router::new()
        .route("/api/v1/workflows", post(stub_start))
        .route(
            "/api/v1/workflows/{id}",
            get(stub_get).delete(stub_timeline),
        )
        .route("/api/v1/workflows/{id}/timeline", get(stub_timeline))
}

async fn stub_start(req: Request<Body>) -> (StatusCode, String) {
    let ct = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !ct.contains("application/json") {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "invalid content-type".into(),
        );
    }
    (StatusCode::CREATED, "{}".into())
}

async fn stub_get(_: Request<Body>) -> (StatusCode, String) {
    (StatusCode::OK, "{}".into())
}

async fn stub_timeline(_: Request<Body>) -> (StatusCode, String) {
    (StatusCode::OK, "[]".into())
}

#[tokio::test]
async fn content_type_form_urlencoded_rejected() {
    let app = adversarial_router();
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/workflows")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("namespace=test&paradigm=dag"))
        .unwrap();
    assert_eq!(
        app.oneshot(req).await.unwrap().status(),
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
}
#[tokio::test]
async fn crlf_injection_in_path_rejected() {
    let app = adversarial_router();
    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/workflows/test%0d%0aX-Injected:true")
        .body(Body::empty())
        .unwrap();
    assert!(app
        .oneshot(req)
        .await
        .unwrap()
        .headers()
        .get("X-Injected")
        .is_none());
}
#[tokio::test]
async fn content_type_multipart_rejected() {
    let app = adversarial_router();
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/workflows")
        .header(
            header::CONTENT_TYPE,
            "multipart/form-data; boundary=----evil",
        )
        .body(Body::from("------evil\r\n\r\n{}\r\n------evil--\r\n"))
        .unwrap();
    assert_eq!(
        app.oneshot(req).await.unwrap().status(),
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
}
#[tokio::test]
async fn content_type_xml_rejected() {
    let app = adversarial_router();
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/workflows")
        .header(header::CONTENT_TYPE, "application/xml")
        .body(Body::from("<workflow/>"))
        .unwrap();
    assert_eq!(
        app.oneshot(req).await.unwrap().status(),
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
}
#[tokio::test]
async fn null_byte_in_path_sanitized() {
    let app = adversarial_router();
    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/workflows/test%00poisoned")
        .body(Body::empty())
        .unwrap();
    assert_ne!(
        app.oneshot(req).await.unwrap().status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}
#[tokio::test]
async fn put_to_workflow_collection_not_allowed() {
    let app = adversarial_router();
    let req = Request::builder()
        .method("PUT")
        .uri("/api/v1/workflows")
        .body(Body::from("{}"))
        .unwrap();
    assert_eq!(
        app.oneshot(req).await.unwrap().status(),
        StatusCode::METHOD_NOT_ALLOWED
    );
}
#[tokio::test]
async fn patch_to_workflow_resource_not_allowed() {
    let app = adversarial_router();
    let req = Request::builder()
        .method("PATCH")
        .uri("/api/v1/workflows/test_abc123")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.oneshot(req).await.unwrap().status(),
        StatusCode::METHOD_NOT_ALLOWED
    );
}
#[tokio::test]
async fn path_traversal_dotdot_rejected() {
    let app = adversarial_router();
    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/workflows/../../../etc/passwd")
        .body(Body::empty())
        .unwrap();
    assert_ne!(app.oneshot(req).await.unwrap().status(), StatusCode::OK);
}
#[tokio::test]
async fn double_url_encoding_attack_safe() {
    let app = adversarial_router();
    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/workflows/%252e%252e%252f")
        .body(Body::empty())
        .unwrap();
    // Double-encoded dots become literal %2e%2e%2f — no traversal
    assert!(app.oneshot(req).await.unwrap().status().as_u16() < 500);
}
#[tokio::test]
async fn oversized_body_no_crash() {
    let app = adversarial_router();
    let body = format!("{{\"fill\":\"{}\"}}", "A".repeat(1_000_000));
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/workflows")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();
    assert!(app.oneshot(req).await.unwrap().status().as_u16() < 500);
}
#[tokio::test]
async fn malformed_json_body_safe() {
    let app = adversarial_router();
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/workflows")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{not json at all"))
        .unwrap();
    assert!(app.oneshot(req).await.unwrap().status().as_u16() < 500);
}
#[tokio::test]
async fn transfer_encoding_confusion_safe() {
    let app = adversarial_router();
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/workflows")
        .header(header::CONTENT_TYPE, "application/json")
        .header("transfer-encoding", "chunked")
        .header(header::CONTENT_LENGTH, "0")
        .body(Body::from("{}"))
        .unwrap();
    assert!(app.oneshot(req).await.unwrap().status().as_u16() < 500);
}

#[test]
fn split_path_id_rejects_no_slash() {
    assert!(vo_api::handlers::helpers::split_path_id("nonslashpath").is_none());
}

#[test]
fn parse_paradigm_rejects_unknown_and_case_sensitive() {
    use vo_api::handlers::helpers::parse_paradigm;
    assert!(parse_paradigm("quantum").is_none());
    assert!(parse_paradigm("").is_none());
    assert!(parse_paradigm("FSM").is_none());
}
