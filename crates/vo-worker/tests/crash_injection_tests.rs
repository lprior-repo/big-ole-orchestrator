#[cfg(test)]
mod crash_injection_tests {
    use super::*;

    #[tokio::test]
    async fn crash_on_commit_returns_retryable_error() {
        let c = CrashOnCommitConnector::new(0);
        let pe = c
            .prepare(serde_json::json!({}), "fx-crash".into(), 1)
            .await
            .unwrap();
        let result = c.commit(pe).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().is_retryable());
    }

    #[tokio::test]
    async fn crash_after_n_commits_then_crash() {
        let c = CrashOnCommitConnector::new(2);
        for i in 0..2 {
            let pe = c
                .prepare(serde_json::json!({}), format!("fx-ok-{}", i), i as u64 + 1)
                .await
                .unwrap();
            let outcome = c.commit(pe).await.unwrap();
            assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        }

        let pe = c
            .prepare(serde_json::json!({}), "fx-crash-3".into(), 3)
            .await
            .unwrap();
        let result = c.commit(pe).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn crash_then_reconcile_finds_committed() {
        let c = CrashOnCommitConnector::new(1);
        let pe = c
            .prepare(serde_json::json!({}), "fx-commit-then-crash".into(), 1)
            .await
            .unwrap();
        let _ = c.commit(pe).await.unwrap();

        let pe2 = c
            .prepare(serde_json::json!({}), "fx-will-crash".into(), 2)
            .await
            .unwrap();
        let result = c.commit(pe2).await;
        assert!(result.is_err());

        let reconcile = c.reconcile("fx-commit-then-crash").await.unwrap();
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));

        let reconcile2 = c.reconcile("fx-will-crash").await.unwrap();
        assert_eq!(reconcile2, ReconcileOutcome::NotCommitted);
    }

    #[tokio::test]
    async fn sql_crash_injection_recovery() {
        let c = SqlConnector::new();
        c.crash_on_commit.store(true, Ordering::SeqCst);

        let pe = c
            .prepare(
                serde_json::json!({"query": "INSERT"}),
                "tx-inject".into(),
                1,
            )
            .await
            .unwrap();
        let result = c.commit(pe).await;
        assert!(result.is_err());

        c.crash_on_commit.store(false, Ordering::SeqCst);

        let pe2 = c
            .prepare(
                serde_json::json!({"query": "INSERT"}),
                "tx-recover".into(),
                2,
            )
            .await
            .unwrap();
        let outcome = c.commit(pe2).await.unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        let reconcile = c.reconcile("tx-recover").await.unwrap();
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn sql_compensate_after_crash_reverses() {
        let c = SqlConnector::new();
        let pe = c
            .prepare(
                serde_json::json!({"query": "INSERT"}),
                "tx-comp-crash".into(),
                1,
            )
            .await
            .unwrap();
        let _ = c.commit(pe).await.unwrap();

        let outcome = c
            .compensate(
                serde_json::json!({"rollback_query": "DELETE FROM orders WHERE id = 1"}),
                "tx-comp-crash".into(),
                1,
            )
            .await
            .unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        let reconcile = c.reconcile("tx-comp-crash").await.unwrap();
        assert_eq!(reconcile, ReconcileOutcome::NotCommitted);
    }

    #[tokio::test]
    async fn multiple_sequential_crashes_all_retryable() {
        struct AlwaysCrashConnector;

        #[async_trait]
        impl Connector for AlwaysCrashConnector {
            fn connector_type(&self) -> &str {
                "always-crash"
            }
            fn connector_version(&self) -> &str {
                "1.0.0"
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
                    payload: serde_json::json!({}),
                    fence,
                })
            }

            async fn commit(
                &self,
                _prepared: PreparedEffect,
            ) -> Result<CommitOutcome, ConnectorError> {
                Err(ConnectorError::retryable("connection lost"))
            }

            async fn reconcile(
                &self,
                _effect_id: &str,
            ) -> Result<ReconcileOutcome, ConnectorError> {
                Ok(ReconcileOutcome::NotCommitted)
            }
        }

        let c = AlwaysCrashConnector;
        for i in 0..5 {
            let pe = c
                .prepare(
                    serde_json::json!({}),
                    format!("fx-crash-{}", i),
                    i as u64 + 1,
                )
                .await
                .unwrap();
            let result = c.commit(pe).await;
            assert!(result.is_err());
            assert!(result.unwrap_err().is_retryable());
        }
    }
}
