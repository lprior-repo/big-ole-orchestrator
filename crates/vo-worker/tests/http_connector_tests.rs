#[cfg(test)]
mod http_connector_tests {
    use super::*;

    #[tokio::test]
    async fn http_prepare_builds_idempotency_key() {
        let c = HttpConnector::new("https://api.example.com");
        let pe = c
            .prepare(
                serde_json::json!({"method": "POST", "path": "/charges"}),
                "fx-http-1".into(),
                7,
            )
            .await
            .unwrap();
        assert_eq!(pe.payload["idempotency_key"], "fx-http-1:7");
        assert_eq!(pe.payload["base_url"], "https://api.example.com");
        assert_eq!(pe.payload["request"]["method"], "POST");
        assert_eq!(pe.payload["request"]["path"], "/charges");
    }

    #[tokio::test]
    async fn http_prepare_different_effects_different_keys() {
        let c = HttpConnector::new("https://api.example.com");
        let pe1 = c
            .prepare(serde_json::json!({}), "fx-a".into(), 1)
            .await
            .unwrap();
        let pe2 = c
            .prepare(serde_json::json!({}), "fx-b".into(), 1)
            .await
            .unwrap();
        assert_ne!(
            pe1.payload["idempotency_key"],
            pe2.payload["idempotency_key"]
        );
    }

    #[tokio::test]
    async fn http_prepare_same_effect_same_fence_same_key() {
        let c = HttpConnector::new("https://api.example.com");
        let pe1 = c
            .prepare(serde_json::json!({}), "fx-same".into(), 5)
            .await
            .unwrap();
        let pe2 = c
            .prepare(serde_json::json!({}), "fx-same".into(), 5)
            .await
            .unwrap();
        assert_eq!(
            pe1.payload["idempotency_key"],
            pe2.payload["idempotency_key"]
        );
    }

    #[tokio::test]
    async fn http_prepare_same_effect_different_fence_different_key() {
        let c = HttpConnector::new("https://api.example.com");
        let pe1 = c
            .prepare(serde_json::json!({}), "fx-diff".into(), 1)
            .await
            .unwrap();
        let pe2 = c
            .prepare(serde_json::json!({}), "fx-diff".into(), 2)
            .await
            .unwrap();
        assert_ne!(
            pe1.payload["idempotency_key"],
            pe2.payload["idempotency_key"]
        );
    }

    #[tokio::test]
    async fn http_reconcile_always_returns_still_ambiguous() {
        let c = HttpConnector::new("https://api.example.com");
        let outcome = c.reconcile("fx-any").await.unwrap();
        assert_eq!(outcome, ReconcileOutcome::StillAmbiguous);
    }

    #[tokio::test]
    async fn http_compensate_returns_not_supported() {
        let c = HttpConnector::new("https://api.example.com");
        let result = c.compensate(serde_json::json!({}), "cx-1".into(), 1).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn http_prepare_payload_preserves_body() {
        let c = HttpConnector::new("https://api.example.com");
        let body = serde_json::json!({"amount": 500, "currency": "EUR"});
        let pe = c
            .prepare(
                serde_json::json!({
                    "method": "POST",
                    "path": "/payments",
                    "body": body,
                }),
                "fx-body".into(),
                1,
            )
            .await
            .unwrap();
        assert_eq!(pe.payload["request"]["body"]["amount"], 500);
    }
}
