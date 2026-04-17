use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct StatusConfig {
    pub engine_url: String,
    pub instance_id: String,
}

impl Default for StatusConfig {
    fn default() -> Self {
        Self {
            engine_url: "http://localhost:3000".to_string(),
            instance_id: String::new(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StatusError {
    #[error("API unreachable at {url}: {reason}")]
    Unreachable { url: String, reason: String },

    #[error("API returned HTTP {status} for {url}")]
    HttpError { url: String, status: u16 },

    #[error("invalid response from API: {reason}")]
    InvalidResponse { reason: String },

    #[error("not found: workflow {instance_id}")]
    NotFound { instance_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStatusResponse {
    pub instance_id: String,
    pub namespace: String,
    pub workflow_type: String,
    pub paradigm: String,
    pub phase: String,
    pub events_applied: u64,
    #[serde(default)]
    pub registration_status: Option<String>,
    #[serde(default)]
    pub is_quarantined: bool,
}

impl WorkflowStatusResponse {
    pub fn lineage_id(&self) -> &str {
        &self.instance_id
    }
}

pub async fn fetch_workflow_status(
    engine_url: &str,
    instance_id: &str,
) -> Result<WorkflowStatusResponse, StatusError> {
    let url = format!("{}/api/v1/workflows/{}/status", engine_url, instance_id);

    let response = reqwest::get(&url)
        .await
        .map_err(|e| StatusError::Unreachable {
            url: url.clone(),
            reason: e.to_string(),
        })?;

    let status = response.status().as_u16();
    if status == 404 {
        return Err(StatusError::NotFound {
            instance_id: instance_id.to_string(),
        });
    }
    if !response.status().is_success() {
        return Err(StatusError::HttpError { url, status });
    }

    let body = response
        .bytes()
        .await
        .map_err(|e| StatusError::InvalidResponse {
            reason: e.to_string(),
        })?;

    serde_json::from_slice(&body).map_err(|e| StatusError::InvalidResponse {
        reason: e.to_string(),
    })
}

pub async fn run_status(config: &StatusConfig) -> Result<WorkflowStatusResponse, StatusError> {
    fetch_workflow_status(&config.engine_url, &config.instance_id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{matchers::{method, path}, Mock, ResponseTemplate};
    use std::time::Duration;

    #[test]
    fn status_config_default_engine_url() {
        let config = StatusConfig::default();
        assert_eq!(config.engine_url, "http://localhost:3000");
    }

    #[test]
    fn workflow_status_response_lineage_id_returns_instance_id() {
        let response = WorkflowStatusResponse {
            instance_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
            namespace: "default".to_string(),
            workflow_type: "test".to_string(),
            paradigm: "fsm".to_string(),
            phase: "running".to_string(),
            events_applied: 42,
            registration_status: None,
            is_quarantined: false,
        };
        assert_eq!(response.lineage_id(), "01ARZ3NDEKTSV4RRFFQ69G5FAV");
    }

    #[test]
    fn status_error_not_found_displays_correctly() {
        let err = StatusError::NotFound {
            instance_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "not found: workflow 01ARZ3NDEKTSV4RRFFQ69G5FAV"
        );
    }

    #[test]
    fn status_error_unreachable_displays_correctly() {
        let err = StatusError::Unreachable {
            url: "http://localhost:3000".to_string(),
            reason: "connection refused".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "API unreachable at http://localhost:3000: connection refused"
        );
    }

    #[test]
    fn status_error_http_error_displays_correctly() {
        let err = StatusError::HttpError {
            url: "http://localhost:3000/api/v1/workflows/123/status".to_string(),
            status: 500,
        };
        assert_eq!(
            err.to_string(),
            "API returned HTTP 500 for http://localhost:3000/api/v1/workflows/123/status"
        );
    }

    #[test]
    fn status_error_invalid_response_displays_correctly() {
        let err = StatusError::InvalidResponse {
            reason: "unexpected end of JSON".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "invalid response from API: unexpected end of JSON"
        );
    }

    #[test]
    fn status_config_with_custom_values() {
        let config = StatusConfig {
            engine_url: "http://localhost:9000".to_string(),
            instance_id: "test-instance-123".to_string(),
        };
        assert_eq!(config.engine_url, "http://localhost:9000");
        assert_eq!(config.instance_id, "test-instance-123");
    }

    #[tokio::test]
    async fn fetch_workflow_status_returns_success_response() {
        let instance_id = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
        let server = wiremock::MockServer::start().await;
        let server_url = server.uri();

        Mock::given(method("GET"))
            .and(path(format!("/api/v1/workflows/{}/status", instance_id)))
            .respond_with(ResponseTemplate::new(200).set_body_json(WorkflowStatusResponse {
                instance_id: instance_id.to_string(),
                namespace: "default".to_string(),
                workflow_type: "test".to_string(),
                paradigm: "fsm".to_string(),
                phase: "running".to_string(),
                events_applied: 42,
                registration_status: Some("registered".to_string()),
                is_quarantined: false,
            }))
            .mount(&server)
            .await;

        let result = fetch_workflow_status(&server_url, instance_id).await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.instance_id, instance_id);
        assert_eq!(response.namespace, "default");
        assert_eq!(response.phase, "running");
        assert_eq!(response.events_applied, 42);
    }

    #[tokio::test]
    async fn fetch_workflow_status_returns_not_found_for_404() {
        let instance_id = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
        let server = wiremock::MockServer::start().await;
        let server_url = server.uri();

        Mock::given(method("GET"))
            .and(path(format!("/api/v1/workflows/{}/status", instance_id)))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let result = fetch_workflow_status(&server_url, instance_id).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        matches!(err, StatusError::NotFound { instance_id: id } if id == instance_id);
    }

    #[tokio::test]
    async fn fetch_workflow_status_returns_http_error_for_500() {
        let instance_id = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
        let server = wiremock::MockServer::start().await;
        let server_url = server.uri();

        Mock::given(method("GET"))
            .and(path(format!("/api/v1/workflows/{}/status", instance_id)))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let result = fetch_workflow_status(&server_url, instance_id).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        matches!(err, StatusError::HttpError { status: 500, .. });
    }

    #[tokio::test]
    async fn fetch_workflow_status_returns_invalid_response_for_malformed_json() {
        let instance_id = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
        let server = wiremock::MockServer::start().await;
        let server_url = server.uri();

        Mock::given(method("GET"))
            .and(path(format!("/api/v1/workflows/{}/status", instance_id)))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw("not valid json".as_bytes(), "text/plain"),
            )
            .mount(&server)
            .await;

        let result = fetch_workflow_status(&server_url, instance_id).await;
        assert!(result.is_err());
        matches!(result.unwrap_err(), StatusError::InvalidResponse { .. });
    }

    #[tokio::test]
    async fn run_status_delegates_to_fetch_workflow_status() {
        let instance_id = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
        let server = wiremock::MockServer::start().await;
        let server_url = server.uri();

        Mock::given(method("GET"))
            .and(path(format!("/api/v1/workflows/{}/status", instance_id)))
            .respond_with(ResponseTemplate::new(200).set_body_json(WorkflowStatusResponse {
                instance_id: instance_id.to_string(),
                namespace: "test-ns".to_string(),
                workflow_type: "batch".to_string(),
                paradigm: "fsm".to_string(),
                phase: "completed".to_string(),
                events_applied: 100,
                registration_status: None,
                is_quarantined: true,
            }))
            .mount(&server)
            .await;

        let config = StatusConfig {
            engine_url: server_url,
            instance_id: instance_id.to_string(),
        };
        let result = run_status(&config).await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.instance_id, instance_id);
        assert_eq!(response.phase, "completed");
        assert!(response.is_quarantined);
    }

    #[tokio::test]
    async fn fetch_workflow_status_unreachable_network_error() {
        let result = fetch_workflow_status("http://localhost:9999", "test-id").await;
        assert!(result.is_err());
        matches!(result.unwrap_err(), StatusError::Unreachable { .. });
    }

    #[tokio::test]
    async fn status_error_variants_have_correct_display() {
        let not_found = StatusError::NotFound {
            instance_id: "test-123".to_string(),
        };
        assert!(not_found.to_string().contains("not found"));
        assert!(not_found.to_string().contains("test-123"));

        let unreachable = StatusError::Unreachable {
            url: "http://example.com".to_string(),
            reason: "timeout".to_string(),
        };
        assert!(unreachable.to_string().contains("unreachable"));
        assert!(unreachable.to_string().contains("timeout"));

        let http_err = StatusError::HttpError {
            url: "http://example.com".to_string(),
            status: 503,
        };
        assert!(http_err.to_string().contains("HTTP 503"));

        let invalid = StatusError::InvalidResponse {
            reason: "truncated".to_string(),
        };
        assert!(invalid.to_string().contains("invalid response"));
    }
}
