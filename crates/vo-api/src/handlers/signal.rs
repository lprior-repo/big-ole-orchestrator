use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use ractor::ActorRef;
use std::time::Duration;
use vo_actor::OrchestratorMsg;
use vo_storage::dedupe_partition::DedupeStore;
use vo_storage::event_log::{append_event, AppendEventRequest};
use vo_types::events::EventMetadata;

use crate::handlers::helpers::split_path_id;
use crate::types::ApiError;

const ACTOR_CALL_TIMEOUT: Duration = Duration::from_secs(5);

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

async fn abort_reserved_transition(
    master: &ActorRef<OrchestratorMsg>,
    namespace: String,
    instance_id: vo_types::InstanceId,
) {
    match master
        .call(
            |tx| OrchestratorMsg::AbortWorkflowTransition {
                namespace,
                instance_id,
                reply: tx,
            },
            Some(ACTOR_CALL_TIMEOUT),
        )
        .await
    {
        Ok(CallResult::Success(())) => {}
        Ok(other) => tracing::warn!(?other, "failed to abort reserved signal transition"),
        Err(error) => tracing::warn!(?error, "failed to abort reserved signal transition"),
    }
}

fn signal_preflight_rejection(
    call_result: Result<
        CallResult<Result<(), vo_actor::SignalError>>,
        ractor::MessagingErr<OrchestratorMsg>,
    >,
) -> Option<axum::response::Response> {
    match call_result {
        Err(e) => Some(
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ApiError::new("actor_unavailable", e.to_string())),
            )
                .into_response(),
        ),
        Ok(CallResult::Timeout) => Some(
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ApiError::new(
                    "actor_timeout",
                    "orchestrator did not respond in time",
                )),
            )
                .into_response(),
        ),
        Ok(CallResult::SenderError) => Some(
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "actor_error",
                    "orchestrator dropped the reply",
                )),
            )
                .into_response(),
        ),
        Ok(CallResult::Success(Err(e))) => Some(
            (
                StatusCode::NOT_FOUND,
                Json(ApiError::new("signal_failed", e.to_string())),
            )
                .into_response(),
        ),
        Ok(CallResult::Success(Ok(()))) => None,
    }
}

fn persist_lifecycle_event(
    db: &fjall::Database,
    namespace: &str,
    instance_id: &vo_types::InstanceId,
    payload: serde_json::Value,
) -> Result<(), vo_storage::codec::StorageError> {
    let annotations = HashMap::from([("namespace".to_string(), serde_json::json!(namespace))]);
    append_event(
        db,
        AppendEventRequest {
            namespace: namespace.to_string(),
            instance_id: instance_id.clone(),
            timestamp_ms: now_ms(),
            payload,
            metadata: EventMetadata {
                command_metadata: None,
                annotations,
            },
        },
    )
    .map(|_| ())
}

fn now_ms() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => u64::try_from(duration.as_millis()).map_or(u64::MAX, |value| value),
        Err(_) => 0,
    }
}
