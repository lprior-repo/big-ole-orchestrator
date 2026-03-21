//! wtf_client client - HTTP + SSE client for wtf-api

use thiserror::Error;

#[derive(Error, Debug)]
pub enum WtfClientError {
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("Deserialization error: {0}")]
    Deserialize(String),
    #[error("SSE error: {0}")]
    Sse(String),
    #[error("Request failed: {0}")]
    RequestFailed(String),
}

pub type Result<T> = std::result::Result<T, WtfClientError>;

#[derive(Clone)]
pub struct WtfClient {
    base_url: String,
    client: reqwest::Client,
}

impl WtfClient {
    #[must_use]
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub async fn post_json(
        &self,
        path: &str,
        body: String,
    ) -> std::result::Result<reqwest::Response, WtfClientError> {
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), path);
        self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| WtfClientError::RequestFailed(e.to_string()))
    }
}
