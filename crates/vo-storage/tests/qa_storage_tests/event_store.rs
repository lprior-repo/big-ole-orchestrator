//! Section 2: Event Store — append/read on Fjall keyspace

use crate::{encode_event_seq, make_envelope, make_instance_id, open_partition};
use vo_types::InstanceId;

#[test]
fn event_key_encode_decode_roundtrip() {
    let id = make_instance_id();
    let seq = vo_types::SequenceNumber::try_from(42u64).unwrap();

    let encoded = vo_storage::codec::encode_event_key(&id, &seq).expect("encode");
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
    let restored: vo_types::events::EventEnvelope =
        serde_json::from_slice(&stored).expect("deserialize");
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
        let env: vo_types::events::EventEnvelope = serde_json::from_slice(&v).expect("deserialize");
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
    let env1: vo_types::events::EventEnvelope = serde_json::from_slice(&stored1).unwrap();
    assert_eq!(env1.instance_id, id1.to_string());

    let stored2 = ks.get(&encode_event_seq(&id2, 1)).unwrap().unwrap();
    let env2: vo_types::events::EventEnvelope = serde_json::from_slice(&stored2).unwrap();
    assert_eq!(env2.instance_id, id2.to_string());
}
