use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use ractor::ActorRef;
use std::time::Duration;
use vo_actor::OrchestratorMsg;

<<<<<<< HEAD
use crate::types::ApiError;
=======
use crate::handlers::helpers::split_path_id;
use crate::types::{ApiError, V3SignalRequest};

const ACTOR_CALL_TIMEOUT: Duration = Duration::from_secs(5);
>>>>>>> 7e356012 (style: apply consistent rustfmt formatting)

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
