//! QA tests for vo-storage: Fjall persistence and event store.
//!
//! All tests use real Fjall instances in temp directories. No mocks.

use vo_storage::codec::encode_event_key;
use vo_types::events::{EventEnvelope, EventMetadata};
use vo_types::InstanceId;

fn open_partition(name: &str) -> (tempfile::TempDir, fjall::Database, fjall::Keyspace) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = fjall::Database::builder(dir.path())
        .open()
        .expect("fjall open");
    let ks = db
        .keyspace(name, || fjall::KeyspaceCreateOptions::default())
        .expect("partition open");
    (dir, db, ks)
}

fn make_instance_id() -> InstanceId {
    InstanceId::from_bytes([0x01; 16])
}

fn make_envelope(instance_id: &InstanceId, sequence: u64) -> EventEnvelope {
    EventEnvelope {
        schema_version: 1,
        instance_id: instance_id.to_string(),
        sequence,
        timestamp_ms: 1000 + sequence,
        payload: serde_json::json!({"type": "TestEvent", "seq": sequence}),
        metadata: EventMetadata::default(),
    }
}

fn encode_event_seq(id: &InstanceId, seq: u64) -> [u8; 24] {
    let sn = vo_types::SequenceNumber::try_from(seq).unwrap();
    encode_event_key(id, &sn).unwrap()
}

// ══════════════════════════════════════════════════════════════════════════════
// Section 1: Fjall Persistence — put/get/delete on raw keyspaces
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn fjall_put_then_get_returns_stored_value() {
    let (_dir, _db, ks) = open_partition("test_put_get");

    ks.insert(b"key-1", b"value-1").expect("insert");
    let val = ks.get(b"key-1").expect("get").expect("value exists");
    assert_eq!(val.as_ref(), b"value-1");
}

#[test]
fn fjall_get_missing_key_returns_none() {
    let (_dir, _db, ks) = open_partition("test_missing");

    let val = ks.get(b"no-such-key").expect("get");
    assert!(val.is_none());
}

#[test]
fn fjall_delete_removes_key() {
    let (_dir, _db, ks) = open_partition("test_delete");

    ks.insert(b"key-del", b"val-del").expect("insert");
    assert!(ks.get(b"key-del").expect("get").is_some());

    ks.remove(b"key-del").expect("remove");
    assert!(ks.get(b"key-del").expect("get").is_none());
}

#[test]
fn fjall_overwrite_replaces_value() {
    let (_dir, _db, ks) = open_partition("test_overwrite");

    ks.insert(b"k", b"v1").expect("insert 1");
    ks.insert(b"k", b"v2").expect("insert 2");

    let val = ks.get(b"k").expect("get").expect("value");
    assert_eq!(val.as_ref(), b"v2");
}

#[test]
fn fjall_persists_across_multiple_inserts() {
    let (_dir, _db, ks) = open_partition("test_multi");

    for i in 0..100u32 {
        let key = format!("key-{i}");
        let val = format!("val-{i}");
        ks.insert(key.as_bytes(), val.as_bytes()).expect("insert");
    }

    for i in 0..100u32 {
        let key = format!("key-{i}");
        let val = format!("val-{i}");
        let stored = ks.get(key.as_bytes()).expect("get").expect("value");
        assert_eq!(stored.as_ref(), val.as_bytes(), "mismatch at {i}");
    }
}

#[test]
fn fjall_binary_key_values_roundtrip() {
    let (_dir, _db, ks) = open_partition("test_binary");

    let key: [u8; 24] = [0xFF; 24];
    let value: [u8; 128] = {
        let mut v = [0u8; 128];
        v.iter_mut()
            .enumerate()
            .for_each(|(i, b)| *b = (i as u8).wrapping_mul(7));
        v
    };

    ks.insert(key, value).expect("insert");
    let retrieved = ks.get(&key).expect("get").expect("value");
    assert_eq!(retrieved.as_ref(), &value);
}

// ══════════════════════════════════════════════════════════════════════════════
// Section 2: Event Store — append/read on Fjall keyspace
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn event_key_encode_decode_roundtrip() {
    let id = make_instance_id();
    let seq = vo_types::SequenceNumber::try_from(42u64).unwrap();

    let encoded = encode_event_key(&id, &seq).expect("encode");
    assert_eq!(encoded.len(), 24);

    let (decoded_id, decoded_seq) = vo_storage::codec::decode_event_key(&encoded).expect("decode");
    assert_eq!(decoded_id, id);
    assert_eq!(decoded_seq, seq);
}

#[test]
fn event_key_ordering_preserves_instance_then_sequence() {
    let id = make_instance_id();

    let key1 = encode_event_seq(&id, 1);
    let key2 = encode_event_seq(&id, 2);

    assert!(key1 < key2, "lower sequence should sort first");
}

#[test]
fn events_persist_on_fjall_keyspace() {
    let (_dir, _db, ks) = open_partition("events");

    let id = make_instance_id();
    let event = make_envelope(&id, 1);
    let event_bytes = serde_json::to_vec(&event).expect("serialize");
    let key = encode_event_seq(&id, 1);

    ks.insert(key, &event_bytes).expect("insert");

    let stored = ks.get(&key).expect("get").expect("event should exist");
    let restored: EventEnvelope = serde_json::from_slice(&stored).expect("deserialize");
    assert_eq!(restored.sequence, 1);
    assert_eq!(restored.instance_id, id.to_string());
}

#[test]
fn multiple_events_for_same_instance_persist() {
    let (_dir, _db, ks) = open_partition("events_multi");

    let id = make_instance_id();
    for seq in 1..=10u64 {
        let event = make_envelope(&id, seq);
        let event_bytes = serde_json::to_vec(&event).unwrap();
        let key = encode_event_seq(&id, seq);
        ks.insert(key, &event_bytes).unwrap();
    }

    let id_bytes = id.to_bytes().unwrap();
    let mut count = 0u64;
    for item in ks.prefix(&id_bytes) {
        let (k, v) = item.into_inner().expect("item");
        let (_, seq) = vo_storage::codec::decode_event_key(&k).expect("decode");
        let env: EventEnvelope = serde_json::from_slice(&v).expect("deserialize");
        assert_eq!(env.sequence, seq.as_u64());
        count += 1;
    }
    assert_eq!(count, 10);
}

#[test]
fn events_from_different_instances_dont_interfere() {
    let (_dir, _db, ks) = open_partition("events_multi_inst");

    let id1 = InstanceId::from_bytes([0x01; 16]);
    let id2 = InstanceId::from_bytes([0x02; 16]);

    let event1 = make_envelope(&id1, 1);
    let key1 = encode_event_seq(&id1, 1);
    ks.insert(key1, &serde_json::to_vec(&event1).unwrap())
        .unwrap();

    let event2 = make_envelope(&id2, 1);
    let key2 = encode_event_seq(&id2, 1);
    ks.insert(key2, &serde_json::to_vec(&event2).unwrap())
        .unwrap();

    let stored1 = ks.get(&encode_event_seq(&id1, 1)).unwrap().unwrap();
    let env1: EventEnvelope = serde_json::from_slice(&stored1).unwrap();
    assert_eq!(env1.instance_id, id1.to_string());

    let stored2 = ks.get(&encode_event_seq(&id2, 1)).unwrap().unwrap();
    let env2: EventEnvelope = serde_json::from_slice(&stored2).unwrap();
    assert_eq!(env2.instance_id, id2.to_string());
}
