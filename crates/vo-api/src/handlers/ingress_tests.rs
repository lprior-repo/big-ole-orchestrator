//! Tests for ingress admission (ADR-028).

use super::{admit_ingress, admit_signal, IngressAdmission, IngressAdmissionError};
use vo_storage::dedupe_partition::InMemoryDedupeStore;
use vo_types::InstanceId;

fn test_instance_id(n: u8) -> InstanceId {
    InstanceId::from_bytes([n; 16])
}

fn test_store() -> InMemoryDedupeStore {
    InMemoryDedupeStore::new()
}

// --- admit_ingress tests ---

#[test]
fn admit_ingress_returns_admitted_for_new_key() {
    let store = test_store();
    let iid = test_instance_id(1);

    let result = admit_ingress(&store, "webhook-evt-001", &iid, 5000);

    assert_eq!(result, Ok(IngressAdmission::Admitted));
}

#[test]
fn admit_ingress_returns_duplicate_for_existing_key() {
    let store = test_store();
    let iid = test_instance_id(1);

    admit_ingress(&store, "webhook-evt-002", &iid, 60_000).unwrap();
    let result = admit_ingress(&store, "webhook-evt-002", &iid, 60_000);

    assert!(
        matches!(result, Ok(IngressAdmission::Duplicate { .. })),
        "expected Duplicate, got {:?}",
        result
    );
}

#[test]
fn admit_ingress_duplicate_contains_existing_instance_id() {
    let store = test_store();
    let iid = test_instance_id(42);

    admit_ingress(&store, "dup-key-001", &iid, 60_000).unwrap();
    let result = admit_ingress(&store, "dup-key-001", &iid, 60_000).unwrap();

    if let IngressAdmission::Duplicate {
        existing_instance_id,
    } = result
    {
        assert_eq!(existing_instance_id, iid.to_string());
    } else {
        panic!("expected Duplicate variant");
    }
}

#[test]
fn admit_ingress_rejects_empty_key() {
    let store = test_store();
    let iid = test_instance_id(1);

    let result = admit_ingress(&store, "", &iid, 5000);

    assert!(
        matches!(result, Err(IngressAdmissionError::InvalidDedupeKey { .. })),
        "expected InvalidDedupeKey, got {:?}",
        result
    );
}

#[test]
fn admit_ingress_rejects_overlong_key() {
    let store = test_store();
    let iid = test_instance_id(1);
    let long_key = "a".repeat(257);

    let result = admit_ingress(&store, &long_key, &iid, 5000);

    assert!(
        matches!(result, Err(IngressAdmissionError::InvalidDedupeKey { .. })),
        "expected InvalidDedupeKey for 257-char key, got {:?}",
        result
    );
}

#[test]
fn admit_ingress_uses_default_ttl_when_zero_provided() {
    let store = test_store();
    let iid = test_instance_id(1);

    let result = admit_ingress(&store, "zero-ttl-key", &iid, 0);

    assert_eq!(result, Ok(IngressAdmission::Admitted));
}

#[test]
fn admit_ingress_different_keys_both_admitted() {
    let store = test_store();
    let iid = test_instance_id(1);

    let r1 = admit_ingress(&store, "key-alpha", &iid, 5000);
    let r2 = admit_ingress(&store, "key-beta", &iid, 5000);

    assert_eq!(r1, Ok(IngressAdmission::Admitted));
    assert_eq!(r2, Ok(IngressAdmission::Admitted));
}

// --- admit_signal tests ---

#[test]
fn admit_signal_returns_admitted_for_new_signal() {
    let store = test_store();
    let iid = test_instance_id(1);

    let result = admit_signal(&store, &iid, "approve", "sig-001", 5000);

    assert_eq!(result, Ok(IngressAdmission::Admitted));
}

#[test]
fn admit_signal_returns_duplicate_for_same_signal_key() {
    let store = test_store();
    let iid = test_instance_id(1);

    admit_signal(&store, &iid, "approve", "sig-002", 60_000).unwrap();
    let result = admit_signal(&store, &iid, "approve", "sig-002", 60_000);

    assert!(
        matches!(result, Ok(IngressAdmission::Duplicate { .. })),
        "expected Duplicate for repeated signal, got {:?}",
        result
    );
}

#[test]
fn admit_signal_different_signal_names_are_independent() {
    let store = test_store();
    let iid = test_instance_id(1);

    let r1 = admit_signal(&store, &iid, "approve", "sig-003", 5000);
    let r2 = admit_signal(&store, &iid, "reject", "sig-003", 5000);

    assert_eq!(r1, Ok(IngressAdmission::Admitted));
    assert_eq!(r2, Ok(IngressAdmission::Admitted));
}

#[test]
fn admit_signal_different_instances_are_independent() {
    let store = test_store();
    let iid1 = test_instance_id(1);
    let iid2 = test_instance_id(2);

    let r1 = admit_signal(&store, &iid1, "approve", "sig-004", 5000);
    let r2 = admit_signal(&store, &iid2, "approve", "sig-004", 5000);

    assert_eq!(r1, Ok(IngressAdmission::Admitted));
    assert_eq!(r2, Ok(IngressAdmission::Admitted));
}

// --- Retention / expiry tests ---

#[test]
fn admit_ingress_after_expiry_allows_new_admission() {
    let store = test_store();
    let iid1 = test_instance_id(1);
    let iid2 = test_instance_id(2);

    let r1 = admit_ingress(&store, "expiry-key", &iid1, 1);
    assert_eq!(r1, Ok(IngressAdmission::Admitted));

    std::thread::sleep(std::time::Duration::from_millis(5));

    let r2 = admit_ingress(&store, "expiry-key", &iid2, 5000);
    assert_eq!(
        r2,
        Ok(IngressAdmission::Admitted),
        "key should be admitted after TTL expiry"
    );
}

// --- Concurrent admission test ---

#[test]
fn concurrent_admissions_with_same_key_exactly_one_wins() {
    use std::sync::Arc;
    use std::sync::Barrier;

    let store = Arc::new(test_store());
    let key = "concurrent-race-key";
    let num_threads = 8;
    let barrier = Arc::new(Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let iid = test_instance_id(i as u8);
                barrier.wait();
                admit_ingress(&*store, key, &iid, 60_000)
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    let admitted = results
        .iter()
        .filter(|r| matches!(r, Ok(IngressAdmission::Admitted)))
        .count();
    let duplicated = results
        .iter()
        .filter(|r| matches!(r, Ok(IngressAdmission::Duplicate { .. })))
        .count();

    assert_eq!(
        admitted, 1,
        "exactly one thread should win admission, got {} admitted and {} duplicated",
        admitted, duplicated
    );
    assert_eq!(duplicated, num_threads - 1);
}
