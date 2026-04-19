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

#[derive(Debug, thiserror::Error, PartialEq)]
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
    use wiremock::{Match, Mock, MockServer, ResponseTemplate};

    struct AnyMatch;

    impl Match for AnyMatch {
        fn matches(&self, _request: &wiremock::Request) -> bool {
            true
        }
    }

    fn sample_response() -> WorkflowStatusResponse {
        WorkflowStatusResponse {
            instance_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
            namespace: "default".to_string(),
            workflow_type: "test-workflow".to_string(),
            paradigm: "fsm".to_string(),
            phase: "running".to_string(),
            events_applied: 42,
            registration_status: Some("active".to_string()),
            is_quarantined: false,
        }
    }

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
    async fn fetch_workflow_status_success() {
        let server = MockServer::start().await;
        Mock::given(AnyMatch)
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_response()))
            .mount(&server)
            .await;

        let result = fetch_workflow_status(&server.uri(), "01ARZ3NDEKTSV4RRFFQ69G5FAV").await;
        let response = result.expect("should succeed");
        assert_eq!(response.instance_id, "01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert_eq!(response.namespace, "default");
        assert_eq!(response.phase, "running");
        assert_eq!(response.events_applied, 42);
        assert_eq!(response.registration_status, Some("active".to_string()));
        assert!(!response.is_quarantined);
    }

    #[tokio::test]
    async fn fetch_workflow_status_404_returns_not_found() {
        let server = MockServer::start().await;
        Mock::given(AnyMatch)
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let result = fetch_workflow_status(&server.uri(), "missing-id").await;
        let err = result.expect_err("should return NotFound");
        match err {
            StatusError::NotFound { instance_id } => {
                assert_eq!(instance_id, "missing-id");
            }
            other => panic!("expected NotFound, got: {other}"),
        }
    }

    #[tokio::test]
    async fn fetch_workflow_status_500_returns_http_error() {
        let server = MockServer::start().await;
        Mock::given(AnyMatch)
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let result = fetch_workflow_status(&server.uri(), "some-id").await;
        let err = result.expect_err("should return HttpError");
        match err {
            StatusError::HttpError { status, .. } => {
                assert_eq!(status, 500);
            }
            other => panic!("expected HttpError, got: {other}"),
        }
    }

    #[tokio::test]
    async fn fetch_workflow_status_503_returns_http_error() {
        let server = MockServer::start().await;
        Mock::given(AnyMatch)
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let result = fetch_workflow_status(&server.uri(), "some-id").await;
        let err = result.expect_err("should return HttpError");
        match err {
            StatusError::HttpError { status, .. } => {
                assert_eq!(status, 503);
            }
            other => panic!("expected HttpError, got: {other}"),
        }
    }

    #[tokio::test]
    async fn fetch_workflow_status_network_error_returns_unreachable() {
        let result = fetch_workflow_status("http://127.0.0.1:1", "some-id").await;
        let err = result.expect_err("should return Unreachable");
        match err {
            StatusError::Unreachable { url, reason } => {
                assert!(url.contains("127.0.0.1:1"));
                assert!(!reason.is_empty());
            }
            other => panic!("expected Unreachable, got: {other}"),
        }
    }

    #[tokio::test]
    async fn fetch_workflow_status_invalid_json_returns_invalid_response() {
        let server = MockServer::start().await;
        Mock::given(AnyMatch)
            .respond_with(ResponseTemplate::new(200).set_body_string("not json at all"))
            .mount(&server)
            .await;

        let result = fetch_workflow_status(&server.uri(), "some-id").await;
        let err = result.expect_err("should return InvalidResponse");
        match err {
            StatusError::InvalidResponse { reason } => {
                assert!(reason.contains("expected") || reason.contains("JSON"));
            }
            other => panic!("expected InvalidResponse, got: {other}"),
        }
    }

    #[tokio::test]
    async fn fetch_workflow_status_truncated_json_returns_invalid_response() {
        let server = MockServer::start().await;
        Mock::given(AnyMatch)
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"instance_id":"abc"#))
            .mount(&server)
            .await;

        let result = fetch_workflow_status(&server.uri(), "some-id").await;
        let err = result.expect_err("should return InvalidResponse");
        match err {
            StatusError::InvalidResponse { reason } => {
                assert!(!reason.is_empty());
            }
            other => panic!("expected InvalidResponse, got: {other}"),
        }
    }

    #[tokio::test]
    async fn fetch_workflow_status_401_returns_http_error() {
        let server = MockServer::start().await;
        Mock::given(AnyMatch)
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let result = fetch_workflow_status(&server.uri(), "some-id").await;
        let err = result.expect_err("should return HttpError");
        match err {
            StatusError::HttpError { status, .. } => {
                assert_eq!(status, 401);
            }
            other => panic!("expected HttpError, got: {other}"),
        }
    }

    #[tokio::test]
    async fn run_status_delegates_to_fetch_workflow_status() {
        let server = MockServer::start().await;
        Mock::given(AnyMatch)
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_response()))
            .mount(&server)
            .await;

        let config = StatusConfig {
            engine_url: server.uri(),
            instance_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
        };

        let result = run_status(&config).await;
        let response = result.expect("should succeed");
        assert_eq!(response.instance_id, "01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert_eq!(response.phase, "running");
    }

    #[tokio::test]
    async fn run_status_propagates_404_error() {
        let server = MockServer::start().await;
        Mock::given(AnyMatch)
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let config = StatusConfig {
            engine_url: server.uri(),
            instance_id: "missing-instance".to_string(),
        };

        let result = run_status(&config).await;
        let err = result.expect_err("should propagate NotFound");
        match err {
            StatusError::NotFound { instance_id } => {
                assert_eq!(instance_id, "missing-instance");
            }
            other => panic!("expected NotFound, got: {other}"),
        }
    }
}
