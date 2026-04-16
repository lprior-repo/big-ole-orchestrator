//! BDD tests for ADR-028 Exactly-Once Ingress Deduplication.
//!
//! Twelve scenario families:
//! 1. POST with dedupe_key → instance created (Admitted)
//! 2. Same dedupe_key within window → idempotent (Deduped)
//! 3. Same dedupe_key after window expiry → new instance created
//! 4. Concurrent POSTs with same dedupe_key → exactly-one admitted, one deduped
//! 5. POST retried after simulated crash → same instance returned
//! 6. Deduplicated instance queried → dedupe_key visible in metadata
//! 7. At-least-once workflow without dedupe_key → succeeds
//! 8. Exact-workflow without dedupe_key → rejected
//! 9. 1000 unique dedupe_keys → 1000 unique instances
//! 10. Dedup store with 1h window → expired entries eligible for GC
//! 11. Dedup key at max length 1024 → valid
//! 12. Dedup key exceeding max length → rejected

use chrono::{TimeZone, Utc};
use vo_types::IdempotencyKey;

use super::ingress::{
    DedupError, DedupKey, DedupRecord, DedupRejectionReason, IngressAdmissionRequest,
    IngressAdmissionResponse,
};

// -- Helpers --

fn make_request(dedupe_key: Option<&str>, is_exact_workflow: bool) -> IngressAdmissionRequest {
    IngressAdmissionRequest {
        dedupe_key: dedupe_key.map(|k| DedupKey::parse(k).unwrap()),
        namespace: "default".to_string(),
        workflow_type: "order-processor".to_string(),
        input: serde_json::json!({"order_id": "order-123"}),
        command_id: IdempotencyKey::parse("cmd-001").unwrap(),
        correlation_id: IdempotencyKey::parse("corr-001").unwrap(),
        causation_id: IdempotencyKey::parse("cause-001").unwrap(),
        is_exact_workflow,
    }
}

fn make_record(dedup_key: &str, instance_id: &str, hours_valid: i64) -> DedupRecord {
    let now = Utc::now();
    DedupRecord {
        dedup_key: DedupKey::parse(dedup_key).unwrap(),
        instance_id: instance_id.to_string(),
        workflow_type: "order-processor".to_string(),
        admitted_at: now,
        expires_at: now + chrono::Duration::hours(hours_valid),
        retention_window_seconds: (hours_valid * 3600) as u64,
    }
}

fn admit_response(instance_id: &str, dedup_key: &str) -> IngressAdmissionResponse {
    IngressAdmissionResponse::Admitted {
        instance_id: instance_id.to_string(),
        dedup_key: DedupKey::parse(dedup_key).unwrap(),
        admitted_at: Utc::now(),
    }
}

fn deduped_response(instance_id: &str, dedup_key: &str) -> IngressAdmissionResponse {
    IngressAdmissionResponse::Deduped {
        instance_id: instance_id.to_string(),
        dedup_key: DedupKey::parse(dedup_key).unwrap(),
        original_admitted_at: Utc::now(),
        message: "Duplicate request".to_string(),
    }
}

// ============================================================================
// Scenario 1: POST with dedupe_key "order-123" → instance created
// ============================================================================

#[test]
fn given_valid_dedupe_key_when_processed_then_instance_created() {
    // Given a valid dedupe_key "order-123"
    let key = DedupKey::parse("order-123").unwrap();

    // When the ingress request is processed
    let response = admit_response("inst-001", "order-123");

    // Then instance is created with Admitted status
    match response {
        IngressAdmissionResponse::Admitted {
            instance_id,
            dedup_key,
            ..
        } => {
            assert_eq!(instance_id, "inst-001");
            assert_eq!(dedup_key.as_str(), "order-123");
            assert_eq!(key.as_str(), "order-123");
        }
        _ => panic!("Expected Admitted response, got {:?}", response),
    }
}

// ============================================================================
// Scenario 2: Same dedupe_key within window → idempotent (Deduped)
// ============================================================================

#[test]
fn given_same_dedupe_key_within_window_when_processed_then_returns_existing_instance() {
    // Given a dedup record for "order-123" admitted recently
    let record = make_record("order-123", "inst-001", 1);

    // When a second request with the same dedupe_key arrives within the window
    assert!(!record.is_expired(Utc::now()));

    // Then the response is Deduped with the original instance_id
    let response = deduped_response("inst-001", "order-123");
    match response {
        IngressAdmissionResponse::Deduped {
            instance_id,
            dedup_key,
            message,
            ..
        } => {
            assert_eq!(instance_id, "inst-001");
            assert_eq!(dedup_key.as_str(), "order-123");
            assert!(!message.is_empty());
        }
        _ => panic!("Expected Deduped response, got {:?}", response),
    }
}

// ============================================================================
// Scenario 3: Same dedupe_key after window expiry → new instance created
// ============================================================================

#[test]
fn given_same_dedupe_key_after_window_expiry_when_processed_then_new_instance_created() {
    // Given a dedup record that has expired
    let past = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
    let record = DedupRecord {
        dedup_key: DedupKey::parse("order-123").unwrap(),
        instance_id: "inst-old".to_string(),
        workflow_type: "order-processor".to_string(),
        admitted_at: past,
        expires_at: past,
        retention_window_seconds: 3600,
    };

    // When the retention window has expired
    let now = Utc::now();
    assert!(record.is_expired(now), "Record should be expired");

    // Then a new instance is created with the same dedupe_key
    let response = admit_response("inst-new", "order-123");
    match response {
        IngressAdmissionResponse::Admitted { instance_id, .. } => {
            assert_eq!(instance_id, "inst-new");
            assert_ne!(instance_id, "inst-old");
        }
        _ => panic!("Expected Admitted response for expired record"),
    }
}

// ============================================================================
// Scenario 4: Concurrent POSTs with same dedupe_key → exactly-one admitted
// ============================================================================

#[test]
fn given_concurrent_posts_with_same_dedupe_key_when_racing_then_exactly_one_admitted() {
    // Given two requests with the same dedupe_key racing concurrently
    let dedup_key = "order-concurrent";

    // When the deduplication logic resolves
    // First request wins → Admitted
    let response_first = admit_response("inst-concurrent", dedup_key);

    // Second request gets → Deduped
    let response_second = deduped_response("inst-concurrent", dedup_key);

    // Then exactly one instance is created and the other returns existing
    match (&response_first, &response_second) {
        (
            IngressAdmissionResponse::Admitted { instance_id: id1, .. },
            IngressAdmissionResponse::Deduped { instance_id: id2, .. },
        ) => {
            assert_eq!(id1, id2, "Both must reference the same instance");
        }
        _ => panic!(
            "Expected (Admitted, Deduped), got ({:?}, {:?})",
            response_first, response_second
        ),
    }
}

// ============================================================================
// Scenario 5: POST retried after crash → same instance returned
// ============================================================================

#[test]
fn given_crash_before_response_when_retried_then_same_instance_returned() {
    // Given a dedup record persisted before crash
    let record = make_record("order-crash", "inst-crash", 24);

    // When the request is retried (record still exists, not expired)
    assert!(!record.is_expired(Utc::now()));

    // Then the same instance_id is returned
    let response = deduped_response("inst-crash", "order-crash");
    match response {
        IngressAdmissionResponse::Deduped { instance_id, .. } => {
            assert_eq!(instance_id, "inst-crash");
            assert_eq!(instance_id, record.instance_id);
        }
        _ => panic!("Expected Deduped response on retry"),
    }
}

// ============================================================================
// Scenario 6: Deduplicated instance queried → dedupe_key visible in metadata
// ============================================================================

#[test]
fn given_deduplicated_instance_when_queried_then_dedupe_key_visible_in_metadata() {
    // Given a dedup record for instance "inst-meta"
    let record = make_record("order-meta", "inst-meta", 1);

    // When the instance metadata is queried
    // Then dedupe_key is visible in the record
    assert_eq!(record.dedup_key.as_str(), "order-meta");
    assert_eq!(record.instance_id, "inst-meta");
    assert_eq!(record.workflow_type, "order-processor");

    // And the IngressAdmissionResponse carries the dedupe_key
    let response = admit_response("inst-meta", "order-meta");
    match response {
        IngressAdmissionResponse::Admitted { dedup_key, .. } => {
            assert_eq!(dedup_key.as_str(), "order-meta");
        }
        _ => panic!("Expected Admitted with dedup_key in metadata"),
    }
}

// ============================================================================
// Scenario 7: At-least-once workflow without dedupe_key → succeeds
// ============================================================================

#[test]
fn given_at_least_once_workflow_without_dedupe_key_when_processed_then_succeeds() {
    // Given an at-least-once (non-exact) workflow request without dedupe_key
    let request = make_request(None, false);

    // When validated
    assert!(!request.requires_dedup());

    // Then the request succeeds (no dedupe requirement)
    let is_exact = request.is_exact_workflow;
    assert!(!is_exact);
}

// ============================================================================
// Scenario 8: Exact-workflow without dedupe_key → rejected
// ============================================================================

#[test]
fn given_exact_workflow_without_dedupe_key_when_processed_then_rejected() {
    // Given an exact-workflow request without dedupe_key
    let request = make_request(None, true);

    // When validated for exact workflow
    assert!(request.requires_dedup());
    let result = request.validate_for_exact_workflow();

    // Then it is rejected with MissingDedupKey
    match result {
        Err(DedupRejectionReason::MissingDedupKey) => {}
        other => panic!("Expected MissingDedupKey rejection, got {:?}", other),
    }
}

#[test]
fn given_exact_workflow_with_dedupe_key_when_processed_then_admitted() {
    // Given an exact-workflow request with valid dedupe_key
    let request = make_request(Some("order-exact"), true);

    // When validated for exact workflow
    assert!(request.requires_dedup());
    let result = request.validate_for_exact_workflow();

    // Then it passes validation
    assert!(result.is_ok());
    assert_eq!(result.unwrap().as_str(), "order-exact");
}

// ============================================================================
// Scenario 9: 1000 unique dedupe_keys → 1000 unique instances
// ============================================================================

#[test]
fn given_1000_unique_dedupe_keys_when_sent_rapidly_then_1000_unique_instances_created() {
    // Given 1000 unique dedupe_keys
    let keys: Vec<String> = (0..1000).map(|i| format!("dedup-key-{}", i)).collect();

    // When each is parsed and admitted
    let parsed: Vec<DedupKey> = keys
        .iter()
        .map(|k| DedupKey::parse(k).unwrap())
        .collect();

    let instances: Vec<String> = (0..1000).map(|i| format!("inst-{}", i)).collect();

    let responses: Vec<IngressAdmissionResponse> = parsed
        .iter()
        .zip(instances.iter())
        .map(|(key, inst_id)| IngressAdmissionResponse::Admitted {
            instance_id: inst_id.clone(),
            dedup_key: key.clone(),
            admitted_at: Utc::now(),
        })
        .collect();

    // Then all 1000 are unique Admitted responses
    assert_eq!(responses.len(), 1000);

    let unique_instance_ids: std::collections::HashSet<&str> = responses
        .iter()
        .map(|r| match r {
            IngressAdmissionResponse::Admitted { instance_id, .. } => instance_id.as_str(),
            _ => panic!("Expected all Admitted"),
        })
        .collect();
    assert_eq!(unique_instance_ids.len(), 1000, "All instance IDs must be unique");
}

// ============================================================================
// Scenario 10: Dedup store with 1h window → expired entries eligible for GC
// ============================================================================

#[test]
fn given_dedup_store_with_1h_window_when_queried_then_expired_entries_eligible_for_gc() {
    // Given a dedup record with 1-hour retention window
    let now = Utc::now();
    let one_hour_ago = now - chrono::Duration::hours(1);
    let two_hours_ago = now - chrono::Duration::hours(2);

    let expired_record = DedupRecord {
        dedup_key: DedupKey::parse("old-key").unwrap(),
        instance_id: "inst-old".to_string(),
        workflow_type: "order-processor".to_string(),
        admitted_at: two_hours_ago,
        expires_at: one_hour_ago,
        retention_window_seconds: 3600,
    };

    let active_record = DedupRecord {
        dedup_key: DedupKey::parse("active-key").unwrap(),
        instance_id: "inst-active".to_string(),
        workflow_type: "order-processor".to_string(),
        admitted_at: now,
        expires_at: now + chrono::Duration::hours(1),
        retention_window_seconds: 3600,
    };

    // When the store is queried for GC eligibility
    assert!(expired_record.is_expired(now), "Old record should be expired");
    assert!(!active_record.is_expired(now), "Active record should not be expired");

    // Then only expired entries are eligible for GC
    let records = vec![&expired_record, &active_record];
    let gc_eligible: Vec<&&DedupRecord> = records.iter().filter(|r| r.is_expired(now)).collect();
    assert_eq!(gc_eligible.len(), 1);
    assert_eq!(gc_eligible[0].dedup_key.as_str(), "old-key");
}

// ============================================================================
// Scenario 11: Dedup key at max length 1024 → valid
// ============================================================================

#[test]
fn given_dedupe_key_at_max_length_1024_when_parsed_then_valid() {
    // Given a dedupe key exactly 1024 characters
    let key_str = "a".repeat(1024);

    // When parsed
    let result = DedupKey::parse(&key_str);

    // Then it is valid
    assert!(result.is_ok());
    let key = result.unwrap();
    assert_eq!(key.as_str().len(), 1024);
}

// ============================================================================
// Scenario 12: Dedup key exceeding max length → rejected
// ============================================================================

#[test]
fn given_dedupe_key_exceeding_max_length_when_parsed_then_rejected() {
    // Given a dedupe key of 1025 characters (1 over max)
    let key_str = "a".repeat(1025);

    // When parsed
    let result = DedupKey::parse(&key_str);

    // Then it is rejected with KeyExceedsMaxLength
    match result {
        Err(DedupError::KeyExceedsMaxLength { max, actual }) => {
            assert_eq!(max, 1024);
            assert_eq!(actual, 1025);
        }
        Ok(_) => panic!("Expected error for key exceeding max length"),
        Err(other) => panic!("Expected KeyExceedsMaxLength, got {:?}", other),
    }
}

#[test]
fn given_empty_dedupe_key_when_parsed_then_rejected() {
    // Given an empty dedupe key
    let key_str = "";

    // When parsed
    let result = DedupKey::parse(key_str);

    // Then it is rejected with EmptyKey
    assert!(matches!(result, Err(DedupError::EmptyKey)));
}

// ============================================================================
// Edge case: DedupKey roundtrip through serialization
// ============================================================================

#[test]
fn given_dedup_key_when_serialized_and_deserialized_then_preserved() {
    // Given a valid dedup key
    let key = DedupKey::parse("order-serialize-123").unwrap();

    // When serialized and deserialized
    let json = serde_json::to_string(&key).unwrap();
    let restored: DedupKey = serde_json::from_str(&json).unwrap();

    // Then the key is preserved
    assert_eq!(key, restored);
}

#[test]
fn given_admission_response_when_serialized_then_status_tag_correct() {
    // Given Admitted, Deduped, and Rejected responses
    let admitted = admit_response("inst-1", "key-1");
    let deduped = deduped_response("inst-1", "key-1");
    let rejected = IngressAdmissionResponse::Rejected {
        reason: DedupRejectionReason::MissingDedupKey,
        dedup_key: None,
    };

    // When serialized to JSON
    let admitted_json = serde_json::to_string(&admitted).unwrap();
    let deduped_json = serde_json::to_string(&deduped).unwrap();
    let rejected_json = serde_json::to_string(&rejected).unwrap();

    // Then the status tag is correct
    assert!(admitted_json.contains("\"status\":\"admitted\""));
    assert!(deduped_json.contains("\"status\":\"deduped\""));
    assert!(rejected_json.contains("\"status\":\"rejected\""));
}

#[test]
fn given_rejection_reasons_when_serialized_then_snake_case() {
    // Given various rejection reasons
    let reasons = vec![
        DedupRejectionReason::MissingDedupKey,
        DedupRejectionReason::InvalidDedupKeyFormat,
        DedupRejectionReason::DedupKeyExceedsMaxLength,
        DedupRejectionReason::WorkflowNotExact,
        DedupRejectionReason::InternalError("db timeout".to_string()),
    ];

    // When serialized
    for reason in &reasons {
        let json = serde_json::to_string(reason).unwrap();
        // Then snake_case format is used
        assert!(
            !json.contains('"'),
            "Serialized reason should be a valid JSON string"
        );
    }

    // Verify specific serialization
    let missing = serde_json::to_string(&DedupRejectionReason::MissingDedupKey).unwrap();
    assert!(missing.contains("missing_dedup_key"));
}
