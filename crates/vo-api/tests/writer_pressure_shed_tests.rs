//! BDD tests for HTTP ingress load shedding at DbWriter 80% capacity.
//!
//! Bead: tw-4y6h.16.4
//! ADR refs: ADR-006, ADR-015
//!
//! BDD scenario:
//!   Given DbWriter mailbox reaches 80 percent capacity
//!   When new HTTP workflow start arrives
//!   Then API returns 429 or 503 with Retry-After and writes no partial admission

use axum::{
    body::Body,
    extract::Extension,
    http::{Request, StatusCode},
    routing::post,
    Router,
};
use ractor::ActorRef;
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;
use vo_actor::OrchestratorMsg;
use vo_api::handlers::start_workflow;
use vo_api::router::AppState;
use vo_api::types::V3StartRequest;
use vo_core::admission::{PressureGuardResult, WatchdogPressureGuard, WriterPressureGuard};
use vo_core::circuit_breaker::CircuitBreakerState;
use vo_core::storage_watchdog::types::{
    StorageHealth, StorageWatchdogConfig,
};
use vo_api::projection::ProjectionService;
use vo_storage::dedupe_partition::{DedupeStore, InMemoryDedupeStore};

struct SheddingGuard;

impl WriterPressureGuard for SheddingGuard {
    fn check(&self) -> PressureGuardResult {
        PressureGuardResult::Shed {
            retry_after_secs: 5,
            reason: "writer queue at shed threshold (>8000/10000 capacity)".to_string(),
        }
    }
}

struct AdmittingGuard;

impl WriterPressureGuard for AdmittingGuard {
    fn check(&self) -> PressureGuardResult {
        PressureGuardResult::Admitted
    }
}

struct DummyOrch;

impl ractor::Actor for DummyOrch {
    type Msg = OrchestratorMsg;
    type State = ();
    type Arguments = ();
    async fn pre_start(
        &self,
        _: ActorRef<Self::Msg>,
        _: Self::Arguments,
    ) -> Result<Self::State, ractor::ActorProcessingErr> {
        Ok(())
    }
}

use std::sync::atomic::{AtomicU64, Ordering};

static ORCH_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_orch_name() -> String {
    format!("test-orch-{}", ORCH_COUNTER.fetch_add(1, Ordering::Relaxed))
}

async fn spawn_dummy_master() -> ActorRef<OrchestratorMsg> {
    let (ref_, _) = ractor::Actor::spawn(Some(next_orch_name()), DummyOrch, ())
        .await
        .expect("spawn");
    ref_
}

fn build_test_app(
    dedupe_store: Arc<dyn DedupeStore>,
    writer_pressure: Arc<dyn WriterPressureGuard>,
    master: ActorRef<OrchestratorMsg>,
) -> Router {
    use vo_core::circuit_breaker::CircuitBreakerState;
    let circuit_breaker = Arc::new(CircuitBreakerState::new());
    let tmp = tempfile::tempdir().expect("create temp dir");
    let db = Arc::new(
        fjall::Database::open(fjall::Config::new(tmp.path()))
            .expect("open test db"),
    );
    db.keyspace("events", fjall::KeyspaceCreateOptions::default)
        .expect("create events partition");
    std::mem::forget(tmp);
    Router::new()
        .route("/api/v1/workflows", post(start_workflow))
        .layer(Extension(master))
        .layer(Extension(writer_pressure))
        .layer(Extension(dedupe_store))
        .layer(Extension(circuit_breaker))
        .layer(Extension(db))
}

fn valid_start_request(dedupe_key: &str) -> V3StartRequest {
    V3StartRequest {
        namespace: "payments".to_string(),
        workflow_type: "checkout".to_string(),
        paradigm: "fsm".to_string(),
        input: json!({"order_id": "ord_123"}),
        instance_id: None,
        dedupe_key: Some(dedupe_key.to_string()),
        workflow_binary_hash: None,
    }
}

// ─── BDD: Shed at threshold ─────────────────────────────────────────────────

#[tokio::test]
async fn given_dbwriter_at_shed_threshold_when_start_arrives_then_retry_after_is_returned() {
    // Given: DbWriter mailbox reaches 80% capacity (simulated via SheddingGuard)
    let dedupe_store: Arc<dyn DedupeStore> = Arc::new(InMemoryDedupeStore::new());
    let writer_pressure: Arc<dyn WriterPressureGuard> = Arc::new(SheddingGuard);
    let master = spawn_dummy_master().await;

    let app = build_test_app(dedupe_store, writer_pressure, master);

    // When: new HTTP workflow start arrives
    let req_body = valid_start_request("dedupe-test-shed-threshold");
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflows")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&req_body).expect("serialize"),
                ))
                .expect("valid request"),
        )
        .await
        .expect("response");

    // Then: API returns 429
    let status = response.status();
    let headers = response.headers().clone();
    let body_bytes = axum::body::to_bytes(response.into_body(), 1024)
        .await
        .expect("read body");
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).expect("parse json");
    eprintln!("DEBUG: status={status}, body={body}");

    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "expected 429 Too Many Requests when DbWriter is at shed threshold, got {status}: {body}"
    );

    // And: Retry-After header is present
    let retry_after = headers
        .get("retry-after")
        .expect("Retry-After header must be present");
    assert_eq!(retry_after, "5", "Retry-After must be 5 seconds");

    // And: response body contains shed error code
    assert_eq!(
        body["error"], "writer_pressure_shed",
        "error code must be writer_pressure_shed"
    );
    assert!(
        body["message"]
            .as_str()
            .expect("message string")
            .contains("writer queue"),
        "message must describe writer queue pressure"
    );
}

#[tokio::test]
async fn given_dbwriter_healthy_when_start_arrives_then_no_shed_occurs() {
    // Given: DbWriter is healthy (simulated via AdmittingGuard)
    let dedupe_store: Arc<dyn DedupeStore> = Arc::new(InMemoryDedupeStore::new());
    let writer_pressure: Arc<dyn WriterPressureGuard> = Arc::new(AdmittingGuard);
    let master = spawn_dummy_master().await;

    let app = build_test_app(dedupe_store, writer_pressure, master);

    // When: new HTTP workflow start arrives
    let req_body = valid_start_request("dedupe-test-shed-healthy");
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflows")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&req_body).expect("serialize"),
                ))
                .expect("valid request"),
        )
        .await
        .expect("response");

    // Then: API does NOT return 429 (it may return 500 since no orchestrator is wired,
    // but it must NOT be a pressure shed rejection)
    assert_ne!(
        response.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "must not shed when writer is healthy"
    );
}

// ─── WatchdogPressureGuard integration ──────────────────────────────────────

#[test]
fn watchdog_guard_sheds_when_degraded_with_writer_pressure() {
    let (tx, rx) = tokio::sync::watch::channel(StorageHealth::Degraded {
        indicators: vec![vo_core::admission::PressureIndicator::WriterQueueDepth],
    });
    let _tx = tx;
    let guard = WatchdogPressureGuard::new(
        rx,
        StorageWatchdogConfig {
            writer_queue_depth_threshold: 500,
            ..StorageWatchdogConfig::default()
        },
    );

    match guard.check() {
        PressureGuardResult::Shed {
            retry_after_secs, ..
        } => {
            assert_eq!(retry_after_secs, 5);
        }
        other => panic!("expected Shed, got {other:?}"),
    }
}

#[test]
fn watchdog_guard_admits_when_degraded_without_writer_pressure() {
    let (tx, rx) = tokio::sync::watch::channel(StorageHealth::Degraded {
        indicators: vec![vo_core::admission::PressureIndicator::CompactionStall],
    });
    let _tx = tx;
    let guard = WatchdogPressureGuard::new(rx, StorageWatchdogConfig::default());

    assert_eq!(guard.check(), PressureGuardResult::Admitted);
}

#[test]
fn watchdog_guard_admits_when_healthy() {
    let (tx, rx) = tokio::sync::watch::channel(StorageHealth::Healthy);
    let _tx = tx;
    let guard = WatchdogPressureGuard::new(rx, StorageWatchdogConfig::default());

    assert_eq!(guard.check(), PressureGuardResult::Admitted);
}

// ─── AppState includes writer_pressure ──────────────────────────────────────

#[tokio::test]
async fn app_state_includes_writer_pressure_field() {
    struct DummyOrch;
    impl ractor::Actor for DummyOrch {
        type Msg = vo_actor::OrchestratorMsg;
        type State = ();
        type Arguments = ();
        async fn pre_start(
            &self,
            _: ractor::ActorRef<Self::Msg>,
            _: Self::Arguments,
        ) -> Result<Self::State, ractor::ActorProcessingErr> {
            Ok(())
        }
    }

    let (master_ref, _) = ractor::Actor::spawn(Some(next_orch_name()), DummyOrch, ())
        .await
        .expect("spawn");

    let projection = Arc::new(ProjectionService::new());
    let state = AppState {
        query: vo_api::handlers::query::QueryState::new(
            Arc::new(vo_storage::partitions::StorageEngine::open(
                tempfile::tempdir().expect("tempdir").path(),
            )
            .expect("open")
            .db()
            .clone()),
            Arc::new(std::sync::RwLock::new(
                vo_types::workspace::WorkspaceIndex::new(),
            )),
            Arc::new(std::sync::RwLock::new(
                vo_types::search::SearchEngine::new(),
            )),
            projection: projection.clone(),
        ),
        sse: vo_api::handlers::sse::SseState::new(),
        ws: vo_api::handlers::ws::WsState::new(),
        master: Arc::new(master_ref),
        circuit_breaker: Arc::new(CircuitBreakerState::new()),
        dedupe_store: Arc::new(InMemoryDedupeStore::new()),
        writer_pressure: Arc::new(WatchdogPressureGuard::permissive()),
        projection,
    };
    let _ = state;
}
