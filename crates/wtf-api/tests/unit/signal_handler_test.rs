use axum::{
    body::Body,
    extract::Extension,
    http::{Request, StatusCode},
    routing::post,
    Router,
};
use tower::ServiceExt;
use serde_json::json;
use ractor::{Actor, ActorRef, ActorProcessingErr};
use wtf_actor::{OrchestratorMsg};
use crate::handlers::signal::send_signal;
use crate::types::{SignalResponse, ApiError};

struct MockOrchestrator;

#[ractor::async_trait]
impl Actor for MockOrchestrator {
    type Msg = OrchestratorMsg;
    type State = ();
    type Arguments = ();

    async fn pre_start(&self, _myself: ActorRef<Self::Msg>, _args: Self::Arguments) -> Result<Self::State, ActorProcessingErr> {
        Ok(())
    }

    async fn handle(&self, _myself: ActorRef<Self::Msg>, msg: Self::Msg, _state: &mut Self::State) -> Result<(), ActorProcessingErr> {
        if let OrchestratorMsg::Signal { instance_id, reply, .. } = msg {
            if instance_id.as_str() == "nonexistent" {
                let _ = reply.send(Err(wtf_common::WtfError::instance_not_found("nonexistent")));
            } else if instance_id.as_str() == "timeout" {
                // Don't reply to simulate timeout
            } else {
                let _ = reply.send(Ok(()));
            }
        }
        Ok(())
    }
}

#[tokio::test]
async fn test_send_signal_success() {
    let (actor, _handle) = Actor::spawn(None, MockOrchestrator, ()).await.unwrap();
    
    let app = Router::new()
        .route("/api/v1/workflows/:id/signals", post(send_signal))
        .layer(Extension(actor));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflows/default%2Fvalid-id/signals")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "signal_name": "test_signal",
                        "payload": {"foo": "bar"}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body = axum::body::to_bytes(response.into_body(), 1024).await.unwrap();
    let res: SignalResponse = serde_json::from_slice(&body).unwrap();
    assert!(res.acknowledged);
}

#[tokio::test]
async fn test_send_signal_invalid_id() {
    let (actor, _handle) = Actor::spawn(None, MockOrchestrator, ()).await.unwrap();
    
    let app = Router::new()
        .route("/api/v1/workflows/:id/signals", post(send_signal))
        .layer(Extension(actor));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflows/invalid-id-format/signals")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "signal_name": "test_signal",
                        "payload": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = axum::body::to_bytes(response.into_body(), 1024).await.unwrap();
    let err: ApiError = serde_json::from_slice(&body).unwrap();
    assert_eq!(err.error, "invalid_id");
}

#[tokio::test]
async fn test_send_signal_not_found() {
    let (actor, _handle) = Actor::spawn(None, MockOrchestrator, ()).await.unwrap();
    
    let app = Router::new()
        .route("/api/v1/workflows/:id/signals", post(send_signal))
        .layer(Extension(actor));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflows/default%2Fnonexistent/signals")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "signal_name": "test_signal",
                        "payload": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = axum::body::to_bytes(response.into_body(), 1024).await.unwrap();
    let err: ApiError = serde_json::from_slice(&body).unwrap();
    assert_eq!(err.error, "instance_not_found");
}
