//! BDD: Replace placeholder query projection API test with production handler test.
//!
//! ADR-037: Exercises the production query handlers (timeline, history,
//! effect-journal, version) backed by a real fjall database, asserting
//! actual projection data rather than `assert!(true)`.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::get,
    Router,
};
use fjall::Database;
use std::sync::Arc;
use tower::ServiceExt;
use vo_api::handlers::query::QueryState;
use vo_storage::event_log::{append_event, AppendEventRequest};
use vo_types::events::EventMetadata;
use vo_types::workspace::WorkspaceIndex;
use vo_types::InstanceId;

fn append_projection_event(
    db: &Database,
    namespace: &str,
    instance_id: &InstanceId,
    event_type: &str,
) {
    let request = AppendEventRequest {
        namespace: namespace.to_string(),
        instance_id: instance_id.clone(),
        timestamp_ms: 1_000,
        payload: serde_json::json!({"type": event_type, "step_id": "step-1"}),
        metadata: EventMetadata::default(),
    };
    append_event(db, request).expect("append projection event");
}

fn append_projection_event_with_semantics(
    db: &Database,
    namespace: &str,
    instance_id: &InstanceId,
    event_type: &str,
    semantics: &str,
) {
    let mut annotations = std::collections::HashMap::new();
    annotations.insert("semantics".to_string(), serde_json::json!(semantics));
    let request = AppendEventRequest {
        namespace: namespace.to_string(),
        instance_id: instance_id.clone(),
        timestamp_ms: 1_000,
        payload: serde_json::json!({"type": event_type}),
        metadata: EventMetadata {
            command_metadata: None,
            annotations,
        },
    };
    append_event(db, request).expect("append projection event with semantics");
}

fn setup_db() -> (tempfile::TempDir, Database) {
    let folder = tempfile::tempdir().expect("temp dir");
    let db = Database::builder(folder.path()).open().expect("database");
    (folder, db)
}

fn encode_path_id(path_id: &str) -> String {
    path_id.replace('/', "%2F")
}

fn query_app(db: Arc<Database>) -> Router {
    let workspace_index = Arc::new(std::sync::RwLock::new(WorkspaceIndex::new()));
    Router::new()
        .route(
            "/api/v1/workflows/{id}/timeline",
            get(vo_api::handlers::get_timeline),
        )
        .route(
            "/api/v1/workflows/{id}/history",
            get(vo_api::handlers::get_history),
        )
        .route(
            "/api/v1/workflows/{id}/effect-journal",
            get(vo_api::handlers::get_effect_journal),
        )
        .route(
            "/api/v1/workflows/{id}/version",
            get(vo_api::handlers::get_workflow_version),
        )
        .with_state(QueryState {
            db,
            workspace_index,
        })
}

async fn send(req: Request<Body>, app: Router) -> (StatusCode, serde_json::Value) {
    let resp = app.oneshot(req).await.expect("oneshot");
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .expect("read body");
    let v: serde_json::Value = serde_json::from_slice(&body).expect("parse json");
    (status, v)
}

#[tokio::test]
async fn given_projection_query_test_when_run_then_production_handler_is_exercised() {
    let (_dir, db) = setup_db();
    let db = Arc::new(db);

    let namespace = "ns";
    let instance_id = InstanceId::parse(&ulid::Ulid::new().to_string()).expect("instance id");
    let path_id = format!("{namespace}/{instance_id}");

    append_projection_event(&db, namespace, &instance_id, "WorkflowStarted");
    append_projection_event(&db, namespace, &instance_id, "SignalAccepted");
    append_projection_event(
        &db,
        namespace,
        &instance_id,
        "WorkflowCompensationInitiated",
    );
    append_projection_event(&db, namespace, &instance_id, "WorkflowTerminated");

    let app = query_app(Arc::clone(&db));

    let encoded_id = encode_path_id(&path_id);

    let req = Request::builder()
        .uri(format!("/api/v1/workflows/{encoded_id}/timeline"))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(req, app).await;
    assert_eq!(status, StatusCode::OK, "timeline response: {body}");
    assert_eq!(body["instance_id"], path_id);
    let entries = body["entries"].as_array().expect("entries array");
    assert_eq!(entries.len(), 4, "should have 4 lifecycle timeline entries");
    assert_eq!(entries[0]["event_type"], "WorkflowStarted");
    assert_eq!(entries[0]["sequence"], 1);
    assert_eq!(entries[1]["event_type"], "SignalAccepted");
    assert_eq!(entries[1]["payload"]["step_id"], "step-1");
    assert_eq!(entries[2]["event_type"], "WorkflowCompensationInitiated");
    assert_eq!(entries[3]["event_type"], "WorkflowTerminated");
    assert_eq!(body["total_replayed"], 4);

    let app = query_app(Arc::clone(&db));
    let req = Request::builder()
        .uri(format!("/api/v1/workflows/{encoded_id}/history"))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(req, app).await;
    assert_eq!(status, StatusCode::OK, "history response: {body}");
    assert_eq!(body["instance_id"], path_id);
    let hist_entries = body["entries"].as_array().expect("history entries");
    assert_eq!(hist_entries.len(), 4);
    assert_eq!(hist_entries[1]["step_id"], "step-1");
    assert_eq!(hist_entries[1]["event_type"], "SignalAccepted");

    let app = query_app(Arc::clone(&db));
    let req = Request::builder()
        .uri(format!("/api/v1/workflows/{encoded_id}/effect-journal"))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(req, app).await;
    assert_eq!(status, StatusCode::OK, "effect-journal response: {body}");
    let ej_entries = body["entries"].as_array().expect("effect journal entries");
    assert_eq!(ej_entries.len(), 4);
    for entry in ej_entries {
        assert!(
            entry.get("semantics").is_some(),
            "each entry must have semantics field"
        );
    }

    let app = query_app(db);
    let req = Request::builder()
        .uri(format!("/api/v1/workflows/{encoded_id}/version"))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(req, app).await;
    assert_eq!(status, StatusCode::OK, "version response: {body}");
    assert_eq!(body["instance_id"], path_id);
    assert_eq!(body["schema_version"], 1);
    assert_eq!(body["event_count"], 4);
    assert_eq!(body["last_sequence"], 4);
    assert!(body["last_timestamp_ms"].is_number());
}

#[tokio::test]
async fn given_invalid_id_when_timeline_queried_then_400() {
    let (_dir, db) = setup_db();
    let app = query_app(Arc::new(db));

    let req = Request::builder()
        .uri("/api/v1/workflows/no-slash/timeline")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(req, app).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "invalid_id");
}

#[tokio::test]
async fn given_empty_stream_when_version_queried_then_zero_events() {
    let (_dir, db) = setup_db();
    let app = query_app(Arc::new(db));

    let instance_id = InstanceId::parse(&ulid::Ulid::new().to_string()).expect("instance id");
    let path_id = format!("ns/{instance_id}");
    let encoded_id = encode_path_id(&path_id);

    let req = Request::builder()
        .uri(format!("/api/v1/workflows/{encoded_id}/version"))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(req, app).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["event_count"], 0);
    assert_eq!(body["last_sequence"], serde_json::Value::Null);
}

#[tokio::test]
async fn given_effect_with_exact_semantics_when_journal_queried_then_exact_in_response() {
    let (_dir, db) = setup_db();
    let db = Arc::new(db);

    let namespace = "ns";
    let instance_id = InstanceId::parse(&ulid::Ulid::new().to_string()).expect("instance id");
    let path_id = format!("{namespace}/{instance_id}");
    let encoded_id = encode_path_id(&path_id);

    append_projection_event_with_semantics(
        &db,
        namespace,
        &instance_id,
        "EffectCommitted",
        "exact",
    );

    let app = query_app(db);
    let req = Request::builder()
        .uri(format!("/api/v1/workflows/{encoded_id}/effect-journal"))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(req, app).await;
    assert_eq!(status, StatusCode::OK);
    let entries = body["entries"].as_array().expect("entries");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["semantics"], "exact");
}
