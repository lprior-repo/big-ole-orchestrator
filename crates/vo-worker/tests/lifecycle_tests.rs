#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    #[tokio::test]
    async fn happy_path_prepare_commit_reconcile() {
        let c = AlwaysCommittedConnector;
        let pe = c
            .prepare(serde_json::json!({"key": "val"}), "fx-1".into(), 1)
            .await
            .unwrap();
        assert_eq!(pe.effect_id, "fx-1");
        assert_eq!(pe.fence, 1);

        let outcome = c.commit(pe).await.unwrap();
        assert!(
            matches!(outcome, CommitOutcome::Committed { receipt } if receipt == "receipt:fx-1")
        );

        let reconcile = c.reconcile("fx-1").await.unwrap();
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn prepare_preserves_intent_in_payload() {
        let c = AlwaysCommittedConnector;
        let intent = serde_json::json!({
            "action": "charge",
            "amount": 100,
            "currency": "USD"
        });
        let pe = c.prepare(intent.clone(), "fx-2".into(), 5).await.unwrap();
        assert_eq!(pe.payload["status"], "prepared");
    }

    #[tokio::test]
    async fn commit_after_prepare_uses_effect_id() {
        let c = AlwaysCommittedConnector;
        let pe = c
            .prepare(serde_json::json!({}), "fx-unique".into(), 42)
            .await
            .unwrap();
        let outcome = c.commit(pe).await.unwrap();
        if let CommitOutcome::Committed { receipt } = outcome {
            assert!(receipt.contains("fx-unique"));
        } else {
            panic!("expected committed");
        }
    }

    #[tokio::test]
    async fn failed_commit_returns_failed_outcome() {
        let c = AlwaysFailedConnector;
        let pe = c
            .prepare(serde_json::json!({}), "fx-fail".into(), 1)
            .await
            .unwrap();
        let outcome = c.commit(pe).await.unwrap();
        assert_eq!(outcome, CommitOutcome::Failed);
    }

    #[tokio::test]
    async fn reconcile_after_failure_returns_not_committed() {
        let c = AlwaysFailedConnector;
        let pe = c
            .prepare(serde_json::json!({}), "fx-fail".into(), 1)
            .await
            .unwrap();
        let _ = c.commit(pe).await.unwrap();
        let reconcile = c.reconcile("fx-fail").await.unwrap();
        assert_eq!(reconcile, ReconcileOutcome::NotCommitted);
    }

    #[tokio::test]
    async fn sql_connector_full_lifecycle() {
        let c = SqlConnector::new();
        let pe = c
            .prepare(
                serde_json::json!({"query": "INSERT INTO orders (id) VALUES (1)"}),
                "tx-sql-1".into(),
                1,
            )
            .await
            .unwrap();
        assert_eq!(pe.payload["query"], "INSERT INTO orders (id) VALUES (1)");
        assert_eq!(pe.payload["transaction"], "tx-sql-1");

        let outcome = c.commit(pe).await.unwrap();
        assert!(
            matches!(outcome, CommitOutcome::Committed { receipt } if receipt == "txn:tx-sql-1")
        );

        let reconcile = c.reconcile("tx-sql-1").await.unwrap();
        assert!(
            matches!(reconcile, ReconcileOutcome::Committed { receipt } if receipt.contains("tx-sql-1"))
        );
    }

    #[tokio::test]
    async fn sql_connector_compensate_reverses_commit() {
        let c = SqlConnector::new();
        let pe = c
            .prepare(serde_json::json!({"query": "INSERT"}), "tx-comp".into(), 1)
            .await
            .unwrap();
        let _ = c.commit(pe).await.unwrap();

        let outcome = c
            .compensate(
                serde_json::json!({"rollback_query": "DELETE"}),
                "tx-comp".into(),
                1,
            )
            .await
            .unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        let reconcile = c.reconcile("tx-comp").await.unwrap();
        assert_eq!(reconcile, ReconcileOutcome::NotCommitted);
    }

    #[tokio::test]
    async fn compensate_on_non_supporting_connector_returns_error() {
        let c = HttpConnector::new("https://api.example.com");
        let result = c.compensate(serde_json::json!({}), "cx-1".into(), 1).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("http"));
    }

    #[tokio::test]
    async fn compensating_connector_compensate_succeeds() {
        let c = CompensatingConnector::new();
        let outcome = c
            .compensate(serde_json::json!({}), "cx-2".into(), 1)
            .await
            .unwrap();
        assert!(
            matches!(outcome, CommitOutcome::Committed { receipt } if receipt == "compensated")
        );
        assert!(c.compensated.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn fence_increments_produce_different_prepared_effects() {
        let c = AlwaysCommittedConnector;
        let pe1 = c
            .prepare(serde_json::json!({}), "fx-1".into(), 1)
            .await
            .unwrap();
        let pe2 = c
            .prepare(serde_json::json!({}), "fx-1".into(), 2)
            .await
            .unwrap();
        assert_eq!(pe1.effect_id, pe2.effect_id);
        assert_ne!(pe1.fence, pe2.fence);
    }
}
