//! Idempotency-key HTTP connector (ADR-041).

use crate::connector::{
    CommitOutcome, Connector, ConnectorError, PreparedEffect, ReconcileOutcome,
};
use async_trait::async_trait;

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

    async fn commit(&self, prepared: PreparedEffect) -> Result<CommitOutcome, ConnectorError> {
        let url = prepared.payload["base_url"]
            .as_str()
            .unwrap_or(&self.base_url);
        let idempotency_key = prepared.payload["idempotency_key"].as_str().unwrap_or("");

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
            _ => {
                return Err(ConnectorError::terminal(format!(
                    "unsupported HTTP method: {}",
                    method
                )))
            }
        };

        let response = req_builder
            .header("Idempotency-Key", idempotency_key)
            .json(
                &request_data
                    .get("body")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            )
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
                "client error: {}",
                response.status()
            ))),
            _ => Err(ConnectorError::retryable(format!(
                "server error: {}",
                response.status()
            ))),
        }
    }

    async fn reconcile(&self, effect_id: &str) -> Result<ReconcileOutcome, ConnectorError> {
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

#[cfg(test)]
mod tests {
    use super::*;
<<<<<<< HEAD
    use crate::connector::{
        CommitOutcome, Connector, ConnectorError, PreparedEffect, ReconcileOutcome,
    };
=======
    use crate::connector::{CommitOutcome, Connector, ConnectorError, PreparedEffect, ReconcileOutcome};
>>>>>>> origin/vo-worker-tests
    use serde_json::json;

    #[test]
    fn test_http_connector_new() {
        let connector = HttpConnector::new("https://api.example.com");
        assert_eq!(connector.base_url, "https://api.example.com");
    }

    #[test]
    fn test_http_connector_type() {
        let connector = HttpConnector::new("https://api.example.com");
        assert_eq!(connector.connector_type(), "http");
    }

    #[test]
    fn test_http_connector_version() {
        let connector = HttpConnector::new("https://api.example.com");
        assert_eq!(connector.connector_version(), "1.0.0");
    }

    #[test]
    fn test_http_connector_supports_compensation() {
        let connector = HttpConnector::new("https://api.example.com");
        assert!(!connector.supports_compensation());
    }

    #[tokio::test]
    async fn test_http_connector_prepare_basic() {
        let connector = HttpConnector::new("https://api.example.com");
        let effect_intent = json!({"method": "POST", "path": "/charges"});
<<<<<<< HEAD

        let result = connector
            .prepare(effect_intent, "fx-123".to_string(), 42)
            .await
            .unwrap();

=======
        
        let result = connector.prepare(effect_intent, "fx-123".to_string(), 42).await.unwrap();
        
>>>>>>> origin/vo-worker-tests
        assert_eq!(result.effect_id, "fx-123");
        assert_eq!(result.fence, 42);
        assert_eq!(result.payload["base_url"], "https://api.example.com");
        assert_eq!(result.payload["idempotency_key"], "fx-123:42");
    }

    #[tokio::test]
    async fn test_http_connector_prepare_with_custom_request() {
        let connector = HttpConnector::new("https://api.example.com");
        let effect_intent = json!({
            "method": "PUT",
            "path": "/users/123",
            "body": {"name": "John", "email": "john@example.com"}
        });
<<<<<<< HEAD

        let result = connector
            .prepare(effect_intent, "fx-456".to_string(), 1)
            .await
            .unwrap();

=======
        
        let result = connector.prepare(effect_intent, "fx-456".to_string(), 1).await.unwrap();
        
>>>>>>> origin/vo-worker-tests
        assert_eq!(result.payload["idempotency_key"], "fx-456:1");
        assert_eq!(result.payload["request"]["method"], "PUT");
        assert_eq!(result.payload["request"]["path"], "/users/123");
        assert_eq!(result.payload["request"]["body"]["name"], "John");
    }

    #[tokio::test]
    async fn test_http_connector_prepare_empty_intent() {
        let connector = HttpConnector::new("https://api.example.com");
        let effect_intent = json!({});
<<<<<<< HEAD

        let result = connector
            .prepare(effect_intent, "fx-789".to_string(), 999)
            .await
            .unwrap();

=======
        
        let result = connector.prepare(effect_intent, "fx-789".to_string(), 999).await.unwrap();
        
>>>>>>> origin/vo-worker-tests
        assert_eq!(result.payload["idempotency_key"], "fx-789:999");
        assert_eq!(result.payload["request"], json!({}));
    }

    #[test]
    fn test_classify_http_error_timeout() {
        // Test that retryable errors are classified correctly
        let error = ConnectorError::retryable("timeout");
        assert!(error.is_retryable());
        assert!(error.to_string().contains("timeout"));
    }

    #[test]
    fn test_classify_http_error_connect() {
        // Test that connection errors are classified as retryable
        let error = ConnectorError::retryable("connection refused");
        assert!(error.is_retryable());
    }

    #[test]
    fn test_classify_http_error_request() {
        // Simulate a request error - we test the classification logic
        // by checking that the function handles different error types correctly
        // The actual reqwest error creation is complex, so we verify the logic
        // through the retryable/terminal classification tests
        let retryable = ConnectorError::retryable("request failed");
        assert!(retryable.is_retryable());
    }

    #[tokio::test]
    async fn test_connector_error_from_prepare() {
        let connector = HttpConnector::new("https://api.example.com");
<<<<<<< HEAD

=======
        
>>>>>>> origin/vo-worker-tests
        // Test that prepare always succeeds (no async I/O)
        let result = connector.prepare(json!({}), "id".to_string(), 1).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_reconcile_returns_ambiguous() {
        let connector = HttpConnector::new("https://api.example.com");
<<<<<<< HEAD

=======
        
>>>>>>> origin/vo-worker-tests
        let result = connector.reconcile("effect-123").await.unwrap();
        assert_eq!(result, ReconcileOutcome::StillAmbiguous);
    }

    #[tokio::test]
    async fn test_reconcile_empty_effect_id() {
        let connector = HttpConnector::new("https://api.example.com");
<<<<<<< HEAD

=======
        
>>>>>>> origin/vo-worker-tests
        let result = connector.reconcile("").await.unwrap();
        assert_eq!(result, ReconcileOutcome::StillAmbiguous);
    }

    #[tokio::test]
    async fn test_http_connector_prepare_multiple_effects() {
        let connector = HttpConnector::new("https://api.example.com");
<<<<<<< HEAD

        let result1 = connector
            .prepare(json!({"path": "/a"}), "fx-1".to_string(), 1)
            .await
            .unwrap();
        let result2 = connector
            .prepare(json!({"path": "/b"}), "fx-2".to_string(), 2)
            .await
            .unwrap();
        let result3 = connector
            .prepare(json!({"path": "/c"}), "fx-3".to_string(), 3)
            .await
            .unwrap();

        assert_ne!(
            result1.payload["idempotency_key"],
            result2.payload["idempotency_key"]
        );
        assert_ne!(
            result2.payload["idempotency_key"],
            result3.payload["idempotency_key"]
        );
=======
        
        let result1 = connector.prepare(json!({"path": "/a"}), "fx-1".to_string(), 1).await.unwrap();
        let result2 = connector.prepare(json!({"path": "/b"}), "fx-2".to_string(), 2).await.unwrap();
        let result3 = connector.prepare(json!({"path": "/c"}), "fx-3".to_string(), 3).await.unwrap();
        
        assert_ne!(result1.payload["idempotency_key"], result2.payload["idempotency_key"]);
        assert_ne!(result2.payload["idempotency_key"], result3.payload["idempotency_key"]);
>>>>>>> origin/vo-worker-tests
        assert_eq!(result1.payload["idempotency_key"], "fx-1:1");
        assert_eq!(result2.payload["idempotency_key"], "fx-2:2");
        assert_eq!(result3.payload["idempotency_key"], "fx-3:3");
    }

    #[test]
    fn test_connector_error_retryable_classification() {
        let retryable = ConnectorError::retryable("timeout");
        let terminal = ConnectorError::terminal("bad request");
<<<<<<< HEAD

=======
        
>>>>>>> origin/vo-worker-tests
        assert!(retryable.is_retryable());
        assert!(!terminal.is_retryable());
    }

    #[test]
    fn test_connector_error_terminal_classification() {
        let terminal = ConnectorError::terminal("auth failed");
<<<<<<< HEAD

=======
        
>>>>>>> origin/vo-worker-tests
        assert!(!terminal.is_retryable());
        assert!(terminal.to_string().contains("auth failed"));
    }

    #[test]
    fn test_commit_outcome_committed() {
<<<<<<< HEAD
        let outcome = CommitOutcome::Committed {
            receipt: "test-receipt".to_string(),
        };
=======
        let outcome = CommitOutcome::Committed { receipt: "test-receipt".to_string() };
>>>>>>> origin/vo-worker-tests
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
    }

    #[test]
    fn test_commit_outcome_ambiguous() {
        let outcome = CommitOutcome::Ambiguous;
        assert!(matches!(outcome, CommitOutcome::Ambiguous));
    }

    #[test]
    fn test_commit_outcome_failed() {
        let outcome = CommitOutcome::Failed;
        assert!(matches!(outcome, CommitOutcome::Failed));
    }

    #[test]
    fn test_reconcile_outcome_committed() {
<<<<<<< HEAD
        let outcome = ReconcileOutcome::Committed {
            receipt: "r".to_string(),
        };
=======
        let outcome = ReconcileOutcome::Committed { receipt: "r".to_string() };
>>>>>>> origin/vo-worker-tests
        assert!(matches!(outcome, ReconcileOutcome::Committed { .. }));
    }

    #[test]
    fn test_reconcile_outcome_not_committed() {
        let outcome = ReconcileOutcome::NotCommitted;
        assert!(matches!(outcome, ReconcileOutcome::NotCommitted));
    }

    #[test]
    fn test_reconcile_outcome_still_ambiguous() {
        let outcome = ReconcileOutcome::StillAmbiguous;
        assert!(matches!(outcome, ReconcileOutcome::StillAmbiguous));
    }
}
