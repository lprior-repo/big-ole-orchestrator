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
use std::sync::{Arc, Mutex};
use std::mem;
use wtf_actor::{OrchestratorMsg};
use crate::handlers::signal::send_signal;
use crate::types::{SignalResponse, ApiError};

#[derive(Clone)]
struct MockOrchestrator {
    received: Arc<Mutex<Option<(String, String, Vec<u8>)>>>,
}

impl MockOrchestrator {
    fn new() -> Self {
        Self {
            received: Arc::new(Mutex::new(None)),
        }
    }
}

#[ractor::async_trait]
impl Actor for MockOrchestrator {
    type Msg = OrchestratorMsg;
    type State = ();
    type Arguments = ();

    async fn pre_start(&self, _myself: ActorRef<Self::Msg>, _args: Self::Arguments) -> Result<Self::State, ActorProcessingErr> {
        Ok(())
    }

    async fn handle(&self, _myself: ActorRef<Self::Msg>, msg: Self::Msg, _state: &mut Self::State) -> Result<(), ActorProcessingErr> {
        if let OrchestratorMsg::Signal { instance_id, signal_name, payload, reply } = msg {
            if instance_id.as_str() == "nonexistent" {
                let _ = reply.send(Err(wtf_common::WtfError::instance_not_found("nonexistent")));
            } else if instance_id.as_str() == "timeout" {
                mem::forget(reply);
            } else if instance_id.as_str() == "failure" {
                let _ = reply.send(Err(wtf_common::WtfError::InvalidInput { detail: "mock failure".into() }));
            } else {
                *self.received.lock().unwrap() = Some((instance_id.to_string(), signal_name, payload.to_vec()));
                let _ = reply.send(Ok(()));
            }
        }
        Ok(())
    }
}

#[tokio::test]
async fn test_send_signal_success() {
    let (actor, _handle) = Actor::spawn(None, MockOrchestrator::new(), ()).await.unwrap();
    
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
    let (actor, _handle) = Actor::spawn(None, MockOrchestrator::new(), ()).await.unwrap();
    
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
    let (actor, _handle) = Actor::spawn(None, MockOrchestrator::new(), ()).await.unwrap();
    
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

#[tokio::test]
async fn test_returns_internal_server_error_on_actor_timeout() {
    let (actor, _handle) = Actor::spawn(None, MockOrchestrator::new(), ()).await.unwrap();
    
    let app = Router::new()
        .route("/api/v1/workflows/:id/signals", post(send_signal))
        .layer(Extension(actor));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflows/default%2Ftimeout/signals")
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

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = axum::body::to_bytes(response.into_body(), 1024).await.unwrap();
    let err: ApiError = serde_json::from_slice(&body).unwrap();
    assert_eq!(err.error, "actor_timeout");
}

#[tokio::test]
async fn test_returns_internal_server_error_on_actor_failure() {
    let (actor, _handle) = Actor::spawn(None, MockOrchestrator::new(), ()).await.unwrap();
    
    let app = Router::new()
        .route("/api/v1/workflows/:id/signals", post(send_signal))
        .layer(Extension(actor));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflows/default%2Ffailure/signals")
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

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = axum::body::to_bytes(response.into_body(), 1024).await.unwrap();
    let err: ApiError = serde_json::from_slice(&body).unwrap();
    assert_eq!(err.error, "actor_error");
}

#[tokio::test]
async fn test_handles_null_payload_gracefully() {
    let (actor, _handle) = Actor::spawn(None, MockOrchestrator::new(), ()).await.unwrap();
    
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
                        "payload": null
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
    assert_eq!(res.acknowledged, true);
}

#[tokio::test]
async fn test_postcondition_actor_message_sent() {
    let mock = MockOrchestrator::new();
    let received = mock.received.clone();
    let (actor, _handle) = Actor::spawn(None, mock, ()).await.unwrap();
    
    let app = Router::new()
        .route("/api/v1/workflows/:id/signals", post(send_signal))
        .layer(Extension(actor));

    let payload_json = json!({"approved": true});
    let payload_bytes = serde_json::to_vec(&payload_json).unwrap();
    
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflows/ns%2F01ARZ3NDEKTSV4RRFFQ69G5FAV/signals")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "signal_name": "payment_approved",
                        "payload": payload_json
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    
    let received_data = received.lock().unwrap();
    let (instance_id, signal_name, payload) = received_data.as_ref().unwrap();
    assert_eq!(instance_id.as_str(), "01ARZ3NDEKTSV4RRFFQ69G5FAV");
    assert_eq!(signal_name.as_str(), "payment_approved");
    assert_eq!(payload.as_slice(), payload_bytes.as_slice());
}

#[tokio::test]
async fn test_orchestrator_not_called_on_invalid_id() {
    let mock = MockOrchestrator::new();
    let received = mock.received.clone();
    let (actor, _handle) = Actor::spawn(None, mock, ()).await.unwrap();
    
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
    
    let received_data = received.lock().unwrap();
    assert!(received_data.is_none(), "orchestrator should not have been called");
}
