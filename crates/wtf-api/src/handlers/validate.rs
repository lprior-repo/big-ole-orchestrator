use axum::{http::StatusCode, response::IntoResponse, Json};
use std::panic::AssertUnwindSafe;

use crate::types::{ApiError, DiagnosticDto, ValidateWorkflowRequest, ValidateWorkflowResponse};

/// POST /api/v1/workflows/validate — lint workflow Rust source.
pub async fn validate_workflow(Json(req): Json<ValidateWorkflowRequest>) -> impl IntoResponse {
    let lint_result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        wtf_linter::lint_workflow_source(&req.source)
    }));

    match lint_result {
        Ok(Ok(result)) => {
            let diagnostics = result
                .diagnostics
                .into_iter()
                .map(|d| DiagnosticDto {
                    code: d.code.as_str().to_owned(),
                    severity: d.severity.to_string(),
                    message: d.message,
                    suggestion: d.suggestion,
                    span: d.span,
                })
                .collect::<Vec<_>>();

            let has_error = diagnostics
                .iter()
                .any(|diag| diag.severity.eq_ignore_ascii_case("error"));

            (
                StatusCode::OK,
                Json(ValidateWorkflowResponse {
                    valid: !has_error,
                    diagnostics,
                }),
            )
                .into_response()
        }
        Ok(Err(e)) => (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("parse_error", e.to_string())),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("internal_error", "linter crashed")),
        )
            .into_response(),
    }
}
