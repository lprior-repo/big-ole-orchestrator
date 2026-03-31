use std::time::Duration;
use axum::{
    extract::Extension,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use ractor::rpc::CallResult;
use ractor::ActorRef;
use wtf_actor::OrchestratorMsg;

use crate::types::{ApiError, V3StatusResponse};
use crate::handlers::helpers::{paradigm_to_str, phase_to_str};

const ACTOR_CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// GET /api/v1/workflows — list all active workflow instances.
pub async fn list_workflows(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
) -> impl IntoResponse {
    let call_result = master
        .call(
            |tx| OrchestratorMsg::ListActive { reply: tx },
            Some(ACTOR_CALL_TIMEOUT),
        )
        .await;

    match call_result {
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new("actor_unavailable", e.to_string())),
        )
            .into_response(),
        Ok(CallResult::Timeout) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new(
                "actor_timeout",
                "orchestrator did not respond",
            )),
        )
            .into_response(),
        Ok(CallResult::SenderError) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "actor_error",
                "orchestrator dropped the reply",
            )),
        )
            .into_response(),
        Ok(CallResult::Success(snapshots)) => {
            let views: Vec<V3StatusResponse> = snapshots
                .into_iter()
                .map(|s| V3StatusResponse {
                    instance_id: s.instance_id.to_string(),
                    namespace: s.namespace.to_string(),
                    workflow_type: s.workflow_type,
                    paradigm: paradigm_to_str(s.paradigm).to_owned(),
                    phase: phase_to_str(s.phase).to_owned(),
                    events_applied: s.events_applied,
                })
                .collect();
            (StatusCode::OK, Json(views)).into_response()
        }
    }
}
