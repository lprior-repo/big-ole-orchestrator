//! Pure data-transformation mappers for workflow HTTP handlers.

#![allow(clippy::type_complexity)]

use super::{paradigm_to_str, parse_paradigm, phase_to_str};
use crate::types::{ApiError, V3StartRequest, V3StartResponse, V3StatusResponse};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use bytes::Bytes;
use ractor::{rpc::CallResult, MessagingErr};
use wtf_actor::{
    GetStatusError, InstanceStatusSnapshot, OrchestratorMsg, StartError, TerminateError,
};
use wtf_common::{InstanceId, NamespaceId, WorkflowParadigm};

pub(crate) fn validate_start_req(
    req: &V3StartRequest,
) -> Result<(NamespaceId, InstanceId, WorkflowParadigm, Bytes), (StatusCode, Json<ApiError>)> {
    let ns = NamespaceId::try_new(&req.namespace).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("invalid_namespace", "bad namespace")),
        )
    })?;
    let p = parse_paradigm(&req.paradigm).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("invalid_paradigm", "bad paradigm")),
        )
    })?;
    let id = req
        .instance_id
        .as_ref()
        .map_or(Ok(InstanceId::new(ulid::Ulid::new().to_string())), |s| {
            InstanceId::try_new(s)
        })
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new("invalid_instance_id", "bad instance_id")),
            )
        })?;
    let input = serde_json::to_vec(&req.input).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("invalid_input", e.to_string())),
        )
    })?;
    Ok((ns, id, p, Bytes::from(input)))
}

pub(crate) fn map_start_result(
    res: Result<CallResult<Result<InstanceId, StartError>>, MessagingErr<OrchestratorMsg>>,
    wf_type: String,
) -> Response {
    match res {
        Ok(CallResult::Success(Ok(id))) => (
            StatusCode::CREATED,
            Json(V3StartResponse {
                instance_id: id.to_string(),
                namespace: "".to_owned(),
                workflow_type: wf_type,
            }),
        )
            .into_response(),
        Ok(CallResult::Success(Err(e))) => map_start_error(e).into_response(),
        _ => map_actor_error(res).into_response(),
    }
}

pub(crate) fn map_start_error(err: StartError) -> Response {
    match err {
        StartError::AtCapacity { running, max } => (
            StatusCode::SERVICE_UNAVAILABLE,
            [("Retry-After", "5")],
            Json(ApiError::new("at_capacity", format!("{running}/{max}"))),
        )
            .into_response(),
        StartError::AlreadyExists(id) => (
            StatusCode::CONFLICT,
            Json(ApiError::new("already_exists", id.to_string())),
        )
            .into_response(),
        StartError::SpawnFailed(msg) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("spawn_failed", msg)),
        )
            .into_response(),
        StartError::PersistenceFailed(msg) => (
            StatusCode::SERVICE_UNAVAILABLE,
            [("Retry-After", "5")],
            Json(ApiError::new("persistence_failed", msg)),
        )
            .into_response(),
    }
}

pub(crate) fn map_status_result(
    res: Result<
        CallResult<Result<Option<InstanceStatusSnapshot>, GetStatusError>>,
        MessagingErr<OrchestratorMsg>,
    >,
    id: String,
) -> Response {
    match res {
        Ok(CallResult::Success(Ok(Some(s)))) => {
            (StatusCode::OK, Json(V3StatusResponse::from(s))).into_response()
        }
        Ok(CallResult::Success(Ok(None))) => {
            (StatusCode::NOT_FOUND, Json(ApiError::new("not_found", id))).into_response()
        }
        Ok(CallResult::Success(Err(GetStatusError::Timeout))) => (
            StatusCode::SERVICE_UNAVAILABLE,
            [("Retry-After", "5")],
            Json(ApiError::new(
                "instance_timeout",
                "instance actor timed out",
            )),
        )
            .into_response(),
        Ok(CallResult::Success(Err(GetStatusError::ActorDied))) => (
            StatusCode::NOT_FOUND,
            Json(ApiError::new("actor_died", "instance actor is dead")),
        )
            .into_response(),
        _ => map_actor_error(res).into_response(),
    }
}

pub(crate) fn map_terminate_result(
    res: Result<CallResult<Result<(), TerminateError>>, MessagingErr<OrchestratorMsg>>,
) -> Response {
    match res {
        Ok(CallResult::Success(Ok(()))) => StatusCode::NO_CONTENT.into_response(),
        Ok(CallResult::Success(Err(TerminateError::NotFound(id)))) => (
            StatusCode::NOT_FOUND,
            Json(ApiError::new("not_found", id.to_string())),
        )
            .into_response(),
        Ok(CallResult::Success(Err(TerminateError::Timeout(id)))) => (
            StatusCode::SERVICE_UNAVAILABLE,
            [("Retry-After", "5")],
            Json(ApiError::new(
                "instance_timeout",
                format!("cancel timed out: {id}"),
            )),
        )
            .into_response(),
        _ => map_actor_error(res).into_response(),
    }
}

pub(crate) fn map_actor_error<T>(
    res: Result<CallResult<T>, MessagingErr<OrchestratorMsg>>,
) -> Response {
    match &res {
        Ok(CallResult::Timeout) => (
            StatusCode::SERVICE_UNAVAILABLE,
            [("Retry-After", "5")],
            Json(ApiError::new(
                "actor_timeout",
                "orchestrator call timed out",
            )),
        )
            .into_response(),
        Ok(CallResult::SenderError) => (
            StatusCode::SERVICE_UNAVAILABLE,
            [("Retry-After", "5")],
            Json(ApiError::new(
                "sender_error",
                "failed to send to orchestrator",
            )),
        )
            .into_response(),
        Err(MessagingErr::ChannelClosed) => (
            StatusCode::SERVICE_UNAVAILABLE,
            [("Retry-After", "5")],
            Json(ApiError::new(
                "channel_closed",
                "orchestrator channel closed",
            )),
        )
            .into_response(),
        Err(MessagingErr::InvalidActorType) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "invalid_actor_type",
                "orchestrator actor type mismatch",
            )),
        )
            .into_response(),
        Err(MessagingErr::SendErr(_)) => (
            StatusCode::SERVICE_UNAVAILABLE,
            [("Retry-After", "5")],
            Json(ApiError::new("send_error", "orchestrator mailbox error")),
        )
            .into_response(),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("actor_error", "orchestrator call failed")),
        )
            .into_response(),
    }
}

impl From<InstanceStatusSnapshot> for V3StatusResponse {
    fn from(s: InstanceStatusSnapshot) -> Self {
        Self {
            instance_id: s.instance_id.to_string(),
            namespace: s.namespace.to_string(),
            workflow_type: s.workflow_type,
            paradigm: paradigm_to_str(s.paradigm).to_owned(),
            phase: phase_to_str(s.phase).to_owned(),
            events_applied: s.events_applied,
            current_state: s.current_state,
        }
    }
}
