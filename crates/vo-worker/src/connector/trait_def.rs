//! Core Connector trait (ADR-041 §1).

use crate::connector::{CommitOutcome, ConnectorError, PreparedEffect, ReconcileOutcome};
use async_trait::async_trait;

/// The uniform runtime contract for all managed connectors (ADR-041 §1).
#[async_trait]
pub trait Connector: Send + Sync + 'static {
    fn connector_type(&self) -> &str;
    fn connector_version(&self) -> &str;
    fn supports_compensation(&self) -> bool;

    async fn prepare(
        &self,
        effect_intent: serde_json::Value,
        effect_id: String,
        fence: u64,
    ) -> Result<PreparedEffect, ConnectorError>;

    async fn commit(&self, prepared: PreparedEffect) -> Result<CommitOutcome, ConnectorError>;

    async fn reconcile(&self, effect_id: &str) -> Result<ReconcileOutcome, ConnectorError>;

    async fn compensate(
        &self,
        _compensation_intent: serde_json::Value,
        _compensation_effect_id: String,
        _fence: u64,
    ) -> Result<CommitOutcome, ConnectorError> {
        Err(ConnectorError::compensation_not_supported(
            self.connector_type(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct TestConnector {
        connector_type: String,
        connector_version: String,
        supports_compensation: bool,
    }

    #[async_trait]
    impl Connector for TestConnector {
        fn connector_type(&self) -> &str {
            &self.connector_type
        }

        fn connector_version(&self) -> &str {
            &self.connector_version
        }

        fn supports_compensation(&self) -> bool {
            self.supports_compensation
        }

        async fn prepare(
            &self,
            _effect_intent: serde_json::Value,
            effect_id: String,
            fence: u64,
        ) -> Result<PreparedEffect, ConnectorError> {
            Ok(PreparedEffect {
                effect_id,
                payload: json!({"connector": &self.connector_type}),
                fence,
            })
        }

        async fn commit(&self, _prepared: PreparedEffect) -> Result<CommitOutcome, ConnectorError> {
            Ok(CommitOutcome::Committed {
                receipt: format!("{}:committed", self.connector_type),
            })
        }

        async fn reconcile(&self, _effect_id: &str) -> Result<ReconcileOutcome, ConnectorError> {
            Ok(ReconcileOutcome::NotCommitted)
        }
    }

    #[tokio::test]
    async fn test_connector_trait_bounds() {
        fn assert_send_sync<T: Send + Sync + 'static + ?Sized>() {}
        assert_send_sync::<dyn Connector>();
    }

    #[tokio::test]
    async fn test_connector_trait_methods() {
        let connector = TestConnector {
            connector_type: "test".to_string(),
            connector_version: "1.0.0".to_string(),
            supports_compensation: true,
        };

        assert_eq!(connector.connector_type(), "test");
        assert_eq!(connector.connector_version(), "1.0.0");
        assert!(connector.supports_compensation());
    }

    #[tokio::test]
    async fn test_connector_trait_prepare() {
        let connector = TestConnector {
            connector_type: "test".to_string(),
            connector_version: "1.0.0".to_string(),
            supports_compensation: false,
        };

        let result = connector
            .prepare(json!({}), "fx-123".to_string(), 42)
            .await
            .unwrap();
        assert_eq!(result.effect_id, "fx-123");
        assert_eq!(result.fence, 42);
    }

    #[tokio::test]
    async fn test_connector_trait_commit() {
        let connector = TestConnector {
            connector_type: "test".to_string(),
            connector_version: "1.0.0".to_string(),
            supports_compensation: true,
        };

        let prepared = PreparedEffect {
            effect_id: "fx-123".to_string(),
            payload: json!({}),
            fence: 1,
        };

        let result = connector.commit(prepared).await.unwrap();
        assert!(matches!(result, CommitOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn test_connector_trait_reconcile() {
        let connector = TestConnector {
            connector_type: "test".to_string(),
            connector_version: "1.0.0".to_string(),
            supports_compensation: false,
        };

        let result = connector.reconcile("effect-123").await.unwrap();
        assert!(matches!(result, ReconcileOutcome::NotCommitted));
    }

    #[tokio::test]
    async fn test_connector_trait_compensate_default() {
        let connector = TestConnector {
            connector_type: "test".to_string(),
            connector_version: "1.0.0".to_string(),
            supports_compensation: false,
        };

        let result = connector
            .compensate(json!({}), "cx-123".to_string(), 1)
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_retryable());
        assert!(err.to_string().contains("test"));
        assert!(err.to_string().contains("compensation"));
    }

    #[tokio::test]
    async fn test_connector_trait_compensate_overridden() {
        struct CompensatingConnector;

        #[async_trait]
        impl Connector for CompensatingConnector {
            fn connector_type(&self) -> &str {
                "comp"
            }
            fn connector_version(&self) -> &str {
                "1.0.0"
            }
            fn supports_compensation(&self) -> bool {
                true
            }

            async fn prepare(
                &self,
                _effect_intent: serde_json::Value,
                effect_id: String,
                fence: u64,
            ) -> Result<PreparedEffect, ConnectorError> {
                Ok(PreparedEffect {
                    effect_id,
                    payload: json!({}),
                    fence,
                })
            }

            async fn commit(
                &self,
                _prepared: PreparedEffect,
            ) -> Result<CommitOutcome, ConnectorError> {
                Ok(CommitOutcome::Committed {
                    receipt: "c".into(),
                })
            }

            async fn reconcile(
                &self,
                _effect_id: &str,
            ) -> Result<ReconcileOutcome, ConnectorError> {
                Ok(ReconcileOutcome::NotCommitted)
            }

            async fn compensate(
                &self,
                _compensation_intent: serde_json::Value,
                _compensation_effect_id: String,
                _fence: u64,
            ) -> Result<CommitOutcome, ConnectorError> {
                Ok(CommitOutcome::Committed {
                    receipt: "compensated".into(),
                })
            }
        }

        let connector = CompensatingConnector;
        let result = connector
            .compensate(json!({}), "cx-123".to_string(), 1)
            .await
            .unwrap();
        assert!(matches!(result, CommitOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn test_connector_trait_type_identity() {
        let connector = TestConnector {
            connector_type: "my-connector".to_string(),
            connector_version: "2.0.0".to_string(),
            supports_compensation: true,
        };

        assert_eq!(connector.connector_type(), "my-connector");
        assert_eq!(connector.connector_version(), "2.0.0");
    }

    #[test]
    fn test_connector_trait_empty_strings() {
        let connector = TestConnector {
            connector_type: "".to_string(),
            connector_version: "".to_string(),
            supports_compensation: false,
        };

        assert_eq!(connector.connector_type(), "");
        assert_eq!(connector.connector_version(), "");
        assert!(!connector.supports_compensation());
    }
}
