use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use bytes::Bytes;
use ractor::rpc::CallResult;
use ractor::ActorRef;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use vo_actor::OrchestratorMsg;
use vo_storage::dedupe_partition::DedupeStore;
use vo_storage::event_log::{append_event, AppendEventRequest};
use vo_types::events::EventMetadata;

use crate::handlers::helpers::split_path_id;
use crate::handlers::ingress::{admit_signal, IngressAdmission, DEFAULT_DEDUPE_TTL_MS};
use crate::types::{ApiError, V3SignalRequest};

const ACTOR_CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// POST /api/v1/workflows/:id/signals -- send a signal to a running instance (bead vo-meua).
///
/// Per ADR-028, signals are deduplicated using a composite key of
/// signal_name + instance_id to prevent duplicate signal delivery.
#[tracing::instrument(skip_all)]
pub async fn send_signal(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Extension(dedupe_store): Extension<Arc<dyn DedupeStore>>,
    Extension(event_db): Extension<Arc<fjall::Database>>,
    Path(id): Path<String>,
    Json(req): Json<V3SignalRequest>,
) -> impl IntoResponse {
    let (namespace, instance_id) = match split_path_id(&id) {
        Some(pair) => pair,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    "invalid_id",
                    "id must be <namespace>/<instance_id>",
                )),
            )
                .into_response();
        }
    };

    let actor_namespace = namespace.clone();
    let event_instance_id = instance_id.clone();

    let preflight_result = master
        .call(
            |tx| OrchestratorMsg::ReserveSignal {
                namespace: actor_namespace.clone(),
                instance_id: instance_id.clone(),
                signal_name: req.signal_name.clone(),
                reply: tx,
            },
            Some(ACTOR_CALL_TIMEOUT),
        )
        .await;

    if let Some(response) = signal_preflight_rejection(preflight_result) {
        return response;
    }

    // ADR-028: Signal dedupe check using composite key.
    // Uses namespace + signal_name + instance_id as the dedupe key source.
    let signal_dedupe_source =
        format!("{}:{}:{}", namespace, req.signal_name, instance_id.as_str());
    match admit_signal(
        dedupe_store.as_ref(),
        &namespace,
        &instance_id,
        &req.signal_name,
        &signal_dedupe_source,
        DEFAULT_DEDUPE_TTL_MS,
    ) {
        Ok(IngressAdmission::Admitted) => {}
        Ok(IngressAdmission::Duplicate {
            existing_instance_id,
        }) => {
            return (
                StatusCode::CONFLICT,
                Json(ApiError::new(
                    "duplicate_signal",
                    format!(
                        "signal '{}' already delivered to instance {}",
                        req.signal_name, existing_instance_id
                    ),
                )),
            )
                .into_response();
        }
        Err(e) => {
            // Log but do not block signal delivery on dedupe store errors.
            // This follows the ADR-028 principle that dedupe failures should
            // be visible but not block the critical path.
            tracing::warn!(error = %e, "dedupe check failed for signal, proceeding anyway");
        }
    }

    // Serialize signal payload to bytes.
    let payload = match serde_json::to_vec(&req.payload) {
        Ok(v) => Bytes::from(v),
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    "invalid_payload",
                    format!("failed to encode payload: {e}"),
                )),
            )
                .into_response();
        }
    };

    if let Err(error) = persist_lifecycle_event(
        &event_db,
        &namespace,
        &event_instance_id,
        serde_json::json!({
            "type": "SignalAccepted",
            "namespace": namespace.clone(),
            "signal_name": req.signal_name.clone(),
        }),
    ) {
        abort_reserved_transition(&master, namespace.clone(), event_instance_id.clone()).await;
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("event_persist_failed", error.to_string())),
        )
            .into_response();
    }

    let call_result = master
        .call(
            |tx| OrchestratorMsg::CommitSignal {
                namespace: actor_namespace,
                instance_id,
                signal_name: req.signal_name.clone(),
                payload,
                reply: tx,
            },
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
                "orchestrator did not respond in time",
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
        Ok(CallResult::Success(Err(e))) => (
            StatusCode::NOT_FOUND,
            Json(ApiError::new("signal_failed", e.to_string())),
        )
            .into_response(),
        Ok(CallResult::Success(Ok(()))) => StatusCode::ACCEPTED.into_response(),
    }
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
