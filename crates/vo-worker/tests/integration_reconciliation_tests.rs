#[cfg(test)]
mod integration_reconciliation_tests {
    use super::*;

    #[tokio::test]
    async fn sql_connector_full_reconciliation_after_crash() {
        let c = SqlConnector::new();

        let pe = c
            .prepare(
                serde_json::json!({"query": "INSERT INTO t VALUES (1)"}),
                "tx-full".into(),
                1,
            )
            .await
            .unwrap();
        let outcome = c.commit(pe).await.unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        c.crash_on_commit.store(true, Ordering::SeqCst);
        let pe2 = c
            .prepare(
                serde_json::json!({"query": "INSERT INTO t VALUES (2)"}),
                "tx-crash".into(),
                2,
            )
            .await
            .unwrap();
        let result = c.commit(pe2).await;
        assert!(result.is_err());

        let r1 = c.reconcile("tx-full").await.unwrap();
        assert!(matches!(r1, ReconcileOutcome::Committed { .. }));

        let r2 = c.reconcile("tx-crash").await.unwrap();
        assert_eq!(r2, ReconcileOutcome::NotCommitted);
    }

    #[tokio::test]
    async fn registry_based_connector_dispatch() {
        let mut reg = ConnectorRegistry::new();
        reg.register(
            "http".to_string(),
            Box::new(HttpConnector::new("https://api.example.com")),
        );
        reg.register("sql".to_string(), Box::new(SqlConnector::new()));

        let http = reg.get("http").unwrap();
        let pe = http
            .prepare(
                serde_json::json!({"method": "GET", "path": "/health"}),
                "fx-dispatch-http".into(),
                1,
            )
            .await
            .unwrap();
        assert_eq!(pe.payload["idempotency_key"], "fx-dispatch-http:1");

        let sql = reg.get("sql").unwrap();
        let pe = sql
            .prepare(
                serde_json::json!({"query": "SELECT 1"}),
                "fx-dispatch-sql".into(),
                1,
            )
            .await
            .unwrap();
        let outcome = sql.commit(pe).await.unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        let reconcile = sql.reconcile("fx-dispatch-sql").await.unwrap();
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn compensating_connector_full_lifecycle() {
        let c = CompensatingConnector::new();

        let pe = c
            .prepare(
                serde_json::json!({"action": "reserve"}),
                "fx-comp-full".into(),
                1,
            )
            .await
            .unwrap();
        let outcome = c.commit(pe).await.unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        let comp_outcome = c
            .compensate(
                serde_json::json!({"action": "release"}),
                "fx-comp-full".into(),
                1,
            )
            .await
            .unwrap();
        assert!(matches!(comp_outcome, CommitOutcome::Committed { .. }));
        assert!(c.compensated.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn timeout_then_reconcile_committed_flow() {
        let c = TimeoutOnCommitConnector::new(0);

        let pe = c
            .prepare(serde_json::json!({}), "fx-t-zero".into(), 1)
            .await
            .unwrap();
        let outcome = c.commit(pe).await.unwrap();
        assert_eq!(outcome, CommitOutcome::Ambiguous);

        let reconcile = c.reconcile("fx-t-zero").await.unwrap();
        assert_eq!(reconcile, ReconcileOutcome::NotCommitted);
    }
}
