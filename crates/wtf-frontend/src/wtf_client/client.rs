//! wtf_client client - placeholder for full implementation

use thiserror::Error;

#[derive(Error, Debug)]
pub enum WtfClientError {
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("Deserialization error: {0}")]
    Deserialize(String),
    #[error("SSE error: {0}")]
    Sse(String),
}

pub type Result<T> = std::result::Result<T, WtfClientError>;

pub struct WtfClient {
    base_url: String,
}

impl WtfClient {
    #[must_use]
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
        }
    }
}
