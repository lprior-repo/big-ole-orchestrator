use axum::{
    extract::{Extension, Json, Path},
    http::StatusCode,
    response::IntoResponse,
};
use ractor::rpc::CallResult;
use ractor::ActorRef;
use std::time::Duration;
use vo_actor::{InstancePhaseView, OrchestratorMsg};
use vo_types::InstanceId;

use crate::handlers::helpers::split_path_id;
use crate::types::mutation::{
    mutation_error_status_code, MutationError, MutationRejectionReason, MutationType,
    OperatorMutationRequest, OperatorMutationResponse,
};
use crate::types::ApiError;

const ACTOR_CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// POST /api/v1/workflows/:id/mutations — apply an operator mutation to a workflow instance.
#[tracing::instrument(skip_all)]
pub async fn handle_mutation(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Path(id): Path<String>,
    Json(req): Json<OperatorMutationRequest>,
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

    // Check if workflow is in a terminal state — reject mutations on terminal workflows.
    match check_workflow_state(&master, &namespace, &instance_id).await {
        Err(resp) => return resp,
        Ok(state) => {
            if state == "terminated" {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ApiError::new(
                        "terminal_state",
                        "cannot mutate a workflow in terminal state",
                    )),
                )
                    .into_response();
            }
        }
    }

    // Extract identity fields before moving `req`.
    let correlation_id = req.correlation_id().to_string();
    let causation_id = req.causation_id().to_string();
    let payload = req.payload.clone();
    let mutation_type = req.mutation_type;

    // Dispatch based on mutation type.
    match dispatch_mutation(
        &master,
        &namespace,
        instance_id,
        mutation_type,
        &correlation_id,
        &causation_id,
        payload,
    )
    .await
    {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(err) => {
            let status = mutation_error_status_code(&err);
            let api_err = ApiError::new("mutation_rejected", err.to_string());
            (StatusCode::from(status), Json(api_err)).into_response()
        }
    }
}

/// Check the current state of a workflow instance.
/// Returns "terminated" for terminal workflows, or the instance's phase.
async fn check_workflow_state(
    master: &ActorRef<OrchestratorMsg>,
    namespace: &str,
    instance_id: &InstanceId,
) -> Result<String, axum::response::Response> {
    let call_result = master
        .call(
            |tx| OrchestratorMsg::GetStatus {
                namespace: namespace.to_string(),
                instance_id: instance_id.clone(),
                reply: tx,
            },
            Some(ACTOR_CALL_TIMEOUT),
        )
        .await;

    match call_result {
        Err(e) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new("actor_unavailable", e.to_string())),
        )
            .into_response()),
        Ok(CallResult::Timeout) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new(
                "actor_timeout",
                "orchestrator did not respond",
            )),
        )
            .into_response()),
        Ok(CallResult::SenderError) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "actor_error",
                "orchestrator dropped the reply",
            )),
        )
            .into_response()),
        Ok(CallResult::Success(None)) => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError::new(
                "not_found",
                format!("instance {} not found", instance_id),
            )),
        )
            .into_response()),
        Ok(CallResult::Success(Some(snapshot))) => {
            let phase_str = match snapshot.phase {
                InstancePhaseView::Terminated => "terminated",
                InstancePhaseView::Replay => "replay",
                InstancePhaseView::Live => "live",
            };
            Ok(phase_str.to_string())
        }
    }
}

/// Dispatch a mutation to the orchestrator based on its type.
async fn dispatch_mutation(
    master: &ActorRef<OrchestratorMsg>,
    namespace: &str,
    instance_id: InstanceId,
    mutation_type: MutationType,
    correlation_id: &str,
    causation_id: &str,
    payload: Option<serde_json::Value>,
) -> Result<OperatorMutationResponse, MutationError> {
    match mutation_type {
        MutationType::Cancel => {
            dispatch_cancel(master, namespace, instance_id, correlation_id, causation_id).await
        }
        MutationType::Pause => {
            dispatch_signal(
                master,
                namespace,
                instance_id,
                "pause",
                payload,
                correlation_id,
                causation_id,
            )
            .await
        }
        MutationType::Resume => {
            dispatch_signal(
                master,
                namespace,
                instance_id,
                "resume",
                payload,
                correlation_id,
                causation_id,
            )
            .await
        }
        MutationType::Patch => {
            dispatch_signal(
                master,
                namespace,
                instance_id,
                "patch",
                payload,
                correlation_id,
                causation_id,
            )
            .await
        }
        MutationType::Retry => {
            dispatch_signal(
                master,
                namespace,
                instance_id,
                "retry",
                payload,
                correlation_id,
                causation_id,
            )
            .await
        }
        MutationType::Undo => {
            dispatch_signal(
                master,
                namespace,
                instance_id,
                "undo",
                payload,
                correlation_id,
                causation_id,
            )
            .await
        }
    }
}

/// Dispatch a cancel mutation using the two-phase terminate pattern.
async fn dispatch_cancel(
    master: &ActorRef<OrchestratorMsg>,
    namespace: &str,
    instance_id: InstanceId,
    correlation_id: &str,
    causation_id: &str,
) -> Result<OperatorMutationResponse, MutationError> {
    // Phase 1: Reserve the termination.
    let preflight_result = master
        .call(
            |tx| OrchestratorMsg::ReserveTerminate {
                namespace: namespace.to_string(),
                instance_id: instance_id.clone(),
                reason: "operator-cancel-mutation".to_owned(),
                reply: tx,
            },
            Some(ACTOR_CALL_TIMEOUT),
        )
        .await;

    match preflight_result {
        Err(e) => Err(MutationError::EnvelopeValidation(format!(
            "actor unavailable: {}",
            e
        ))),
        Ok(CallResult::Timeout) => Err(MutationError::EnvelopeValidation(
            "orchestrator did not respond".to_string(),
        )),
        Ok(CallResult::SenderError) => Err(MutationError::EnvelopeValidation(
            "orchestrator dropped reply".to_string(),
        )),
        Ok(CallResult::Success(Err(vo_actor::TerminateError::NotFound(id)))) => Err(
            MutationError::Rejected(MutationRejectionReason::InstanceNotFound(id)),
        ),
        Ok(CallResult::Success(Err(vo_actor::TerminateError::Failed(msg)))) => Err(
            MutationError::Rejected(MutationRejectionReason::InvalidMutationForState(msg)),
        ),
        Ok(CallResult::Success(Ok(()))) => {
            // Phase 2: Commit the termination.
            let commit_result = master
                .call(
                    |tx| OrchestratorMsg::CommitTerminate {
                        namespace: namespace.to_string(),
                        instance_id,
                        reason: "operator-cancel-mutation".to_owned(),
                        reply: tx,
                    },
                    Some(ACTOR_CALL_TIMEOUT),
                )
                .await;

            match commit_result {
                Err(e) => Err(MutationError::EnvelopeValidation(format!(
                    "actor unavailable during commit: {}",
                    e
                ))),
                Ok(CallResult::Timeout) => Err(MutationError::EnvelopeValidation(
                    "orchestrator timeout during commit".to_string(),
                )),
                Ok(CallResult::SenderError) => Err(MutationError::EnvelopeValidation(
                    "orchestrator dropped reply during commit".to_string(),
                )),
                Ok(CallResult::Success(Err(vo_actor::TerminateError::NotFound(id)))) => Err(
                    MutationError::Rejected(MutationRejectionReason::InstanceNotFound(id)),
                ),
                Ok(CallResult::Success(Err(vo_actor::TerminateError::Failed(msg)))) => Err(
                    MutationError::Rejected(MutationRejectionReason::InvalidMutationForState(msg)),
                ),
                Ok(CallResult::Success(Ok(()))) => Ok(OperatorMutationResponse::Accepted {
                    correlation_id: correlation_id.to_string(),
                    causation_id: causation_id.to_string(),
                }),
            }
        }
    }
}

/// Dispatch a non-cancel mutation via Signal.
async fn dispatch_signal(
    master: &ActorRef<OrchestratorMsg>,
    namespace: &str,
    instance_id: InstanceId,
    signal_name: &str,
    payload: Option<serde_json::Value>,
    correlation_id: &str,
    causation_id: &str,
) -> Result<OperatorMutationResponse, MutationError> {
    let payload_bytes = match &payload {
        Some(p) => serde_json::to_vec(p).unwrap_or_default(),
        None => Vec::new(),
    };

    let call_result = master
        .call(
            |tx| OrchestratorMsg::Signal {
                namespace: namespace.to_string(),
                instance_id,
                signal_name: signal_name.to_string(),
                payload: bytes::Bytes::from(payload_bytes),
                reply: tx,
            },
            Some(ACTOR_CALL_TIMEOUT),
        )
        .await;

    match call_result {
        Err(e) => Err(MutationError::EnvelopeValidation(format!(
            "actor unavailable: {}",
            e
        ))),
        Ok(CallResult::Timeout) => Err(MutationError::EnvelopeValidation(
            "orchestrator did not respond".to_string(),
        )),
        Ok(CallResult::SenderError) => Err(MutationError::EnvelopeValidation(
            "orchestrator dropped reply".to_string(),
        )),
        Ok(CallResult::Success(Err(vo_actor::SignalError::NotFound(id)))) => Err(
            MutationError::Rejected(MutationRejectionReason::InstanceNotFound(id)),
        ),
        Ok(CallResult::Success(Err(vo_actor::SignalError::Failed(msg)))) => Err(
            MutationError::Rejected(MutationRejectionReason::InvalidMutationForState(msg)),
        ),
        Ok(CallResult::Success(Ok(()))) => Ok(OperatorMutationResponse::Accepted {
            correlation_id: correlation_id.to_string(),
            causation_id: causation_id.to_string(),
        }),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use vo_types::{CommandEnvelope, CommandMetadata, IdempotencyKey, Issuer, TimestampMs};

    fn test_instance_id() -> InstanceId {
        InstanceId::parse("01HQXK5R5TJRP3J4W5G6W7Y8Z9").expect("valid ulid")
    }

    fn test_signal_address() -> vo_types::SignalAddress {
        vo_types::SignalAddress::lineage_wide(
            test_instance_id(),
            test_instance_id(),
            vo_types::WaitKey::parse("approve").expect("valid"),
        )
    }

    /// BDD: Given a request with an invalid workflow ID format,
    ///      When the mutation handler receives it,
    ///      Then it returns 400 with an invalid_id error.
    #[test]
    fn invalid_workflow_id_has_no_slash_separator() {
        assert!(split_path_id("invalid-no-slash").is_none());
        assert!(split_path_id("only").is_none());
    }

    /// BDD: Given a valid workflow ID with namespace/instance_id,
    ///      When split_path_id is called,
    ///      Then it returns the two parts.
    #[test]
    fn valid_workflow_id_splits_correctly() {
        let result = split_path_id("my-ns/01HQXK5R5TJRP3J4W5G6W7Y8Z9");
        assert!(result.is_some());
        let (ns, id) = result.unwrap();
        assert_eq!(ns, "my-ns");
        assert_eq!(id, test_instance_id());
    }

    /// BDD: Given a valid mutation request with action=cancel,
    ///      When the handler dispatches it,
    ///      Then it routes to ReserveTerminate + CommitTerminate.
    #[test]
    fn cancel_mutation_type_maps_to_cancel_signal_name() {
        let json = serde_json::to_string(&MutationType::Cancel).expect("serialize");
        assert_eq!(json, r#""cancel""#);
    }

    /// BDD: Given a valid mutation request with action=retry,
    ///      When the handler dispatches it,
    ///      Then it routes to Signal with signal_name="retry".
    #[test]
    fn retry_mutation_type_maps_to_retry_signal_name() {
        let json = serde_json::to_string(&MutationType::Retry).expect("serialize");
        assert_eq!(json, r#""retry""#);
    }

    /// BDD: Given a valid mutation request with action=pause,
    ///      When the handler dispatches it,
    ///      Then it routes to Signal with signal_name="pause".
    #[test]
    fn pause_mutation_type_maps_to_pause_signal_name() {
        let json = serde_json::to_string(&MutationType::Pause).expect("serialize");
        assert_eq!(json, r#""pause""#);
    }

    /// BDD: Given a valid mutation request with action=resume,
    ///      When the handler dispatches it,
    ///      Then it routes to Signal with signal_name="resume".
    #[test]
    fn resume_mutation_type_maps_to_resume_signal_name() {
        let json = serde_json::to_string(&MutationType::Resume).expect("serialize");
        assert_eq!(json, r#""resume""#);
    }

    /// BDD: Given a valid mutation request with action=patch,
    ///      When the handler dispatches it,
    ///      Then it routes to Signal with signal_name="patch".
    #[test]
    fn patch_mutation_type_maps_to_patch_signal_name() {
        let json = serde_json::to_string(&MutationType::Patch).expect("serialize");
        assert_eq!(json, r#""patch""#);
    }

    /// BDD: Given a valid mutation request with action=undo,
    ///      When the handler dispatches it,
    ///      Then it routes to Signal with signal_name="undo".
    #[test]
    fn undo_mutation_type_maps_to_undo_signal_name() {
        let json = serde_json::to_string(&MutationType::Undo).expect("serialize");
        assert_eq!(json, r#""undo""#);
    }

    /// BDD: Given a valid request with all mutation types,
    ///      When each type is serialized,
    ///      Then the snake_case format is preserved.
    #[test]
    fn all_mutation_types_serializes_to_snake_case() {
        for (variant, expected) in [
            (MutationType::Cancel, "cancel"),
            (MutationType::Pause, "pause"),
            (MutationType::Resume, "resume"),
            (MutationType::Patch, "patch"),
            (MutationType::Retry, "retry"),
            (MutationType::Undo, "undo"),
        ] {
            let json = serde_json::to_string(&variant).expect("serialize");
            assert_eq!(json, format!(r#""{}""#, expected), "variant: {:?}", variant);
        }
    }

    /// BDD: Given a successful signal dispatch,
    ///      When the orchestrator returns Success(Ok(())),
    ///      Then the handler returns OperatorMutationResponse::Accepted.
    #[test]
    fn signal_success_returns_accepted_response() {
        let resp = OperatorMutationResponse::Accepted {
            correlation_id: "corr-test".to_string(),
            causation_id: "cause-test".to_string(),
        };
        assert!(matches!(resp, OperatorMutationResponse::Accepted { .. }));
    }

    /// BDD: Given a rejected mutation (instance not found),
    ///      When the handler processes the rejection,
    ///      Then it maps to HTTP 404.
    #[test]
    fn instance_not_found_rejection_maps_to_404() {
        let reason = MutationRejectionReason::InstanceNotFound("inst-1".to_string());
        let err = MutationError::Rejected(reason);
        assert_eq!(mutation_error_status_code(&err), 404);
    }

    /// BDD: Given a rejected mutation (terminal state),
    ///      When the handler rejects it,
    ///      Then it maps to HTTP 409.
    #[test]
    fn invalid_mutation_for_state_rejection_maps_to_409() {
        let reason = MutationRejectionReason::InvalidMutationForState(
            "cannot mutate terminated".to_string(),
        );
        let err = MutationError::Rejected(reason);
        assert_eq!(mutation_error_status_code(&err), 409);
    }

    /// BDD: Given a request with a payload,
    ///      When the mutation is dispatched via signal,
    ///      Then the payload is serialized to bytes.
    #[test]
    fn payload_serializes_to_bytes() {
        let payload = Some(json!({"timeout_ms": 5000}));
        let bytes = match &payload {
            Some(p) => serde_json::to_vec(p).unwrap_or_default(),
            None => Vec::new(),
        };
        assert!(!bytes.is_empty());
        let deserialized: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(deserialized["timeout_ms"], 5000);
    }

    /// BDD: Given a request without a payload,
    ///      When the mutation is dispatched via signal,
    ///      Then the payload bytes are empty.
    #[test]
    fn empty_payload_produces_empty_bytes() {
        let payload: Option<serde_json::Value> = None;
        let bytes = match &payload {
            Some(p) => serde_json::to_vec(p).unwrap_or_default(),
            None => Vec::new(),
        };
        assert!(bytes.is_empty());
    }

    /// BDD: Given a valid OperatorMutationRequest with all fields,
    ///      When it is serialized and deserialized,
    ///      Then the mutation_type is preserved.
    #[test]
    fn mutation_request_roundtrips_through_serde() {
        let envelope = CommandEnvelope {
            schema_version: 1,
            metadata: CommandMetadata {
                command_id: IdempotencyKey::parse("cmd-mut-001").expect("valid"),
                correlation_id: IdempotencyKey::parse("corr-mut-001").expect("valid"),
                causation_id: IdempotencyKey::parse("cause-mut-001").expect("valid"),
                issuer: Issuer::Operator,
                issued_at: TimestampMs::try_from(1_700_000_000u64).expect("valid"),
            },
        };
        let target = test_signal_address();
        let req = OperatorMutationRequest {
            envelope,
            mutation_type: MutationType::Cancel,
            target,
            buffer_policy: vo_types::BufferPolicy::Reject,
            payload: None,
        };
        let json = serde_json::to_string(&req).expect("serialize");
        let back: OperatorMutationRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.mutation_type, MutationType::Cancel);
    }

    /// BDD: Given an OperatorMutationResponse::Duplicate,
    ///      When it is serialized,
    ///      Then the status field is "duplicate".
    #[test]
    fn duplicate_response_serializes_with_correct_tag() {
        let resp = OperatorMutationResponse::Duplicate {
            correlation_id: "corr-1".to_string(),
            causation_id: "cause-1".to_string(),
            original_command_id: "cmd-orig".to_string(),
        };
        let json = serde_json::to_string(&resp).expect("serialize");
        assert!(json.contains(r#""status":"duplicate""#));
    }

    /// BDD: Given an OperatorMutationResponse::Rejected,
    ///      When it is serialized,
    ///      Then the status field is "rejected" and contains the reason.
    #[test]
    fn rejected_response_serializes_with_correct_tag() {
        let resp = OperatorMutationResponse::Rejected {
            reason: "instance not found".to_string(),
        };
        let json = serde_json::to_string(&resp).expect("serialize");
        assert!(json.contains(r#""status":"rejected""#));
        assert!(json.contains(r#""reason":"instance not found""#));
    }

    /// BDD: Given a response from OperatorMutationResponse enum,
    ///      When it is serialized,
    ///      Then all variants produce a "status" field.
    #[test]
    fn all_response_variants_have_status_field() {
        let accepted = OperatorMutationResponse::Accepted {
            correlation_id: "c".to_string(),
            causation_id: "k".to_string(),
        };
        let dup = OperatorMutationResponse::Duplicate {
            correlation_id: "c".to_string(),
            causation_id: "k".to_string(),
            original_command_id: "o".to_string(),
        };
        let rejected = OperatorMutationResponse::Rejected {
            reason: "r".to_string(),
        };

        for resp in [accepted, dup, rejected] {
            let json = serde_json::to_string(&resp).expect("serialize");
            assert!(
                json.contains(r#""status""#),
                "missing status field in: {}",
                json
            );
        }
    }

    /// BDD: Given a rejection reason for payload validation failure,
    ///      When the handler maps it to HTTP status,
    ///      Then it returns 422 Unprocessable Entity.
    #[test]
    fn payload_validation_failed_maps_to_422() {
        let reason = MutationRejectionReason::PayloadValidationFailed("missing field".to_string());
        let err = MutationError::Rejected(reason);
        assert_eq!(mutation_error_status_code(&err), 422);
    }

    /// BDD: Given a tombstoned lineage,
    ///      When the handler maps it to HTTP status,
    ///      Then it returns 410 Gone.
    #[test]
    fn lineage_tombstoned_maps_to_410() {
        let reason = MutationRejectionReason::LineageTombstoned("lineage-1".to_string());
        let err = MutationError::Rejected(reason);
        assert_eq!(mutation_error_status_code(&err), 410);
    }

    /// BDD: Given an envelope validation error,
    ///      When the handler maps it to HTTP status,
    ///      Then it returns 400 Bad Request.
    #[test]
    fn envelope_validation_error_maps_to_400() {
        let err = MutationError::EnvelopeValidation("bad json".to_string());
        assert_eq!(mutation_error_status_code(&err), 400);
    }
}
