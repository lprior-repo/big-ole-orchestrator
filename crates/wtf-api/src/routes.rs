//! routes.rs - HTTP routes for wtf-api

use axum::{
    routing::{delete, get, post},
    Router,
};
use ractor::ActorRef;
use wtf_actor::OrchestratorMsg;

use crate::handlers;

pub fn create_routes(master: ActorRef<OrchestratorMsg>) -> Router {
    Router::new()
        .route("/workflows", get(handlers::list_workflows))
        .route("/workflows/:invocation_id", get(handlers::get_workflow))
        .route(
            "/workflows/:invocation_id",
            delete(handlers::terminate_workflow),
        )
        .route(
            "/workflows/:invocation_id/events",
            get(handlers::get_events),
        )
        .route("/definitions/:type", post(handlers::ingest_definition))
        .with_state(master)
}
