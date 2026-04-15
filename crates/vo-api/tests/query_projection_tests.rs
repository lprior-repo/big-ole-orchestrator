//! TDD-RED: Failing tests for query projection routing and correctness.
//!
//! These tests verify that vo-api correctly routes projection queries
//! through the handler layer and returns correctly projected read models.
//!
//! Per ADR-037, all query projection surfaces live in vo-api.
//! Tests exercise the handlers directly with real fjall keystores.

#![allow(clippy::unwrap_used)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::get,
    Router,
};
use fjall::{Config, Keyspace};
use http_body_util::BodyExt;
use tower::ServiceExt;
use vo_api::handlers::query::QueryState;
use vo_types::events::EventMetadata;
use vo_types::{EventEnvelope, InstanceId};
use std::sync::Arc;

fn make_envelope(seq: u64, event_type: &str, instance_id: &str) -> EventEnvelope {
    let mut annotations = std::collections::HashMap::new();
    if event_type.contains("effect") {
        annotations.insert("semantics".to_string(), serde_json::json!("exact"));
    }
    EventEnvelope {
        schema_version: 1,
        instance_id: instance_id.to_string(),
        sequence: seq,
        timestamp_ms: 1000 + seq * 100,
        payload: serde_json::json!({"type": event_type}),
        metadata: EventMetadata {
            command_metadata: None,
            annotations,
        },
    }
}

fn envelope_to_bytes(envelope: &EventEnvelope) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "version": envelope.schema_version,
        "instance_id": envelope.instance_id,
        "sequence": envelope.sequence,
        "timestamp_ms": envelope.timestamp_ms,
        "payload": envelope.payload,
        "metadata": envelope.metadata,
    })).unwrap()
}

async fn setup_keyspace_with_events(instance_id: &InstanceId, count: u64) -> Arc<Keyspace> {
    let dir = tempfile::tempdir().unwrap();
    let keyspace = Config::new(dir.path()).open().unwrap();
    let partition = keyspace
        .open_partition("events", fjall::PartitionCreateOptions::default())
        .unwrap();

    let event_types = ["WorkflowStarted", "StepCompleted", "StepFailed", "EffectCommitted"];
    for seq in 1..=count {
        let event_type = event_types[((seq - 1) % event_types.len() as u64) as usize];
        let envelope = make_envelope(seq, event_type, instance_id.as_str());
        let mut key = instance_id.as_str().as_bytes().to_vec();
        key.extend_from_slice(&seq.to_be_bytes());
        let value = envelope_to_bytes(&envelope);
        partition.insert(&key, &value).unwrap();
    }

    Arc::new(keyspace)
}

fn build_router(state: QueryState) -> Router {
    async fn timeline_handler(
        axum::extract::Path((ns, inst)): axum::extract::Path<(String, String)>,
        axum::extract::State(st): axum::extract::State<QueryState>,
    ) -> impl axum::response::IntoResponse {
        let id = format!("{}/{}", ns, inst);
        vo_api::handlers::query::get_timeline(axum::extract::Path(id), axum::extract::State(st)).await
    }
    async fn history_handler(
        axum::extract::Path((ns, inst)): axum::extract::Path<(String, String)>,
        axum::extract::State(st): axum::extract::State<QueryState>,
    ) -> impl axum::response::IntoResponse {
        let id = format!("{}/{}", ns, inst);
        vo_api::handlers::query::get_history(axum::extract::Path(id), axum::extract::State(st)).await
    }
    async fn effect_journal_handler(
        axum::extract::Path((ns, inst)): axum::extract::Path<(String, String)>,
        axum::extract::State(st): axum::extract::State<QueryState>,
    ) -> impl axum::response::IntoResponse {
        let id = format!("{}/{}", ns, inst);
        vo_api::handlers::query::get_effect_journal(axum::extract::Path(id), axum::extract::State(st)).await
    }
    async fn version_handler(
        axum::extract::Path((ns, inst)): axum::extract::Path<(String, String)>,
        axum::extract::State(st): axum::extract::State<QueryState>,
    ) -> impl axum::response::IntoResponse {
        let id = format!("{}/{}", ns, inst);
        vo_api::handlers::query::get_workflow_version(axum::extract::Path(id), axum::extract::State(st)).await
    }

    Router::new()
        .route("/api/v1/workflows/:ns/:inst/timeline", get(timeline_handler))
        .route("/api/v1/workflows/:ns/:inst/history", get(history_handler))
        .route("/api/v1/workflows/:ns/:inst/effect-journal", get(effect_journal_handler))
        .route("/api/v1/workflows/:ns/:inst/version", get(version_handler))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Routing tests: verify requests reach correct handlers
// ---------------------------------------------------------------------------

#[tokio::test]
async fn timeline_route_returns_200_for_valid_instance() {
    let instance_id = InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    let keyspace = setup_keyspace_with_events(&instance_id, 3).await;
    let state = QueryState { keyspace };
    let app = build_router(state);

    let req = Request::builder()
        .uri("/api/v1/workflows/payments/01ARZ3NDEKTSV4RRFFQ69G5FAV/timeline")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn history_route_returns_200_for_valid_instance() {
    let instance_id = InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    let keyspace = setup_keyspace_with_events(&instance_id, 3).await;
    let state = QueryState { keyspace };
    let app = build_router(state);

    let req = Request::builder()
        .uri("/api/v1/workflows/payments/01ARZ3NDEKTSV4RRFFQ69G5FAV/history")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn effect_journal_route_returns_200_for_valid_instance() {
    let instance_id = InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    let keyspace = setup_keyspace_with_events(&instance_id, 3).await;
    let state = QueryState { keyspace };
    let app = build_router(state);

    let req = Request::builder()
        .uri("/api/v1/workflows/payments/01ARZ3NDEKTSV4RRFFQ69G5FAV/effect-journal")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn version_route_returns_200_for_valid_instance() {
    let instance_id = InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    let keyspace = setup_keyspace_with_events(&instance_id, 3).await;
    let state = QueryState { keyspace };
    let app = build_router(state);

    let req = Request::builder()
        .uri("/api/v1/workflows/payments/01ARZ3NDEKTSV4RRFFQ69G5FAV/version")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ---------------------------------------------------------------------------
// Error routing: malformed IDs return 400
// ---------------------------------------------------------------------------

#[tokio::test]
async fn timeline_returns_400_for_malformed_id_no_slash() {
    let instance_id = InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    let keyspace = setup_keyspace_with_events(&instance_id, 1).await;
    let state = QueryState { keyspace };

    async fn single_path_timeline(
        axum::extract::Path(id): axum::extract::Path<String>,
        axum::extract::State(st): axum::extract::State<QueryState>,
    ) -> impl axum::response::IntoResponse {
        vo_api::handlers::query::get_timeline(axum::extract::Path(id), axum::extract::State(st)).await
    }

    let app = Router::new()
        .route("/api/v1/workflows/:id/timeline", get(single_path_timeline))
        .with_state(state);

    let req = Request::builder()
        .uri("/api/v1/workflows/no-slash-id/timeline")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// Projection correctness: verify response shapes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn timeline_projection_returns_entries_in_order() {
    let instance_id = InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    let keyspace = setup_keyspace_with_events(&instance_id, 5).await;
    let state = QueryState { keyspace };
    let app = build_router(state);

    let req = Request::builder()
        .uri("/api/v1/workflows/payments/01ARZ3NDEKTSV4RRFFQ69G5FAV/timeline")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let entries = json["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 5);

    let seqs: Vec<u64> = entries.iter().map(|e| e["sequence"].as_u64().unwrap()).collect();
    for i in 1..seqs.len() {
        assert!(seqs[i] > seqs[i - 1], "timeline entries must be ascending: {:?}", seqs);
    }
    assert_eq!(json["total_replayed"].as_u64(), Some(5));
}

#[tokio::test]
async fn history_projection_includes_event_type() {
    let instance_id = InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    let keyspace = setup_keyspace_with_events(&instance_id, 4).await;
    let state = QueryState { keyspace };
    let app = build_router(state);

    let req = Request::builder()
        .uri("/api/v1/workflows/payments/01ARZ3NDEKTSV4RRFFQ69G5FAV/history")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let entries = json["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 4);

    let has_event_type = entries.iter().all(|e| e["event_type"].is_string());
    assert!(has_event_type, "all entries must have event_type");
}

#[tokio::test]
async fn effect_journal_projection_includes_semantics() {
    let instance_id = InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    let keyspace = setup_keyspace_with_events(&instance_id, 4).await;
    let state = QueryState { keyspace };
    let app = build_router(state);

    let req = Request::builder()
        .uri("/api/v1/workflows/payments/01ARZ3NDEKTSV4RRFFQ69G5FAV/effect-journal")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let entries = json["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 4);

    let has_semantics = entries.iter().all(|e| e["semantics"].is_string());
    assert!(has_semantics, "all entries must have semantics field");
}

#[tokio::test]
async fn version_projection_returns_correct_count() {
    let instance_id = InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    let keyspace = setup_keyspace_with_events(&instance_id, 7).await;
    let state = QueryState { keyspace };
    let app = build_router(state);

    let req = Request::builder()
        .uri("/api/v1/workflows/payments/01ARZ3NDEKTSV4RRFFQ69G5FAV/version")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["event_count"].as_u64(), Some(7));
    assert_eq!(json["last_sequence"].as_u64(), Some(7));
    assert!(json["last_timestamp_ms"].is_number());
}

#[tokio::test]
async fn timeline_projection_empty_for_unknown_instance() {
    let instance_id = InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    let keyspace = setup_keyspace_with_events(&instance_id, 3).await;
    let state = QueryState { keyspace };
    let app = build_router(state);

    let req = Request::builder()
        .uri("/api/v1/workflows/payments/01ZZZZZZZZZZZZZZZZZZZZZZZZ/timeline")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let entries = json["entries"].as_array().unwrap();
    assert!(entries.is_empty());
    assert_eq!(json["total_replayed"].as_u64(), Some(0));
}
