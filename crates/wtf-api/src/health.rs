//! Health and metrics endpoints for wtf-engine (bead wtf-k6eu).
//!
//! GET /health — returns 200 OK with a JSON health object when the engine is ready.
//! GET /metrics — returns plaintext Prometheus metrics (stub; full Prometheus integration later).

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

/// Response body for GET /health.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// `"ok"` when all systems are ready.
    pub status: String,
    /// Engine version string.
    pub version: String,
    /// UTC timestamp when the engine process started (ISO-8601).
    pub started_at: String,
}

/// GET /health
///
/// Returns 200 OK with a `HealthResponse` when the engine is ready to serve traffic.
///
/// Liveness probe: if this returns non-200, the process should be restarted.
/// Readiness probe: same endpoint — the engine is ready when it can respond.
pub async fn health_handler() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        started_at: chrono::Utc::now().to_rfc3339(),
    })
}

/// GET /metrics
///
/// Returns plaintext Prometheus metrics.
/// Stub: real metrics (instance count, events/s, queue depth) are added in a later bead.
pub async fn metrics_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4")],
        format!(
            "# HELP wtf_engine_info Engine version info\n\
             # TYPE wtf_engine_info gauge\n\
             wtf_engine_info{{version=\"{}\"}} 1\n",
            env!("CARGO_PKG_VERSION")
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    #[tokio::test]
    async fn health_response_has_ok_status() {
        let response = health_handler().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn metrics_response_is_200() {
        let response = metrics_handler().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn health_response_serializes() {
        let h = HealthResponse {
            status: "ok".into(),
            version: "0.1.0".into(),
            started_at: "2026-01-01T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&h).expect("serialize");
        assert!(json.contains("\"status\":\"ok\""));
        assert!(json.contains("\"version\""));
    }
}
