use std::time::Duration;
use reqwest::Client;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct CompensateConfig {
    pub engine_url: String,
    pub workflow_id: String,
    pub force: bool,
}

impl Default for CompensateConfig {
    fn default() -> Self {
        Self {
            engine_url: "http://localhost:3000".to_string(),
            workflow_id: String::new(),
            force: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CompensateError {
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("API error: HTTP {status} - {message}")]
    ApiError { status: u16, message: String },

    #[error("Engine not reachable at {0}")]
    EngineNotReachable(String),

    #[error("Compensate failed: {0}")]
    CompensateFailed(String),

    #[error("Compensation aborted by user")]
    Aborted,
}

pub async fn run_compensate(config: &CompensateConfig) -> Result<(), CompensateError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| CompensateError::EngineNotReachable(e.to_string()))?;

    let url = format!("{}/api/v1/workflows/{}/compensate", config.engine_url, config.workflow_id);

    let response = client
        .post(&url)
        .send()
        .await
        .map_err(|e| CompensateError::EngineNotReachable(e.to_string()))?;

    let status = response.status().as_u16();

    if status == 202 {
        println!("Compensation initiated for workflow {}.", config.workflow_id);
        Ok(())
    } else if status == 0 {
        Err(CompensateError::CompensateFailed(
            "Connection failed".to_string(),
        ))
    } else {
        let error_body = response
            .text()
            .await
            .unwrap_or_else(|_| "unknown error".to_string());
        Err(CompensateError::ApiError {
            status,
            message: error_body,
        })
    }
}

pub fn prompt_confirmation(workflow_id: &str) -> bool {
    print!("Compensate workflow {}? This will attempt to undo its effects. [y/N] ", workflow_id);
    std::io::Write::flush(&mut std::io::stdout()).ok();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    input.trim().eq_ignore_ascii_case("y")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compensate_config_default_engine_url() {
        let config = CompensateConfig::default();
        assert_eq!(config.engine_url, "http://localhost:3000");
    }

    #[test]
    fn compensate_config_force_defaults_to_false() {
        let config = CompensateConfig::default();
        assert!(!config.force);
    }
}
