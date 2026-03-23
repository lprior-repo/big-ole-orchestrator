use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::delete,
    Router,
};
use ractor::Actor;
use tower::ServiceExt;
use wtf_actor::{OrchestratorMsg, TerminateError};

use crate::handlers::workflow::terminate_workflow;
use crate::types::ApiError;

struct TerminateMock;

#[ractor::async_trait]
impl Actor for TerminateMock {
    type Msg = OrchestratorMsg;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ractor::ActorRef<Self::Msg>,
        _args: Self::Arguments,
    ) -> Result<Self::State, ractor::ActorProcessingErr> {
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ractor::ActorRef<Self::Msg>,
        msg: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ractor::ActorProcessingErr> {
        if let OrchestratorMsg::Terminate {
            instance_id, reply, ..
        } = msg
        {
            match instance_id.as_str() {
                "nonexistent" => {
                    let _ = reply.send(Err(TerminateError::NotFound(instance_id)));
                }
                "inst-timeout" => {
                    let _ = reply.send(Err(TerminateError::Timeout(instance_id)));
                }
                _ => {
                    let _ = reply.send(Ok(()));
                }
            }
        }
        Ok(())
    }
}

fn build_app(actor: ractor::ActorRef<OrchestratorMsg>) -> Router {
    Router::new()
        .route("/api/v1/workflows/:id", delete(terminate_workflow))
        .layer(axum::Extension(actor))
}

fn terminate_request(uri: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(uri)
        .body(Body::empty())
        .expect("valid request")
}

#[tokio::test]
async fn terminate_existing_returns_204() {
    let (actor, _handle) = Actor::spawn(None, TerminateMock, ()).await.expect("spawn");

    let app = build_app(actor);
    let res = app
        .oneshot(terminate_request("/api/v1/workflows/default%2Fvalid-id"))
        .await
        .expect("response");

    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn terminate_unknown_returns_404() {
    let (actor, _handle) = Actor::spawn(None, TerminateMock, ()).await.expect("spawn");

    let app = build_app(actor);
    let res = app
        .oneshot(terminate_request("/api/v1/workflows/default%2Fnonexistent"))
        .await
        .expect("response");

    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let body = axum::body::to_bytes(res.into_body(), 1024).await.expect("body");
    let err: ApiError = serde_json::from_slice(&body).expect("json");
    assert_eq!(err.error, "not_found");
}

#[tokio::test]
async fn terminate_bad_path_returns_400() {
    let (actor, _handle) = Actor::spawn(None, TerminateMock, ()).await.expect("spawn");

    let app = build_app(actor);
    let res = app
        .oneshot(terminate_request("/api/v1/workflows/no-slash-here"))
        .await
        .expect("response");

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = axum::body::to_bytes(res.into_body(), 1024).await.expect("body");
    let err: ApiError = serde_json::from_slice(&body).expect("json");
    assert_eq!(err.error, "invalid_id");
}

#[tokio::test]
async fn terminate_timeout_returns_503() {
    let (actor, _handle) = Actor::spawn(None, TerminateMock, ()).await.expect("spawn");

    let app = build_app(actor);
    let res = app
        .oneshot(terminate_request("/api/v1/workflows/default%2Finst-timeout"))
        .await
        .expect("response");

    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = axum::body::to_bytes(res.into_body(), 1024).await.expect("body");
    let err: ApiError = serde_json::from_slice(&body).expect("json");
    assert_eq!(err.error, "instance_timeout");
}
