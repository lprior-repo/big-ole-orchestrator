//! Ingress admission handler — ADR-028 exactly-once ingress deduplication.

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_admit_ingress_exists() {
        // This test should fail to compile because admit_ingress doesn't exist yet.
        let _fn_ptr: fn(axum::Json<crate::types::ingress::IngressAdmissionRequest>) -> _ = super::admit_ingress;
    }
}
