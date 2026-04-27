//! Fjall-backed workflow event log used by the live API server.
//!
//! The HTTP replay path consumes [`vo_types::EventEnvelope::from_bytes`], which
//! expects a JSON field named `version`. This module owns that storage contract
//! so API handlers do not hand-roll event keys or envelope serialization.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::json;
use vo_types::events::EventMetadata;
use vo_types::{EventEnvelope, InstanceId};

use crate::codec::{StorageError, EVENT_KEY_VERSION};
use crate::query::{replay_events_by_prefix, EventReplayIterator};

const EVENTS_PARTITION: &str = "events";
const SEQUENCE_BYTES: usize = 8;

static STREAM_LOCKS: OnceLock<Mutex<HashMap<Vec<u8>, Arc<Mutex<()>>>>> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct AppendEventRequest {
    pub namespace: String,
    pub instance_id: InstanceId,
    pub timestamp_ms: u64,
    pub payload: serde_json::Value,
    pub metadata: EventMetadata,
}

pub fn append_event(
    db: &fjall::Database,
    request: AppendEventRequest,
) -> Result<EventEnvelope, StorageError> {
    let prefix = namespaced_stream_prefix(&request.namespace, &request.instance_id)?;
    let stream_lock = lock_for_stream(&prefix)?;
    let _guard = stream_lock.lock().map_err(|_| StorageError::Storage)?;

    let partition = events_partition(db)?;
    let sequence = next_sequence(&partition, &prefix)?;
    let envelope = envelope_from_request(request, sequence);
    let key = event_key(prefix, sequence);
    let value = encode_event_value(&envelope)?;

    partition.insert(key, value)?;
    db.persist(fjall::PersistMode::SyncAll)?;

    Ok(envelope)
}

pub fn replay_events_in_namespace(
    db: &fjall::Database,
    namespace: &str,
    instance_id: &InstanceId,
) -> EventReplayIterator {
    match namespaced_stream_prefix(namespace, instance_id) {
        Ok(prefix) => replay_events_by_prefix(db, prefix),
        Err(error) => EventReplayIterator::error(error),
    }
}

pub fn namespaced_stream_prefix(
    namespace: &str,
    instance_id: &InstanceId,
) -> Result<Vec<u8>, StorageError> {
    if namespace.is_empty() || namespace.as_bytes().contains(&b'\0') || namespace.contains('/') {
        return Err(StorageError::InvalidArgument);
    }

    let namespace_len = u8::try_from(namespace.len()).map_err(|_| StorageError::InvalidArgument)?;
    let instance_bytes = instance_id
        .to_bytes()
        .map_err(|_| StorageError::InvalidArgument)?;
    let mut prefix = Vec::with_capacity(3 + namespace.len() + instance_bytes.len());
    prefix.push(EVENT_KEY_VERSION);
    prefix.push(namespace_len);
    prefix.extend_from_slice(namespace.as_bytes());
    prefix.push(b'/');
    prefix.extend_from_slice(&instance_bytes);
    Ok(prefix)
}

fn events_partition(db: &fjall::Database) -> Result<fjall::Keyspace, StorageError> {
    db.keyspace(EVENTS_PARTITION, fjall::KeyspaceCreateOptions::default)
        .map_err(|_| StorageError::Storage)
}

fn lock_for_stream(prefix: &[u8]) -> Result<Arc<Mutex<()>>, StorageError> {
    let locks = STREAM_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = locks.lock().map_err(|_| StorageError::Storage)?;
    let entry = guard
        .entry(prefix.to_vec())
        .or_insert_with(|| Arc::new(Mutex::new(())));
    Ok(entry.clone())
}

fn next_sequence(partition: &fjall::Keyspace, prefix: &[u8]) -> Result<u64, StorageError> {
    let start = event_key(prefix.to_vec(), 1);
    let end = event_key(prefix.to_vec(), u64::MAX);
    match partition.range(start..=end).next_back() {
        Some(guard) => {
            let (key, _) = guard.into_inner().map_err(|_| StorageError::Storage)?;
            let last = sequence_from_key(&key)?;
            last.checked_add(1).ok_or(StorageError::SequenceGap)
        }
        None => Ok(1),
    }
}

fn event_key(mut prefix: Vec<u8>, sequence: u64) -> Vec<u8> {
    prefix.extend_from_slice(&sequence.to_be_bytes());
    prefix
}

fn sequence_from_key(key: &[u8]) -> Result<u64, StorageError> {
    if key.len() < SEQUENCE_BYTES {
        return Err(StorageError::CorruptKey);
    }
    let start = key.len() - SEQUENCE_BYTES;
    let bytes: [u8; SEQUENCE_BYTES] = key[start..]
        .try_into()
        .map_err(|_| StorageError::CorruptKey)?;
    let sequence = u64::from_be_bytes(bytes);
    if sequence == 0 {
        return Err(StorageError::InvalidArgument);
    }
    Ok(sequence)
}

fn envelope_from_request(request: AppendEventRequest, sequence: u64) -> EventEnvelope {
    EventEnvelope {
        schema_version: 1,
        instance_id: request.instance_id.to_string(),
        sequence,
        timestamp_ms: request.timestamp_ms,
        payload: request.payload,
        metadata: request.metadata,
    }
}

fn encode_event_value(envelope: &EventEnvelope) -> Result<Vec<u8>, StorageError> {
    serde_json::to_vec(&json!({
        "version": envelope.schema_version,
        "instance_id": envelope.instance_id,
        "sequence": envelope.sequence,
        "timestamp_ms": envelope.timestamp_ms,
        "payload": envelope.payload,
        "metadata": envelope.metadata,
    }))
    .map_err(|_| StorageError::SerializationFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> fjall::Database {
        let temp = tempfile::tempdir().expect("tempdir");
        fjall::Database::builder(temp.into_path())
            .open()
            .expect("db")
    }

    fn request(namespace: &str, instance_id: &InstanceId) -> AppendEventRequest {
        AppendEventRequest {
            namespace: namespace.to_string(),
            instance_id: instance_id.clone(),
            timestamp_ms: 1,
            payload: json!({"type":"WorkflowStarted"}),
            metadata: EventMetadata::default(),
        }
    }

    #[test]
    fn namespaced_event_streams_with_same_instance_id_do_not_collide() {
        let db = test_db();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("id");

        append_event(&db, request("payments", &instance_id)).expect("append payments");
        append_event(&db, request("billing", &instance_id)).expect("append billing");

        let payments = replay_events_in_namespace(&db, "payments", &instance_id)
            .collect::<Result<Vec<_>, _>>()
            .expect("payments replay");
        let billing = replay_events_in_namespace(&db, "billing", &instance_id)
            .collect::<Result<Vec<_>, _>>()
            .expect("billing replay");

        assert_eq!(payments.len(), 1);
        assert_eq!(billing.len(), 1);
        assert_eq!(payments[0].sequence, 1);
        assert_eq!(billing[0].sequence, 1);
    }

    #[test]
    fn concurrent_appends_to_same_namespaced_stream_get_unique_monotonic_sequences() {
        let db = Arc::new(test_db());
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("id");
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let db = db.clone();
                let instance_id = instance_id.clone();
                std::thread::spawn(move || append_event(&db, request("payments", &instance_id)))
            })
            .collect();

        for handle in handles {
            handle.join().expect("thread").expect("append");
        }

        let events = replay_events_in_namespace(&db, "payments", &instance_id)
            .collect::<Result<Vec<_>, _>>()
            .expect("replay");
        let sequences: Vec<u64> = events.iter().map(|event| event.sequence).collect();
        assert_eq!(sequences, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }
}
