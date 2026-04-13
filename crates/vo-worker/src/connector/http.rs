//! Idempotency-key HTTP connector (ADR-041).

use async_trait::async_trait;
use crate::connector::{
    CommitOutcome, Connector, ConnectorError, PreparedEffect, ReconcileOutcome,
};

/// HTTP connector with idempotency-key support for REST APIs.
pub struct HttpConnector {
    base_url: String,
    client: reqwest::Client,
}

impl HttpConnector {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Connector for HttpConnector {
    fn connector_type(&self) -> &str {
        "http"
    }

    fn connector_version(&self) -> &str {
        "1.0.0"
    }

    fn supports_compensation(&self) -> bool {
        false
    }

    async fn prepare(
        &self,
        effect_intent: serde_json::Value,
        effect_id: String,
        fence: u64,
    ) -> Result<PreparedEffect, ConnectorError> {
        let payload = serde_json::json!({
            "base_url": self.base_url,
            "idempotency_key": format!("{}:{}", effect_id, fence),
            "request": effect_intent,
        });
        Ok(PreparedEffect {
            effect_id,
            payload,
            fence,
        })
    }

    async fn commit(
        &self,
        prepared: PreparedEffect,
    ) -> Result<CommitOutcome, ConnectorError> {
        let url = prepared.payload["base_url"]
            .as_str()
            .unwrap_or(&self.base_url);
        let idempotency_key = prepared.payload["idempotency_key"]
            .as_str()
            .unwrap_or("");

        let request_data = &prepared.payload["request"];

        let method = request_data["method"].as_str().unwrap_or("POST");
        let path = request_data["path"].as_str().unwrap_or("/");

        let full_url = format!("{}{}", url, path);

        let req_builder = match method {
            "GET" => self.client.get(&full_url),
            "POST" => self.client.post(&full_url),
            "PUT" => self.client.put(&full_url),
            "DELETE" => self.client.delete(&full_url),
            "PATCH" => self.client.patch(&full_url),
            _ => return Err(ConnectorError::terminal(format!("unsupported HTTP method: {}", method))),
        };

        let response = req_builder
            .header("Idempotency-Key", idempotency_key)
            .json(&request_data.get("body").cloned().unwrap_or(serde_json::Value::Null))
            .send()
            .await
            .map_err(|e| classify_http_error(&e))?;

        match response.status().as_u16() {
            200..=299 => Ok(CommitOutcome::Committed {
                receipt: format!("http:{}:{}", response.status().as_u16(), idempotency_key),
            }),
            409 => Ok(CommitOutcome::Ambiguous),
            429 => Err(ConnectorError::retryable("rate limited")),
            400..=499 => Err(ConnectorError::terminal(format!(
                "client error: {}", response.status()
            ))),
            _ => Err(ConnectorError::retryable(format!(
                "server error: {}", response.status()
            ))),
        }
    }

    async fn reconcile(
        &self,
        effect_id: &str,
    ) -> Result<ReconcileOutcome, ConnectorError> {
        // For HTTP connectors with idempotency keys, we can't reliably
        // determine if a request was processed. Return StillAmbiguous
        // to trigger retry with the same idempotency key.
        let _ = effect_id;
        Ok(ReconcileOutcome::StillAmbiguous)
    }
}

fn classify_http_error(err: &reqwest::Error) -> ConnectorError {
    if err.is_timeout() || err.is_connect() {
        ConnectorError::retryable(err.to_string())
    } else if err.is_request() {
        ConnectorError::terminal(err.to_string())
    } else {
        ConnectorError::retryable(err.to_string())
    }
}
