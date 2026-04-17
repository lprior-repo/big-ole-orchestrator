//! Axum Router with all endpoint wiring.
//!
//! Call [`create_router`] to produce a fully configured `Router` ready to serve
//! via `axum::serve`. Shared state (query DB, SSE/WS broadcasters, actor refs)
//! is injected through [`AppState`].

use axum::{
    extract::Extension,
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;
use std::time::Duration;
#[allow(deprecated)]
use tower_http::{cors::CorsLayer, timeout::TimeoutLayer, trace::TraceLayer};

use crate::handlers::query::QueryState;
use crate::handlers::sse::SseState;
use crate::handlers::ws::WsState;
use ractor::ActorRef;
use vo_actor::OrchestratorMsg;

// ---------------------------------------------------------------------------
// Shared application state
// ---------------------------------------------------------------------------

/// Top-level state container held by every route.
///
/// Individual handler groups receive their own typed sub-state (`State<T>`),
/// while the orchestrator actor ref is injected as an `Extension` so that
/// adding or removing actor-dependent handlers does not change the `State`
/// type signature.
#[derive(Clone)]
pub struct AppState {
    pub query: QueryState,
    pub sse: SseState,
    pub ws: WsState,
    pub master: Arc<ActorRef<OrchestratorMsg>>,
}

// ---------------------------------------------------------------------------
// Router construction
// ---------------------------------------------------------------------------

/// Build the complete API router.
///
/// All state is provided up-front via [`AppState`]. The returned router is
/// ready to pass to `axum::serve(listener, router)`.
#[allow(deprecated)]
pub fn create_router(state: AppState) -> Router {
    // Workflow CRUD — uses Extension<ActorRef<OrchestratorMsg>>
    let workflow_routes = Router::new()
        .route("/api/v1/workflows", post(crate::handlers::start_workflow))
        .route("/api/v1/workflows", get(crate::handlers::list_workflows))
        .route("/api/v1/workflows/{id}", get(crate::handlers::get_workflow))
        .route(
            "/api/v1/workflows/{id}",
            delete(crate::handlers::terminate_workflow),
        )
        .layer(Extension(state.master.clone()));

    // Query endpoints — uses State<QueryState>
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

    // Signal endpoint — uses Extension<ActorRef<OrchestratorMsg>>
    let signal_routes = Router::new()
        .route(
            "/api/v1/workflows/{id}/signals",
            post(crate::handlers::send_signal),
        )
        .layer(Extension(state.master.clone()));

    // Events endpoint — uses Extension<ActorRef<OrchestratorMsg>>
    let event_routes = Router::new()
        .route(
            "/api/v1/workflows/{id}/events",
            get(crate::handlers::get_events),
        )
        .layer(Extension(state.master.clone()));

    // SSE streaming — uses Extension + State<SseState>
    let sse_routes = Router::new()
        .route("/api/v1/watch/{id}", get(crate::handlers::watch_workflow))
        .with_state(state.sse.clone())
        .layer(Extension(state.master.clone()));

    // WebSocket streaming — uses State<WsState>
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
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_state_is_clone() {
        // AppState must be Clone for axum State extraction.
        fn assert_clone<T: Clone>() {}
        assert_clone::<AppState>();
    }
}
