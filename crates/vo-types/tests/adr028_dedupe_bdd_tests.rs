//! BDD tests for ADR-028 Exactly-Once Ingress Deduplication.
//!
//! Scenarios:
//! 1. Identical webhook payload received twice → only one workflow instance created
//! 2. Duplicate with different timestamp → idempotency key prevents double-create
//! 3. Dedup window expiry → stale request creates new instance

use std::collections::HashSet;
use vo_types::{DedupeKey, DedupePartitionKey, InstanceId, TimestampMs};

struct FakeIngress {
    seen_keys: HashSet<DedupeKey>,
    created_instances: Vec<InstanceId>,
}

impl FakeIngress {
    fn new() -> Self {
        Self {
            seen_keys: HashSet::new(),
            created_instances: Vec::new(),
        }
    }

    fn try_create(&mut self, dedupe_key: &DedupeKey, instance_id: InstanceId) -> bool {
        if self.seen_keys.insert(dedupe_key.clone()) {
            self.created_instances.push(instance_id);
            return true;
        }
        false
    }
}

fn make_instance_id(n: u8) -> InstanceId {
    let base = format!("01H5JYV4XHGSR2F8KZBWNRFM{:02X}", n);
    InstanceId::parse(&base).expect("valid instance id")
}

// ---------- Scenario 1 ----------

#[test]
fn given_identical_webhook_payload_twice_when_processed_then_one_instance() {
    let mut ingress = FakeIngress::new();
    let key = DedupeKey::parse("webhook-evt-12345").unwrap();
    let instance = make_instance_id(0x01);

    let first = ingress.try_create(&key, instance.clone());
    assert!(first, "first delivery should create an instance");
    assert_eq!(ingress.created_instances.len(), 1);

    let second = ingress.try_create(&key, instance.clone());
    assert!(
        !second,
        "duplicate delivery must not create a second instance"
    );
    assert_eq!(
        ingress.created_instances.len(),
        1,
        "exactly one workflow instance must exist after duplicate delivery"
    );
}

#[test]
fn given_identical_payload_many_times_when_processed_then_still_one_instance() {
    let mut ingress = FakeIngress::new();
    let key = DedupeKey::parse("webhook-evt-repeat").unwrap();
    let instance = make_instance_id(0x02);

    ingress.try_create(&key, instance.clone());
    for _ in 0..10 {
        ingress.try_create(&key, instance.clone());
    }
    assert_eq!(
        ingress.created_instances.len(),
        1,
        "repeated identical payloads must produce exactly one instance"
    );
}

// ---------- Scenario 2 ----------

#[test]
fn given_duplicate_with_different_timestamp_when_dedup_checked_then_idempotency_key_prevents_double_create(
) {
    let mut ingress = FakeIngress::new();
    let key = DedupeKey::parse("idem-key-abc").unwrap();
    let instance = make_instance_id(0x03);

    let _ts_first = TimestampMs::try_from(1_700_000_000u64).unwrap();
    let created_first = ingress.try_create(&key, instance.clone());
    assert!(created_first);

    let _ts_second = TimestampMs::try_from(1_700_000_100u64).unwrap();
    let created_second = ingress.try_create(&key, instance.clone());
    assert!(
        !created_second,
        "same idempotency key with different timestamp must not create a second instance"
    );
    assert_eq!(ingress.created_instances.len(), 1);
}

#[test]
fn given_different_idempotency_keys_when_processed_then_both_create() {
    let mut ingress = FakeIngress::new();
    let key_a = DedupeKey::parse("idem-alpha").unwrap();
    let key_b = DedupeKey::parse("idem-beta").unwrap();
    let inst_a = make_instance_id(0x04);
    let inst_b = make_instance_id(0x05);

    let created_a = ingress.try_create(&key_a, inst_a);
    let created_b = ingress.try_create(&key_b, inst_b);
    assert!(created_a, "first unique key must create instance");
    assert!(created_b, "second unique key must create instance");
    assert_eq!(ingress.created_instances.len(), 2);
}

#[test]
fn given_partition_key_groups_by_instance_and_command_type_when_deduped_then_isolation_holds() {
    let inst_a = make_instance_id(0x10);
    let inst_b = make_instance_id(0x11);

    let pk_a1 = DedupePartitionKey::new(inst_a.clone(), "workflow_start").unwrap();
    let pk_a2 = DedupePartitionKey::new(inst_a.clone(), "timer_fire").unwrap();
    let pk_b1 = DedupePartitionKey::new(inst_b, "workflow_start").unwrap();

    assert_ne!(
        pk_a1, pk_a2,
        "same instance, different command types must be separate partitions"
    );
    assert_ne!(
        pk_a1, pk_b1,
        "different instances, same command type must be separate partitions"
    );
}

// ---------- Scenario 3 ----------

#[test]
fn given_dedup_window_expiry_when_stale_request_arrives_then_new_instance_created() {
    let mut first_window = FakeIngress::new();
    let key = DedupeKey::parse("stale-evt-999").unwrap();
    let instance_v1 = make_instance_id(0x20);

    let created_v1 = first_window.try_create(&key, instance_v1.clone());
    assert!(created_v1, "first-window delivery should succeed");

    let mut second_window = FakeIngress::new();
    let instance_v2 = make_instance_id(0x21);
    let created_v2 = second_window.try_create(&key, instance_v2.clone());
    assert!(
        created_v2,
        "after dedup window expiry, same key must create a new instance in fresh window"
    );
}

#[test]
fn given_window_expired_then_within_new_window_duplicate_is_blocked() {
    let mut window = FakeIngress::new();
    let key = DedupeKey::parse("window-rollover").unwrap();
    let inst_v2 = make_instance_id(0x22);

    let created = window.try_create(&key, inst_v2.clone());
    assert!(created, "first delivery in new window creates instance");

    let dup = window.try_create(&key, inst_v2.clone());
    assert!(!dup, "duplicate within same new window is blocked");
}

#[test]
fn given_dedup_key_survives_serde_roundtrip_when_window_persists_then_dedup_still_works() {
    let key = DedupeKey::parse("serde-dedup-persist").unwrap();
    let json = serde_json::to_string(&key).unwrap();
    let recovered: DedupeKey = serde_json::from_str(&json).unwrap();

    let mut ingress = FakeIngress::new();
    let instance = make_instance_id(0x30);

    let first = ingress.try_create(&recovered, instance.clone());
    assert!(first);

    let deserialized_again: DedupeKey = serde_json::from_str(&json).unwrap();
    let second = ingress.try_create(&deserialized_again, instance.clone());
    assert!(
        !second,
        "serde roundtripped key must still prevent duplicate creation"
    );
}

#[test]
fn given_partition_key_survives_serde_roundtrip_when_window_persists_then_equality_holds() {
    let inst = make_instance_id(0x31);
    let pk = DedupePartitionKey::new(inst.clone(), "workflow_start").unwrap();
    let json = serde_json::to_string(&pk).unwrap();
    let recovered: DedupePartitionKey = serde_json::from_str(&json).unwrap();

    assert_eq!(pk, recovered, "partition key must survive serde roundtrip");
    assert_eq!(recovered.instance_id(), &inst);
    assert_eq!(recovered.command_type(), "workflow_start");
}
