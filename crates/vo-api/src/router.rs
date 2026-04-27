//! Axum Router with all endpoint wiring.
//!
//! Call [`create_router`] to produce a fully configured `Router` ready to serve
//! via `axum::serve`. Shared state (query DB, SSE/WS broadcasters, actor refs,
//! dedupe store) is injected through [`AppState`].

use axum::{
    extract::Extension,
    response::Html,
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;
use std::time::Duration;
use tower_http::{cors::CorsLayer, limit::RequestBodyLimitLayer, timeout::TimeoutLayer, trace::TraceLayer};

use crate::handlers::query::QueryState;
use crate::handlers::sse::SseState;
use crate::handlers::ws::WsState;
use ractor::ActorRef;
use vo_actor::OrchestratorMsg;
use vo_core::admission::WriterPressureGuard;
use vo_core::circuit_breaker::CircuitBreakerState;
use vo_storage::dedupe_partition::DedupeStore;

// ---------------------------------------------------------------------------
// Shared application state
// ---------------------------------------------------------------------------

/// Top-level state container held by every route.
///
/// Individual handler groups receive their own typed sub-state (`State<T>`),
/// while the orchestrator actor ref and dedupe store are injected as
/// `Extension`s so that adding or removing handlers does not change the
/// `State` type signature.
#[derive(Clone)]
pub struct AppState {
    pub query: QueryState,
    pub sse: SseState,
    pub ws: WsState,
    pub master: Arc<ActorRef<OrchestratorMsg>>,
    pub circuit_breaker: Arc<CircuitBreakerState>,
    /// Dedupe store for ADR-028 exactly-once ingress deduplication.
    pub dedupe_store: Arc<dyn DedupeStore>,
    /// Writer pressure guard for ADR-006/ADR-015 ingress load shedding.
    pub writer_pressure: Arc<dyn WriterPressureGuard>,
}

// ---------------------------------------------------------------------------
// Router construction
// ---------------------------------------------------------------------------

/// Build the complete API router.
///
/// All state is provided up-front via [`AppState`]. The returned router is
/// ready to pass to `axum::serve(listener, router)`.
pub fn create_router(state: AppState) -> Router {
    // Workflow CRUD -- uses Extension<ActorRef<OrchestratorMsg>> + DedupeStore + CircuitBreaker
    let workflow_routes = Router::new()
        .route("/api/v1/workflows", post(crate::handlers::start_workflow))
        .route("/api/v1/workflows", get(crate::handlers::list_workflows))
        .route("/api/v1/workflows/{id}", get(crate::handlers::get_workflow))
        .route(
            "/api/v1/workflows/{id}",
            delete(crate::handlers::terminate_workflow),
        )
        .route(
            "/api/v1/workflows/{id}/status",
            get(crate::handlers::get_workflow_status),
        )
        .route(
            "/api/v1/workflows/{id}/unquarantine",
            post(crate::handlers::unquarantine_workflow),
        )
        .route(
            "/api/v1/workflows/{id}/compensate",
            post(crate::handlers::compensate_workflow),
        )
        .layer(Extension(state.master.as_ref().clone()))
        .layer(Extension(state.circuit_breaker.clone()))
        .layer(Extension(state.dedupe_store.clone()))
        .layer(Extension(state.writer_pressure.clone()))
        .layer(Extension(state.query.db.clone()));

    // Query endpoints -- uses State<QueryState>
    let query_routes = Router::new()
        .route(
            "/api/v1/workflows/{id}/timeline",
            get(crate::handlers::get_timeline),
        )
        .route(
            "/api/v1/workflows/{id}/history",
            get(crate::handlers::get_history),
        )
        .route(
            "/api/v1/workflows/{id}/effect-journal",
            get(crate::handlers::get_effect_journal),
        )
        .route(
            "/api/v1/workflows/{id}/version",
            get(crate::handlers::get_workflow_version),
        )
        .route("/api/v1/search", get(crate::handlers::search))
        .with_state(state.query.clone());

    // Signal endpoint -- uses Extension<ActorRef<OrchestratorMsg>> + Extension<Arc<dyn DedupeStore>>
    let signal_routes = Router::new()
        .route(
            "/api/v1/workflows/{id}/signals",
            post(crate::handlers::send_signal),
        )
        .layer(Extension(state.master.as_ref().clone()))
        .layer(Extension(state.dedupe_store.clone()))
        .layer(Extension(state.query.db.clone()));

    // Events endpoint -- uses Extension<ActorRef<OrchestratorMsg>>
    let event_routes = Router::new()
        .route(
            "/api/v1/workflows/{namespace}/{id}/events",
            get(crate::handlers::get_events_namespaced),
        )
        .route(
            "/api/v1/workflows/{id}/events",
            get(crate::handlers::get_events),
        )
        .layer(Extension(state.master.as_ref().clone()))
        .layer(Extension(state.query.db.clone()));

    let ui_routes = Router::new().route("/wtf/ui", get(wtf_ui));

    // SSE streaming -- uses Extension + State<SseState>
    let sse_routes = Router::new()
        .route("/api/v1/watch/{id}", get(crate::handlers::watch_workflow))
        .with_state(state.sse.clone())
        .layer(Extension(state.master.as_ref().clone()));

    // WebSocket streaming -- uses State<WsState>
    let ws_routes = Router::new()
        .route("/api/v1/ws/{id}", get(crate::handlers::ws_workflow))
        .with_state(state.ws.clone());

    Router::new()
        .merge(workflow_routes)
        .merge(query_routes)
        .merge(signal_routes)
        .merge(event_routes)
        .merge(sse_routes)
        .merge(ws_routes)
        .layer(RequestBodyLimitLayer::new(1048576))
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

async fn wtf_ui() -> Html<&'static str> {
    Html(
        r#"<!doctype html>
<html>
<head><title>Veloxide WTF UI</title></head>
<body>
<h1>Veloxide WTF UI</h1>
<p>Dioxus app shell route is ready.</p>
</body>
</html>"#,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handlers::{WorkflowSseEvent, WorkflowWsEvent};

    #[test]
    fn app_state_is_clone() {
        fn assert_clone<T: Clone>() {}
        assert_clone::<AppState>();
    }

    struct DummyOrchestrator;

    impl ractor::Actor for DummyOrchestrator {
        type Msg = OrchestratorMsg;
        type State = ();
        type Arguments = ();

        async fn pre_start(
            &self,
            _myself: ActorRef<Self::Msg>,
            _args: Self::Arguments,
        ) -> Result<Self::State, ractor::ActorProcessingErr> {
            Ok(())
        }
    }

    /// BDD: Given a server bootstrap creates AppState,
    ///      When workflow start and query handlers are registered,
    ///      Then both handlers share the same storage/orchestrator handles
    ///      instead of independent test-only state.
    ///
    /// Proves handle identity for: storage DB, circuit breaker, orchestrator ref.
    /// SSE/WS broadcaster sharing verified via cross-cloned receiver receiving events.
    #[tokio::test]
    async fn given_server_bootstrap_when_app_state_created_then_handlers_share_runtime_handles() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let storage =
            vo_storage::partitions::StorageEngine::open(tmp.path()).expect("StorageEngine::open");

        let circuit_breaker = Arc::new(vo_core::circuit_breaker::CircuitBreakerState::new());
        let db_handle = Arc::new(storage.db().clone());
        let workspace_index = Arc::new(std::sync::RwLock::new(
            vo_types::workspace::WorkspaceIndex::new(),
        ));
        let query_state = QueryState {
            db: db_handle.clone(),
            workspace_index,
        };

        let (master_ref, _handle) =
            ractor::Actor::spawn(Some("test-orchestrator".to_string()), DummyOrchestrator, ())
                .await
                .expect("spawn dummy orchestrator");
        let master = Arc::new(master_ref);

        let state = AppState {
            query: query_state,
            sse: SseState::new(),
            ws: WsState::new(),
            master: master.clone(),
            circuit_breaker: circuit_breaker.clone(),
            dedupe_store: Arc::new(vo_storage::dedupe_partition::InMemoryDedupeStore::new()),
            writer_pressure: Arc::new(vo_core::admission::WatchdogPressureGuard::permissive()),
        };

        let _router = create_router(state.clone());
        let cloned = state.clone();

        assert!(
            Arc::ptr_eq(&cloned.query.db, &db_handle),
            "query handler must share the same storage DB handle"
        );
        assert!(
            Arc::ptr_eq(&cloned.circuit_breaker, &circuit_breaker),
            "workflow handler must share the same circuit breaker handle"
        );
        assert!(
            Arc::ptr_eq(&cloned.master, &master),
            "workflow/signal/event handlers must share the same orchestrator actor ref"
        );

        let mut sse_rx = cloned.sse.broadcaster.subscribe();
        state
            .sse
            .broadcaster
            .send(WorkflowSseEvent::InstanceCompleted)
            .expect("sse send");
        let received = sse_rx.recv().await;
        assert!(
            received.is_ok(),
            "SSE clone must share the same broadcaster channel"
        );

        let mut ws_rx = cloned.ws.broadcaster.subscribe();
        state
            .ws
            .broadcaster
            .send(WorkflowWsEvent::InstanceCompleted)
            .expect("ws send");
        let received = ws_rx.recv().await;
        assert!(
            received.is_ok(),
            "WS clone must share the same broadcaster channel"
        );
    }
}
