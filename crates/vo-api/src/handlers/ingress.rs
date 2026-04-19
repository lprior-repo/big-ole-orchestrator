//! Ingress admission handler — ADR-028 exactly-once ingress deduplication.

use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use fjall::Database;
use vo_storage::dedupe_partition::{AdmissionResult, DedupeStore, FjallDedupeStore};
use vo_types::DedupeKey;

use crate::types::{IngressAdmissionRequest, IngressAdmissionResponse};

/// Shared state for ingress handlers.
#[derive(Clone)]
pub struct IngressState {
    pub db: Arc<Database>,
    pub dedupe_store: Arc<FjallDedupeStore>,
}

impl IngressState {
    pub fn new(db: Arc<Database>) -> Self {
        let dedupe_store = Arc::new(
            FjallDedupeStore::open(db.as_ref())
                .expect("failed to open dedupe partition"),
        );
        Self { db, dedupe_store }
    }
}

/// Default retention window: 7 days in milliseconds
const DEFAULT_RETENTION_WINDOW_MS: u64 = 7 * 24 * 60 * 60 * 1000;

/// POST /api/v1/ingress/admit — admit an ingress request (ADR-028).
#[tracing::instrument(skip_all)]
pub async fn admit_ingress(
    State(state): State<IngressState>,
    Json(req): Json<IngressAdmissionRequest>,
) -> impl IntoResponse {
    // Validate exact workflow requires dedup key
    if req.is_exact_workflow && !req.requires_dedup() {
        return (
            StatusCode::BAD_REQUEST,
            Json(IngressAdmissionResponse::Rejected {
                reason: crate::types::ingress::DedupRejectionReason::MissingDedupKey,
                dedup_key: None,
            }),
        )
            .into_response();
    }

    // For exact workflows, validate dedup key
    let dedup_key = if req.is_exact_workflow {
        match req.validate_for_exact_workflow() {
            Ok(key) => key,
            Err(reason) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(IngressAdmissionResponse::Rejected {
                        reason,
                        dedup_key: None,
                    }),
                )
                    .into_response();
            }
        }
    } else {
        // Non-exact workflows: if dedup key provided, use it; otherwise admit without dedup
        match req.dedupe_key {
            Some(key) => key,
            None => {
                // Admit without deduplication for non-exact workflows
                // Use a synthetic dedup key for tracking purposes
                let synthetic_key = DedupeKey::parse("no-dedup-required").unwrap();
                return (
                    StatusCode::OK,
                    Json(IngressAdmissionResponse::Admitted {
                        instance_id: "pending".to_string(),
                        dedup_key: synthetic_key.clone(),
                        admitted_at: chrono::Utc::now(),
                    }),
                )
                    .into_response();
            }
        }
    };

    // Attempt atomic check-and-insert
    let instance_id = "pending".to_string(); // Will be set by workflow engine
    let admission_result = match state.dedupe_store.check_and_insert(
        &dedup_key,
        &vo_types::InstanceId::from_bytes([0u8; 16]),
        DEFAULT_RETENTION_WINDOW_MS,
    ) {
        Ok(result) => result,
        Err(e) => {
            tracing::error!(error = %e, "dedupe store check_and_insert failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(IngressAdmissionResponse::Rejected {
                    reason: crate::types::ingress::DedupRejectionReason::InternalError(
                        "dedupe storage error".to_string(),
                    ),
                    dedup_key: Some(dedup_key),
                }),
            )
                .into_response();
        }
    };

    match admission_result {
        AdmissionResult::Admitted => (
            StatusCode::OK,
            Json(IngressAdmissionResponse::Admitted {
                instance_id: instance_id.clone(),
                dedup_key,
                admitted_at: chrono::Utc::now(),
            }),
        )
            .into_response(),
        AdmissionResult::Duplicate { instance_id } => (
            StatusCode::OK,
            Json(IngressAdmissionResponse::Deduped {
                instance_id,
                dedup_key,
                original_admitted_at: chrono::Utc::now(),
                message: "duplicate request rejected".to_string(),
            }),
        )
            .into_response(),
    }
}
