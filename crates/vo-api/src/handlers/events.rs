use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use ractor::ActorRef;
use vo_actor::OrchestratorMsg;

use crate::types::ApiError;

/// GET /api/v1/workflows/:id/events — fetch the JetStream event log (bead vo-k0ck).
pub async fn get_events(
    Extension(_master): Extension<ActorRef<OrchestratorMsg>>,
    Path(_id): Path<String>,
) -> impl IntoResponse {
    // Full implementation requires JetStream access injected via Extension<Context>.
    // That's wired up in bead vo-k0ck alongside the NATS connection setup.
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(ApiError::new(
            "not_implemented",
            "event log streaming: see bead vo-k0ck",
        )),
    )
        .into_response()
}
