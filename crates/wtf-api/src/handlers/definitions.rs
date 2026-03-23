use crate::types::{ApiError, DefinitionRequest, DefinitionResponse, DiagnosticDto};
use axum::{extract::Path, http::StatusCode, response::IntoResponse, Json};

/// POST /api/v1/definitions/:type — ingest and lint a workflow definition (bead wtf-qyxl).
pub async fn ingest_definition(
    Path(_definition_type): Path<String>,
    Json(req): Json<DefinitionRequest>,
) -> impl IntoResponse {
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
            (
                StatusCode::OK,
                Json(DefinitionResponse {
                    valid,
                    diagnostics: dtos,
                }),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("parse_error", e.to_string())),
        )
            .into_response(),
    }
}
