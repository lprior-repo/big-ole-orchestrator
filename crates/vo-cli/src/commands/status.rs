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
}
