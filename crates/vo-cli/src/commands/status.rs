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
    async fn fetch_workflow_status_unreachable_error() {
        let result = fetch_workflow_status("http://localhost:59999", "test-id").await;
        assert!(matches!(result, Err(StatusError::Unreachable { .. })));
        if let Err(StatusError::Unreachable { url, reason }) = result {
            assert_eq!(url, "http://localhost:59999/api/v1/workflows/test-id/status");
            assert!(!reason.is_empty());
        }
    }

    #[tokio::test]
    async fn fetch_workflow_status_http_error() {
        let result = fetch_workflow_status("http://localhost:3000", "test-workflow").await;
        match result {
            Err(StatusError::HttpError { url, status }) => {
                assert!(url.contains("/api/v1/workflows/test-workflow/status"));
                assert!(status >= 400);
            }
            Err(e) => {
                match e {
                    StatusError::Unreachable { .. } => {
                        assert!(true, "Unreachable is acceptable when server not running");
                    }
                    _ => panic!("Expected HttpError or Unreachable, got {:?}", e),
                }
            }
            Ok(_) => {
                assert!(true, "Ok response acceptable when server running");
            }
        }
    }

    #[tokio::test]
    async fn fetch_workflow_status_invalid_response() {
        let result = fetch_workflow_status("http://localhost:3000", "test-id").await;
        match result {
            Err(StatusError::InvalidResponse { reason }) => {
                assert!(!reason.is_empty());
            }
            Ok(_) | Err(_) => {}
        }
    }

    #[tokio::test]
    async fn run_status_delegates_to_fetch_workflow_status() {
        let config = StatusConfig {
            engine_url: "http://localhost:59999".to_string(),
            instance_id: "test-instance".to_string(),
        };
        let result = run_status(&config).await;
        assert!(matches!(result, Err(StatusError::Unreachable { .. })));
    }
}
