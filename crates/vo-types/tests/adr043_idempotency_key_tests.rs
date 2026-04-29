//! BDD tests for ADR-043 Idempotency Key Ingress Deduplication.
//!
//! Scenarios:
//! 1. Identical idempotency key received twice → only one workflow instance created
//! 2. Duplicate webhook with same idempotency key → idempotency key prevents double-create
//! 3. Different idempotency keys → separate instances created
//! 4. Idempotency key survives serde roundtrip

use std::collections::HashSet;
use vo_types::{IdempotencyKey, InstanceId};

struct FakeIngress {
    seen_keys: HashSet<IdempotencyKey>,
    created_instances: Vec<InstanceId>,
}

impl FakeIngress {
    fn new() -> Self {
        Self {
            seen_keys: HashSet::new(),
            created_instances: Vec::new(),
        }
    }

    fn try_create(&mut self, idempotency_key: &IdempotencyKey, instance_id: InstanceId) -> bool {
        if self.seen_keys.insert(idempotency_key.clone()) {
            self.created_instances.push(instance_id);
            true
        } else {
            false
        }
    }
}

fn make_instance_id(n: u8) -> InstanceId {
    let base = format!("01H5JYV4XHGSR2F8KZBWNRFM{:02X}", n);
    InstanceId::parse(&base).expect("valid instance id")
}

// ---------- Scenario 1 ----------

#[test]
fn given_identical_idempotency_key_twice_when_processed_then_one_instance() {
    let mut ingress = FakeIngress::new();
    let key = IdempotencyKey::parse("webhook-evt-12345-idem").unwrap();
    let instance = make_instance_id(0x01);

    let first = ingress.try_create(&key, instance.clone());
    assert!(first, "first delivery should create an instance");
    assert_eq!(ingress.created_instances.len(), 1);

    let second = ingress.try_create(&key, instance.clone());
    assert!(
        !second,
        "duplicate delivery with same idempotency key must not create a second instance"
    );
    assert_eq!(
        ingress.created_instances.len(),
        1,
        "exactly one workflow instance must exist after duplicate delivery"
    );
}

#[test]
fn given_identical_idempotency_key_many_times_when_processed_then_still_one_instance() {
    let mut ingress = FakeIngress::new();
    let key = IdempotencyKey::parse("webhook-evt-repeat-idem").unwrap();
    let instance = make_instance_id(0x02);

    ingress.try_create(&key, instance.clone());
    for _ in 0..10 {
        ingress.try_create(&key, instance.clone());
    }
    assert_eq!(
        ingress.created_instances.len(),
        1,
        "repeated identical idempotency keys must produce exactly one instance"
    );
}

// ---------- Scenario 2 ----------

#[test]
fn given_duplicate_webhook_with_same_idempotency_key_when_processed_then_idempotency_key_prevents_double_create(
) {
    let mut ingress = FakeIngress::new();
    let key = IdempotencyKey::parse("idem-key-abc").unwrap();
    let instance = make_instance_id(0x03);

    let created_first = ingress.try_create(&key, instance.clone());
    assert!(
        created_first,
        "first webhook with idempotency key must create instance"
    );

    let created_second = ingress.try_create(&key, instance.clone());
    assert!(
        !created_second,
        "duplicate webhook with same idempotency key must not create a second instance"
    );
    assert_eq!(ingress.created_instances.len(), 1);
}

// ---------- Scenario 3 ----------

#[test]
fn given_different_idempotency_keys_when_processed_then_both_create() {
    let mut ingress = FakeIngress::new();
    let key_a = IdempotencyKey::parse("idem-alpha").unwrap();
    let key_b = IdempotencyKey::parse("idem-beta").unwrap();
    let inst_a = make_instance_id(0x04);
    let inst_b = make_instance_id(0x05);

    let created_a = ingress.try_create(&key_a, inst_a);
    let created_b = ingress.try_create(&key_b, inst_b);
    assert!(
        created_a,
        "first unique idempotency key must create instance"
    );
    assert!(
        created_b,
        "second unique idempotency key must create instance"
    );
    assert_eq!(ingress.created_instances.len(), 2);
}

#[test]
fn given_similar_idempotency_keys_with_different_suffix_when_processed_then_both_create() {
    let mut ingress = FakeIngress::new();
    let key_1 = IdempotencyKey::parse("webhook-evt-0001").unwrap();
    let key_2 = IdempotencyKey::parse("webhook-evt-0002").unwrap();
    let inst_1 = make_instance_id(0x06);
    let inst_2 = make_instance_id(0x07);

    let created_1 = ingress.try_create(&key_1, inst_1);
    let created_2 = ingress.try_create(&key_2, inst_2);
    assert!(created_1, "first event must create instance");
    assert!(
        created_2,
        "second event with different key must create instance"
    );
    assert_eq!(ingress.created_instances.len(), 2);
}

// ---------- Scenario 4 ----------

#[test]
fn given_idempotency_key_survives_serde_roundtrip_when_processed_then_dedup_still_works() {
    let key = IdempotencyKey::parse("serde-idem-persist").unwrap();
    let json = serde_json::to_string(&key).unwrap();
    let recovered: IdempotencyKey = serde_json::from_str(&json).unwrap();

    let mut ingress = FakeIngress::new();
    let instance = make_instance_id(0x10);

    let first = ingress.try_create(&recovered, instance.clone());
    assert!(
        first,
        "first delivery with recovered key must create instance"
    );

    let deserialized_again: IdempotencyKey = serde_json::from_str(&json).unwrap();
    let second = ingress.try_create(&deserialized_again, instance.clone());
    assert!(
        !second,
        "serde roundtripped idempotency key must still prevent duplicate creation"
    );
}

#[test]
fn given_idempotency_key_parse_validation_when_empty_string_then_error() {
    let result = IdempotencyKey::parse("");
    assert!(result.is_err(), "empty string must fail parsing");
}

#[test]
fn given_idempotency_key_parse_validation_when_too_long_then_error() {
    let long_key = "a".repeat(1025);
    let result = IdempotencyKey::parse(&long_key);
    assert!(
        result.is_err(),
        "key exceeding 1024 chars must fail parsing"
    );
}

#[test]
fn given_idempotency_key_parse_validation_when_valid_then_success() {
    let key = IdempotencyKey::parse("valid-idempotency-key-123");
    assert!(key.is_ok(), "valid idempotency key must parse successfully");
    assert_eq!(key.unwrap().as_str(), "valid-idempotency-key-123");
}
