//! Unquarantine command for circuit-breaker recovery (ADR-026).

use std::time::Duration;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Result of an unquarantine operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnquarantineResult {
    pub workflow_name: String,
    pub previous_status: String,
    pub new_status: String,
    pub failures_cleared: usize,
}

/// Error type for the unquarantine command.
<<<<<<< HEAD
#[derive(Debug, Error, PartialEq)]
=======
#[derive(Debug, Error)]
>>>>>>> origin/vo-worker-tests
pub enum UnquarantineError {
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("API error: {status} - {message}")]
    ApiError { status: u16, message: String },

    #[error("Invalid workflow name: {0}")]
    InvalidWorkflowName(String),

    #[error("Engine not reachable at {0}")]
    EngineNotReachable(String),

    #[error("Unquarantine failed: {0}")]
    UnquarantineFailed(String),
}

/// Execute the unquarantine command.
///
/// # Arguments
/// * `engine_url` - The vo-engine API URL
/// * `workflow_name` - The name of the workflow to unquarantine
/// * `operator` - The operator performing the unquarantine
///
/// # Returns
/// `UnquarantineResult` on success
///
/// # Errors
/// Returns `UnquarantineError` if the operation fails
pub async fn unquarantine_workflow(
    engine_url: &str,
    workflow_name: &str,
    operator: &str,
) -> Result<UnquarantineResult, UnquarantineError> {
    // Validate workflow name
    if workflow_name.is_empty() {
        return Err(UnquarantineError::InvalidWorkflowName(
            "workflow name cannot be empty".to_string(),
        ));
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| UnquarantineError::EngineNotReachable(e.to_string()))?;

    let url = format!("{}/api/v1/workflows/{}/unquarantine", engine_url, workflow_name);

    let response = client
        .post(&url)
        .json(&serde_json::json!({
            "operator": operator
        }))
        .send()
        .await
        .map_err(|e| UnquarantineError::EngineNotReachable(e.to_string()))?;

    let status = response.status();

    if status.is_success() {
        let result: UnquarantineResult = response
            .json()
            .await
            .map_err(|e| UnquarantineError::UnquarantineFailed(e.to_string()))?;
        Ok(result)
    } else {
        let error_body = response
            .text()
            .await
            .unwrap_or_else(|_| "unknown error".to_string());

        Err(UnquarantineError::ApiError {
            status: status.as_u16(),
            message: error_body,
        })
    }
}

/// Format and display the unquarantine result.
pub fn display_result(result: &UnquarantineResult) {
    println!(
        "Workflow '{}' unquarantine successful:",
        result.workflow_name
    );
    println!("  Previous status: {}", result.previous_status);
    println!("  New status: {}", result.new_status);
    println!("  Failures cleared: {}", result.failures_cleared);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_invalid_workflow_name_returns_error() {
        let result = unquarantine_workflow("http://localhost:3000", "", "operator").await;
        assert!(matches!(result, Err(UnquarantineError::InvalidWorkflowName(_))));
    }

    #[tokio::test]
    async fn test_empty_workflow_name() {
        let result = unquarantine_workflow("http://localhost:3000", "", "operator").await;
        assert!(matches!(result, Err(UnquarantineError::InvalidWorkflowName(msg)) if msg.contains("empty")));
    }
}
