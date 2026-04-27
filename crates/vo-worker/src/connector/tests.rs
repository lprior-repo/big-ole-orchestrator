//! Connector tests.

use super::{
    CommitOutcome, Connector, ConnectorError, ConnectorRegistry, HttpConnector, PreparedEffect,
    ReconcileOutcome,
};
use async_trait::async_trait;
use serde_json::json;
use vo_types::ReconcileAction;

struct NoopConnector;

#[async_trait]
impl Connector for NoopConnector {
    fn connector_type(&self) -> &str {
        "noop"
    }
    fn connector_version(&self) -> &str {
        "0.1.0"
    }
    fn supports_compensation(&self) -> bool {
        false
    }
    async fn prepare(
        &self,
        _intent: serde_json::Value,
        effect_id: String,
        fence: u64,
    ) -> Result<PreparedEffect, ConnectorError> {
        Ok(PreparedEffect {
            effect_id,
            payload: json!({}),
            fence,
        })
    }
    async fn commit(&self, _prepared: PreparedEffect) -> Result<CommitOutcome, ConnectorError> {
        Ok(CommitOutcome::Committed {
            receipt: "noop".into(),
        })
    }
    async fn reconcile(&self, _effect_id: &str) -> Result<ReconcileOutcome, ConnectorError> {
        Ok(ReconcileOutcome::NotCommitted)
    }
}

#[test]
fn connector_error_retryable_is_retryable() {
    let err = ConnectorError::retryable("timeout");
    assert!(err.is_retryable());
}

#[test]
fn connector_error_terminal_is_not_retryable() {
    let err = ConnectorError::terminal("auth failed");
    assert!(!err.is_retryable());
}

#[test]
fn connector_error_compensation_not_supported() {
    let err = ConnectorError::compensation_not_supported("http");
    assert!(err.is_retryable());
    assert!(err.to_string().contains("http"));
}

#[test]
fn prepared_effect_carries_fields() {
    let pe = PreparedEffect {
        effect_id: "fx-1".to_string(),
        payload: json!({"method": "POST"}),
        fence: 42,
    };
    assert_eq!(pe.effect_id, "fx-1");
    assert_eq!(pe.fence, 42);
}

#[test]
fn prepared_effect_serde_round_trip() {
    let pe = PreparedEffect {
        effect_id: "fx-2".to_string(),
        payload: json!({"key": "val"}),
        fence: 7,
    };
    let s = serde_json::to_string(&pe).unwrap();
    let recovered: PreparedEffect = serde_json::from_str(&s).unwrap();
    assert_eq!(recovered.effect_id, pe.effect_id);
    assert_eq!(recovered.fence, pe.fence);
}

#[test]
fn commit_outcome_variants() {
    let _ = CommitOutcome::Committed {
        receipt: "r".into(),
    };
    let _ = CommitOutcome::Failed;
    let _ = CommitOutcome::Ambiguous;
}

#[test]
fn reconcile_outcome_maps_to_reconcile_action() {
    assert_eq!(
        ReconcileAction::from(ReconcileOutcome::Committed {
            receipt: "r".into()
        }),
        ReconcileAction::Commit,
    );
    assert_eq!(
        ReconcileAction::from(ReconcileOutcome::NotCommitted),
        ReconcileAction::Rollback,
    );
    assert_eq!(
        ReconcileAction::from(ReconcileOutcome::StillAmbiguous),
        ReconcileAction::Retry,
    );
}

#[tokio::test]
async fn default_compensate_returns_not_supported() {
    let c = NoopConnector;
    let result = c.compensate(json!({}), "cx-1".into(), 1).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("noop"));
}

#[tokio::test]
async fn noop_connector_prepare_commit_cycle() {
    let c = NoopConnector;
    let pe = c
        .prepare(json!({"url": "https://example.com"}), "fx-1".into(), 1)
        .await
        .unwrap();
    assert_eq!(pe.effect_id, "fx-1");
    let outcome = c.commit(pe).await.unwrap();
    assert_eq!(
        outcome,
        CommitOutcome::Committed {
            receipt: "noop".into()
        }
    );
}

#[tokio::test]
async fn registry_register_and_get() {
    let mut reg = ConnectorRegistry::new();
    assert!(reg.get("noop").is_none());
    reg.register("noop".to_string(), Box::new(NoopConnector));
    assert!(reg.get("noop").is_some());
}

#[tokio::test]
async fn registry_list() {
    let mut reg = ConnectorRegistry::new();
    assert!(reg.list().is_empty());
    reg.register("noop".to_string(), Box::new(NoopConnector));
    assert_eq!(reg.list(), vec!["noop"]);
}

#[tokio::test]
async fn http_connector_type_and_version() {
    let c = HttpConnector::new("https://api.example.com");
    assert_eq!(c.connector_type(), "http");
    assert_eq!(c.connector_version(), "1.0.0");
    assert!(!c.supports_compensation());
}

#[tokio::test]
async fn http_connector_prepare_includes_idempotency_key() {
    let c = HttpConnector::new("https://api.example.com");
    let pe = c
        .prepare(
            json!({"method": "POST", "path": "/charges"}),
            "fx-42".into(),
            7,
        )
        .await
        .unwrap();
    assert_eq!(pe.effect_id, "fx-42");
    assert_eq!(pe.fence, 7);
    assert_eq!(pe.payload["idempotency_key"], "fx-42:7");
    assert_eq!(pe.payload["base_url"], "https://api.example.com");
}

#[test]
fn connector_error_kind_classification() {
    let retryable = ConnectorError::retryable("timeout");
    assert!(retryable.is_retryable());
    let terminal = ConnectorError::terminal("bad request");
    assert!(!terminal.is_retryable());
}
