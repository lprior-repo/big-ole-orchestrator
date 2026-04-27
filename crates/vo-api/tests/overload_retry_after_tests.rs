//! BDD tests for Retry-After on overloaded workflow start.
//!
//! Bead: tw-4y6h.14.8
//! ADR refs: ADR-006, ADR-013, ADR-033
//!
//! BDD scenario:
//!   Given execution/write queues exceed load-shedding threshold
//!   When a workflow start request arrives
//!   Then API returns 429 or 503 with Retry-After and writes no admission records

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
use vo_api::types::V3StartRequest;
use vo_core::admission::{PressureGuardResult, WriterPressureGuard};
use vo_storage::dedupe_partition::{DedupeStore, InMemoryDedupeStore};
use vo_types::DedupeKey;

struct OverloadedGuard;

impl WriterPressureGuard for OverloadedGuard {
    fn check(&self) -> PressureGuardResult {
        PressureGuardResult::Shed {
            retry_after_secs: 10,
            reason: "execution/write queues exceed load-shedding threshold".to_string(),
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
    format!(
        "test-orch-overload-{}",
        ORCH_COUNTER.fetch_add(1, Ordering::Relaxed)
    )
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
    Router::new()
        .route("/api/v1/workflows", post(start_workflow))
        .layer(Extension(master))
        .layer(Extension(writer_pressure))
        .layer(Extension(dedupe_store))
}

fn valid_start_request(dedupe_key: &str) -> V3StartRequest {
    V3StartRequest {
        namespace: "payments".to_string(),
        workflow_type: "checkout".to_string(),
        paradigm: "fsm".to_string(),
        input: json!({"order_id": "ord_123"}),
        instance_id: None,
        dedupe_key: Some(dedupe_key.to_string()),
    }
}

// ─── BDD: Overload returns Retry-After ──────────────────────────────────────

#[tokio::test]
async fn given_overload_when_workflow_start_requested_then_retry_after_is_returned() {
    // Given: execution/write queues exceed load-shedding threshold
    let dedupe_store: Arc<dyn DedupeStore> = Arc::new(InMemoryDedupeStore::new());
    let writer_pressure: Arc<dyn WriterPressureGuard> = Arc::new(OverloadedGuard);
    let master = spawn_dummy_master().await;

    let app = build_test_app(dedupe_store.clone(), writer_pressure, master);

    // When: a workflow start request arrives
    let dedupe_key = "dedupe-overload-test-001";
    let req_body = valid_start_request(dedupe_key);
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

    // Then: API returns 429 or 503
    let status = response.status();
    assert!(
        status == StatusCode::TOO_MANY_REQUESTS || status == StatusCode::SERVICE_UNAVAILABLE,
        "expected 429 or 503 under overload, got {status}"
    );

    // And: Retry-After header is present
    let headers = response.headers().clone();
    let retry_after = headers
        .get("retry-after")
        .expect("Retry-After header must be present under overload");
    assert_eq!(retry_after, "10", "Retry-After must match guard value");

    // And: response body contains shed error code
    let body_bytes = axum::body::to_bytes(response.into_body(), 1024)
        .await
        .expect("read body");
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).expect("parse json");
    assert_eq!(
        body["error"], "writer_pressure_shed",
        "error code must be writer_pressure_shed"
    );

    // And: writes no admission records (dedupe store must be empty)
    let dedupe_key_parsed = DedupeKey::parse(dedupe_key).expect("valid dedupe key");
    let contains = dedupe_store
        .contains(&dedupe_key_parsed)
        .expect("contains check");
    assert!(
        !contains,
        "dedupe store must NOT contain the key when request was shed — no admission records written"
    );
}

// ─── BDD: No shed when not overloaded ───────────────────────────────────────

#[tokio::test]
async fn given_no_overload_when_workflow_start_requested_then_no_retry_after() {
    let dedupe_store: Arc<dyn DedupeStore> = Arc::new(InMemoryDedupeStore::new());
    let writer_pressure: Arc<dyn WriterPressureGuard> = Arc::new(AdmittingGuard);
    let master = spawn_dummy_master().await;

    let app = build_test_app(dedupe_store, writer_pressure, master);

    let req_body = valid_start_request("dedupe-no-overload-test");
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

    let status = response.status();
    assert_ne!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "must not return 429 when not overloaded"
    );
    assert_ne!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "must not return 503 when not overloaded (unless actor unavailable)"
    );
}
