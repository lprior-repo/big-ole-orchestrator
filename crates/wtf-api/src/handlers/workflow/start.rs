use std::time::Duration;
use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use bytes::Bytes;
use ractor::rpc::CallResult;
use ractor::ActorRef;
use ulid::Ulid;
use wtf_actor::{OrchestratorMsg, StartError};
use wtf_common::{InstanceId, NamespaceId};

use crate::types::{ApiError, V3StartRequest, V3StartResponse};
use crate::handlers::helpers::parse_paradigm;

const ACTOR_CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// POST /api/v1/workflows — start a new workflow instance (bead wtf-7mif).
pub async fn start_workflow(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Json(req): Json<V3StartRequest>,
) -> impl IntoResponse {
    // Validate namespace.
    let namespace = match NamespaceId::try_new(&req.namespace) {
        Ok(ns) => ns,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    "invalid_namespace",
                    format!("namespace contains illegal characters: {:?}", req.namespace),
                )),
            )
                .into_response();
        }
    };

    // Parse paradigm.
    let paradigm = match parse_paradigm(&req.paradigm) {
        Some(p) => p,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    "invalid_paradigm",
                    format!(
                        "paradigm must be 'fsm', 'dag', or 'procedural', got: {:?}",
                        req.paradigm
                    ),
                )),
            )
                .into_response();
        }
    };

    // Generate or validate instance_id.
    let instance_id = match req.instance_id {
        Some(ref id) => match InstanceId::try_new(id) {
            Ok(id) => id,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ApiError::new(
                        "invalid_instance_id",
                        "instance_id contains NATS-illegal characters",
                    )),
                )
                    .into_response();
            }
        },
        None => InstanceId::new(Ulid::new().to_string()),
    };

    // Serialize input to msgpack bytes.
    let input = match serde_json::to_vec(&req.input) {
        Ok(v) => Bytes::from(v),
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    "invalid_input",
                    format!("failed to encode input: {e}"),
                )),
            )
                .into_response();
        }
    };

    let workflow_type = req.workflow_type.clone();
    let captured_namespace = namespace.clone();
    let captured_id = instance_id.clone();

    let call_result = master
        .call(
            |tx| OrchestratorMsg::StartWorkflow {
                namespace,
                instance_id,
                workflow_type,
                paradigm,
                input,
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
        Ok(CallResult::Success(Err(StartError::AtCapacity { running, max }))) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new(
                "at_capacity",
                format!("engine at capacity: {running}/{max} instances running"),
            )),
        )
            .into_response(),
        Ok(CallResult::Success(Err(StartError::AlreadyExists(id)))) => (
            StatusCode::CONFLICT,
            Json(ApiError::new(
                "already_exists",
                format!("instance {id} already exists"),
            )),
        )
            .into_response(),
        Ok(CallResult::Success(Err(StartError::SpawnFailed(msg)))) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("spawn_failed", msg)),
        )
            .into_response(),
        Ok(CallResult::Success(Ok(_))) => (
            StatusCode::CREATED,
            Json(V3StartResponse {
                instance_id: captured_id.to_string(),
                namespace: captured_namespace.to_string(),
                workflow_type: req.workflow_type,
            }),
        )
            .into_response(),
    }
}
