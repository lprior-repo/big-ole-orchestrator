//! Axum application assembly — router, Extension injection, middleware (bead wtf-egjj).
//!
//! Builds the full `axum::Router` with:
//! - API routes under `/api/v1/`
//! - Health + metrics at `/health` and `/metrics`
//! - `OrchestratorMsg` ActorRef injected via `Extension`
//! - Request tracing middleware (tower-http `TraceLayer`)

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use std::net::SocketAddr;

use axum::{
    extract::Extension,
    routing::{delete, get, post},
    Router,
};
use ractor::ActorRef;
use tower_http::trace::TraceLayer;
use wtf_actor::OrchestratorMsg;

use crate::{handlers, health};

/// Build the complete axum `Router` for wtf-api.
///
/// Injects the `OrchestratorMsg` actor ref as an `Extension` for all API routes.
///
/// # Routes
/// - `GET /health` — liveness/readiness probe
/// - `GET /metrics` — Prometheus metrics (stub)
/// - `POST /api/v1/workflows` — start a new workflow instance
/// - `GET /api/v1/workflows` — list active instances
/// - `GET /api/v1/workflows/:id` — get instance status
/// - `DELETE /api/v1/workflows/:id` — terminate instance
/// - `POST /api/v1/workflows/:id/signals` — send a signal
/// - `GET /api/v1/workflows/:id/events` — stream event log
/// - `POST /api/v1/definitions/:type` — ingest and lint workflow definition
#[must_use]
pub fn build_app(master: ActorRef<OrchestratorMsg>) -> Router {
    let api_routes = Router::new()
        .route("/workflows", post(handlers::start_workflow))
        .route("/workflows", get(handlers::list_workflows))
        .route("/workflows/:id", get(handlers::get_workflow))
        .route("/workflows/:id", delete(handlers::terminate_workflow))
        .route("/workflows/:id/signals", post(handlers::send_signal))
        .route("/workflows/:id/events", get(handlers::get_events))
        .layer(Extension(master));

    Router::new()
        .route("/health", get(health::health_handler))
        .route("/metrics", get(health::metrics_handler))
        .nest("/api/v1", api_routes)
        .layer(TraceLayer::new_for_http())
}

/// Bind and serve the API on `addr` until `shutdown_rx` fires.
///
/// # Errors
/// Returns an error if the TCP listener fails to bind.
pub async fn serve(
    addr: SocketAddr,
    app: Router,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> Result<(), std::io::Error> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(addr = %addr, "wtf-api listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.changed().await;
            tracing::info!("wtf-api shutting down gracefully");
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::util::ServiceExt;

    // We can't easily create a real ActorRef in tests without a running ractor system.
    // Health and metrics don't need the ActorRef, so we test just those routes.

    fn app_without_actor() -> Router {
        Router::new()
            .route("/health", get(health::health_handler))
            .route("/metrics", get(health::metrics_handler))
            .layer(TraceLayer::new_for_http())
    }

    #[tokio::test]
    async fn health_endpoint_returns_200() {
        let app = app_without_actor();
        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .expect("build request");

        let response = app.oneshot(request).await.expect("call");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn metrics_endpoint_returns_200() {
        let app = app_without_actor();
        let request = Request::builder()
            .uri("/metrics")
            .body(Body::empty())
            .expect("build request");

        let response = app.oneshot(request).await.expect("call");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn unknown_route_returns_404() {
        let app = app_without_actor();
        let request = Request::builder()
            .uri("/nonexistent")
            .body(Body::empty())
            .expect("build request");

        let response = app.oneshot(request).await.expect("call");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
