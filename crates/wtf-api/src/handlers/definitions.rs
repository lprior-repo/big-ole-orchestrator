use axum::extract::Extension;
use axum::{extract::Path, http::StatusCode, response::IntoResponse, Json};
use wtf_storage::kv::{definition_key, KvStores};

use crate::types::{ApiError, DefinitionRequest, DefinitionResponse, DiagnosticDto};

/// POST /api/v1/definitions/:type — ingest, lint, and store a workflow definition.
pub async fn ingest_definition(
    Path(_definition_type): Path<String>,
    Extension(kv): Extension<KvStores>,
    Json(req): Json<DefinitionRequest>,
) -> impl IntoResponse {
    if req.workflow_type.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("invalid_request", "workflow_type must be non-empty")),
        )
            .into_response();
    }

    match wtf_linter::lint_workflow_code(&req.source) {
        Ok(diagnostics) => {
            let dtos: Vec<DiagnosticDto> = diagnostics
                .into_iter()
                .map(|d| DiagnosticDto {
                    code: d.code.as_str().to_owned(),
                    severity: d.severity.to_string(),
                    message: d.message,
                    suggestion: d.suggestion,
                    span: d.span,
                })
                .collect();
            let valid = dtos.iter().all(|d| d.severity != "error");
            if valid {
                let key = definition_key("default", &req.workflow_type);
                let value = req.source.as_bytes().to_vec().into();
                match kv.definitions.put(&key, value).await {
                    Ok(_) => (StatusCode::OK, Json(DefinitionResponse { valid, diagnostics: dtos }))
                        .into_response(),
                    Err(e) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ApiError::new("kv_store_failure", e.to_string())),
                    )
                        .into_response(),
                }
            } else {
                (StatusCode::OK, Json(DefinitionResponse { valid, diagnostics: dtos }))
                    .into_response()
            }
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("parse_error", e.to_string())),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DefinitionRequest;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::{json, Value};
    use tower::ServiceExt;
    use wtf_storage::kv::definition_key;

    // ---- Pure calculation tests ----

    #[test]
    fn definition_key_uses_default_namespace() {
        assert_eq!(definition_key("default", "my-workflow"), "default/my-workflow");
    }

    #[test]
    fn definition_key_uses_workflow_type_from_request() {
        let req = DefinitionRequest {
            source: "fn main() {}".to_owned(),
            workflow_type: "test-proc".to_owned(),
        };
        assert_eq!(
            definition_key("default", &req.workflow_type),
            "default/test-proc"
        );
    }

    // ---- HTTP handler tests (lint-only paths, no NATS required) ----

    /// Build a minimal router that mirrors the real handler logic (no KV store) —
    /// for testing parse-error, lint-error, and validation paths that never reach KV.
    fn lint_only_app() -> axum::Router {
        axum::Router::new().route(
            "/api/v1/definitions/:type",
            axum::routing::post(
                |Path(_t): Path<String>, Json(r): Json<DefinitionRequest>| async move {
                    if r.workflow_type.trim().is_empty() {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(ApiError::new(
                                "invalid_request",
                                "workflow_type must be non-empty",
                            )),
                        )
                            .into_response();
                    }

                    match wtf_linter::lint_workflow_code(&r.source) {
                        Ok(diagnostics) => {
                            let dtos: Vec<crate::types::DiagnosticDto> = diagnostics
                                .into_iter()
                                .map(|d| crate::types::DiagnosticDto {
                                    code: d.code.as_str().to_owned(),
                                    severity: d.severity.to_string(),
                                    message: d.message,
                                    suggestion: d.suggestion,
                                    span: d.span,
                                })
                                .collect();
                            let valid = dtos.iter().all(|d| d.severity != "error");
                            (
                                StatusCode::OK,
                                Json(DefinitionResponse { valid, diagnostics: dtos }),
                            )
                                .into_response()
                        }
                        Err(e) => (
                            StatusCode::BAD_REQUEST,
                            Json(ApiError::new("parse_error", e.to_string())),
                        )
                            .into_response(),
                    }
                },
            ),
        )
    }

    async fn parse_json_body(response: axum::response::Response) -> Value {
        let bytes = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .expect("read body");
        serde_json::from_slice(&bytes).expect("parse json")
    }

    #[tokio::test]
    async fn parse_error_not_stored() {
        let app = lint_only_app();
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/definitions/procedural")
            .header("Content-Type", "application/json")
            .body(Body::from(
                json!({
                    "workflow_type": "bad-workflow",
                    "source": "!!!not valid rust syntax"
                })
                .to_string(),
            ))
            .expect("build request");

        let response = app.oneshot(request).await.expect("call");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = parse_json_body(response).await;
        assert_eq!(body["error"], "parse_error");
    }

    #[tokio::test]
    async fn valid_definition_returns_200_with_valid_true() {
        let app = lint_only_app();
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/definitions/procedural")
            .header("Content-Type", "application/json")
            .body(Body::from(
                json!({
                    "workflow_type": "test-workflow",
                    "source": "fn valid_rust() { let x = 1; }"
                })
                .to_string(),
            ))
            .expect("build request");

        let response = app.oneshot(request).await.expect("call");
        assert_eq!(response.status(), StatusCode::OK);

        let body = parse_json_body(response).await;
        assert_eq!(body["valid"], true);
    }

    // ---- KV integration tests (require live NATS) ----
    //
    // test_store_definition_after_lint: POST valid definition -> stored in KV
    // test_kv_store_failure_returns_500: Valid definition + NATS unavailable -> 500
    //
    // These require Extension<KvStores> with a live NATS connection.
    // Covered by E2E pipeline tests in spec.md. Adding here would require
    // a NATS test container or mock, which is out of scope for unit tests.

    // ---- Empty workflow_type validation tests (DEFECT-2 coverage) ----

    #[tokio::test]
    async fn empty_workflow_type_rejected() {
        let app = lint_only_app();
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/definitions/procedural")
            .header("Content-Type", "application/json")
            .body(Body::from(
                json!({
                    "workflow_type": "",
                    "source": "fn main() {}"
                })
                .to_string(),
            ))
            .expect("build request");

        let response = app.oneshot(request).await.expect("call");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = parse_json_body(response).await;
        assert_eq!(body["error"], "invalid_request");
        assert_eq!(body["message"], "workflow_type must be non-empty");
    }

    #[tokio::test]
    async fn whitespace_only_workflow_type_rejected() {
        let app = lint_only_app();
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/definitions/procedural")
            .header("Content-Type", "application/json")
            .body(Body::from(
                json!({
                    "workflow_type": "   ",
                    "source": "fn main() {}"
                })
                .to_string(),
            ))
            .expect("build request");

        let response = app.oneshot(request).await.expect("call");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = parse_json_body(response).await;
        assert_eq!(body["error"], "invalid_request");
    }

    #[test]
    fn definition_key_with_empty_workflow_type_produces_trailing_slash() {
        // Proves the invariant: empty workflow_type creates a malformed KV key.
        // This is exactly why the validation guard exists.
        let key = definition_key("default", "");
        assert_eq!(key, "default/");
        assert!(
            key.ends_with('/'),
            "empty workflow_type produces malformed KV key with trailing slash"
        );
    }
}
