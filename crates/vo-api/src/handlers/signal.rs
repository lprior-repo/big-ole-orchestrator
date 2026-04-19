use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use ractor::ActorRef;
use vo_actor::OrchestratorMsg;

use crate::types::ApiError;

/// POST /api/v1/workflows/:id/signals — send a signal to a running instance (bead vo-meua).
///
/// Temporarily returns 501 until OrchestratorMsg gains a Signal variant.
#[tracing::instrument(skip_all)]
pub async fn send_signal(
    Extension(_master): Extension<ActorRef<OrchestratorMsg>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let _ = split_path_id(&id);
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(ApiError::new(
            "not_implemented",
            "signal dispatch: awaiting OrchestratorMsg::Signal variant (see bead vo-meua)",
        )),
    )
        .into_response()
}

use crate::handlers::helpers::split_path_id;
