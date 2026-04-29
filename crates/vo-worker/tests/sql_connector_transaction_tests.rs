#[cfg(test)]
mod sql_connector_transaction_tests {
    use super::*;

    #[tokio::test]
    async fn sql_connector_handles_dangerous_looking_query_drop_table() {
        let c = SqlConnector::new();
        let pe = c
            .prepare(
                serde_json::json!({"query": "'; DROP TABLE users; --"}),
                "tx-sqli-drop".into(),
                1,
            )
            .await
            .expect("prepare should succeed");
        let outcome = c.commit(pe).await.expect("commit should succeed");
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        let reconcile = c
            .reconcile("tx-sqli-drop")
            .await
            .expect("reconcile should succeed");
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn sql_connector_handles_dangerous_looking_query_or_1_equals_1() {
        let c = SqlConnector::new();
        let pe = c
            .prepare(
                serde_json::json!({"query": "\" OR \"1\"=\"1"}),
                "tx-sqli-or".into(),
                1,
            )
            .await
            .expect("prepare should succeed");
        let outcome = c.commit(pe).await.expect("commit should succeed");
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        let reconcile = c
            .reconcile("tx-sqli-or")
            .await
            .expect("reconcile should succeed");
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn sql_connector_handles_dangerous_looking_query_select_secrets() {
        let c = SqlConnector::new();
        let pe = c
            .prepare(
                serde_json::json!({"query": "1; SELECT * FROM secrets"}),
                "tx-sqli-select".into(),
                1,
            )
            .await
            .expect("prepare should succeed");
        let outcome = c.commit(pe).await.expect("commit should succeed");
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        let reconcile = c
            .reconcile("tx-sqli-select")
            .await
            .expect("reconcile should succeed");
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn sql_connector_handles_dangerous_looking_query_admin_comment() {
        let c = SqlConnector::new();
        let pe = c
            .prepare(
                serde_json::json!({"query": "admin'--"}),
                "tx-sqli-admin".into(),
                1,
            )
            .await
            .expect("prepare should succeed");
        let outcome = c.commit(pe).await.expect("commit should succeed");
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        let reconcile = c
            .reconcile("tx-sqli-admin")
            .await
            .expect("reconcile should succeed");
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn sql_connector_handles_dangerous_looking_query_update_balance() {
        let c = SqlConnector::new();
        let pe = c
            .prepare(
                serde_json::json!({"query": "'; UPDATE accounts SET balance=0; --"}),
                "tx-sqli-update".into(),
                1,
            )
            .await
            .expect("prepare should succeed");
        let outcome = c.commit(pe).await.expect("commit should succeed");
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        let reconcile = c
            .reconcile("tx-sqli-update")
            .await
            .expect("reconcile should succeed");
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn sql_connector_handles_dangerous_looking_query_delete_transactions() {
        let c = SqlConnector::new();
        let pe = c
            .prepare(
                serde_json::json!({"query": "1; DELETE FROM transactions"}),
                "tx-sqli-delete".into(),
                1,
            )
            .await
            .expect("prepare should succeed");
        let outcome = c.commit(pe).await.expect("commit should succeed");
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        let reconcile = c
            .reconcile("tx-sqli-delete")
            .await
            .expect("reconcile should succeed");
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn sql_connector_handles_dangerous_looking_query_union_select() {
        let c = SqlConnector::new();
        let pe = c
            .prepare(
                serde_json::json!({"query": "' UNION SELECT password FROM users--"}),
                "tx-sqli-union".into(),
                1,
            )
            .await
            .expect("prepare should succeed");
        let outcome = c.commit(pe).await.expect("commit should succeed");
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        let reconcile = c
            .reconcile("tx-sqli-union")
            .await
            .expect("reconcile should succeed");
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn sql_connector_handles_dangerous_looking_query_insert_admin() {
        let c = SqlConnector::new();
        let pe = c
            .prepare(
                serde_json::json!({"query": "'; INSERT INTO admin VALUES ('hacker'); --"}),
                "tx-sqli-insert".into(),
                1,
            )
            .await
            .expect("prepare should succeed");
        let outcome = c.commit(pe).await.expect("commit should succeed");
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        let reconcile = c
            .reconcile("tx-sqli-insert")
            .await
            .expect("reconcile should succeed");
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn sql_connector_handles_dangerous_query_then_normal_transaction() {
        let c = SqlConnector::new();

        let pe_inject = c
            .prepare(
                serde_json::json!({"query": "'; DROP TABLE users; --"}),
                "fx-inject".into(),
                1,
            )
            .await
            .expect("prepare should succeed");
        let outcome = c.commit(pe_inject).await.expect("commit should succeed");
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        let pe_normal = c
            .prepare(
                serde_json::json!({"query": "SELECT * FROM users"}),
                "fx-normal".into(),
                2,
            )
            .await
            .expect("prepare should succeed");
        let outcome = c.commit(pe_normal).await.expect("commit should succeed");
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        let reconcile_inject = c.reconcile("fx-inject").await.unwrap();
        let reconcile_normal = c.reconcile("fx-normal").await.unwrap();
        assert!(matches!(
            reconcile_inject,
            ReconcileOutcome::Committed { .. }
        ));
        assert!(matches!(
            reconcile_normal,
            ReconcileOutcome::Committed { .. }
        ));
    }

    #[tokio::test]
    async fn sql_connector_handles_dangerous_query_with_compensation() {
        let c = SqlConnector::new();

        let pe = c
            .prepare(
                serde_json::json!({"query": "INSERT INTO secrets VALUES ('malicious')"}),
                "fx-sqli-comp".into(),
                1,
            )
            .await
            .expect("prepare should succeed");
        let outcome = c.commit(pe).await.expect("commit should succeed");
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        let comp_outcome = c
            .compensate(
                serde_json::json!({"rollback_query": "DELETE FROM secrets WHERE data='malicious'"}),
                "fx-sqli-comp".into(),
                1,
            )
            .await
            .expect("compensate should succeed");
        assert!(matches!(comp_outcome, CommitOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn sql_connector_multiple_dangerous_queries_idempotent() {
        let c = SqlConnector::new();

        for i in 0..3 {
            let pe = c
                .prepare(
                    serde_json::json!({"query": "'; SELECT evil; --"}),
                    format!("fx-multi-{}", i).into(),
                    i as u64 + 1,
                )
                .await
                .expect("prepare should succeed");
            let outcome = c.commit(pe).await.expect("commit should succeed");
            assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        }

        for i in 0..3 {
            let reconcile = c
                .reconcile(&format!("fx-multi-{}", i))
                .await
                .expect("reconcile should succeed");
            assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
        }
    }

    #[tokio::test]
    async fn sql_connector_stores_query_content_without_execution() {
        let c = SqlConnector::new();

        let dangerous_queries = vec![
            ("fx-d1", "'; DROP ALL TABLES; --"),
            ("fx-d2", "1; EXECUTE sp_executesql"),
            ("fx-d3", "' OR '1'='1' OR '"),
        ];

        for (id, query) in &dangerous_queries {
            let pe = c
                .prepare(serde_json::json!({"query": query}), (*id).into(), 1)
                .await
                .expect("prepare should succeed");
            let outcome = c.commit(pe).await.expect("commit should succeed");
            assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        }

        for (id, _) in &dangerous_queries {
            let reconcile = c.reconcile(id).await.expect("reconcile should succeed");
            assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
        }
    }
}
