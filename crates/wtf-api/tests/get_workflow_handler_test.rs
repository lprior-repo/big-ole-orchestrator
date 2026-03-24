use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::get,
    Router,
};
use ractor::{Actor, ActorProcessingErr, ActorRef};
use tower::ServiceExt;
use wtf_actor::{GetStatusError, InstancePhaseView, InstanceStatusSnapshot, OrchestratorMsg};
use wtf_api::handlers::get_workflow;
use wtf_api::types::{ApiError, V3StatusResponse};
use wtf_common::{InstanceId, NamespaceId, WorkflowParadigm};

/// Shared snapshot returned by successful GetStatus calls.
fn test_snapshot() -> InstanceStatusSnapshot {
    InstanceStatusSnapshot {
        instance_id: InstanceId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned()),
        namespace: NamespaceId::new("test"),
        workflow_type: "checkout".to_owned(),
        paradigm: WorkflowParadigm::Fsm,
        phase: InstancePhaseView::Live,
        events_applied: 42,
        current_state: Some("Authorized".to_owned()),
    }
}

struct MockOrchestrator;

#[ractor::async_trait]
impl Actor for MockOrchestrator {
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
        msg: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        if let OrchestratorMsg::GetStatus { instance_id, reply } = msg {
            if instance_id.as_str() == "01ARZ3NDEKTSV4RRFFQ69G5FAV" {
                let _ = reply.send(Ok(Some(test_snapshot())));
            } else if instance_id.as_str() == "nonexistent" {
                let _ = reply.send(Ok(None));
            } else if instance_id.as_str() == "dead" {
                let _ = reply.send(Err(GetStatusError::ActorDied));
            } else if instance_id.as_str() == "timeout" {
                // Drop reply to simulate timeout
            } else {
                let _ = reply.send(Ok(None));
            }
        }
        Ok(())
    }
}

/// Build the test app with a mock orchestrator and the get_workflow route.
fn build_app(actor: ActorRef<OrchestratorMsg>) -> Router {
    Router::new()
        .route("/api/v1/workflows/:id", get(get_workflow))
        .layer(axum::Extension(actor))
}

#[tokio::test]
async fn get_existing_workflow_returns_200() {
    let (actor, _handle) = Actor::spawn(None, MockOrchestrator, ()).await.unwrap();
    let app = build_app(actor);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/workflows/test%2F01ARZ3NDEKTSV4RRFFQ69G5FAV")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let res: V3StatusResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(res.instance_id, "01ARZ3NDEKTSV4RRFFQ69G5FAV");
    assert_eq!(res.namespace, "test");
    assert_eq!(res.workflow_type, "checkout");
    assert_eq!(res.paradigm, "fsm");
    assert_eq!(res.events_applied, 42);
    assert_eq!(res.current_state.as_deref(), Some("Authorized"));
}

#[tokio::test]
async fn get_unknown_workflow_returns_404() {
    let (actor, _handle) = Actor::spawn(None, MockOrchestrator, ()).await.unwrap();
    let app = build_app(actor);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/workflows/test%2Fnonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let err: ApiError = serde_json::from_slice(&body).unwrap();
    assert_eq!(err.error, "not_found");
}

#[tokio::test]
async fn get_workflow_bad_path_returns_400() {
    let (actor, _handle) = Actor::spawn(None, MockOrchestrator, ()).await.unwrap();
    let app = build_app(actor);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/workflows/no-slash-here")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let err: ApiError = serde_json::from_slice(&body).unwrap();
    assert_eq!(err.error, "invalid_id");
}

#[tokio::test]
async fn get_workflow_timeout_returns_503_with_retry_after() {
    let (actor, _handle) = Actor::spawn(None, MockOrchestrator, ()).await.unwrap();
    let app = build_app(actor);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/workflows/test%2Ftimeout")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers().get("retry-after").unwrap(), "5");
}
