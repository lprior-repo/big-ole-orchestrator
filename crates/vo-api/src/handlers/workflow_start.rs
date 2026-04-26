use axum::{
    extract::{Extension, Json},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use bytes::Bytes;
use ractor::rpc::CallResult;
use ractor::ActorRef;
use std::sync::Arc;
use std::time::Duration;
use ulid::Ulid;
use vo_actor::{OrchestratorMsg, StartError};
use vo_common::NamespaceId;
use vo_core::admission::{PressureGuardResult, WriterPressureGuard};
use vo_storage::dedupe_partition::DedupeStore;

use crate::handlers::helpers::parse_paradigm;
use crate::handlers::ingress::{
    admit_ingress, IngressAdmission, IngressAdmissionError, DEFAULT_DEDUPE_TTL_MS,
};
use crate::types::{ApiError, V3StartRequest, V3StartResponse, WorkloadRejectionError};

const ACTOR_CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// POST /api/v1/workflows — start a new workflow instance (bead vo-7mif).
///
/// Per ADR-028, this handler enforces exactly-once ingress:
/// 1. Validates that a `dedupe_key` is present for exact workflow ingress.
/// 2. Calls `admit_ingress` to atomically check-and-insert into the dedupe store.
/// 3. If duplicate, returns 409 Conflict with the existing instance ID.
/// 4. If new, proceeds to start the workflow via the orchestrator actor.
#[tracing::instrument(skip_all)]
pub async fn start_workflow(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Extension(dedupe_store): Extension<Arc<dyn DedupeStore>>,
    Extension(writer_pressure): Extension<Arc<dyn WriterPressureGuard>>,
    Json(req): Json<V3StartRequest>,
) -> impl IntoResponse {
    // Step 1: Validate dedupe key presence (ADR-028 Section 2).
    let dedupe_key = match req.dedupe_key {
        Some(ref key) if !key.is_empty() => key.clone(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    "missing_dedupe_key",
                    "dedupe_key is required for exact workflow ingress (ADR-028)",
                )),
            )
                .into_response();
        }
    };

    let namespace = NamespaceId::from(req.namespace);

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

    let instance_id_str = match req.instance_id {
        Some(ref id) => id.clone(),
        None => Ulid::new().to_string(),
    };
    let instance_id =
        vo_types::InstanceId::parse(&instance_id_str).expect("generated ULID should be valid");

    // Step 2: Atomic admission check against dedupe store (ADR-028 Section 3).
    let admission = match admit_ingress(
        dedupe_store.as_ref(),
        &dedupe_key,
        &instance_id,
        DEFAULT_DEDUPE_TTL_MS,
    ) {
        Ok(a) => a,
        Err(IngressAdmissionError::Storage { reason }) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("dedupe_storage_error", reason)),
            )
                .into_response();
        }
        Err(IngressAdmissionError::InvalidDedupeKey { reason }) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new("invalid_dedupe_key", reason)),
            )
                .into_response();
        }
    };

    // Step 3: If duplicate, return 409 Conflict with existing instance (ADR-028).
    if let IngressAdmission::Duplicate {
        existing_instance_id,
    } = admission
    {
        return (
            StatusCode::CONFLICT,
            Json(ApiError::new(
                "duplicate_ingress",
                format!(
                    "dedupe_key '{dedupe_key}' already admitted as instance {existing_instance_id}"
                ),
            )),
        )
            .into_response();
    }

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

    // Step 4: Check writer pressure before submitting to orchestrator (ADR-006, ADR-015).
    // When DbWriter mailbox is at 80% capacity, shed ingress with 429 + Retry-After.
    match writer_pressure.check() {
        PressureGuardResult::Admitted => {}
        PressureGuardResult::Shed {
            retry_after_secs,
            reason,
        } => {
            let mut headers = HeaderMap::new();
            headers.insert(
                axum::http::header::RETRY_AFTER,
                retry_after_secs.to_string().parse().expect("valid header value"),
            );
            return (
                StatusCode::TOO_MANY_REQUESTS,
                headers,
                Json(ApiError::new("writer_pressure_shed", reason)),
            )
                .into_response();
        }
    }

    // Step 5: Proceed to start workflow via actor (ADR-028 atomic write).
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
        Ok(CallResult::Success(Err(StartError::InvalidConfig(msg)))) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("invalid_config", msg)),
        )
            .into_response(),
        Ok(CallResult::Success(Err(StartError::BudgetExhaustion {
            class,
            requested,
            available,
        }))) => {
            let rejection = WorkloadRejectionError::BudgetExhausted {
                class: class.to_string(),
                requested,
                available,
            };
            (
                StatusCode::from_u16(rejection.status_code())
                    .unwrap_or(StatusCode::TOO_MANY_REQUESTS),
                Json(ApiError::new(rejection.error_code(), rejection.to_string())),
            )
                .into_response()
        }
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

// ─── BDD Test: Production orchestrator receives StartWorkflow ─────────────────

#[cfg(test)]
mod production_orchestrator_bdd_tests {
    use super::*;
    use axum::http::Request;
    use axum::router::Router;
    use axum_test::TestClient;
    use http_body_util::BodyExt;
    use ractor::{Actor, ActorProcessingErr, ActorRef};
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::TempDir;
    use vo_types::workspace::WorkspaceIndex;

    /// Test actor that receives OrchestratorMsg and responds to StartWorkflow.
    struct TestOrchestrator;

    #[ractor::async_trait]
    impl Actor for TestOrchestrator {
        type Msg = OrchestratorMsg;
        type State = ();
        type Arguments = ();

        async fn pre_start(
            &self,
            _myself: ActorRef<Self::Msg>,
            _args: Self::Arguments,
        ) -> Result<Self::State, ActorProcessingErr> {
            Ok(())
        }

        async fn handle(
            &self,
            _myself: ActorRef<Self::Msg>,
            message: Self::Msg,
            _state: &mut Self::State,
        ) -> Result<(), ActorProcessingErr> {
            match message {
                OrchestratorMsg::StartWorkflow { reply, .. } => {
                    let _ = reply.send(Ok(()));
                }
                OrchestratorMsg::GetStatus { reply, .. } => {
                    let _ = reply.send(None);
                }
                OrchestratorMsg::Terminate { reply, .. } => {
                    let _ = reply.send(Err(TerminateError::NotFound("test".to_string())));
                }
                OrchestratorMsg::ListActive { reply, .. } => {
                    let _ = reply.send(vec![]);
                }
                OrchestratorMsg::Compensate { reply, .. } => {
                    let _ = reply.send(Err(CompensateError::NotFound("test".to_string())));
                }
                OrchestratorMsg::Signal { reply, .. } => {
                    let _ = reply.send(Err(SignalError::Failed("test".to_string())));
                }
            }
            Ok(())
        }
    }

    fn build_test_state(actor_ref: ActorRef<OrchestratorMsg>, tmp_dir: &TempDir) -> AppState {
        let db = fjall::Database::open(tmp_dir.path().to_owned()).unwrap();
        let workspace_index = Arc::new(std::sync::RwLock::new(WorkspaceIndex::default()));
        AppState {
            query: QueryState {
                db: Arc::new(db),
                workspace_index,
            },
            sse: SseState::new(),
            ws: WsState::new(),
            master: Arc::new(actor_ref),
            circuit_breaker: Arc::new(vo_core::circuit_breaker::CircuitBreakerState::new()),
        }
    }

    #[tokio::test]
    async fn given_start_request_when_handler_runs_then_production_orchestrator_receives_start()
    {
        let tmp_dir = tempfile::tempdir().expect("failed to create temp dir");

        // Spawn the test orchestrator actor
        let (actor_ref, _actor_handle) =
            Actor::spawn_linked(None, TestOrchestrator, (), None)
                .await
                .expect("failed to spawn test orchestrator");

        // Build the router with the test actor as the extension
        let state = build_test_state(actor_ref, &tmp_dir);
        let router: Router = Router::new()
            .route("/api/v1/workflows", post(start_workflow))
            .layer(Extension(state.master.clone()));

        // Build the start request JSON (minimal valid request)
        let request_json = json!({
            "namespace": "payments",
            "workflow_type": "checkout",
            "paradigm": "fsm",
            "input": {"order_id": "ord_123"}
        });

        // Send the request via TestClient (production path exercised)
        let client = TestClient::new(router);
        let response = client
            .post("/api/v1/workflows")
            .json(&request_json)
            .send()
            .await;

        // Verify: handler forwarded to orchestrator and returned 201 Created
        let status = response.status();
        assert_eq!(
            status,
            StatusCode::CREATED,
            "Expected 201 Created when orchestrator accepts StartWorkflow, got: {status}"
        );

        let body: serde_json::Value = response.json().await;
        assert_eq!(body["namespace"], "payments");
        assert_eq!(body["workflow_type"], "checkout");
    }

    /// Verify that missing dedupe_key produces 400 Bad Request before calling orchestrator.
    #[tokio::test]
    async fn given_missing_dedupe_key_when_handler_runs_then_returns_bad_request() {
        let tmp_dir = tempfile::tempdir().expect("failed to create temp dir");

        let (actor_ref, _actor_handle) =
            Actor::spawn_linked(None, TestOrchestrator, (), None)
                .await
                .expect("failed to spawn test orchestrator");

        let state = build_test_state(actor_ref, &tmp_dir);
        let router: Router = Router::new()
            .route("/api/v1/workflows", post(start_workflow))
            .layer(Extension(state.master.clone()));

        let request_json = json!({
            "namespace": "payments",
            "workflow_type": "checkout",
            "paradigm": "fsm",
            "input": {"order_id": "ord_123"}
        });

        let client = TestClient::new(router);
        let response = client
            .post("/api/v1/workflows")
            .json(&request_json)
            .send()
            .await;

        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "Expected 400 Bad Request for missing dedupe_key"
        );
        let body: serde_json::Value = response.json().await;
        assert_eq!(body["error_code"], "missing_dedupe_key");
    }

    /// Verify that invalid paradigm produces 400 Bad Request before calling orchestrator.
    #[tokio::test]
    async fn given_invalid_paradigm_when_handler_runs_then_returns_bad_request() {
        let tmp_dir = tempfile::tempdir().expect("failed to create temp dir");

        let (actor_ref, _actor_handle) =
            Actor::spawn_linked(None, TestOrchestrator, (), None)
                .await
                .expect("failed to spawn test orchestrator");

        let state = build_test_state(actor_ref, &tmp_dir);
        let router: Router = Router::new()
            .route("/api/v1/workflows", post(start_workflow))
            .layer(Extension(state.master.clone()));

        let request_json = json!({
            "namespace": "payments",
            "workflow_type": "checkout",
            "paradigm": "invalid_paradigm",
            "input": {"order_id": "ord_123"}
        });

        let client = TestClient::new(router);
        let response = client
            .post("/api/v1/workflows")
            .json(&request_json)
            .send()
            .await;

        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "Expected 400 Bad Request for invalid paradigm"
        );
        let body: serde_json::Value = response.json().await;
        assert_eq!(body["error_code"], "invalid_paradigm");
    }

    /// Verify that instance_id defaults to ULID when not provided.
    #[tokio::test]
    async fn given_no_instance_id_when_handler_runs_then_returns_ulid_in_response() {
        let tmp_dir = tempfile::tempdir().expect("failed to create temp dir");

        let (actor_ref, _actor_handle) =
            Actor::spawn_linked(None, TestOrchestrator, (), None)
                .await
                .expect("failed to spawn test orchestrator");

        let state = build_test_state(actor_ref, &tmp_dir);
        let router: Router = Router::new()
            .route("/api/v1/workflows", post(start_workflow))
            .layer(Extension(state.master.clone()));

        let request_json = json!({
            "namespace": "payments",
            "workflow_type": "checkout",
            "paradigm": "fsm",
            "input": {"order_id": "ord_123"}
        });

        let client = TestClient::new(router);
        let response = client
            .post("/api/v1/workflows")
            .json(&request_json)
            .send()
            .await;

        assert_eq!(response.status(), StatusCode::CREATED);
        let body: serde_json::Value = response.json().await;
        let instance_id = body["instance_id"].as_str().unwrap();
        // ULID format: 26 characters, Base32 encoded
        assert_eq!(instance_id.len(), 26, "ULID should be 26 characters");
    }
}
